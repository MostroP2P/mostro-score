//! The console renderer (002 FR-001, FR-013, FR-015): renders a fully assembled
//! `Report`'s 5 ordered sections — node identity, relay fetch summary, activity grid,
//! general statistics, recommendations — to an arbitrary writer, colored and
//! width-adaptive when color is enabled and the destination is a real terminal.
//!
//! PR 7d supersedes PR 1 Step C's original ad hoc `render_*_section` functions entirely
//! (they predated the `Report` model and never showed the fiat/payment-method
//! breakdowns, premium signal, bond policy, disputes, or activity grid PR 5/PR 6/PR 7a-c
//! already compute). The two pure debug/scaffolding blocks those functions printed
//! (`=== SAMPLE EVENTS ===`, `=== DEBUG INFORMATION ===`) are removed outright, not
//! migrated: they were PR1-era debugging aids, never part of spec 002's 5-section report
//! (FR-001), and are superseded by this renderer's own fetch section (FR-003).

use crate::report::content::ReportRecommendations;
use crate::report::format::{
    color_enabled_for_stdout, display_or_not_applicable, format_decimal_thousands,
    format_relative_time, format_sats_thousands,
};
use crate::report::model::{
    RelayStatus, Report, ReportActivity, ReportFetch, ReportNode, ReportStats,
};
use crate::stats::grid::Granularity;
use anstyle::{AnsiColor, Style};
use comfy_table::{presets::UTF8_FULL, ContentArrangement, Table};
use std::io::Write;

fn heading_style() -> Style {
    Style::new().bold()
}

fn warning_style() -> Style {
    Style::new().fg_color(Some(AnsiColor::Yellow.into()))
}

/// Applies `style`'s ANSI codes around `text` only when `color_enabled` is true — the
/// one place every colored line in this renderer goes through, so FR-015's "color
/// disabled" case is a single code path rather than one per call site.
fn styled(text: &str, style: Style, color_enabled: bool) -> String {
    if color_enabled {
        format!("{style}{text}{style:#}")
    } else {
        text.to_string()
    }
}

fn new_table() -> Table {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic);
    table
}

fn heading(out: &mut impl Write, title: &str, color_enabled: bool) -> std::io::Result<()> {
    writeln!(out, "{}", styled(title, heading_style(), color_enabled))
}

/// 002 FR-002: the node identity header — both pubkey encodings, so a trader can confirm
/// they queried the intended node.
fn render_node_section(
    out: &mut impl Write,
    node: &ReportNode,
    color_enabled: bool,
) -> std::io::Result<()> {
    heading(out, "=== NODE IDENTITY ===", color_enabled)?;
    writeln!(out, "Pubkey (npub): {}", node.pubkey_npub)?;
    writeln!(out, "Pubkey (hex):  {}", node.pubkey_hex)?;
    writeln!(out)
}

/// 002 FR-003: which relays succeeded or failed, plus the deduplicated per-kind event
/// counts backing every other section of the report.
fn render_fetch_section(
    out: &mut impl Write,
    fetch: &ReportFetch,
    color_enabled: bool,
) -> std::io::Result<()> {
    heading(out, "=== RELAY FETCH SUMMARY ===", color_enabled)?;

    let mut table = new_table();
    table.set_header(vec!["Relay", "Status", "Error"]);
    for relay in &fetch.relays {
        let status = match relay.status {
            RelayStatus::Success => "success",
            RelayStatus::Failed => "failed",
        };
        table.add_row(vec![
            relay.url.clone(),
            status.to_string(),
            relay.error.clone().unwrap_or_else(|| "-".to_string()),
        ]);
    }
    writeln!(out, "{table}")?;

    writeln!(
        out,
        "Dev-fee events (backs Longevity):        {}",
        fetch.dev_fee_events
    )?;
    writeln!(
        out,
        "Order events (before dedup):             {}",
        fetch.order_events
    )?;
    writeln!(
        out,
        "Unique orders (after dedup):             {}",
        fetch.unique_orders
    )?;
    writeln!(
        out,
        "Dispute events (backs Dispute Signals):  {}",
        fetch.dispute_events
    )?;
    writeln!(
        out,
        "Instance status found (backs Bond Policy): {}",
        if fetch.instance_status_found {
            "yes"
        } else {
            "no"
        }
    )?;
    writeln!(out)
}

