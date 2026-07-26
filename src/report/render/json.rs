//! The JSON renderer (002 FR-001, FR-011, FR-012, FR-012a; plan.md's JSON output
//! contract): the machine-readable format — the 5 sections plus `schema_version`,
//! `generated_at`, and `metric_definitions` always present, per FR-012's stable-field-set
//! contract — and a distinct fatal-error envelope (002 FR-011) for the failure path.
//! `--format`/`--sections` CLI flag wiring belongs to PR 9/PR 11; this module only builds
//! and unit-tests the JSON shape in isolation.
//!
//! `Report`/`ReportStats` keep their `Option<T>` fields for the console/plain-text
//! renderers (which never touch this module); every field that can be not-applicable is
//! converted into `MetricValue<T>` only here, at the JSON serialization boundary, per
//! plan.md's "`null` must mean not-applicable and nothing else" rule.

use crate::error::exit_code::json_error_code_for;
use crate::error::AppError;
use crate::fetch::client::RelayConnectFailure;
use crate::models::core::MetricValue;
use crate::report::content::ReportRecommendations;
use crate::report::model::{
    metric_definitions, MetricDefinition, Report, ReportActivity, ReportActivityBucket,
    ReportFetch, ReportLiveness, ReportLongevity, ReportNode, ReportStats, SCHEMA_VERSION,
};
use crate::stats::context::{
    FiatBreakdown, FiatCurrencyShare, PaymentMethodBreakdown, PaymentMethodShare, PremiumSignal,
};
use crate::stats::disputes::DisputeSignals;
use crate::stats::grid::Granularity;
use crate::stats::lifecycle::CumulativePerformance;
use crate::stats::trade_size::TradeSizeStats;
use crate::stats::{ActivityConsistency, BondPolicy};
use serde::Serialize;
use std::collections::BTreeMap;
use std::io::Write;

/// Converts an `Option<f64>` into `MetricValue<f64>` — the JSON renderer's own defensive
/// boundary, since every `stats/` computation is already audited to guard NaN/infinity at
/// its own not-applicable condition (PR 8) rather than let a degenerate float reach here.
/// A non-finite `Some` is coerced to `NotApplicable` unconditionally, in every build
/// profile, so this guarantee never depends on whether debug assertions are enabled.
fn checked_f64(value: Option<f64>) -> MetricValue<f64> {
    MetricValue::from(value.filter(|v| v.is_finite()))
}

#[derive(Serialize)]
struct JsonActivityBucket {
    bucket_start: String,
    successful_trades: usize,
    volume_sats: u64,
    median_trade_sats: MetricValue<f64>,
}

impl From<&ReportActivityBucket> for JsonActivityBucket {
    fn from(bucket: &ReportActivityBucket) -> Self {
        JsonActivityBucket {
            bucket_start: bucket.bucket_start.clone(),
            successful_trades: bucket.successful_trades,
            volume_sats: bucket.volume_sats,
            median_trade_sats: checked_f64(bucket.median_trade_sats),
        }
    }
}

#[derive(Serialize)]
struct JsonActivity {
    granularity: MetricValue<Granularity>,
    range_start: MetricValue<String>,
    range_end: MetricValue<String>,
    buckets: Vec<JsonActivityBucket>,
}

impl From<&ReportActivity> for JsonActivity {
    fn from(activity: &ReportActivity) -> Self {
        JsonActivity {
            granularity: activity.granularity.into(),
            range_start: activity.range_start.clone().into(),
            range_end: activity.range_end.clone().into(),
            buckets: activity
                .buckets
                .iter()
                .map(JsonActivityBucket::from)
                .collect(),
        }
    }
}

#[derive(Serialize)]
struct JsonLongevity {
    first_seen_at: MetricValue<String>,
    days_active: MetricValue<f64>,
}

impl From<&ReportLongevity> for JsonLongevity {
    fn from(longevity: &ReportLongevity) -> Self {
        JsonLongevity {
            first_seen_at: longevity.first_seen_at.clone().into(),
            days_active: checked_f64(longevity.days_active),
        }
    }
}

