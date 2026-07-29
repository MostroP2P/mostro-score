//! The plain-text renderer (002 FR-009): the same 5 ordered sections and content as the
//! console renderer (`report::render::console`) -- the same section headings, the same
//! per-metric labels and explanatory context (FR-008b) -- with no color and no
//! decoration. Every metric is its own `label: value` line, including each field of a
//! repeated record (a relay, an activity bucket, a breakdown share), which becomes one
//! line per metric rather than a table row, so the output stays easy to grep or parse
//! line by line in scripts. `--sections` (003 FR-008) may narrow the 4 filterable
//! sections, matching the console renderer; the node identity header always renders.

use crate::report::content::{ReportRecommendations, SectionFilter};
use crate::report::format::{
    display_or_not_applicable, format_decimal_thousands, format_sats_thousands, granularity_label,
    relative_time_from_rfc3339,
};
use crate::report::model::{
    RelayStatus, Report, ReportActivity, ReportFetch, ReportNode, ReportStats,
};
use std::io::Write;

/// 002 FR-002: the node identity header -- both pubkey encodings, so a trader can confirm
/// they queried the intended node.
fn render_node_section(out: &mut impl Write, node: &ReportNode) -> std::io::Result<()> {
    writeln!(out, "NODE IDENTITY")?;
    writeln!(out, "Pubkey (npub): {}", node.pubkey_npub)?;
    writeln!(out, "Pubkey (hex):  {}", node.pubkey_hex)?;
    writeln!(out)
}

