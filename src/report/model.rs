//! The report's top-level data model (002 FR-001, FR-002, FR-006, FR-007): the JSON-
//! serializable `Report` struct this PR assembles from PR 3's `RelayFetchOutcome`/
//! `RelayConnectionOutcome` and PR 6's now-complete `NodeMetrics`. Not yet wired into
//! `run()` or any renderer — this PR only builds and unit-tests the model in isolation.

use crate::fetch::client::RelayConnectionOutcome;
use crate::fetch::filters_summary::RelayFetchOutcome;
use crate::report::content::{assemble_recommendations_section, ReportRecommendations};
use crate::stats::context::{FiatBreakdown, PaymentMethodBreakdown, PremiumSignal};
use crate::stats::disputes::DisputeSignals;
use crate::stats::grid::{ActivityGrid, Granularity};
use crate::stats::lifecycle::CumulativePerformance;
use crate::stats::trade_size::TradeSizeStats;
use crate::stats::{ActivityConsistency, BondPolicy, NodeMetrics};
use chrono::{DateTime, SecondsFormat, Utc};
use nostr_sdk::prelude::*;
use serde::Serialize;
use std::collections::BTreeMap;

/// 002 FR-012a: shared by both the success-path `Report` and the JSON renderer's
/// fatal-error envelope (`report::render::json`), since the envelope is versioned on the
/// same schedule as the report.
pub const SCHEMA_VERSION: &str = "1.0.0";

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
/// and `generated_at`.
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

/// Assembles the complete `Report` from every section, including `recommendations`
/// (002 FR-008, scoped to `report::content`'s 3 currently-implemented triggers — see
/// its module doc comment). `activity_grid` is pre-computed by the caller
/// (`stats::grid::compute_activity_grid`), not recomputed here — this function only
/// assembles, matching every other `assemble_*_section` function's pattern. Called by
/// `run()` in `lib.rs` (PR 7d), which passes the assembled `Report` to
/// `report::render::console::render`.
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
        recommendations: assemble_recommendations_section(metrics),
    })
}

/// plan.md's JSON output contract, `metric_definitions`: one entry's `label`/`meaning`/
/// `unit_and_direction`, all three grounded in FR prose since FR-008b's companion
/// decisions document does not exist in this repository.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct MetricDefinition {
    pub label: &'static str,
    pub meaning: &'static str,
    pub unit_and_direction: &'static str,
}

