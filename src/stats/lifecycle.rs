use serde::Serialize;
use std::collections::HashSet;

/// Longevity (001 FR-001): `first_seen_at` is `Some` only via the primary dev-fee anchor
/// path; the fallback path (no dev-fee anchor, but at least one qualifying successful
/// order) reports `first_seen_at` as `None` since it has no dev-fee event to derive it
/// from, even though `days_active` is still computable in that case.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Longevity {
    pub first_seen_at: Option<i64>,
    pub days_active: Option<f64>,
}

/// Compute longevity (Section 4.1.1, 001 FR-001). Primary anchor is the oldest
/// qualifying kind `8383` dev-fee-payment event's `created_at`. When the node has none,
/// falls back to the elapsed time between its first qualifying successful order's
/// `created_at` and `now` — matching the primary path's own "elapsed time to now"
/// semantic, rather than spanning first order to *last* order (which would stop
/// increasing at the last trade and read `0` for a node with exactly one successful
/// order). When neither anchor exists, both fields are reported as not applicable.
pub fn compute_longevity(
    first_dev_fee_ts: Option<i64>,
    successful_trade_timestamps: &[i64],
    now: i64,
) -> Longevity {
    if let Some(start_ts) = first_dev_fee_ts {
        return Longevity {
            first_seen_at: Some(start_ts),
            days_active: Some((now - start_ts) as f64 / 86400.0),
        };
    }

    match successful_trade_timestamps.iter().min() {
        Some(&first_successful_order_ts) => Longevity {
            first_seen_at: None,
            days_active: Some((now - first_successful_order_ts) as f64 / 86400.0),
        },
        None => Longevity {
            first_seen_at: None,
            days_active: None,
        },
    }
}

/// Cumulative performance (Section 4.1.2, 001 FR-002). Pure exposure of values already
/// computed by `models::order::aggregate_order_events` — no new aggregation logic.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct CumulativePerformance {
    pub total_successful_trades: usize,
    pub total_volume_sats: u64,
}

pub fn compute_cumulative_performance(
    successful_orders: usize,
    total_volume_sats: u64,
) -> CumulativePerformance {
    CumulativePerformance {
        total_successful_trades: successful_orders,
        total_volume_sats,
    }
}

/// Liveness (Section 4.2.1/4.2.2, 001 FR-004): last successful trade and elapsed time
/// since it, both not applicable when the node has zero successful orders, plus the
/// rolling 7/30/90-day successful-trade counts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Liveness {
    pub last_successful_trade_at: Option<i64>,
    pub days_since_last_trade: Option<u64>,
    pub trades_last_7d: usize,
    pub trades_last_30d: usize,
    pub trades_last_90d: usize,
}

pub fn compute_liveness(successful_trade_timestamps: &[i64], now: i64) -> Liveness {
    let (trades_last_7d, trades_last_30d, trades_last_90d) =
        compute_rolling_windows(successful_trade_timestamps, now);

    let last_successful_trade_at = successful_trade_timestamps.iter().max().copied();
    let days_since_last_trade =
        last_successful_trade_at.map(|ts| ((now - ts) as f64 / 86400.0).floor() as u64);

    Liveness {
        last_successful_trade_at,
        days_since_last_trade,
        trades_last_7d,
        trades_last_30d,
        trades_last_90d,
    }
}

/// Compute rolling window metrics (Section 4.2.2)
pub fn compute_rolling_windows(timestamps: &[i64], now: i64) -> (usize, usize, usize) {
    let day_7 = now - (7 * 86400);
    let day_30 = now - (30 * 86400);
    let day_90 = now - (90 * 86400);

    let last_7d = timestamps.iter().filter(|&&ts| ts >= day_7).count();
    let last_30d = timestamps.iter().filter(|&&ts| ts >= day_30).count();
    let last_90d = timestamps.iter().filter(|&&ts| ts >= day_90).count();

    (last_7d, last_30d, last_90d)
}