fn granularity_label(granularity: Granularity) -> &'static str {
    match granularity {
        Granularity::Daily => "daily",
        Granularity::Monthly => "monthly",
        Granularity::Yearly => "yearly",
    }
}

/// 002 FR-004/FR-005: one row per time bucket. A node with zero successful orders has no
/// order timestamp to anchor a range on (002 FR-019's zero-order Edge Case), so this
/// shows an explicit message instead of an empty table.
fn render_activity_section(
    out: &mut impl Write,
    activity: &ReportActivity,
    color_enabled: bool,
) -> std::io::Result<()> {
    heading(out, "=== ACTIVITY GRID ===", color_enabled)?;

    if activity.buckets.is_empty() {
        writeln!(out, "No order history to build an activity grid from.")?;
        return writeln!(out);
    }

    writeln!(
        out,
        "Granularity: {}",
        activity
            .granularity
            .map(granularity_label)
            .unwrap_or("unknown")
    )?;

    let mut table = new_table();
    table.set_header(vec![
        "Bucket Start",
        "Trades",
        "Volume (sats)",
        "Median Trade (sats)",
    ]);
    for bucket in &activity.buckets {
        table.add_row(vec![
            bucket.bucket_start.clone(),
            bucket.successful_trades.to_string(),
            format_sats_thousands(bucket.volume_sats),
            bucket
                .median_trade_sats
                .map(format_decimal_thousands)
                .unwrap_or_else(|| display_or_not_applicable::<f64>(None)),
        ]);
    }
    writeln!(out, "{table}")?;
    writeln!(out)
}