/// 002 FR-003: which relays succeeded or failed, plus the deduplicated per-kind event
/// counts backing every other section of the report. Each relay's URL identifies its
/// record, and every metric gets its own `label: value` line per FR-009, not combined
/// with others.
fn render_fetch_section(out: &mut impl Write, fetch: &ReportFetch) -> std::io::Result<()> {
    writeln!(out, "RELAY FETCH SUMMARY")?;

    for relay in &fetch.relays {
        let status = match relay.status {
            RelayStatus::Success => "success",
            RelayStatus::Failed => "failed",
        };
        writeln!(out, "Relay {} Status: {status}", relay.url)?;
        writeln!(
            out,
            "Relay {} Error: {}",
            relay.url,
            relay.error.as_deref().unwrap_or("-")
        )?;
    }

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

/// 002 FR-004/FR-005: one line group per time bucket. A node with zero successful orders
/// has no order timestamp to anchor a range on (002 FR-019's zero-order Edge Case), so
/// this shows an explicit message instead of an empty grid. Each bucket's start time
/// identifies its record, and every metric gets its own line per FR-009.
fn render_activity_section(out: &mut impl Write, activity: &ReportActivity) -> std::io::Result<()> {
    writeln!(out, "ACTIVITY GRID")?;

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

    for bucket in &activity.buckets {
        writeln!(
            out,
            "Bucket {} Trades: {}",
            bucket.bucket_start, bucket.successful_trades
        )?;
        writeln!(
            out,
            "Bucket {} Volume: {} sats",
            bucket.bucket_start,
            format_sats_thousands(bucket.volume_sats)
        )?;
        writeln!(
            out,
            "Bucket {} Median Trade: {} sats",
            bucket.bucket_start,
            bucket
                .median_trade_sats
                .map(format_decimal_thousands)
                .unwrap_or_else(|| display_or_not_applicable::<f64>(None))
        )?;
    }
    writeln!(out)
}

/// 002 FR-006/FR-007, FR-008b: the general statistics section -- the same sub-block
/// headings and per-field labels as the console renderer, each carrying enough context
/// that a trader unfamiliar with Mostro's reputation metrics understands what it
/// measures without leaving the tool.
fn render_stats_section(
    out: &mut impl Write,
    stats: &ReportStats,
    now_rfc3339: &str,
) -> std::io::Result<()> {
    writeln!(out, "GENERAL STATISTICS")?;

    writeln!(out, "Longevity (time since this node's first activity)")?;
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

    writeln!(out, "Cumulative Performance (lifetime trade history)")?;
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
        "Trade Statistics (higher consistency, i.e. lower coefficient of variation, is favorable)"
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

    writeln!(out, "Liveness (most direct signal of current activity)")?;
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
            writeln!(out, "Last successful trade: No successful trades recorded")?;
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

    writeln!(out, "Activity Consistency (fixed 30-day window)")?;
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
        "Dispute Signals (no cross-node baseline; raw counts only)"
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

    writeln!(out, "Fiat Breakdown")?;
    match &stats.fiat_breakdown.distribution {
        Some(distribution) if !distribution.is_empty() => {
            for share in distribution {
                writeln!(out, "Fiat {} Orders: {}", share.currency, share.orders)?;
                writeln!(
                    out,
                    "Fiat {} Share: {}%",
                    share.currency,
                    format_decimal_thousands(share.share_percent)
                )?;
            }
        }
        _ => writeln!(out, "No fiat currency data available.")?,
    }
    writeln!(out)?;

    writeln!(out, "Payment Method Breakdown")?;
    match &stats.payment_method_breakdown.distribution {
        Some(distribution) if !distribution.is_empty() => {
            for share in distribution {
                writeln!(
                    out,
                    "Payment Method {} Mentions: {}",
                    share.method, share.mentions
                )?;
                writeln!(
                    out,
                    "Payment Method {} Share: {}%",
                    share.method,
                    format_decimal_thousands(share.share_percent)
                )?;
            }
        }
        _ => writeln!(out, "No payment-method data available.")?,
    }
    writeln!(out)?;

    writeln!(
        out,
        "Premium Signal (compared against this node's own history only)"
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
        "Bond Policy (whether this node requires a bond deposit from traders)"
    )?;
    writeln!(out, "Status: {}", stats.bond_policy.status)?;
    writeln!(out)
}

/// 002 FR-008/FR-008a: plain-language guidance, explicitly stating there is nothing
/// notable to flag when no trigger fired, rather than omitting the block.
fn render_recommendations_section(
    out: &mut impl Write,
    recommendations: &ReportRecommendations,
) -> std::io::Result<()> {
    writeln!(out, "RECOMMENDATIONS")?;
    if recommendations.nothing_notable {
        writeln!(out, "Nothing notable to flag.")?;
    } else {
        for item in &recommendations.items {
            writeln!(out, "- {}", item.message)?;
        }
    }
    writeln!(out)
}

/// The plain-text renderer's public entry point (002 FR-009): renders every one of the
/// `Report`'s 5 ordered sections to `out`, except that `sections` (003 FR-008) may
/// narrow the 4 filterable ones; the node identity header always renders, unaffected
/// by `sections`. The same content and labels the console renderer shows, with no
/// color and no decorative tables, so the output stays easy to grep or parse line by
/// line in scripts.
pub fn render(
    out: &mut impl Write,
    report: &Report,
    sections: &SectionFilter,
) -> std::io::Result<()> {
    render_node_section(out, &report.node)?;
    if sections.fetch {
        render_fetch_section(out, &report.fetch)?;
    }
    if sections.activity {
        render_activity_section(out, &report.activity)?;
    }
    if sections.stats {
        render_stats_section(out, &report.stats, &report.generated_at)?;
    }
    if sections.recommendations {
        render_recommendations_section(out, &report.recommendations)?;
    }
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
    fn render_node_section_shows_both_pubkey_encodings_with_no_decoration() {
        let node = ReportNode {
            pubkey_hex: "abcd".to_string(),
            pubkey_npub: "npub1abcd".to_string(),
        };
        let output = rendered(|out| render_node_section(out, &node));
        assert!(output.contains("Pubkey (hex):  abcd"));
        assert!(output.contains("Pubkey (npub): npub1abcd"));
    }

    #[test]
    fn render_activity_section_shows_a_message_when_the_grid_is_empty() {
        let activity = ReportActivity {
            granularity: None,
            range_start: None,
            range_end: None,
            buckets: Vec::new(),
        };
        let output = rendered(|out| render_activity_section(out, &activity));
        assert!(output.contains("No order history"));
    }

    /// FR-009: plain-text carries the same explanatory content as console (FR-008b),
    /// just without color or a decorative table -- not a bare machine-oriented path.
    #[test]
    fn render_stats_section_preserves_the_same_explanatory_headings_as_console() {
        let stats = crate::report::model::ReportStats {
            longevity: crate::report::model::ReportLongevity {
                first_seen_at: None,
                days_active: None,
            },
            cumulative: crate::stats::lifecycle::CumulativePerformance {
                total_successful_trades: 0,
                total_volume_sats: 0,
            },
            trade_size: crate::stats::trade_size::TradeSizeStats {
                min_trade_sats: None,
                max_trade_sats: None,
                mean_trade_sats: None,
                median_trade_sats: None,
                std_dev_trade_sats: None,
                coefficient_of_variation: None,
            },
            liveness: crate::report::model::ReportLiveness {
                last_successful_trade_at: None,
                days_since_last_trade: None,
                successful_trades_last_7d: 0,
                successful_trades_last_30d: 0,
                successful_trades_last_90d: 0,
            },
            consistency: crate::stats::ActivityConsistency {
                active_days_last_30d: 0,
                max_consecutive_inactive_days_last_30d: 30,
            },
            disputes: crate::stats::disputes::DisputeSignals {
                total_disputes: 0,
                resolved_disputes: 0,
                active_disputes: 0,
                unknown_status_disputes: 0,
                disputes_per_100_trades: None,
            },
            fiat_breakdown: crate::stats::context::FiatBreakdown {
                orders_considered: 0,
                distribution: None,
            },
            payment_method_breakdown: crate::stats::context::PaymentMethodBreakdown {
                total_mentions: 0,
                distribution: None,
            },
            premium: crate::stats::context::PremiumSignal {
                premium_baseline_percent: None,
                premium_dispersion_percent: None,
            },
            bond_policy: crate::stats::BondPolicy { status: "unknown" },
        };

        let output = rendered(|out| render_stats_section(out, &stats, "2026-07-24T10:15:00Z"));

        assert!(output.contains("Longevity (time since this node's first activity)"));
        assert!(output.contains(
            "Trade Statistics (higher consistency, i.e. lower coefficient of variation, is favorable)"
        ));
        assert!(output.contains("Dispute Signals (no cross-node baseline; raw counts only)"));
        assert!(output.contains("Premium Signal (compared against this node's own history only)"));
    }

    #[test]
    fn plain_text_output_never_contains_ansi_escape_codes_or_box_drawing_characters() {
        let node = ReportNode {
            pubkey_hex: "abcd".to_string(),
            pubkey_npub: "npub1abcd".to_string(),
        };
        let output = rendered(|out| render_node_section(out, &node));
        assert!(!output.contains('\u{1b}'));
        assert!(!output.contains('│'));
        assert!(!output.contains('┌'));
    }

    /// FR-009: no top-level section heading keeps the `=== ... ===` banner style --
    /// "no color or decoration" is not limited to the table-vs-line-per-metric example.
    #[test]
    fn no_top_level_section_heading_uses_the_banner_decoration_style() {
        let node = ReportNode {
            pubkey_hex: "abcd".to_string(),
            pubkey_npub: "npub1abcd".to_string(),
        };
        let output = rendered(|out| render_node_section(out, &node));
        assert!(!output.contains("==="));
    }

    /// FR-009: "each metric rendered as one `label: value` line" means each metric gets
    /// its own line, even for a repeated record (a relay here) -- not several fields
    /// combined onto one line with a separator.
    #[test]
    fn render_fetch_section_puts_each_relay_metric_on_its_own_line() {
        let fetch = ReportFetch {
            relays: vec![crate::report::model::RelaySummary {
                url: "wss://relay-a.example".to_string(),
                status: RelayStatus::Failed,
                error: Some("connection refused".to_string()),
            }],
            dev_fee_events: 0,
            order_events: 0,
            unique_orders: 0,
            dispute_events: 0,
            instance_status_found: false,
        };
        let output = rendered(|out| render_fetch_section(out, &fetch));

        assert!(!output.contains('|'));
        assert!(output.contains("Relay wss://relay-a.example Status: failed"));
        assert!(output.contains("Relay wss://relay-a.example Error: connection refused"));
    }

    fn empty_report() -> Report {
        crate::report::model::Report {
            schema_version: "2.0.0".to_string(),
            node: ReportNode {
                pubkey_hex: "abcd".to_string(),
                pubkey_npub: "npub1abcd".to_string(),
            },
            fetch: ReportFetch {
                relays: vec![],
                dev_fee_events: 0,
                order_events: 0,
                unique_orders: 0,
                dispute_events: 0,
                instance_status_found: false,
            },
            activity: ReportActivity {
                granularity: None,
                range_start: None,
                range_end: None,
                buckets: Vec::new(),
            },
            stats: crate::report::model::ReportStats {
                longevity: crate::report::model::ReportLongevity {
                    first_seen_at: None,
                    days_active: None,
                },
                cumulative: crate::stats::lifecycle::CumulativePerformance {
                    total_successful_trades: 0,
                    total_volume_sats: 0,
                },
                trade_size: crate::stats::trade_size::TradeSizeStats {
                    min_trade_sats: None,
                    max_trade_sats: None,
                    mean_trade_sats: None,
                    median_trade_sats: None,
                    std_dev_trade_sats: None,
                    coefficient_of_variation: None,
                },
                liveness: crate::report::model::ReportLiveness {
                    last_successful_trade_at: None,
                    days_since_last_trade: None,
                    successful_trades_last_7d: 0,
                    successful_trades_last_30d: 0,
                    successful_trades_last_90d: 0,
                },
                consistency: crate::stats::ActivityConsistency {
                    active_days_last_30d: 0,
                    max_consecutive_inactive_days_last_30d: 30,
                },
                disputes: crate::stats::disputes::DisputeSignals {
                    total_disputes: 0,
                    resolved_disputes: 0,
                    active_disputes: 0,
                    unknown_status_disputes: 0,
                    disputes_per_100_trades: None,
                },
                fiat_breakdown: crate::stats::context::FiatBreakdown {
                    orders_considered: 0,
                    distribution: None,
                },
                payment_method_breakdown: crate::stats::context::PaymentMethodBreakdown {
                    total_mentions: 0,
                    distribution: None,
                },
                premium: crate::stats::context::PremiumSignal {
                    premium_baseline_percent: None,
                    premium_dispersion_percent: None,
                },
                bond_policy: crate::stats::BondPolicy { status: "unknown" },
            },
            recommendations: ReportRecommendations {
                nothing_notable: true,
                items: Vec::new(),
            },
            generated_at: "2026-07-24T10:15:00Z".to_string(),
        }
    }

    /// 003 FR-008: the node identity header always renders regardless of `--sections`,
    /// while a narrowed `SectionFilter` suppresses the excluded sections' output.
    #[test]
    fn render_honors_a_narrowed_section_filter_while_the_node_header_always_renders() {
        let report = empty_report();
        let sections = SectionFilter {
            fetch: false,
            activity: false,
            stats: true,
            recommendations: false,
        };

        let output = rendered(|out| render(out, &report, &sections));

        assert!(output.contains("NODE IDENTITY"));
        assert!(!output.contains("RELAY FETCH SUMMARY"));
        assert!(!output.contains("ACTIVITY GRID"));
        assert!(output.contains("GENERAL STATISTICS"));
        assert!(!output.contains("RECOMMENDATIONS"));
    }

    /// 003 FR-008: `SectionFilter::all()` (the omitted-flag default) renders every
    /// section, matching current behavior.
    #[test]
    fn render_shows_every_section_with_the_default_unfiltered_section_set() {
        let report = empty_report();

        let output = rendered(|out| render(out, &report, &SectionFilter::all()));

        assert!(output.contains("NODE IDENTITY"));
        assert!(output.contains("RELAY FETCH SUMMARY"));
        assert!(output.contains("ACTIVITY GRID"));
        assert!(output.contains("GENERAL STATISTICS"));
        assert!(output.contains("RECOMMENDATIONS"));
    }
}
