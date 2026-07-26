//! The report's top-level data model (002 FR-001, FR-002, FR-006, FR-007): the JSON-
//! serializable `Report` struct this PR assembles from PR 3's `RelayFetchOutcome`/
//! `RelayConnectionOutcome` and PR 6's now-complete `NodeMetrics`. Not yet wired into
//! `run()` or any renderer — this PR only builds and unit-tests the model in isolation.

use crate::fetch::client::RelayConnectionOutcome;
use crate::fetch::filters_summary::RelayFetchOutcome;
use crate::stats::context::{FiatBreakdown, PaymentMethodBreakdown, PremiumSignal};
use crate::stats::disputes::DisputeSignals;
use crate::stats::grid::{ActivityGrid, Granularity};
use crate::stats::lifecycle::CumulativePerformance;
use crate::stats::trade_size::TradeSizeStats;
use crate::stats::{ActivityConsistency, BondPolicy, NodeMetrics};
use chrono::{DateTime, SecondsFormat, Utc};
use nostr_sdk::prelude::*;
use serde::Serialize;

const SCHEMA_VERSION: &str = "1.0.0";

/// Renders a stored epoch-second timestamp as an RFC 3339 UTC string (with a literal
/// `Z` suffix, matching plan.md's JSON examples), rather than the bare epoch integer
/// `NodeMetrics` stores it as. Pure formatting, not a new computation: every timestamp
/// this report shows is already measured in UTC.
fn rfc3339_from_epoch_seconds(timestamp: i64) -> String {
    DateTime::<Utc>::from_timestamp(timestamp, 0)
        .unwrap_or_default()
        .to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// 002 FR-002: the node identity header. Neither field is ever `null`: a `Report` only
/// exists once the pubkey has already parsed successfully.
#[derive(Debug, Clone, Serialize)]
pub struct ReportNode {
    pub pubkey_hex: String,
    pub pubkey_npub: String,
}

/// Pure formatting of an already-parsed `PublicKey` into both encodings the report
/// schema requires; no new parsing or validation logic.
pub fn assemble_node_section(public_key: PublicKey) -> Result<ReportNode> {
    Ok(ReportNode {
        pubkey_hex: public_key.to_hex(),
        pubkey_npub: public_key.to_bech32()?,
    })
}

/// 002 FR-003: whether one configured relay succeeded or failed. Serializes as a
/// lowercase JSON string (`"success"` / `"failed"`) per the schema's string-union type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RelayStatus {
    Success,
    Failed,
}

/// One configured relay's fetch outcome (002 FR-003).
#[derive(Debug, Clone, Serialize)]
pub struct RelaySummary {
    pub url: String,
    pub status: RelayStatus,
    pub error: Option<String>,
}

/// 002 FR-003: the relay fetch summary — one `RelaySummary` per configured relay, plus
/// the deduplicated per-kind event counts this report's other sections draw from.
#[derive(Debug, Clone, Serialize)]
pub struct ReportFetch {
    pub relays: Vec<RelaySummary>,
    pub dev_fee_events: usize,
    pub order_events: usize,
    pub unique_orders: usize,
    pub dispute_events: usize,
    pub instance_status_found: bool,
}

/// 002 FR-003: one `RelaySummary` per configured relay, mapped one to one from
/// `RelayConnectionOutcome::ordered` (already in the user's configured `--relays`
/// order — no merge logic belongs here), plus `RelayFetchOutcome`'s already-computed
/// per-kind counts, copied straight across with no new aggregation.
pub fn assemble_fetch_section(
    connection: &RelayConnectionOutcome,
    outcome: &RelayFetchOutcome,
) -> ReportFetch {
    let relays: Vec<RelaySummary> = connection
        .ordered
        .iter()
        .map(|relay| RelaySummary {
            url: relay.url.clone(),
            status: if relay.succeeded {
                RelayStatus::Success
            } else {
                RelayStatus::Failed
            },
            error: relay.error.clone(),
        })
        .collect();

    ReportFetch {
        relays,
        dev_fee_events: outcome.dev_fee_events,
        order_events: outcome.order_events,
        unique_orders: outcome.unique_orders,
        dispute_events: outcome.dispute_events,
        instance_status_found: outcome.instance_status_found,
    }
}

