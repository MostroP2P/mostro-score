pub mod context;
pub mod disputes;
pub mod lifecycle;
pub mod trade_size;

use context::{
    compute_fiat_breakdown, compute_payment_method_breakdown, compute_premium_signal,
    FiatBreakdown, PaymentMethodBreakdown, PremiumSignal,
};
use disputes::{compute_dispute_signals, DisputeSignals};
use lifecycle::{
    compute_activity_consistency, compute_cumulative_performance, compute_liveness,
    compute_longevity, CumulativePerformance, Liveness, Longevity,
};
use trade_size::{compute_trade_stats, TradeSizeStats};

/// Activity consistency (Section 4.2.3, 001 FR-005): pure wiring around
/// `lifecycle::compute_activity_consistency`'s existing tuple result, giving each of its
/// two values a self-documenting name for `NodeMetrics`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActivityConsistency {
    pub active_days_last_30d: usize,
    pub max_consecutive_inactive_days_last_30d: usize,
}

/// Bond Policy (001 FR-012, 002 FR-007): the node's tri-state anti-abuse-bond
/// enforcement, mapped to the report schema's three-valued string by
/// `models::instance_status::BondEnabled::as_bond_policy_status`. Its own, distinctly
/// named sub-object per 002 FR-007 — never merged into the trade-history metrics above.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BondPolicy {
    pub status: &'static str,
}

/// Every core reputation metric this PR computes (001 FR-001 through FR-006, FR-008,
/// FR-009, FR-010, FR-011, FR-012) for one node, assembled from `stats::lifecycle`,
/// `stats::trade_size`, `stats::disputes`, and `stats::context`'s independently tested
/// computations, plus `models::instance_status`'s bond-policy mapping. Pure struct
/// assembly: no new business logic lives here.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeMetrics {
    pub longevity: Longevity,
    pub cumulative: CumulativePerformance,
    pub trade_size: TradeSizeStats,
    pub liveness: Liveness,
    pub consistency: ActivityConsistency,
    pub disputes: DisputeSignals,
    pub fiat_breakdown: FiatBreakdown,
    pub payment_method_breakdown: PaymentMethodBreakdown,
    pub premium: PremiumSignal,
    pub bond_policy: BondPolicy,
}

impl NodeMetrics {
    #[allow(clippy::too_many_arguments)]
    pub fn compute(
        first_dev_fee_ts: Option<i64>,
        successful_orders: usize,
        total_volume_sats: u64,
        trade_amounts: &[u64],
        successful_trade_timestamps: &[i64],
        total_disputes: usize,
        resolved_disputes: usize,
        active_disputes: usize,
        unknown_status_disputes: usize,
        fiat_values: &[String],
        payment_method_mentions: &[String],
        premium_values: &[i64],
        bond_policy_status: &'static str,
        now: i64,
    ) -> NodeMetrics {
        let (active_days_last_30d, max_consecutive_inactive_days_last_30d) =
            compute_activity_consistency(successful_trade_timestamps, now);

        NodeMetrics {
            longevity: compute_longevity(first_dev_fee_ts, successful_trade_timestamps, now),
            cumulative: compute_cumulative_performance(successful_orders, total_volume_sats),
            trade_size: compute_trade_stats(trade_amounts),
            liveness: compute_liveness(successful_trade_timestamps, now),
            consistency: ActivityConsistency {
                active_days_last_30d,
                max_consecutive_inactive_days_last_30d,
            },
            disputes: compute_dispute_signals(
                total_disputes,
                resolved_disputes,
                active_disputes,
                unknown_status_disputes,
                successful_orders,
            ),
            fiat_breakdown: compute_fiat_breakdown(fiat_values),
            payment_method_breakdown: compute_payment_method_breakdown(payment_method_mentions),
            premium: compute_premium_signal(premium_values),
            bond_policy: BondPolicy {
                status: bond_policy_status,
            },
        }
    }
}

