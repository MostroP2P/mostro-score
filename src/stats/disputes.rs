//! Dispute signals: a disputes-per-100-successful-trades ratio, computed from the
//! already deduplicated-by-`d`-tag, already-classified dispute counts
//! (`models::dispute::DisputeAggregate`) and the successful-trade count already
//! computed by `stats::lifecycle`. Takes plain counts, not a domain-model type,
//! matching `stats::lifecycle`/`stats::trade_size`'s existing pattern of depending on
//! primitives rather than `models::*` types.

/// Dispute signals for one node. The four counts are never `Option`: a count of zero
/// disputes is a real, computable value, not a not-applicable case.
/// `disputes_per_100_trades` is `None` only when `successful_trades` is zero — there is
/// no denominator to divide by — regardless of how many disputes exist.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct DisputeSignals {
    pub total_disputes: usize,
    pub resolved_disputes: usize,
    pub active_disputes: usize,
    pub unknown_status_disputes: usize,
    pub disputes_per_100_trades: Option<f64>,
}

/// Computes dispute signals from already deduplicated/classified dispute counts and
/// the node's successful-trade count.
pub fn compute_dispute_signals(
    total_disputes: usize,
    resolved_disputes: usize,
    active_disputes: usize,
    unknown_status_disputes: usize,
    successful_trades: usize,
) -> DisputeSignals {
    let disputes_per_100_trades = if successful_trades == 0 {
        None
    } else {
        Some((total_disputes as f64 / successful_trades as f64) * 100.0)
    };

    DisputeSignals {
        total_disputes,
        resolved_disputes,
        active_disputes,
        unknown_status_disputes,
        disputes_per_100_trades,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_dispute_signals_zero_successful_trades_reports_ratio_as_not_applicable() {
        let signals = compute_dispute_signals(3, 1, 1, 1, 0);

        assert_eq!(signals.total_disputes, 3);
        assert_eq!(signals.resolved_disputes, 1);
        assert_eq!(signals.active_disputes, 1);
        assert_eq!(signals.unknown_status_disputes, 1);
        assert_eq!(signals.disputes_per_100_trades, None);
    }

    #[test]
    fn compute_dispute_signals_computes_the_ratio_when_trades_exist() {
        let signals = compute_dispute_signals(5, 2, 2, 1, 50);

        assert_eq!(signals.disputes_per_100_trades, Some(10.0));
    }

    /// Zero disputes with a nonzero trade count is a real, computable ratio of exactly
    /// `0.0`, not a not-applicable case — only a zero denominator is not applicable.
    #[test]
    fn compute_dispute_signals_zero_disputes_with_trades_reports_a_zero_ratio() {
        let signals = compute_dispute_signals(0, 0, 0, 0, 20);

        assert_eq!(signals.total_disputes, 0);
        assert_eq!(signals.disputes_per_100_trades, Some(0.0));
    }

    #[test]
    fn compute_dispute_signals_ratio_is_not_capped_at_100() {
        let signals = compute_dispute_signals(10, 5, 5, 0, 5);

        assert_eq!(signals.disputes_per_100_trades, Some(200.0));
    }
}