/// plan.md's JSON output contract: the static `metric_definitions` table, keyed by the
/// same dotted path `recommendations.items[].metric` already uses (for example
/// `stats.trade_size.coefficient_of_variation`). The key set is exhaustive and
/// mechanically checkable, not judgment-based: every field already listed in plan.md's
/// `stats` table (all ten sub-objects), plus `activity.granularity` and
/// `buckets[].median_trade_sats`, plus `fetch.relays[].status` and
/// `fetch.relays[].error` — nothing else. `metric_definitions` is static per build: this
/// function recomputes the same fixed table every call rather than caching a global, since
/// the table itself never varies with a report's contents.
pub fn metric_definitions() -> BTreeMap<&'static str, MetricDefinition> {
    BTreeMap::from([
        // 001 FR-001
        (
            "stats.longevity.first_seen_at",
            MetricDefinition {
                label: "First seen",
                meaning: "The created_at of this node's oldest qualifying dev-fee-payment \
                          event, the earliest activity this report can anchor on.",
                unit_and_direction: "RFC 3339 UTC timestamp; a historical fact, no favorable \
                                     direction.",
            },
        ),
        // 001 FR-001
        (
            "stats.longevity.days_active",
            MetricDefinition {
                label: "Days active",
                meaning: "Elapsed days since first_seen_at, or, when no dev-fee anchor \
                          exists, since this node's first qualifying successful order, \
                          measured to report-generation time.",
                unit_and_direction: "Days; a higher value means a longer observed history.",
            },
        ),
        // 001 FR-002
        (
            "stats.cumulative.total_successful_trades",
            MetricDefinition {
                label: "Total successful trades",
                meaning: "Count of this node's qualifying orders whose deduplicated final \
                          state is s=success.",
                unit_and_direction: "Count; a higher value indicates more completed trade \
                                     history.",
            },
        ),
        // 001 FR-002
        (
            "stats.cumulative.total_volume_sats",
            MetricDefinition {
                label: "Total volume",
                meaning: "Sum of amt across the successful-order set, restricted to orders \
                          whose amt tag parsed as a non-negative integer.",
                unit_and_direction: "Satoshis; a higher value indicates greater cumulative \
                                     trade volume.",
            },
        ),
        // 001 FR-003
        (
            "stats.trade_size.min_trade_sats",
            MetricDefinition {
                label: "Minimum trade size",
                meaning: "Smallest amt among the amt-restricted successful-order set.",
                unit_and_direction: "Satoshis; describes the range, no favorable direction.",
            },
        ),
        // 001 FR-003
        (
            "stats.trade_size.max_trade_sats",
            MetricDefinition {
                label: "Maximum trade size",
                meaning: "Largest amt among the amt-restricted successful-order set.",
                unit_and_direction: "Satoshis; describes the range, no favorable direction.",
            },
        ),
        // 001 FR-003
        (
            "stats.trade_size.mean_trade_sats",
            MetricDefinition {
                label: "Mean trade size",
                meaning: "Arithmetic mean of amt across the amt-restricted successful-order \
                          set.",
                unit_and_direction: "Satoshis; no fixed favorable direction.",
            },
        ),
        // 001 FR-003
        (
            "stats.trade_size.median_trade_sats",
            MetricDefinition {
                label: "Median trade size",
                meaning: "Median amt across the amt-restricted successful-order set — the \
                          primary reference for typical trade size in any risk assessment.",
                unit_and_direction: "Satoshis; no fixed favorable direction.",
            },
        ),
        // 001 FR-010
        (
            "stats.trade_size.std_dev_trade_sats",
            MetricDefinition {
                label: "Trade size standard deviation",
                meaning: "Population standard deviation (divide by N) of amt across the \
                          same amt-restricted set.",
                unit_and_direction: "Satoshis; a lower value indicates less spread around \
                                     the mean trade size.",
            },
        ),
        // 001 FR-010
        (
            "stats.trade_size.coefficient_of_variation",
            MetricDefinition {
                label: "Trade size coefficient of variation",
                meaning: "std_dev_trade_sats divided by median_trade_sats — the figure to \
                          use when comparing trade-size consistency across nodes of \
                          different trade volume.",
                unit_and_direction: "Unitless ratio with no fixed upper bound; a lower value \
                                     means more consistent trade sizing.",
            },
        ),
        // 001 FR-004
        (
            "stats.liveness.last_successful_trade_at",
            MetricDefinition {
                label: "Last successful trade",
                meaning: "The created_at of this node's most recent successful order.",
                unit_and_direction: "RFC 3339 UTC timestamp; more recent is a stronger \
                                     current-activity signal.",
            },
        ),
        // 001 FR-004
        (
            "stats.liveness.days_since_last_trade",
            MetricDefinition {
                label: "Days since last trade",
                meaning: "Elapsed days between last_successful_trade_at and \
                          report-generation time.",
                unit_and_direction: "Days; a lower value indicates more recent activity.",
            },
        ),
        // 001 FR-004
        (
            "stats.liveness.successful_trades_last_7d",
            MetricDefinition {
                label: "Successful trades, last 7 days",
                meaning: "Count of successful orders whose created_at falls within the last \
                          7×24 hours before report-generation time, both ends inclusive.",
                unit_and_direction: "Count; a higher value indicates more recent trading \
                                     activity.",
            },
        ),
        // 001 FR-004
        (
            "stats.liveness.successful_trades_last_30d",
            MetricDefinition {
                label: "Successful trades, last 30 days",
                meaning: "Count of successful orders whose created_at falls within the last \
                          30×24 hours before report-generation time, both ends inclusive.",
                unit_and_direction: "Count; a higher value indicates more recent trading \
                                     activity.",
            },
        ),
        // 001 FR-004
        (
            "stats.liveness.successful_trades_last_90d",
            MetricDefinition {
                label: "Successful trades, last 90 days",
                meaning: "Count of successful orders whose created_at falls within the last \
                          90×24 hours before report-generation time, both ends inclusive.",
                unit_and_direction: "Count; a higher value indicates more recent trading \
                                     activity.",
            },
        ),
        // 001 FR-005
        (
            "stats.consistency.active_days_last_30d",
            MetricDefinition {
                label: "Active days (last 30 days)",
                meaning: "Count of distinct UTC calendar days with at least one successful \
                          order within the 30 UTC calendar days ending on and including \
                          today.",
                unit_and_direction: "Count out of 30; a higher value indicates more \
                                     consistent day-to-day activity.",
            },
        ),
        // 001 FR-005
        (
            "stats.consistency.max_consecutive_inactive_days_last_30d",
            MetricDefinition {
                label: "Longest inactive streak (last 30 days)",
                meaning: "Longest run of consecutive UTC calendar days with zero successful \
                          orders within that same 30-day window.",
                unit_and_direction: "Days, 0 to 30; a lower value indicates more consistent \
                                     activity.",
            },
        ),
        // 001 FR-006
        (
            "stats.disputes.disputes_per_100_trades",
            MetricDefinition {
                label: "Disputes per 100 trades",
                meaning: "The deduplicated dispute count expressed as a ratio per 100 \
                          successful trades.",
                unit_and_direction: "Ratio per 100 trades with no fixed upper bound; a lower \
                                     value indicates fewer disputes relative to trade volume.",
            },
        ),
        // 001 FR-006
        (
            "stats.disputes.total_disputes",
            MetricDefinition {
                label: "Total disputes",
                meaning: "Count of unique disputes, deduplicated by d tag, regardless of \
                          status.",
                unit_and_direction: "Count; no cross-node baseline exists for this figure \
                                     alone.",
            },
        ),
        // 001 FR-006
        (
            "stats.disputes.resolved_disputes",
            MetricDefinition {
                label: "Resolved disputes",
                meaning: "Count of deduplicated disputes whose latest status is settled, \
                          seller-refunded, or released.",
                unit_and_direction: "Count; a fact, not itself favorable or unfavorable.",
            },
        ),
        // 001 FR-006
        (
            "stats.disputes.active_disputes",
            MetricDefinition {
                label: "Active disputes",
                meaning: "Count of deduplicated disputes whose latest status is initiated or \
                          in-progress.",
                unit_and_direction: "Count; a fact, not itself favorable or unfavorable.",
            },
        ),
        // 001 FR-006
        (
            "stats.disputes.unknown_status_disputes",
            MetricDefinition {
                label: "Unknown-status disputes",
                meaning: "Count of deduplicated disputes whose latest status is missing or \
                          does not match any known value, so it could not be classified as \
                          resolved or active.",
                unit_and_direction: "Count; a fact, not itself favorable or unfavorable.",
            },
        ),
        // 001 FR-008
        (
            "stats.fiat_breakdown.orders_considered",
            MetricDefinition {
                label: "Fiat orders considered",
                meaning: "Count of qualifying successful orders carrying a non-empty f \
                          (fiat currency) value — the distribution's denominator.",
                unit_and_direction: "Count; describes sample size, no favorable direction.",
            },
        ),
        // 001 FR-008
        (
            "stats.fiat_breakdown.distribution",
            MetricDefinition {
                label: "Fiat currency distribution",
                meaning: "Each currency's share of orders_considered, computed as \
                          orders_in_that_currency / orders_considered.",
                unit_and_direction: "Percent per currency, summing to 100%; descriptive \
                                     only, no favorable direction.",
            },
        ),
        // 001 FR-009
        (
            "stats.payment_method_breakdown.total_mentions",
            MetricDefinition {
                label: "Payment method mentions",
                meaning: "Count of every pm mention across qualifying successful orders — \
                          the usage ranking's denominator.",
                unit_and_direction: "Count; describes sample size, no favorable direction.",
            },
        ),
        // 001 FR-009
        (
            "stats.payment_method_breakdown.distribution",
            MetricDefinition {
                label: "Payment method usage ranking",
                meaning: "Each method's share of total_mentions, computed as \
                          mentions_of_that_method / total_mentions, compared byte for byte \
                          with no trimming or case normalization.",
                unit_and_direction: "Percent per method, summing to 100%; descriptive only, \
                                     no favorable direction.",
            },
        ),
        // 001 FR-011
        (
            "stats.premium.premium_baseline_percent",
            MetricDefinition {
                label: "Premium baseline",
                meaning: "Median premium (a market-price markup/discount percentage) across \
                          this node's orders carrying a valid premium tag.",
                unit_and_direction: "Percentage points; positive is a markup over market \
                                     price, negative is a discount — no favorable direction \
                                     on its own.",
            },
        ),
        // 001 FR-011
        (
            "stats.premium.premium_dispersion_percent",
            MetricDefinition {
                label: "Premium dispersion",
                meaning: "Population standard deviation (divide by N) of premium across \
                          that same set.",
                unit_and_direction: "Percentage points; a lower value indicates more \
                                     consistent pricing relative to this node's own \
                                     baseline.",
            },
        ),
        // 001 FR-012, 002 FR-007
        (
            "stats.bond_policy.status",
            MetricDefinition {
                label: "Bond policy",
                meaning: "Whether this node's kind 38385 instance-status event reports \
                          bond_enabled as true or false, or could not be determined.",
                unit_and_direction: "One of enabled/disabled/unknown; reported neutrally, \
                                     never implying which state is safer.",
            },
        ),
        // 002 FR-005, 003 FR-006
        (
            "activity.granularity",
            MetricDefinition {
                label: "Activity grid granularity",
                meaning: "The activity grid's auto-selected time bucket size, chosen from \
                          this node's observed successful-order range.",
                unit_and_direction: "One of daily/monthly/yearly; not itself favorable or \
                                     unfavorable.",
            },
        ),
        // 002 FR-004, Edge Cases
        (
            "buckets[].median_trade_sats",
            MetricDefinition {
                label: "Bucket median trade size",
                meaning: "Median amt among the successful orders falling inside this \
                          activity-grid time bucket; undefined, not zero, for a bucket with \
                          no orders.",
                unit_and_direction: "Satoshis; no fixed favorable direction.",
            },
        ),
        // 002 FR-003
        (
            "fetch.relays[].status",
            MetricDefinition {
                label: "Relay status",
                meaning: "Whether this configured relay's connection attempt succeeded or \
                          failed for this report.",
                unit_and_direction: "One of success/failed; a connectivity fact, not a node \
                                     metric.",
            },
        ),
        // 002 FR-003
        (
            "fetch.relays[].error",
            MetricDefinition {
                label: "Relay error detail",
                meaning: "The connection failure message for this relay, present only when \
                          its status is failed.",
                unit_and_direction: "Free-text string, or null when the relay succeeded; not \
                                     a node metric.",
            },
        ),
    ])
}

