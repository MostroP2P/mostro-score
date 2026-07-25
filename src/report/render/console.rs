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
pub fn render_dev_fee_section(
    out: &mut impl std::io::Write,
    err: &mut impl std::io::Write,
    aggregate: &DevFeeAggregate,
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
        writeln!(
            err,
            "Falling back to order timestamps for days_active calculation.\n"
        )?;
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

/// PR 3: scoped to what this branch actually knows is empty — dev-fee and order history
/// — not "no events found" in general, since a node reaching this branch may still have
/// usable dispute or instance-status data (PR 3's exit-code-4 gate already confirmed
/// that, or `run()` would have returned `AppError::NoUsableEvents` before this point).
pub fn render_no_events_found(out: &mut impl std::io::Write) -> Result<()> {
    writeln!(out, "No dev fee or order history found for this node.")?;
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
    days_active: f64,
    instance_started: Option<i64>,
) -> Result<()> {
    writeln!(
        out,
        "{}",
        "----------------------------------------".dimmed()
    )?;
    writeln!(out, "{}", "LONGEVITY".bold())?;
    if let Some(start_ts) = instance_started {
        writeln!(
            out,
            "  First Activity:  {}",
            chrono::DateTime::from_timestamp(start_ts, 0).unwrap_or_default()
        )?;
        writeln!(out, "  Days Active:     {:.1} days", days_active)?;
    } else {
        writeln!(
            out,
            "  {} Days Active:     {:.1} days (estimated from orders)",
            "⚠".yellow(),
            days_active
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn render_liveness_section(
    out: &mut impl std::io::Write,
    last_order_ts: i64,
    now: i64,
    days_since_last: u64,
) -> Result<()> {
    writeln!(
        out,
        "{}",
        "----------------------------------------".dimmed()
    )?;
    writeln!(out, "{}", "LIVENESS".bold())?;
    if last_order_ts > 0 {
        let relative_time = format_relative_time(last_order_ts, now);
        let last_trade_display = format!(
            "  Last Trade:      {} ({})",
            chrono::DateTime::from_timestamp(last_order_ts, 0).unwrap_or_default(),
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
    trade_amounts_is_empty: bool,
    min_trade: u64,
    max_trade: u64,
    mean_trade: f64,
    median_trade: u64,
) -> Result<()> {
    if !trade_amounts_is_empty {
        writeln!(
            out,
            "{}",
            "----------------------------------------".dimmed()
        )?;
        writeln!(out, "{}", "TRADE STATISTICS".bold())?;
        writeln!(out, "  Min Trade:       {} sats", min_trade)?;
        writeln!(out, "  Max Trade:       {} sats", max_trade)?;
        writeln!(out, "  Mean Trade:      {:.0} sats", mean_trade)?;
        writeln!(out, "  Median Trade:    {} sats", median_trade)?;
    }
    Ok(())
}

pub fn render_trust_score_section(out: &mut impl std::io::Write, score: u64) -> Result<()> {
    writeln!(
        out,
        "{}",
        "----------------------------------------".dimmed()
    )?;
    let score_display = format!("TRUST SCORE:       {}/100", score);
    if score >= 70 {
        writeln!(out, "{}", score_display.green().bold())?;
    } else if score >= 40 {
        writeln!(out, "{}", score_display.yellow().bold())?;
    } else {
        writeln!(out, "{}", score_display.red().bold())?;
    }
    writeln!(out, "{}", "========================================".cyan())?;
    Ok(())
}