/// Compute activity consistency (Section 4.2.3, 001 FR-005). The window is exactly 30
/// UTC calendar days ending on and including today, not a rolling 30*86400-second
/// cutoff: computing the window's boundary as a raw timestamp comparison (rather than
/// aligning both ends to UTC calendar-day indices first) includes an extra day whenever
/// `now` isn't exactly at a UTC day boundary, since `now - 30*86400` and `now` land on
/// the same day-of-week offset but span 31 distinct day indices, not 30.
pub fn compute_activity_consistency(timestamps: &[i64], now: i64) -> (usize, usize) {
    let today = now.div_euclid(86400);
    let window_start_day = today - 29; // 30 calendar days inclusive: today-29 ..= today

    // Get unique UTC calendar days with a trade within the 30-day window.
    let active_days: HashSet<i64> = timestamps
        .iter()
        .map(|&ts| ts.div_euclid(86400))
        .filter(|&day| day >= window_start_day && day <= today)
        .collect();

    let active_days_count = active_days.len();

    // Calculate max consecutive inactive days
    if active_days.is_empty() {
        return (0, 30);
    }

    let mut days: Vec<i64> = active_days.into_iter().collect();
    days.sort_unstable();

    // The gap before the first active day (inactive days from the window's start up to
    // it) and the gap after the last active day (through today) both count as potential
    // runs of consecutive inactive days, same as any gap between two active days.
    let mut max_gap = (days[0] - window_start_day).max(0) as usize;
    for pair in days.windows(2) {
        let gap = (pair[1] - pair[0] - 1).max(0) as usize;
        max_gap = max_gap.max(gap);
    }
    let final_gap = (today - days[days.len() - 1]).max(0) as usize;
    max_gap = max_gap.max(final_gap);

    (active_days_count, max_gap)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_longevity_uses_dev_fee_anchor_when_present() {
        let now = 100 * 86400;
        let longevity = compute_longevity(Some(10 * 86400), &[50 * 86400], now);
        assert_eq!(longevity.first_seen_at, Some(10 * 86400));
        assert_eq!(longevity.days_active, Some(90.0));
    }

    /// FR-001's core fix: with no dev-fee anchor, `days_active` must span the node's
    /// *first* qualifying successful order to *now*, not first order to last order —
    /// otherwise a node with exactly one successful order would always read `0`, and
    /// `days_active` would stop increasing after the node's last trade.
    #[test]
    fn compute_longevity_falls_back_to_first_successful_order_to_now_when_no_dev_fee_anchor() {
        let now = 100 * 86400;
        let successful_trade_timestamps = vec![10 * 86400, 50 * 86400];

        let longevity = compute_longevity(None, &successful_trade_timestamps, now);

        assert_eq!(longevity.first_seen_at, None);
        assert_eq!(longevity.days_active, Some(90.0));
    }

    #[test]
    fn compute_longevity_single_successful_order_still_spans_to_now_not_zero() {
        let now = 100 * 86400;
        let successful_trade_timestamps = vec![100 * 86400 - 5 * 86400];

        let longevity = compute_longevity(None, &successful_trade_timestamps, now);

        assert_eq!(longevity.days_active, Some(5.0));
    }

    #[test]
    fn compute_longevity_reports_not_applicable_when_no_anchor_and_no_successful_orders() {
        let longevity = compute_longevity(None, &[], 100 * 86400);
        assert_eq!(longevity.first_seen_at, None);
        assert_eq!(longevity.days_active, None);
    }

    #[test]
    fn compute_cumulative_performance_exposes_the_already_aggregated_values() {
        let cumulative = compute_cumulative_performance(42, 1_000_000);
        assert_eq!(cumulative.total_successful_trades, 42);
        assert_eq!(cumulative.total_volume_sats, 1_000_000);
    }

    #[test]
    fn compute_liveness_reports_not_applicable_when_no_successful_trades() {
        let liveness = compute_liveness(&[], 1_000_000);
        assert_eq!(liveness.last_successful_trade_at, None);
        assert_eq!(liveness.days_since_last_trade, None);
        assert_eq!(liveness.trades_last_7d, 0);
        assert_eq!(liveness.trades_last_30d, 0);
        assert_eq!(liveness.trades_last_90d, 0);
    }

    #[test]
    fn compute_liveness_uses_the_most_recent_successful_trade() {
        let now = 100 * 86400_i64;
        let successful_trade_timestamps = vec![now - 50 * 86400, now - 3 * 86400];

        let liveness = compute_liveness(&successful_trade_timestamps, now);

        assert_eq!(liveness.last_successful_trade_at, Some(now - 3 * 86400));
        assert_eq!(liveness.days_since_last_trade, Some(3));
        assert_eq!(liveness.trades_last_7d, 1);
    }

    #[test]
    fn compute_rolling_windows_counts_each_window_independently() {
        let now = 1_000_000_i64;
        let timestamps = vec![
            now - 86400,      // within 7, 30, 90
            now - 10 * 86400, // within 30, 90
            now - 60 * 86400, // within 90 only
            now - 91 * 86400, // outside all windows
        ];
        assert_eq!(compute_rolling_windows(&timestamps, now), (1, 2, 3));
    }

    #[test]
    fn compute_rolling_windows_empty_is_all_zero() {
        assert_eq!(compute_rolling_windows(&[], 1_000_000), (0, 0, 0));
    }

    #[test]
    fn compute_activity_consistency_no_trades_is_zero_active_thirty_gap() {
        assert_eq!(compute_activity_consistency(&[], 1_000_000), (0, 30));
    }

    #[test]
    fn compute_activity_consistency_counts_unique_active_days_and_max_gap() {
        let now = 30 * 86400_i64; // today = day 30, window = days 1..=30
                                  // Day 0 is one day before the window and must be excluded; only day 10 counts,
                                  // leaving a 9-day gap before it (days 1-9) and a 20-day gap after it (days 11-30).
        let timestamps = vec![100, 10 * 86400 + 100];
        let (active_days, max_gap) = compute_activity_consistency(&timestamps, now);
        assert_eq!(active_days, 1);
        assert_eq!(max_gap, 20);
    }

    /// FR-005 regression: the window is exactly 30 UTC calendar days ending on and
    /// including today, not a rolling 30*86400-second cutoff. A trade on the day exactly
    /// 30 days before today (the window's first day) must still count; a trade one day
    /// further back must not.
    #[test]
    fn compute_activity_consistency_window_is_exactly_thirty_calendar_days() {
        let now = 100 * 86400_i64; // today = day 100, window = days 71..=100
        let just_inside_window = vec![71 * 86400]; // day 71: the window's first day
        let just_outside_window = vec![70 * 86400]; // day 70: one day before the window

        let (active_days, _) = compute_activity_consistency(&just_inside_window, now);
        assert_eq!(active_days, 1, "the window's first day must count");

        let (active_days, max_gap) = compute_activity_consistency(&just_outside_window, now);
        assert_eq!(active_days, 0, "one day before the window must not count");
        assert_eq!(max_gap, 30);
    }

    /// Regression: the gap before the first active day, and the gap after the last
    /// active day, must both be counted as potential runs of consecutive inactive days,
    /// same as a gap between two active days.
    #[test]
    fn compute_activity_consistency_counts_gap_before_first_and_after_last_active_day() {
        let now = 100 * 86400_i64; // today = day 100, window = days 71..=100
                                   // Only the window's very last day is active: days 71-99 (29 days) are all
                                   // inactive before it.
        let only_active_on_last_day = vec![100 * 86400];
        let (_, max_gap) = compute_activity_consistency(&only_active_on_last_day, now);
        assert_eq!(max_gap, 29);

        // Only the window's very first day is active: days 72-100 (29 days) are all
        // inactive after it.
        let only_active_on_first_day = vec![71 * 86400];
        let (_, max_gap) = compute_activity_consistency(&only_active_on_first_day, now);
        assert_eq!(max_gap, 29);
    }
}
