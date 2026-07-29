pub mod context;
pub mod disputes;
pub mod grid;
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

/// Wraps `lifecycle::compute_activity_consistency`'s tuple result with self-documenting
/// field names for `NodeMetrics`.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct ActivityConsistency {
    pub active_days_last_30d: usize,
    pub max_consecutive_inactive_days_last_30d: usize,
}

/// The node's tri-state anti-abuse-bond enforcement, mapped to the report schema's
/// three-valued string by `models::instance_status::BondEnabled::as_bond_policy_status`.
/// Its own sub-object, never merged into the trade-history metrics above.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct BondPolicy {
    pub status: &'static str,
}

/// Every core reputation metric for one node, assembled from `stats::lifecycle`,
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
}