#[cfg(test)]
mod tests {
    use crate::fetch::client::{RelayConnectFailure, RelayConnectionOutcome, RelayOutcome};
    use crate::fetch::filters_summary::RelayFetchOutcome;
    use crate::report::model::{
        assemble_activity_section, assemble_fetch_section, assemble_node_section, assemble_report,
        assemble_stats_section, metric_definitions, Granularity, RelayStatus,
    };
    use crate::stats::grid::{compute_activity_grid, ActivityGrid, GridOrder, GridRange};
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
        let activity_grid = compute_activity_grid(
            &[
                GridOrder {
                    created_at: 10 * 86400,
                    amount_sats: Some(100),
                },
                GridOrder {
                    created_at: 90 * 86400,
                    amount_sats: Some(300),
                },
            ],
            GridRange::Unbounded,
            None,
        );

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
        let grid = compute_activity_grid(
            &[GridOrder {
                created_at: 0,
                amount_sats: Some(1000),
            }],
            GridRange::Unbounded,
            None,
        );

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
        let grid: ActivityGrid = compute_activity_grid(&[], GridRange::Unbounded, None);

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

    /// plan.md's JSON output contract: the `metric_definitions` key set is exhaustive and
    /// mechanically checkable — exactly one entry per field plan.md's `stats` table lists
    /// (all ten sub-objects), plus `activity.granularity`, `buckets[].median_trade_sats`,
    /// `fetch.relays[].status`, and `fetch.relays[].error` — no more, no fewer.
    #[test]
    fn metric_definitions_key_set_matches_the_plans_exhaustive_list_exactly() {
        let definitions = metric_definitions();
        let mut keys: Vec<&str> = definitions.keys().copied().collect();
        keys.sort_unstable();

        let mut expected = vec![
            "activity.granularity",
            "buckets[].median_trade_sats",
            "fetch.relays[].error",
            "fetch.relays[].status",
            "stats.bond_policy.status",
            "stats.consistency.active_days_last_30d",
            "stats.consistency.max_consecutive_inactive_days_last_30d",
            "stats.cumulative.total_successful_trades",
            "stats.cumulative.total_volume_sats",
            "stats.disputes.active_disputes",
            "stats.disputes.disputes_per_100_trades",
            "stats.disputes.resolved_disputes",
            "stats.disputes.total_disputes",
            "stats.disputes.unknown_status_disputes",
            "stats.fiat_breakdown.distribution",
            "stats.fiat_breakdown.orders_considered",
            "stats.liveness.days_since_last_trade",
            "stats.liveness.last_successful_trade_at",
            "stats.liveness.successful_trades_last_30d",
            "stats.liveness.successful_trades_last_7d",
            "stats.liveness.successful_trades_last_90d",
            "stats.longevity.days_active",
            "stats.longevity.first_seen_at",
            "stats.payment_method_breakdown.distribution",
            "stats.payment_method_breakdown.total_mentions",
            "stats.premium.premium_baseline_percent",
            "stats.premium.premium_dispersion_percent",
            "stats.trade_size.coefficient_of_variation",
            "stats.trade_size.max_trade_sats",
            "stats.trade_size.mean_trade_sats",
            "stats.trade_size.median_trade_sats",
            "stats.trade_size.min_trade_sats",
            "stats.trade_size.std_dev_trade_sats",
        ];
        expected.sort_unstable();

        assert_eq!(keys, expected);
        assert_eq!(definitions.len(), 33);
    }

    #[test]
    fn every_metric_definition_has_a_non_empty_label_meaning_and_unit_and_direction() {
        for definition in metric_definitions().values() {
            assert!(!definition.label.is_empty());
            assert!(!definition.meaning.is_empty());
            assert!(!definition.unit_and_direction.is_empty());
        }
    }
}