/// 002 FR-006/FR-007, FR-008b: the general statistics section — one sub-block per
/// `ReportStats` field, each labeled with enough context that a trader unfamiliar with
/// Mostro's reputation metrics understands what it measures without leaving the tool.
fn render_stats_section(
    out: &mut impl Write,
    stats: &ReportStats,
    now_rfc3339: &str,
    color_enabled: bool,
) -> std::io::Result<()> {
    heading(out, "=== GENERAL STATISTICS ===", color_enabled)?;

    writeln!(
        out,
        "-- Longevity (time since this node's first activity) --"
    )?;
    writeln!(
        out,
        "First seen:  {}",
        display_or_not_applicable(stats.longevity.first_seen_at.clone())
    )?;
    writeln!(
        out,
        "Days active: {}",
        stats
            .longevity
            .days_active
            .map(format_decimal_thousands)
            .unwrap_or_else(|| display_or_not_applicable::<f64>(None))
    )?;
    writeln!(out)?;

    writeln!(out, "-- Cumulative Performance (lifetime trade history) --")?;
    writeln!(
        out,
        "Total successful trades: {}",
        stats.cumulative.total_successful_trades
    )?;
    writeln!(
        out,
        "Total volume:            {} sats",
        format_sats_thousands(stats.cumulative.total_volume_sats)
    )?;
    writeln!(out)?;

    writeln!(
        out,
        "-- Trade Statistics (higher consistency, i.e. lower coefficient of variation, is favorable) --"
    )?;
    writeln!(
        out,
        "Min trade:    {} sats",
        stats
            .trade_size
            .min_trade_sats
            .map(format_sats_thousands)
            .unwrap_or_else(|| display_or_not_applicable::<u64>(None))
    )?;
    writeln!(
        out,
        "Max trade:    {} sats",
        stats
            .trade_size
            .max_trade_sats
            .map(format_sats_thousands)
            .unwrap_or_else(|| display_or_not_applicable::<u64>(None))
    )?;
    writeln!(
        out,
        "Mean trade:   {} sats",
        stats
            .trade_size
            .mean_trade_sats
            .map(format_decimal_thousands)
            .unwrap_or_else(|| display_or_not_applicable::<f64>(None))
    )?;
    writeln!(
        out,
        "Median trade: {} sats",
        stats
            .trade_size
            .median_trade_sats
            .map(format_decimal_thousands)
            .unwrap_or_else(|| display_or_not_applicable::<f64>(None))
    )?;
    writeln!(
        out,
        "Std dev:      {} sats",
        stats
            .trade_size
            .std_dev_trade_sats
            .map(format_decimal_thousands)
            .unwrap_or_else(|| display_or_not_applicable::<f64>(None))
    )?;
    writeln!(
        out,
        "Coefficient of variation: {}",
        display_or_not_applicable(stats.trade_size.coefficient_of_variation)
    )?;
    writeln!(out)?;

    writeln!(
        out,
        "-- Liveness (most direct signal of current activity) --"
    )?;
    match &stats.liveness.last_successful_trade_at {
        Some(last_successful_trade_at) => {
            let relative = relative_time_from_rfc3339(last_successful_trade_at, now_rfc3339)
                .unwrap_or_default();
            writeln!(
                out,
                "Last successful trade: {last_successful_trade_at} ({relative})"
            )?;
        }
        None => {
            writeln!(
                out,
                "Last successful trade: {}",
                styled(
                    "No successful trades recorded",
                    warning_style(),
                    color_enabled
                )
            )?;
        }
    }
    writeln!(
        out,
        "Days since last trade: {}",
        display_or_not_applicable(stats.liveness.days_since_last_trade)
    )?;
    writeln!(
        out,
        "Trades last 7 days:   {}",
        stats.liveness.successful_trades_last_7d
    )?;
    writeln!(
        out,
        "Trades last 30 days:  {}",
        stats.liveness.successful_trades_last_30d
    )?;
    writeln!(
        out,
        "Trades last 90 days:  {}",
        stats.liveness.successful_trades_last_90d
    )?;
    writeln!(out)?;

    writeln!(out, "-- Activity Consistency (fixed 30-day window) --")?;
    writeln!(
        out,
        "Active days:                    {}/30",
        stats.consistency.active_days_last_30d
    )?;
    writeln!(
        out,
        "Max consecutive inactive days:   {}",
        stats.consistency.max_consecutive_inactive_days_last_30d
    )?;
    writeln!(out)?;

    writeln!(
        out,
        "-- Dispute Signals (no cross-node baseline; raw counts only) --"
    )?;
    writeln!(
        out,
        "Total disputes:          {}",
        stats.disputes.total_disputes
    )?;
    writeln!(
        out,
        "Resolved disputes:       {}",
        stats.disputes.resolved_disputes
    )?;
    writeln!(
        out,
        "Active disputes:         {}",
        stats.disputes.active_disputes
    )?;
    writeln!(
        out,
        "Unknown-status disputes: {}",
        stats.disputes.unknown_status_disputes
    )?;
    writeln!(
        out,
        "Disputes per 100 trades: {}",
        stats
            .disputes
            .disputes_per_100_trades
            .map(format_decimal_thousands)
            .unwrap_or_else(|| display_or_not_applicable::<f64>(None))
    )?;
    writeln!(out)?;

    writeln!(out, "-- Fiat Breakdown --")?;
    match &stats.fiat_breakdown.distribution {
        Some(distribution) if !distribution.is_empty() => {
            let mut table = new_table();
            table.set_header(vec!["Currency", "Orders", "Share %"]);
            for share in distribution {
                table.add_row(vec![
                    share.currency.clone(),
                    share.orders.to_string(),
                    format_decimal_thousands(share.share_percent),
                ]);
            }
            writeln!(out, "{table}")?;
        }
        _ => writeln!(out, "No fiat currency data available.")?,
    }
    writeln!(out)?;

    writeln!(out, "-- Payment Method Breakdown --")?;
    match &stats.payment_method_breakdown.distribution {
        Some(distribution) if !distribution.is_empty() => {
            let mut table = new_table();
            table.set_header(vec!["Method", "Mentions", "Share %"]);
            for share in distribution {
                table.add_row(vec![
                    share.method.clone(),
                    share.mentions.to_string(),
                    format_decimal_thousands(share.share_percent),
                ]);
            }
            writeln!(out, "{table}")?;
        }
        _ => writeln!(out, "No payment-method data available.")?,
    }
    writeln!(out)?;

    writeln!(
        out,
        "-- Premium Signal (compared against this node's own history only) --"
    )?;
    writeln!(
        out,
        "Baseline (median):    {}",
        stats
            .premium
            .premium_baseline_percent
            .map(format_decimal_thousands)
            .unwrap_or_else(|| display_or_not_applicable::<f64>(None))
    )?;
    writeln!(
        out,
        "Dispersion (std dev): {}",
        stats
            .premium
            .premium_dispersion_percent
            .map(format_decimal_thousands)
            .unwrap_or_else(|| display_or_not_applicable::<f64>(None))
    )?;
    writeln!(out)?;

    writeln!(
        out,
        "-- Bond Policy (whether this node requires a bond deposit from traders) --"
    )?;
    writeln!(out, "Status: {}", stats.bond_policy.status)?;
    writeln!(out)
}