#[derive(Serialize)]
struct JsonLiveness {
    last_successful_trade_at: MetricValue<String>,
    days_since_last_trade: MetricValue<u64>,
    successful_trades_last_7d: usize,
    successful_trades_last_30d: usize,
    successful_trades_last_90d: usize,
}

impl From<&ReportLiveness> for JsonLiveness {
    fn from(liveness: &ReportLiveness) -> Self {
        JsonLiveness {
            last_successful_trade_at: liveness.last_successful_trade_at.clone().into(),
            days_since_last_trade: liveness.days_since_last_trade.into(),
            successful_trades_last_7d: liveness.successful_trades_last_7d,
            successful_trades_last_30d: liveness.successful_trades_last_30d,
            successful_trades_last_90d: liveness.successful_trades_last_90d,
        }
    }
}

#[derive(Serialize)]
struct JsonTradeSize {
    min_trade_sats: MetricValue<u64>,
    max_trade_sats: MetricValue<u64>,
    mean_trade_sats: MetricValue<f64>,
    median_trade_sats: MetricValue<f64>,
    std_dev_trade_sats: MetricValue<f64>,
    coefficient_of_variation: MetricValue<f64>,
}

impl From<&TradeSizeStats> for JsonTradeSize {
    fn from(trade_size: &TradeSizeStats) -> Self {
        JsonTradeSize {
            min_trade_sats: trade_size.min_trade_sats.into(),
            max_trade_sats: trade_size.max_trade_sats.into(),
            mean_trade_sats: checked_f64(trade_size.mean_trade_sats),
            median_trade_sats: checked_f64(trade_size.median_trade_sats),
            std_dev_trade_sats: checked_f64(trade_size.std_dev_trade_sats),
            coefficient_of_variation: checked_f64(trade_size.coefficient_of_variation),
        }
    }
}

#[derive(Serialize)]
struct JsonDisputeSignals {
    total_disputes: usize,
    resolved_disputes: usize,
    active_disputes: usize,
    unknown_status_disputes: usize,
    disputes_per_100_trades: MetricValue<f64>,
}

impl From<&DisputeSignals> for JsonDisputeSignals {
    fn from(disputes: &DisputeSignals) -> Self {
        JsonDisputeSignals {
            total_disputes: disputes.total_disputes,
            resolved_disputes: disputes.resolved_disputes,
            active_disputes: disputes.active_disputes,
            unknown_status_disputes: disputes.unknown_status_disputes,
            disputes_per_100_trades: checked_f64(disputes.disputes_per_100_trades),
        }
    }
}

#[derive(Serialize)]
struct JsonFiatBreakdown {
    orders_considered: usize,
    distribution: MetricValue<Vec<FiatCurrencyShare>>,
}

impl From<&FiatBreakdown> for JsonFiatBreakdown {
    fn from(breakdown: &FiatBreakdown) -> Self {
        JsonFiatBreakdown {
            orders_considered: breakdown.orders_considered,
            distribution: breakdown.distribution.clone().into(),
        }
    }
}

#[derive(Serialize)]
struct JsonPaymentMethodBreakdown {
    total_mentions: usize,
    distribution: MetricValue<Vec<PaymentMethodShare>>,
}

impl From<&PaymentMethodBreakdown> for JsonPaymentMethodBreakdown {
    fn from(breakdown: &PaymentMethodBreakdown) -> Self {
        JsonPaymentMethodBreakdown {
            total_mentions: breakdown.total_mentions,
            distribution: breakdown.distribution.clone().into(),
        }
    }
}

#[derive(Serialize)]
struct JsonPremiumSignal {
    premium_baseline_percent: MetricValue<f64>,
    premium_dispersion_percent: MetricValue<f64>,
}

impl From<&PremiumSignal> for JsonPremiumSignal {
    fn from(premium: &PremiumSignal) -> Self {
        JsonPremiumSignal {
            premium_baseline_percent: checked_f64(premium.premium_baseline_percent),
            premium_dispersion_percent: checked_f64(premium.premium_dispersion_percent),
        }
    }
}

