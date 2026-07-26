use crate::models::dev_fee::DevFeeAggregate;
use crate::models::order::OrderAggregate;
use crate::report::format::format_relative_time;
use colored::Colorize;
use nostr_sdk::prelude::*;

/// PR 1 Step C: every remaining formatting/coloring `writeln!` call from the wrapped
/// function body, moved verbatim into one function per report section. `run()`'s only
/// job past this point is to gather already-computed values and call these in sequence.
pub fn render_identity_header(out: &mut impl std::io::Write, public_key: PublicKey) -> Result<()> {
    writeln!(out, "Analyzing Mostro Node: {}", public_key.to_bech32()?)?;
    writeln!(out, "Hex: {}", public_key.to_hex())?;
    Ok(())
}

pub fn render_connecting_message(out: &mut impl std::io::Write) -> Result<()> {
    writeln!(
        out,
        "Connected to relays. Fetching history... (this might take a moment)"
    )?;
    Ok(())
}

pub fn render_fetched_count(out: &mut impl std::io::Write, count: usize) -> Result<()> {
    writeln!(out, "Fetched {} events. Analyzing...", count)?;
    Ok(())
}

pub fn render_sample_events(out: &mut impl std::io::Write, events: &[Event]) -> Result<()> {
    writeln!(out, "\n=== SAMPLE EVENTS (first 3) ===")?;
    for (idx, event) in events.iter().take(3).enumerate() {
        writeln!(out, "\nEvent #{}", idx + 1)?;
        writeln!(out, "  ID: {}", event.id)?;
        writeln!(out, "  created_at: {}", event.created_at)?;
        writeln!(out, "  Tags:")?;
        for tag in event.tags.iter() {
            writeln!(out, "    {:?}", tag.as_slice())?;
        }
    }
    writeln!(out, "==============================\n")?;
    Ok(())
}

pub fn render_partition_summary(
    out: &mut impl std::io::Write,
    dev_fee_count: usize,
    order_count: usize,
) -> Result<()> {
    writeln!(
        out,
        "Found {} dev fee events and {} order events",
        dev_fee_count, order_count
    )?;
    Ok(())
}

/// PR 2 (T070/T071): the success case is report content (`out`); the "no dev fee events"
/// branch is a diagnostic warning about data availability, not a report figure (`err`).
/// PR 4: `has_qualifying_orders` distinguishes an actual fallback (orders exist) from a
/// node with neither anchor at all (e.g. dispute/instance-status-only) — claiming a
/// fallback to "order timestamps" when there are no orders to fall back to would be
/// false diagnostic output.
pub fn render_dev_fee_section(
    out: &mut impl std::io::Write,
    err: &mut impl std::io::Write,
    aggregate: &DevFeeAggregate,
    has_qualifying_orders: bool,
) -> Result<()> {
    if let Some(first_dev_fee_ts) = aggregate.first_dev_fee_ts {
        writeln!(out, "\n=== MOSTRO TRADING ACTIVITY ===")?;
        writeln!(
            out,
            "First dev fee payment: {}",
            chrono::DateTime::from_timestamp(first_dev_fee_ts, 0).unwrap_or_default()
        )?;
        writeln!(out, "Total dev fee events: {}", aggregate.count)?;
        writeln!(out, "================================\n")?;
    } else {
        writeln!(
            err,
            "\n⚠ Warning: No dev fee events found (z=dev-fee-payment, y=mostro)."
        )?;
        if has_qualifying_orders {
            writeln!(
                err,
                "Falling back to order timestamps for days_active calculation.\n"
            )?;
        } else {
            writeln!(
                err,
                "No successful orders either; days_active is not applicable.\n"
            )?;
        }
    }
    Ok(())
}

/// PR 2 (T070/T071): entirely diagnostic (`err`), never report content. T074/T075: the `s`
/// tag distribution is sorted by key before printing, since it comes from `HashMap`
/// iteration and would otherwise vary nondeterministically between runs.
pub fn render_order_debug_section(
    err: &mut impl std::io::Write,
    aggregate: &OrderAggregate,
) -> Result<()> {
    writeln!(err, "\n=== DEBUG INFORMATION ===")?;
    writeln!(
        err,
        "Total order events fetched: {}",
        aggregate.total_order_count
    )?;
    writeln!(
        err,
        "Unique orders after deduplication: {}",
        aggregate.unique_order_count
    )?;

    if !aggregate.s_tag_distribution.is_empty() {
        writeln!(err, "\nStatus distribution for order events (s tag):")?;
        let mut sorted: Vec<(&String, &usize)> = aggregate.s_tag_distribution.iter().collect();
        sorted.sort_by(|a, b| a.0.cmp(b.0));
        for (status, count) in sorted {
            writeln!(err, "  s='{}': {} events", status, count)?;
        }
    } else {
        writeln!(err, "\nNo order events found with s tags")?;
    }
    writeln!(err, "========================\n")?;
    Ok(())
}