/// One row of `activity.buckets[]` (002 FR-004): `bucket_start` re-typed as an RFC 3339
/// string, matching every other timestamp field in this schema, instead of
/// `stats::grid::GridBucket`'s bare epoch integer.
#[derive(Debug, Clone, Serialize)]
pub struct ReportActivityBucket {
    pub bucket_start: String,
    pub successful_trades: usize,
    pub volume_sats: u64,
    pub median_trade_sats: Option<f64>,
}

/// 002 FR-004/FR-005: the activity grid. `granularity`/`range_start`/`range_end` are
/// `null` and `buckets` is empty only when the node has zero successful orders (FR-019's
/// zero-order Edge Case) — the block stays present either way, per FR-012's "every key
/// always present" rule.
#[derive(Debug, Clone, Serialize)]
pub struct ReportActivity {
    pub granularity: Option<Granularity>,
    pub range_start: Option<String>,
    pub range_end: Option<String>,
    pub buckets: Vec<ReportActivityBucket>,
}

/// Pure struct assembly from `stats::grid`'s already-computed `ActivityGrid` — no new
/// bucketing logic, only the RFC 3339 string formatting every other timestamp field in
/// this schema already uses.
pub fn assemble_activity_section(grid: &ActivityGrid) -> ReportActivity {
    ReportActivity {
        granularity: grid.granularity,
        range_start: grid.range_start.map(rfc3339_from_epoch_seconds),
        range_end: grid.range_end.map(rfc3339_from_epoch_seconds),
        buckets: grid
            .buckets
            .iter()
            .map(|bucket| ReportActivityBucket {
                bucket_start: rfc3339_from_epoch_seconds(bucket.bucket_start),
                successful_trades: bucket.successful_trades,
                volume_sats: bucket.volume_sats,
                median_trade_sats: bucket.median_trade_sats,
            })
            .collect(),
    }
}

/// 002 FR-008/FR-008a: the recommendations block. Out of scope for this PR (implemented
/// from T141 onward) — a minimal placeholder so `Report` compiles with its full
/// 5-section shape now.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ReportRecommendations {}

/// `stats.longevity` (001 FR-001): `first_seen_at` re-typed as an RFC 3339 string per
/// plan.md's JSON contract, instead of `stats::lifecycle::Longevity`'s bare epoch
/// integer.
#[derive(Debug, Clone, Serialize)]
pub struct ReportLongevity {
    pub first_seen_at: Option<String>,
    pub days_active: Option<f64>,
}

/// `stats.liveness` (001 FR-004): field names match plan.md's schema table
/// (`successful_trades_last_7d`/`_30d`/`_90d`), and `last_successful_trade_at` is
/// re-typed as an RFC 3339 string — both differ from `stats::lifecycle::Liveness`'s own
/// field names/types, which this wraps rather than recomputes.
#[derive(Debug, Clone, Serialize)]
pub struct ReportLiveness {
    pub last_successful_trade_at: Option<String>,
    pub days_since_last_trade: Option<u64>,
    pub successful_trades_last_7d: usize,
    pub successful_trades_last_30d: usize,
    pub successful_trades_last_90d: usize,
}

/// 002 FR-006/FR-007: general statistics, one sub-object per `NodeMetrics` field.
#[derive(Debug, Clone, Serialize)]
pub struct ReportStats {
    pub longevity: ReportLongevity,
    pub cumulative: CumulativePerformance,
    pub trade_size: TradeSizeStats,
    pub liveness: ReportLiveness,
    pub consistency: ActivityConsistency,
    pub disputes: DisputeSignals,
    pub fiat_breakdown: FiatBreakdown,
    pub payment_method_breakdown: PaymentMethodBreakdown,
    pub premium: PremiumSignal,
    pub bond_policy: BondPolicy,
}

