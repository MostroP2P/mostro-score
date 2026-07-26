//! 002 FR-009: the plain-text renderer must produce no decoration at all -- no ANSI color
//! codes, no box-drawing table characters -- unlike the console renderer.

use mostro_score::report::content::{RecommendationItem, ReportRecommendations};
use mostro_score::report::model::{
    RelayStatus, RelaySummary, Report, ReportActivity, ReportActivityBucket, ReportFetch,
    ReportLiveness, ReportLongevity, ReportNode, ReportStats,
};
use mostro_score::report::render::plain;
use mostro_score::stats::context::{
    FiatBreakdown, FiatCurrencyShare, PaymentMethodBreakdown, PaymentMethodShare, PremiumSignal,
};
use mostro_score::stats::disputes::DisputeSignals;
use mostro_score::stats::grid::Granularity;
use mostro_score::stats::lifecycle::CumulativePerformance;
use mostro_score::stats::trade_size::TradeSizeStats;
use mostro_score::stats::{ActivityConsistency, BondPolicy};

fn rendered_plain(report: &Report) -> String {
    let mut out: Vec<u8> = Vec::new();
    plain::render(&mut out, report).expect("render succeeds");
    String::from_utf8(out).expect("valid utf8")
}

/// A node with a rich history: multiple relays (one failed), a multi-bucket activity
/// grid, every stats sub-object populated, and one recommendation firing (disputes
/// present) -- covers all 5 sections with representative, non-degenerate data.
fn rich_node_report() -> Report {
    Report {
        schema_version: "1.0.0".to_string(),
        generated_at: "2026-07-24T10:15:00Z".to_string(),
        node: ReportNode {
            pubkey_hex: "f8f1e7ffc4d2886f9424523f82da8e295e99fd506d1e43753093da5eefe6b14"
                .to_string(),
            pubkey_npub: "npub1lrc70l7y62yxl9py2glc9k5w990fnl2sd50yxafsj0d9amlxk9xq44h60c"
                .to_string(),
        },
        fetch: ReportFetch {
            relays: vec![
                RelaySummary {
                    url: "wss://relay-a.example".to_string(),
                    status: RelayStatus::Success,
                    error: None,
                },
                RelaySummary {
                    url: "wss://relay-b.example".to_string(),
                    status: RelayStatus::Failed,
                    error: Some("connection refused".to_string()),
                },
            ],
            dev_fee_events: 1,
            order_events: 12,
            unique_orders: 10,
            dispute_events: 2,
            instance_status_found: true,
        },
        activity: ReportActivity {
            granularity: Some(Granularity::Monthly),
            range_start: Some("2026-01-01T00:00:00Z".to_string()),
            range_end: Some("2026-03-01T00:00:00Z".to_string()),
            buckets: vec![
                ReportActivityBucket {
                    bucket_start: "2026-01-01T00:00:00Z".to_string(),
                    successful_trades: 3,
                    volume_sats: 150_000,
                    median_trade_sats: Some(50_000.0),
                },
                ReportActivityBucket {
                    bucket_start: "2026-02-01T00:00:00Z".to_string(),
                    successful_trades: 0,
                    volume_sats: 0,
                    median_trade_sats: None,
                },
                ReportActivityBucket {
                    bucket_start: "2026-03-01T00:00:00Z".to_string(),
                    successful_trades: 7,
                    volume_sats: 1_250_000,
                    median_trade_sats: Some(178_571.4),
                },
            ],
        },
        stats: ReportStats {
            longevity: ReportLongevity {
                first_seen_at: Some("2023-11-14T22:13:20Z".to_string()),
                days_active: Some(984.3),
            },
            cumulative: CumulativePerformance {
                total_successful_trades: 10,
                total_volume_sats: 1_400_000,
            },
            trade_size: TradeSizeStats {
                min_trade_sats: Some(10_000),
                max_trade_sats: Some(200_000),
                mean_trade_sats: Some(140_000.0),
                median_trade_sats: Some(125_000.0),
                std_dev_trade_sats: Some(45_000.5),
                coefficient_of_variation: Some(0.36),
            },
            liveness: ReportLiveness {
                last_successful_trade_at: Some("2026-07-01T00:00:00Z".to_string()),
                days_since_last_trade: Some(23),
                successful_trades_last_7d: 1,
                successful_trades_last_30d: 4,
                successful_trades_last_90d: 10,
            },
            consistency: ActivityConsistency {
                active_days_last_30d: 6,
                max_consecutive_inactive_days_last_30d: 9,
            },
            disputes: DisputeSignals {
                total_disputes: 1,
                resolved_disputes: 1,
                active_disputes: 0,
                unknown_status_disputes: 0,
                disputes_per_100_trades: Some(10.0),
            },
            fiat_breakdown: FiatBreakdown {
                orders_considered: 10,
                distribution: Some(vec![
                    FiatCurrencyShare {
                        currency: "USD".to_string(),
                        orders: 7,
                        share_percent: 70.0,
                    },
                    FiatCurrencyShare {
                        currency: "EUR".to_string(),
                        orders: 3,
                        share_percent: 30.0,
                    },
                ]),
            },
            payment_method_breakdown: PaymentMethodBreakdown {
                total_mentions: 10,
                distribution: Some(vec![PaymentMethodShare {
                    method: "SEPA".to_string(),
                    mentions: 10,
                    share_percent: 100.0,
                }]),
            },
            premium: PremiumSignal {
                premium_baseline_percent: Some(2.5),
                premium_dispersion_percent: Some(0.8),
            },
            bond_policy: BondPolicy { status: "enabled" },
        },
        recommendations: ReportRecommendations {
            nothing_notable: false,
            items: vec![RecommendationItem {
                id: "disputes_present".to_string(),
                metric: Some("stats.disputes.disputes_per_100_trades".to_string()),
                message: "This node has had 1 dispute(s) recorded, alongside 10 completed \
                          trade(s). Review its dispute history before trading with it."
                    .to_string(),
            }],
        },
    }
}

/// 002 FR-009: plain-text output has no decoration at all -- no ANSI color codes, no
/// box-drawing table characters -- unlike the console renderer.
#[test]
fn plain_text_renderer_never_emits_ansi_escape_codes_or_box_drawing_characters() {
    let output = rendered_plain(&rich_node_report());
    assert!(!output.contains('\u{1b}'));
    assert!(!output.contains('│'));
    assert!(!output.contains('┌'));
}