pub fn render_report_header(out: &mut impl std::io::Write, public_key: PublicKey) -> Result<()> {
    writeln!(
        out,
        "\n{}",
        "========================================".cyan()
    )?;
    writeln!(
        out,
        "{}",
        "     MOSTRO NODE REPUTATION REPORT     ".cyan().bold()
    )?;
    writeln!(out, "{}", "========================================".cyan())?;
    writeln!(out, "Node: {}", public_key.to_bech32()?)?;
    Ok(())
}

pub fn render_longevity_section(
    out: &mut impl std::io::Write,
    days_active: Option<f64>,
    instance_started: Option<i64>,
) -> Result<()> {
    writeln!(
        out,
        "{}",
        "----------------------------------------".dimmed()
    )?;
    writeln!(out, "{}", "LONGEVITY".bold())?;
    match (instance_started, days_active) {
        (Some(start_ts), Some(days)) => {
            writeln!(
                out,
                "  First Activity:  {}",
                chrono::DateTime::from_timestamp(start_ts, 0).unwrap_or_default()
            )?;
            writeln!(out, "  Days Active:     {:.1} days", days)?;
        }
        (None, Some(days)) => {
            writeln!(
                out,
                "  {} Days Active:     {:.1} days (estimated from orders)",
                "⚠".yellow(),
                days
            )?;
        }
        (_, None) => {
            writeln!(
                out,
                "  {} Days Active:     N/A (no dev-fee anchor or successful orders)",
                "⚠".yellow()
            )?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn render_liveness_section(
    out: &mut impl std::io::Write,
    last_successful_trade_at: Option<i64>,
    now: i64,
    days_since_last: Option<u64>,
) -> Result<()> {
    writeln!(
        out,
        "{}",
        "----------------------------------------".dimmed()
    )?;
    writeln!(out, "{}", "LIVENESS".bold())?;
    if let (Some(last_successful_trade_at), Some(days_since_last)) =
        (last_successful_trade_at, days_since_last)
    {
        let relative_time = format_relative_time(last_successful_trade_at, now);
        let last_trade_display = format!(
            "  Last Trade:      {} ({})",
            chrono::DateTime::from_timestamp(last_successful_trade_at, 0).unwrap_or_default(),
            relative_time
        );

        // Color based on activity status
        if days_since_last > 30 {
            writeln!(out, "{}", last_trade_display.red())?;
            writeln!(
                out,
                "  Days Since Last: {} {}",
                days_since_last,
                "INACTIVE".red().bold()
            )?;
        } else if days_since_last > 7 {
            writeln!(out, "{}", last_trade_display.yellow())?;
            writeln!(
                out,
                "  Days Since Last: {} {}",
                days_since_last,
                "LOW ACTIVITY".yellow()
            )?;
        } else {
            writeln!(out, "{}", last_trade_display.green())?;
            writeln!(
                out,
                "  Days Since Last: {} {}",
                days_since_last,
                "ACTIVE".green()
            )?;
        }
    } else {
        writeln!(out, "  {} No successful trades recorded", "⚠".yellow())?;
    }
    Ok(())
}

pub fn render_recent_activity_section(
    out: &mut impl std::io::Write,
    trades_7d: usize,
    trades_30d: usize,
    trades_90d: usize,
) -> Result<()> {
    writeln!(
        out,
        "{}",
        "----------------------------------------".dimmed()
    )?;
    writeln!(out, "{}", "RECENT ACTIVITY".bold())?;
    writeln!(out, "  Last 7 days:     {} trades", trades_7d)?;
    writeln!(out, "  Last 30 days:    {} trades", trades_30d)?;
    writeln!(out, "  Last 90 days:    {} trades", trades_90d)?;
    Ok(())
}

pub fn render_activity_consistency_section(
    out: &mut impl std::io::Write,
    active_days_30d: usize,
    max_inactive_gap: usize,
) -> Result<()> {
    writeln!(
        out,
        "{}",
        "----------------------------------------".dimmed()
    )?;
    writeln!(out, "{}", "ACTIVITY CONSISTENCY (30 days)".bold())?;
    writeln!(out, "  Active Days:     {}/30", active_days_30d)?;
    if max_inactive_gap > 7 {
        writeln!(
            out,
            "  Max Inactive Gap: {} days {}",
            max_inactive_gap,
            "⚠".yellow()
        )?;
    } else {
        writeln!(out, "  Max Inactive Gap: {} days", max_inactive_gap)?;
    }
    Ok(())
}

pub fn render_cumulative_performance_section(
    out: &mut impl std::io::Write,
    successful_orders: usize,
    total_volume_sats: u64,
) -> Result<()> {
    writeln!(
        out,
        "{}",
        "----------------------------------------".dimmed()
    )?;
    writeln!(out, "{}", "CUMULATIVE PERFORMANCE".bold())?;
    writeln!(out, "  Successful Trades: {}", successful_orders)?;
    writeln!(
        out,
        "  Total Volume:      {} sats ({:.4} BTC)",
        total_volume_sats,
        total_volume_sats as f64 / 100_000_000.0
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn render_trade_statistics_section(
    out: &mut impl std::io::Write,
    min_trade: Option<u64>,
    max_trade: Option<u64>,
    mean_trade: Option<f64>,
    median_trade: Option<f64>,
) -> Result<()> {
    writeln!(
        out,
        "{}",
        "----------------------------------------".dimmed()
    )?;
    writeln!(out, "{}", "TRADE STATISTICS".bold())?;
    match (min_trade, max_trade, mean_trade, median_trade) {
        (Some(min_trade), Some(max_trade), Some(mean_trade), Some(median_trade)) => {
            writeln!(out, "  Min Trade:       {} sats", min_trade)?;
            writeln!(out, "  Max Trade:       {} sats", max_trade)?;
            writeln!(out, "  Mean Trade:      {:.0} sats", mean_trade)?;
            writeln!(out, "  Median Trade:    {:.1} sats", median_trade)?;
        }
        _ => {
            writeln!(
                out,
                "  {} N/A (no successful orders with a parseable amount)",
                "⚠".yellow()
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered(f: impl FnOnce(&mut Vec<u8>) -> Result<()>) -> String {
        let mut out: Vec<u8> = Vec::new();
        f(&mut out).expect("render succeeds");
        String::from_utf8(out).expect("valid utf8")
    }

    #[test]
    fn render_longevity_section_shows_first_activity_when_dev_fee_anchor_present() {
        let output = rendered(|out| render_longevity_section(out, Some(90.0), Some(1_700_000_000)));
        assert!(output.contains("First Activity"));
        assert!(output.contains("Days Active:     90.0 days"));
    }

    #[test]
    fn render_longevity_section_shows_estimated_from_orders_when_only_fallback_available() {
        let output = rendered(|out| render_longevity_section(out, Some(5.0), None));
        assert!(output.contains("estimated from orders"));
        assert!(!output.contains("First Activity"));
    }

    #[test]
    fn render_longevity_section_shows_not_applicable_when_neither_anchor_exists() {
        let output = rendered(|out| render_longevity_section(out, None, None));
        assert!(output.contains("N/A"));
    }

    #[test]
    fn render_liveness_section_shows_last_trade_when_present() {
        let now = 1_700_100_000;
        let output =
            rendered(|out| render_liveness_section(out, Some(1_700_000_000), now, Some(1)));
        assert!(output.contains("Last Trade"));
    }

    #[test]
    fn render_liveness_section_reports_no_successful_trades_when_not_applicable() {
        let output = rendered(|out| render_liveness_section(out, None, 1_700_000_000, None));
        assert!(output.contains("No successful trades recorded"));
    }

    #[test]
    fn render_trade_statistics_section_shows_values_when_all_present() {
        let output = rendered(|out| {
            render_trade_statistics_section(out, Some(10), Some(40), Some(25.0), Some(25.0))
        });
        assert!(output.contains("Min Trade:       10 sats"));
        assert!(output.contains("Median Trade:    25.0 sats"));
    }

    #[test]
    fn render_trade_statistics_section_shows_not_applicable_when_any_field_missing() {
        let output = rendered(|out| render_trade_statistics_section(out, None, None, None, None));
        assert!(output.contains("N/A"));
        assert!(!output.contains("Min Trade"));
    }
}