/// 002 FR-006/FR-007: pure struct assembly from PR 6's now-complete `NodeMetrics` — no
/// new computation, only the RFC 3339 string formatting `ReportLongevity`/
/// `ReportLiveness` need for their two epoch-second timestamp fields.
pub fn assemble_stats_section(metrics: &NodeMetrics) -> ReportStats {
    ReportStats {
        longevity: ReportLongevity {
            first_seen_at: metrics
                .longevity
                .first_seen_at
                .map(rfc3339_from_epoch_seconds),
            days_active: metrics.longevity.days_active,
        },
        cumulative: metrics.cumulative,
        trade_size: metrics.trade_size,
        liveness: ReportLiveness {
            last_successful_trade_at: metrics
                .liveness
                .last_successful_trade_at
                .map(rfc3339_from_epoch_seconds),
            days_since_last_trade: metrics.liveness.days_since_last_trade,
            successful_trades_last_7d: metrics.liveness.trades_last_7d,
            successful_trades_last_30d: metrics.liveness.trades_last_30d,
            successful_trades_last_90d: metrics.liveness.trades_last_90d,
        },
        consistency: metrics.consistency,
        disputes: metrics.disputes,
        fiat_breakdown: metrics.fiat_breakdown.clone(),
        payment_method_breakdown: metrics.payment_method_breakdown.clone(),
        premium: metrics.premium,
        bond_policy: metrics.bond_policy,
    }
}

/// 002 FR-001: the complete report — 5 ordered sections plus `schema_version` (FR-012a)
/// and `generated_at`. `recommendations` is a placeholder in this PR; a later PR
/// populates its real content.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub schema_version: String,
    pub generated_at: String,
    pub node: ReportNode,
    pub fetch: ReportFetch,
    pub activity: ReportActivity,
    pub stats: ReportStats,
    pub recommendations: ReportRecommendations,
}

/// Assembles the complete `Report` from this PR's fully populated sections (`node`,
/// `fetch`, `activity`, `stats`), leaving `recommendations` as its placeholder default.
/// `activity_grid` is pre-computed by the caller (`stats::grid::compute_activity_grid`),
/// not recomputed here — this function only assembles, matching every other
/// `assemble_*_section` function's pattern. Not yet called by `run()` — a later PR wires
/// this in once a renderer exists to consume it.
pub fn assemble_report(
    public_key: PublicKey,
    connection: &RelayConnectionOutcome,
    fetch_outcome: &RelayFetchOutcome,
    metrics: &NodeMetrics,
    activity_grid: &ActivityGrid,
    generated_at: DateTime<Utc>,
) -> Result<Report> {
    Ok(Report {
        schema_version: SCHEMA_VERSION.to_string(),
        generated_at: generated_at.to_rfc3339_opts(SecondsFormat::Secs, true),
        node: assemble_node_section(public_key)?,
        fetch: assemble_fetch_section(connection, fetch_outcome),
        activity: assemble_activity_section(activity_grid),
        stats: assemble_stats_section(metrics),
        recommendations: ReportRecommendations::default(),
    })
}

#[cfg(test)]
mod tests {
    use crate::fetch::client::{RelayConnectFailure, RelayConnectionOutcome, RelayOutcome};
    use crate::fetch::filters_summary::RelayFetchOutcome;
    use crate::report::model::{
        assemble_activity_section, assemble_fetch_section, assemble_node_section, assemble_report,
        assemble_stats_section, Granularity, RelayStatus,
    };
    use crate::stats::grid::{compute_activity_grid, ActivityGrid, GridOrder};
    use crate::stats::NodeMetrics;
    use chrono::{DateTime, Utc};
    use nostr_sdk::prelude::*;

    #[allow(clippy::too_many_arguments)]
    fn sample_metrics() -> NodeMetrics {
        NodeMetrics::compute(
            Some(5 * 86400),
            2,
            400,
            &[100, 300],
            &[10 * 86400, 90 * 86400],
            1,
            0,
            1,
            0,
            &["USD".to_string(), "USD".to_string()],
            &["SEPA".to_string()],
            &[10, 20],
            "enabled",
            100 * 86400,
        )
    }

    #[test]
    fn report_carries_schema_version_generated_at_and_all_five_sections() {
        let public_key = Keys::generate().public_key();
        let metrics = sample_metrics();
        let connection = RelayConnectionOutcome {
            connected_count: 1,
            connected_urls: vec!["wss://relay.example".to_string()],
            failed: vec![],
            ordered: vec![RelayOutcome {
                url: "wss://relay.example".to_string(),
                succeeded: true,
                error: None,
            }],
        };
        let outcome = RelayFetchOutcome {
            dev_fee_events: 1,
            order_events: 2,
            unique_orders: 2,
            has_valid_orders: true,
            dispute_events: 1,
            instance_status_found: true,
        };
        let generated_at = DateTime::parse_from_rfc3339("2026-07-24T10:15:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let activity_grid = compute_activity_grid(&[
            GridOrder {
                created_at: 10 * 86400,
                amount_sats: Some(100),
            },
            GridOrder {
                created_at: 90 * 86400,
                amount_sats: Some(300),
            },
        ]);