#[derive(Serialize)]
struct JsonStats {
    longevity: JsonLongevity,
    cumulative: CumulativePerformance,
    trade_size: JsonTradeSize,
    liveness: JsonLiveness,
    consistency: ActivityConsistency,
    disputes: JsonDisputeSignals,
    fiat_breakdown: JsonFiatBreakdown,
    payment_method_breakdown: JsonPaymentMethodBreakdown,
    premium: JsonPremiumSignal,
    bond_policy: BondPolicy,
}

impl From<&ReportStats> for JsonStats {
    fn from(stats: &ReportStats) -> Self {
        JsonStats {
            longevity: (&stats.longevity).into(),
            cumulative: stats.cumulative,
            trade_size: (&stats.trade_size).into(),
            liveness: (&stats.liveness).into(),
            consistency: stats.consistency,
            disputes: (&stats.disputes).into(),
            fiat_breakdown: (&stats.fiat_breakdown).into(),
            payment_method_breakdown: (&stats.payment_method_breakdown).into(),
            premium: (&stats.premium).into(),
            bond_policy: stats.bond_policy,
        }
    }
}

#[derive(Serialize)]
struct JsonReport<'a> {
    schema_version: &'a str,
    generated_at: &'a str,
    node: &'a ReportNode,
    fetch: &'a ReportFetch,
    activity: JsonActivity,
    stats: JsonStats,
    recommendations: &'a ReportRecommendations,
    metric_definitions: BTreeMap<&'static str, MetricDefinition>,
}

impl<'a> From<&'a Report> for JsonReport<'a> {
    fn from(report: &'a Report) -> Self {
        JsonReport {
            schema_version: &report.schema_version,
            generated_at: &report.generated_at,
            node: &report.node,
            fetch: &report.fetch,
            activity: (&report.activity).into(),
            stats: (&report.stats).into(),
            recommendations: &report.recommendations,
            metric_definitions: metric_definitions(),
        }
    }
}

/// Serializes a `Report` into its JSON `serde_json::Value`, per plan.md's JSON output
/// contract: all 8 top-level keys always present, and every not-applicable `stats`/
/// `activity` field routed through `MetricValue` so a `null` can only ever mean "not
/// applicable", never a slipped-through NaN/infinity.
pub fn to_value(report: &Report) -> serde_json::Value {
    serde_json::to_value(JsonReport::from(report)).expect("a Report always serializes")
}

/// Renders a `Report` as a single-line JSON document to `out`. The success-path
/// counterpart to `error_envelope` below.
pub fn render(out: &mut impl Write, report: &Report) -> std::io::Result<()> {
    let value = to_value(report);
    writeln!(out, "{value}")
}

#[derive(Serialize)]
struct JsonRelayFailure {
    url: String,
    status: &'static str,
    error: Option<String>,
}

impl From<&RelayConnectFailure> for JsonRelayFailure {
    fn from(relay: &RelayConnectFailure) -> Self {
        JsonRelayFailure {
            url: relay.url.clone(),
            status: "failed",
            error: Some(relay.error.clone()),
        }
    }
}

#[derive(Serialize)]
struct JsonErrorBody {
    code: &'static str,
    message: String,
    relays: Option<Vec<JsonRelayFailure>>,
}

#[derive(Serialize)]
struct JsonErrorEnvelope {
    schema_version: &'static str,
    error: JsonErrorBody,
}

