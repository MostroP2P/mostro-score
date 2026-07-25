use std::collections::HashSet;

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

/// Compute activity consistency (Section 4.2.3)
pub fn compute_activity_consistency(timestamps: &[i64], now: i64) -> (usize, usize) {
    let day_30_ago = now - (30 * 86400);

    // Get unique days with trades in last 30 days
    let active_days: HashSet<i64> = timestamps
        .iter()
        .filter(|&&ts| ts >= day_30_ago)
        .map(|&ts| ts / 86400) // Convert to day number
        .collect();

    let active_days_count = active_days.len();

    // Calculate max consecutive inactive days
    if active_days.is_empty() {
        return (0, 30);
    }

    let mut days: Vec<i64> = active_days.into_iter().collect();
    days.sort_unstable();

    let today = now / 86400;
    let day_30_start = day_30_ago / 86400;

    let mut max_gap = 0usize;
    let mut prev_day = day_30_start;

    for &day in &days {
        let gap = (day - prev_day - 1).max(0) as usize;
        max_gap = max_gap.max(gap);
        prev_day = day;
    }

    // Check gap from last active day to today
    let final_gap = (today - prev_day).max(0) as usize;
    max_gap = max_gap.max(final_gap);

    (active_days_count, max_gap)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let now = 30 * 86400_i64;
        // Active on day 0 and day 10 (relative to the 30-day window start), leaving a
        // 9-day gap between them and a 20-day gap from day 10 to "today" (day 30).
        let timestamps = vec![100, 10 * 86400 + 100];
        let (active_days, max_gap) = compute_activity_consistency(&timestamps, now);
        assert_eq!(active_days, 2);
        assert_eq!(max_gap, 20);
    }
}