/// 002 FR-008/FR-008a: plain-language guidance, explicitly stating there is nothing
/// notable to flag when no trigger fired, rather than omitting the block.
fn render_recommendations_section(
    out: &mut impl Write,
    recommendations: &ReportRecommendations,
    color_enabled: bool,
) -> std::io::Result<()> {
    heading(out, "=== RECOMMENDATIONS ===", color_enabled)?;
    if recommendations.nothing_notable {
        writeln!(out, "Nothing notable to flag.")?;
    } else {
        for item in &recommendations.items {
            writeln!(out, "- {}", item.message)?;
        }
    }
    writeln!(out)
}

/// Parses two RFC 3339 UTC strings and returns `format_relative_time`'s phrase for the
/// gap between them, or `None` if either fails to parse (defensive only — both strings
/// come from this same process's own `report::model` formatting, so a parse failure here
/// would indicate a bug elsewhere, not bad external input).
fn relative_time_from_rfc3339(timestamp_rfc3339: &str, now_rfc3339: &str) -> Option<String> {
    let timestamp = chrono::DateTime::parse_from_rfc3339(timestamp_rfc3339)
        .ok()?
        .timestamp();
    let now = chrono::DateTime::parse_from_rfc3339(now_rfc3339)
        .ok()?
        .timestamp();
    Some(format_relative_time(timestamp, now))
}

/// The console renderer's public entry point (002 FR-001, FR-013, FR-015): renders every
/// one of the `Report`'s 5 ordered sections to `out`. `force_color` threads PR 9's future
/// `--color`/`--no-color` override through today; `None` defers to the automatic
/// tty/`NO_COLOR`/`TERM=dumb` policy in `report::format::color_enabled_for_stdout`.
pub fn render(
    out: &mut impl Write,
    report: &Report,
    force_color: Option<bool>,
) -> std::io::Result<()> {
    let color_enabled = color_enabled_for_stdout(force_color);

    render_node_section(out, &report.node, color_enabled)?;
    render_fetch_section(out, &report.fetch, color_enabled)?;
    render_activity_section(out, &report.activity, color_enabled)?;
    render_stats_section(out, &report.stats, &report.generated_at, color_enabled)?;
    render_recommendations_section(out, &report.recommendations, color_enabled)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered(f: impl FnOnce(&mut Vec<u8>) -> std::io::Result<()>) -> String {
        let mut out: Vec<u8> = Vec::new();
        f(&mut out).expect("render succeeds");
        String::from_utf8(out).expect("valid utf8")
    }

    #[test]
    fn styled_wraps_text_in_ansi_codes_only_when_color_is_enabled() {
        let plain = styled("hello", heading_style(), false);
        let colored = styled("hello", heading_style(), true);

        assert_eq!(plain, "hello");
        assert_ne!(colored, "hello");
        assert!(colored.contains("hello"));
    }

    #[test]
    fn render_node_section_shows_both_pubkey_encodings() {
        let node = ReportNode {
            pubkey_hex: "abcd".to_string(),
            pubkey_npub: "npub1abcd".to_string(),
        };
        let output = rendered(|out| render_node_section(out, &node, false));
        assert!(output.contains("abcd"));
        assert!(output.contains("npub1abcd"));
    }

    #[test]
    fn render_activity_section_shows_a_message_when_the_grid_is_empty() {
        let activity = ReportActivity {
            granularity: None,
            range_start: None,
            range_end: None,
            buckets: Vec::new(),
        };
        let output = rendered(|out| render_activity_section(out, &activity, false));
        assert!(output.contains("No order history"));
    }

    #[test]
    fn relative_time_from_rfc3339_computes_the_gap_between_two_timestamps() {
        let relative = relative_time_from_rfc3339("2026-07-01T00:00:00Z", "2026-07-08T00:00:00Z");
        assert_eq!(relative, Some("1 week ago".to_string()));
    }
}