        let report = assemble_report(
            public_key,
            &connection,
            &outcome,
            &metrics,
            &activity_grid,
            generated_at,
        )
        .unwrap();

        assert_eq!(report.schema_version, "1.0.0");
        assert_eq!(report.generated_at, "2026-07-24T10:15:00Z");
        assert_eq!(report.node.pubkey_hex, public_key.to_hex());
        assert_eq!(report.fetch.dev_fee_events, 1);
        assert_eq!(report.stats.cumulative.total_successful_trades, 2);
        assert_eq!(report.activity.granularity, Some(Granularity::Daily));
        assert!(!report.activity.buckets.is_empty());
    }

    #[test]
    fn assemble_activity_section_formats_bucket_start_as_rfc3339_and_preserves_values() {
        let grid = compute_activity_grid(&[GridOrder {
            created_at: 0,
            amount_sats: Some(1000),
        }]);

        let activity = assemble_activity_section(&grid);

        assert_eq!(activity.granularity, Some(Granularity::Daily));
        assert_eq!(
            activity.range_start,
            Some("1970-01-01T00:00:00Z".to_string())
        );
        assert_eq!(activity.range_end, Some("1970-01-01T00:00:00Z".to_string()));
        assert_eq!(activity.buckets.len(), 1);
        assert_eq!(
            activity.buckets[0].bucket_start,
            "1970-01-01T00:00:00Z".to_string()
        );
        assert_eq!(activity.buckets[0].successful_trades, 1);
        assert_eq!(activity.buckets[0].volume_sats, 1000);
        assert_eq!(activity.buckets[0].median_trade_sats, Some(1000.0));
    }

    /// Edge Cases (002 FR-019/FR-005): zero successful orders reports the whole activity
    /// block as `null`/empty, not an invented default range.
    #[test]
    fn assemble_activity_section_reports_null_range_when_grid_is_empty() {
        let grid: ActivityGrid = compute_activity_grid(&[]);

        let activity = assemble_activity_section(&grid);

        assert_eq!(activity.granularity, None);
        assert_eq!(activity.range_start, None);
        assert_eq!(activity.range_end, None);
        assert!(activity.buckets.is_empty());
    }

    #[test]
    fn assemble_node_section_reports_both_pubkey_encodings() {
        let public_key = Keys::generate().public_key();

        let node = assemble_node_section(public_key).unwrap();

        assert_eq!(node.pubkey_hex, public_key.to_hex());
        assert_eq!(node.pubkey_npub, public_key.to_bech32().unwrap());
    }

    #[test]
    fn assemble_stats_section_maps_every_node_metrics_field() {
        let metrics = sample_metrics();

        let stats = assemble_stats_section(&metrics);

        assert_eq!(
            stats.longevity.first_seen_at,
            Some("1970-01-06T00:00:00Z".to_string())
        );
        assert_eq!(stats.longevity.days_active, metrics.longevity.days_active);
        assert_eq!(stats.cumulative.total_successful_trades, 2);
        assert_eq!(stats.cumulative.total_volume_sats, 400);
        assert_eq!(stats.trade_size.median_trade_sats, Some(200.0));
        assert_eq!(
            stats.liveness.last_successful_trade_at,
            Some("1970-04-01T00:00:00Z".to_string())
        );
        assert_eq!(
            stats.liveness.successful_trades_last_7d,
            metrics.liveness.trades_last_7d
        );
        assert_eq!(
            stats.consistency.active_days_last_30d,
            metrics.consistency.active_days_last_30d
        );
        assert_eq!(stats.disputes.total_disputes, 1);
        assert_eq!(stats.fiat_breakdown.orders_considered, 2);
        assert_eq!(stats.payment_method_breakdown.total_mentions, 1);
        assert_eq!(stats.premium.premium_baseline_percent, Some(15.0));
        assert_eq!(stats.bond_policy.status, "enabled");
    }

    /// Edge Cases: neither anchor exists, no dev-fee event, no successful trades — both
    /// RFC 3339 timestamp fields must serialize as `None` (JSON `null`), never an
    /// empty-string placeholder.
    #[test]
    fn assemble_stats_section_reports_missing_timestamps_as_none() {
        let metrics = NodeMetrics::compute(
            None,
            0,
            0,
            &[],
            &[],
            0,
            0,
            0,
            0,
            &[],
            &[],
            &[],
            "unknown",
            100 * 86400,
        );

        let stats = assemble_stats_section(&metrics);

        assert_eq!(stats.longevity.first_seen_at, None);
        assert_eq!(stats.liveness.last_successful_trade_at, None);
        assert_eq!(stats.bond_policy.status, "unknown");
    }

    #[test]
    fn assemble_fetch_section_builds_one_relay_summary_per_configured_relay() {
        let connection = RelayConnectionOutcome {
            connected_count: 1,
            connected_urls: vec!["wss://relay-a.example".to_string()],
            failed: vec![RelayConnectFailure {
                url: "wss://relay-b.example".to_string(),
                error: "connection refused".to_string(),
            }],
            ordered: vec![
                RelayOutcome {
                    url: "wss://relay-a.example".to_string(),
                    succeeded: true,
                    error: None,
                },
                RelayOutcome {
                    url: "wss://relay-b.example".to_string(),
                    succeeded: false,
                    error: Some("connection refused".to_string()),
                },
            ],
        };
        let outcome = RelayFetchOutcome {
            dev_fee_events: 3,
            order_events: 4,
            unique_orders: 2,
            has_valid_orders: true,
            dispute_events: 1,
            instance_status_found: true,
        };

        let fetch = assemble_fetch_section(&connection, &outcome);

        assert_eq!(fetch.relays.len(), 2);
        let success = fetch
            .relays
            .iter()
            .find(|relay| relay.url == "wss://relay-a.example")
            .expect("success relay present");
        assert_eq!(success.status, RelayStatus::Success);
        assert_eq!(success.error, None);
        let failed = fetch
            .relays
            .iter()
            .find(|relay| relay.url == "wss://relay-b.example")
            .expect("failed relay present");
        assert_eq!(failed.status, RelayStatus::Failed);
        assert_eq!(failed.error, Some("connection refused".to_string()));
        assert_eq!(fetch.dev_fee_events, 3);
        assert_eq!(fetch.order_events, 4);
        assert_eq!(fetch.unique_orders, 2);
        assert_eq!(fetch.dispute_events, 1);
        assert!(fetch.instance_status_found);
    }

    /// Regression: `fetch.relays[]` preserves the user's originally configured
    /// `--relays` order across the success/failure boundary, not "every success first,
    /// then every failure" — a bug the first implementation had, since it built the
    /// list by concatenating two independently-ordered vectors.
    #[test]
    fn assemble_fetch_section_preserves_interleaved_configured_order() {
        let connection = RelayConnectionOutcome {
            connected_count: 2,
            connected_urls: vec![
                "wss://z-relay.example".to_string(),
                "wss://a-relay.example".to_string(),
            ],
            failed: vec![RelayConnectFailure {
                url: "wss://m-relay.example".to_string(),
                error: "connection refused".to_string(),
            }],
            ordered: vec![
                RelayOutcome {
                    url: "wss://z-relay.example".to_string(),
                    succeeded: true,
                    error: None,
                },
                RelayOutcome {
                    url: "wss://m-relay.example".to_string(),
                    succeeded: false,
                    error: Some("connection refused".to_string()),
                },
                RelayOutcome {
                    url: "wss://a-relay.example".to_string(),
                    succeeded: true,
                    error: None,
                },
            ],
        };
        let outcome = RelayFetchOutcome {
            dev_fee_events: 0,
            order_events: 0,
            unique_orders: 0,
            has_valid_orders: false,
            dispute_events: 0,
            instance_status_found: false,
        };

        let fetch = assemble_fetch_section(&connection, &outcome);

        assert_eq!(
            fetch
                .relays
                .iter()
                .map(|relay| relay.url.as_str())
                .collect::<Vec<_>>(),
            vec![
                "wss://z-relay.example",
                "wss://m-relay.example",
                "wss://a-relay.example",
            ]
        );
    }

    #[test]
    fn relay_status_serializes_as_a_lowercase_json_string() {
        assert_eq!(
            serde_json::to_string(&RelayStatus::Success).unwrap(),
            "\"success\""
        );
        assert_eq!(
            serde_json::to_string(&RelayStatus::Failed).unwrap(),
            "\"failed\""
        );
    }
}