/// PR 1 Step C: the legacy trust-score calculation, relocated here verbatim while it is
/// still called by the wrapped function. `pub`, not `pub(crate)`: T039 moves this module
/// into the library crate, so `main.rs`'s binary crate now reaches it across the crate
/// boundary. Takes the individual `NodeMetrics` values it needs directly rather than the
/// whole struct, keeping this legacy scoring function decoupled from PR 4's assembly
/// type. No spec backing (Complexity Tracking); removed entirely in PR 7's T124 once the
/// report model no longer needs it.
pub fn calculate_score(successful_orders: usize, total_volume_sats: u64, days_active: f64) -> u64 {
    let mut score = 0.0;

    // 1. Age (Max 30 pts for > 1 year)
    score += (days_active / 365.0).min(1.0) * 30.0;

    // 2. Volume (Max 40 pts for > 1 BTC volume)
    let btc_vol = total_volume_sats as f64 / 100_000_000.0;
    score += (btc_vol / 1.0).min(1.0) * 40.0;

    // 3. Success Count (Max 30 pts for > 100 orders)
    score += (successful_orders as f64 / 100.0).min(1.0) * 30.0;

    score as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_metrics_compute_assembles_every_field_from_its_own_computation() {
        let now = 100 * 86400_i64;
        let successful_trade_timestamps = vec![10 * 86400, 90 * 86400];
        let trade_amounts = vec![100u64, 300u64];

        let fiat_values = vec!["USD".to_string(), "USD".to_string()];
        let payment_method_mentions = vec!["SEPA".to_string()];
        let premium_values = vec![10i64, 20i64];

        let metrics = NodeMetrics::compute(
            Some(5 * 86400),
            2,
            400,
            &trade_amounts,
            &successful_trade_timestamps,
            1,
            0,
            1,
            0,
            &fiat_values,
            &payment_method_mentions,
            &premium_values,
            "enabled",
            now,
        );

        assert_eq!(metrics.longevity.first_seen_at, Some(5 * 86400));
        assert_eq!(metrics.cumulative.total_successful_trades, 2);
        assert_eq!(metrics.cumulative.total_volume_sats, 400);
        assert_eq!(metrics.trade_size.median_trade_sats, Some(200.0));
        assert_eq!(metrics.liveness.last_successful_trade_at, Some(90 * 86400));
        assert_eq!(metrics.consistency.active_days_last_30d, 1);
        assert_eq!(metrics.disputes.total_disputes, 1);
        assert_eq!(metrics.disputes.resolved_disputes, 0);
        assert_eq!(metrics.disputes.active_disputes, 1);
        assert_eq!(metrics.disputes.unknown_status_disputes, 0);
        // 1 dispute / 2 successful trades * 100 = 50.0.
        assert_eq!(metrics.disputes.disputes_per_100_trades, Some(50.0));
        assert_eq!(metrics.fiat_breakdown.orders_considered, 2);
        assert_eq!(metrics.payment_method_breakdown.total_mentions, 1);
        assert_eq!(metrics.premium.premium_baseline_percent, Some(15.0));
        assert_eq!(metrics.bond_policy.status, "enabled");
    }

    #[test]
    fn calculate_score_zero_activity_is_zero() {
        assert_eq!(calculate_score(0, 0, 0.0), 0);
    }

    #[test]
    fn calculate_score_caps_each_component_at_its_maximum() {
        // total_volume_sats > 1 BTC caps the volume component; successful_orders > 100
        // caps the success-count component. Age caps at 365+ days (30 pts) + volume caps
        // at 40 pts + success caps at 30 pts.
        assert_eq!(calculate_score(500, 200_000_000, 400.0), 100);
    }

    #[test]
    fn calculate_score_partial_credit_is_proportional() {
        // 0.5 BTC -> 20 pts volume; 50/100 orders -> 15 pts success;
        // days_active/365 = 0.5 -> 15 pts age. Total: 50.
        assert_eq!(calculate_score(50, 50_000_000, 182.5), 50);
    }
}
