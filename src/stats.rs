use std::collections::HashSet;

use crate::models::MostroStats;

pub fn compute_trade_stats(amounts: &[u64]) -> (u64, u64, f64, u64) {
    if amounts.is_empty() {
        return (0, 0, 0.0, 0);
    }

    let mut sorted = amounts.to_vec();
    sorted.sort_unstable();

    let min = sorted[0];
    let max = sorted[sorted.len() - 1];
    let sum: u128 = amounts.iter().map(|&v| v as u128).sum();
    let mean = sum as f64 / amounts.len() as f64;

    let median = if sorted.len() % 2 == 0 {
        ((sorted[sorted.len() / 2 - 1] as u128 + sorted[sorted.len() / 2] as u128) / 2) as u64
    } else {
        sorted[sorted.len() / 2]
    };

    (min, max, mean, median)
}

pub fn compute_rolling_windows(timestamps: &[i64], now: i64) -> (usize, usize, usize) {
    let day_7 = now - (7 * 86400);
    let day_30 = now - (30 * 86400);
    let day_90 = now - (90 * 86400);

    let last_7d = timestamps.iter().filter(|&&ts| ts >= day_7 && ts <= now).count();
    let last_30d = timestamps.iter().filter(|&&ts| ts >= day_30 && ts <= now).count();
    let last_90d = timestamps.iter().filter(|&&ts| ts >= day_90 && ts <= now).count();

    (last_7d, last_30d, last_90d)
}

pub fn compute_activity_consistency(timestamps: &[i64], now: i64) -> (usize, usize) {
    let day_30_ago = now - (30 * 86400);

    let active_days: HashSet<i64> = timestamps
        .iter()
        .filter(|&&ts| ts >= day_30_ago && ts <= now)
        .map(|&ts| ts / 86400)
        .collect();

    let active_days_count = active_days.len();

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

    let final_gap = (today - prev_day).max(0) as usize;
    max_gap = max_gap.max(final_gap);

    (active_days_count, max_gap)
}

pub fn format_relative_time(timestamp: i64, now: i64) -> String {
    let diff_secs = now - timestamp;

    if diff_secs < 0 {
        return "in the future".to_string();
    }

    let days = diff_secs / 86400;
    let hours = (diff_secs % 86400) / 3600;

    match days {
        0 => {
            if hours == 0 {
                "less than an hour ago".to_string()
            } else if hours == 1 {
                "1 hour ago".to_string()
            } else {
                format!("{} hours ago", hours)
            }
        }
        1 => "1 day ago".to_string(),
        2..=6 => format!("{} days ago", days),
        7..=13 => "1 week ago".to_string(),
        14..=29 => format!("{} weeks ago", days / 7),
        30..=59 => "1 month ago".to_string(),
        60..=364 => format!("{} months ago", days / 30),
        _ => format!("{} years ago", days / 365),
    }
}

pub fn calculate_score(stats: &MostroStats, days_active: f64) -> u64 {
    let mut score = 0.0;

    score += (days_active / 365.0).clamp(0.0, 1.0) * 30.0;

    let btc_vol = stats.total_volume_sats as f64 / 100_000_000.0;
    score += (btc_vol / 1.0).min(1.0) * 40.0;

    score += (stats.successful_orders as f64 / 100.0).min(1.0) * 30.0;

    score as u64
}