/// 002 FR-011's fatal error envelope: a distinct document, never a report-shaped one with
/// its fields left `null`. The `relays` array uses the exact same `{ url, status, error }`
/// record shape `fetch.relays` already uses (`status` is always `"failed"` here, since
/// every relay in this array is one that failed) and is only present when `error` is
/// `AppError::RelaysUnreachable` (002 FR-019 exit code `3`), extracted directly from the
/// variant's own failed-relay list; `null` for every other code.
pub fn error_envelope(error: &AppError) -> serde_json::Value {
    let relays = match error {
        AppError::RelaysUnreachable(failed) => {
            Some(failed.iter().map(JsonRelayFailure::from).collect())
        }
        _ => None,
    };

    let envelope = JsonErrorEnvelope {
        schema_version: SCHEMA_VERSION,
        error: JsonErrorBody {
            code: json_error_code_for(error),
            message: error.to_string(),
            relays,
        },
    };

    serde_json::to_value(envelope).expect("an error envelope always serializes")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::model::{RelayStatus, RelaySummary};

    fn sample_report() -> Report {
        Report {
            schema_version: "1.0.0".to_string(),
            generated_at: "2026-07-24T10:15:00Z".to_string(),
            node: ReportNode {
                pubkey_hex: "abcd".to_string(),
                pubkey_npub: "npub1abcd".to_string(),
            },
            fetch: ReportFetch {
                relays: vec![RelaySummary {
                    url: "wss://relay.example".to_string(),
                    status: RelayStatus::Success,
                    error: None,
                }],
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
                buckets: vec![],
            },
            stats: ReportStats {
                longevity: ReportLongevity {
                    first_seen_at: None,
                    days_active: None,
                },
                cumulative: CumulativePerformance {
                    total_successful_trades: 0,
                    total_volume_sats: 0,
                },
                trade_size: TradeSizeStats {
                    min_trade_sats: None,
                    max_trade_sats: None,
                    mean_trade_sats: None,
                    median_trade_sats: None,
                    std_dev_trade_sats: None,
                    coefficient_of_variation: None,
                },
                liveness: ReportLiveness {
                    last_successful_trade_at: None,
                    days_since_last_trade: None,
                    successful_trades_last_7d: 0,
                    successful_trades_last_30d: 0,
                    successful_trades_last_90d: 0,
                },
                consistency: ActivityConsistency {
                    active_days_last_30d: 0,
                    max_consecutive_inactive_days_last_30d: 30,
                },
                disputes: DisputeSignals {
                    total_disputes: 0,
                    resolved_disputes: 0,
                    active_disputes: 0,
                    unknown_status_disputes: 0,
                    disputes_per_100_trades: None,
                },
                fiat_breakdown: FiatBreakdown {
                    orders_considered: 0,
                    distribution: None,
                },
                payment_method_breakdown: PaymentMethodBreakdown {
                    total_mentions: 0,
                    distribution: None,
                },
                premium: PremiumSignal {
                    premium_baseline_percent: None,
                    premium_dispersion_percent: None,
                },
                bond_policy: BondPolicy { status: "unknown" },
            },
            recommendations: ReportRecommendations {
                nothing_notable: true,
                items: vec![],
            },
        }
    }

    /// plan.md's JSON output contract: all 8 top-level keys are always present,
    /// regardless of a node's data completeness or PR 11's `--sections` flag, which
    /// filters console/plain-text rendering only (003 FR-008) and never reaches this
    /// module.
    #[test]
    fn to_value_includes_all_eight_top_level_keys_always() {
        let value = to_value(&sample_report());
        let object = value.as_object().expect("a report serializes as an object");
        let mut keys: Vec<&str> = object.keys().map(|key| key.as_str()).collect();
        keys.sort_unstable();

        assert_eq!(
            keys,
            vec![
                "activity",
                "fetch",
                "generated_at",
                "metric_definitions",
                "node",
                "recommendations",
                "schema_version",
                "stats",
            ]
        );
    }

    /// A report document never carries an `error` key — that is the fatal-error
    /// envelope's own distinguishing key, per 002 FR-011.
    #[test]
    fn to_value_never_carries_an_error_key() {
        let value = to_value(&sample_report());
        assert!(value.get("error").is_none());
    }

    #[test]
    fn to_value_serializes_a_not_applicable_field_as_null_not_a_wrapped_object() {
        let value = to_value(&sample_report());
        assert_eq!(
            value["stats"]["longevity"]["days_active"],
            serde_json::Value::Null
        );
        assert_eq!(
            value["stats"]["longevity"]["first_seen_at"],
            serde_json::Value::Null
        );
        assert_eq!(value["activity"]["granularity"], serde_json::Value::Null);
        assert_eq!(
            value["stats"]["trade_size"]["coefficient_of_variation"],
            serde_json::Value::Null
        );
    }

    #[test]
    fn to_value_serializes_a_computed_field_as_a_bare_value_not_a_wrapped_object() {
        let mut report = sample_report();
        report.stats.trade_size.coefficient_of_variation = Some(0.42);
        report.stats.longevity.first_seen_at = Some("2026-01-01T00:00:00Z".to_string());

        let value = to_value(&report);

        assert_eq!(
            value["stats"]["trade_size"]["coefficient_of_variation"],
            serde_json::json!(0.42)
        );
        assert_eq!(
            value["stats"]["longevity"]["first_seen_at"],
            serde_json::json!("2026-01-01T00:00:00Z")
        );
    }

    /// A non-finite float must never reach the serializer as `Computed`, in any build
    /// profile: `checked_f64` coerces it to `NotApplicable` unconditionally, not only via
    /// `debug_assert!` (which compiles out in release), so this guarantee cannot silently
    /// depend on the build profile.
    #[test]
    fn checked_f64_coerces_a_non_finite_value_to_not_applicable_unconditionally() {
        assert_eq!(checked_f64(Some(f64::NAN)), MetricValue::NotApplicable);
        assert_eq!(checked_f64(Some(f64::INFINITY)), MetricValue::NotApplicable);
        assert_eq!(checked_f64(Some(1.5)), MetricValue::Computed(1.5));
        assert_eq!(checked_f64(None), MetricValue::NotApplicable);
    }

    #[test]
    fn error_envelope_shape_matches_the_schema_for_relays_unreachable() {
        let error = AppError::RelaysUnreachable(vec![RelayConnectFailure {
            url: "wss://relay.mostro.network".to_string(),
            error: "connection timed out".to_string(),
        }]);

        let value = error_envelope(&error);

        assert_eq!(value["schema_version"], serde_json::json!("1.0.0"));
        assert_eq!(
            value["error"]["code"],
            serde_json::json!("relays_unreachable")
        );
        assert_eq!(
            value["error"]["message"],
            serde_json::json!(error.to_string())
        );
        assert_eq!(
            value["error"]["relays"],
            serde_json::json!([
                {
                    "url": "wss://relay.mostro.network",
                    "status": "failed",
                    "error": "connection timed out",
                }
            ])
        );
        assert!(value.get("node").is_none());
        assert!(value.get("stats").is_none());
    }

    #[test]
    fn error_envelope_relays_is_null_for_every_non_relay_unreachable_code() {
        let source: Box<dyn std::error::Error> = "boom".into();
        for error in [
            AppError::UsageError("bad flag".to_string()),
            AppError::InvalidPubkey,
            AppError::NoUsableEvents,
            AppError::from(source),
        ] {
            let value = error_envelope(&error);
            assert_eq!(value["error"]["relays"], serde_json::Value::Null);
        }
    }

    /// plan.md's fatal error envelope table: each of the 5 `AppError` variants maps to
    /// its own JSON `code` string.
    #[test]
    fn error_envelope_code_matches_every_one_of_the_five_json_codes() {
        let source: Box<dyn std::error::Error> = "boom".into();
        let cases: Vec<(AppError, &str)> = vec![
            (AppError::from(source), "general_error"),
            (AppError::UsageError("bad".to_string()), "usage_error"),
            (AppError::RelaysUnreachable(vec![]), "relays_unreachable"),
            (AppError::NoUsableEvents, "no_usable_events"),
            (AppError::InvalidPubkey, "invalid_pubkey"),
        ];

        for (error, expected_code) in cases {
            let value = error_envelope(&error);
            assert_eq!(value["error"]["code"], serde_json::json!(expected_code));
        }
    }
}
