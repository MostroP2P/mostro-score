//! Activity grid: bucketing and automatic granularity selection (002 FR-004/FR-005, 003
//! FR-006), plus FR-005a's wide-range warning. Report/activity-grid logic, not a Phase 1
//! lifetime metric, kept apart from `lifecycle.rs` for that reason. Pure, no I/O, per the
//! constitution's dependency direction for `stats`.

use crate::stats::trade_size::compute_trade_stats;
use chrono::{DateTime, Datelike, LocalResult, TimeZone, Utc};
use serde::Serialize;

/// T129 evidence: bucket-count practicality bounds, picked by reasoning about a terminal
/// table's usable row count (a table showing hundreds of rows is unusable), not from a
/// runtime measurement. Daily buckets while the range is a quarter or less, monthly
/// while it is roughly two years or less, yearly beyond that.
const DAILY_GRANULARITY_MAX_RANGE_DAYS: i64 = 90;
const MONTHLY_GRANULARITY_MAX_RANGE_DAYS: i64 = 730;

const SECONDS_PER_DAY: i64 = 86400;

/// The activity grid's bucket size (002 FR-005, 003 FR-006). Serializes as a lowercase
/// JSON string (`"daily"` / `"monthly"` / `"yearly"`), matching `RelayStatus`'s pattern
/// for other string-union report fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Granularity {
    Daily,
    Monthly,
    Yearly,
}

/// One qualifying successful order's contribution to the activity grid (002 FR-004): its
/// UTC timestamp and its trade amount in sats, when the order's `amt` tag parsed
/// successfully. `amount_sats` is `None` for a successful order whose `amt` did not
/// parse — it still counts toward `successful_trades`, but contributes nothing to
/// `volume_sats`/`median_trade_sats`, mirroring `models::order::aggregate_order_events`'s
/// existing distinction between `successful_trade_timestamps` (every successful order)
/// and `trade_amounts` (only the ones with a usable `amt`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridOrder {
    pub created_at: i64,
    pub amount_sats: Option<u64>,
}

/// One row of the activity grid (002 FR-004). An empty bucket still appears with
/// `successful_trades`/`volume_sats` of `0` and `median_trade_sats` of `None` — both are
/// real values for an empty bucket per spec 002's Edge Cases, while a median over zero
/// orders is undefined, not zero (001 FR-003).
#[derive(Debug, Clone, PartialEq)]
pub struct GridBucket {
    pub bucket_start: i64,
    pub successful_trades: usize,
    pub volume_sats: u64,
    pub median_trade_sats: Option<f64>,
}

/// The complete activity grid (002 FR-004/FR-005). `granularity`/`range_start`/
/// `range_end` are `None` and `buckets` is empty only when the node has zero successful
/// orders (002 FR-019's zero-order Edge Case): there is no order timestamp to anchor a
/// default range on, and inventing one would misrepresent the node's actual history.
#[derive(Debug, Clone, PartialEq)]
pub struct ActivityGrid {
    pub granularity: Option<Granularity>,
    pub range_start: Option<i64>,
    pub range_end: Option<i64>,
    pub buckets: Vec<GridBucket>,
}

fn day_index(timestamp: i64) -> i64 {
    timestamp.div_euclid(SECONDS_PER_DAY)
}

fn year_month(timestamp: i64) -> (i32, u32) {
    let date_time = DateTime::<Utc>::from_timestamp(timestamp, 0).unwrap_or_default();
    (date_time.year(), date_time.month())
}

/// UTC never has an ambiguous or skipped local time, so day 1 at midnight always
/// resolves to exactly one instant; the `0` fallback below is unreachable in practice and
/// only guards against ever panicking if that invariant is somehow violated.
fn month_start_epoch(year: i32, month: u32) -> i64 {
    match Utc.with_ymd_and_hms(year, month, 1, 0, 0, 0) {
        LocalResult::Single(date_time) => date_time.timestamp(),
        _ => 0,
    }
}

fn year_start_epoch(year: i32) -> i64 {
    month_start_epoch(year, 1)
}

fn next_month(year: i32, month: u32) -> (i32, u32) {
    if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    }
}

fn select_granularity(range_start: i64, range_end: i64) -> Granularity {
    let range_days = day_index(range_end) - day_index(range_start);
    if range_days <= DAILY_GRANULARITY_MAX_RANGE_DAYS {
        Granularity::Daily
    } else if range_days <= MONTHLY_GRANULARITY_MAX_RANGE_DAYS {
        Granularity::Monthly
    } else {
        Granularity::Yearly
    }
}

/// N+1 chronological UTC calendar-day boundaries covering every day from `range_start`'s
/// day through `range_end`'s day, inclusive, plus one trailing boundary — the exclusive
/// end of the last bucket — so every window in `boundaries.windows(2)` is `[start, end)`.
fn daily_boundaries(range_start: i64, range_end: i64) -> Vec<i64> {
    let start_day = day_index(range_start);
    let end_day = day_index(range_end);
    (start_day..=end_day + 1)
        .map(|day| day * SECONDS_PER_DAY)
        .collect()
}

/// Same contract as `daily_boundaries`, one whole UTC calendar month per bucket.
fn monthly_boundaries(range_start: i64, range_end: i64) -> Vec<i64> {
    let (end_year, end_month) = year_month(range_end);
    let (mut year, mut month) = year_month(range_start);

    let mut boundaries = Vec::new();
    loop {
        boundaries.push(month_start_epoch(year, month));
        if year == end_year && month == end_month {
            let (next_year, next_month_value) = next_month(year, month);
            boundaries.push(month_start_epoch(next_year, next_month_value));
            return boundaries;
        }
        let (next_year, next_month_value) = next_month(year, month);
        year = next_year;
        month = next_month_value;
    }
}

/// Same contract as `daily_boundaries`, one whole UTC calendar year per bucket.
fn yearly_boundaries(range_start: i64, range_end: i64) -> Vec<i64> {
    let start_year = year_month(range_start).0;
    let end_year = year_month(range_end).0;
    (start_year..=end_year + 1).map(year_start_epoch).collect()
}

fn bucket_boundaries(granularity: Granularity, range_start: i64, range_end: i64) -> Vec<i64> {
    match granularity {
        Granularity::Daily => daily_boundaries(range_start, range_end),
        Granularity::Monthly => monthly_boundaries(range_start, range_end),
        Granularity::Yearly => yearly_boundaries(range_start, range_end),
    }
}

/// 002 FR-004/FR-005: builds the activity grid from a node's qualifying successful
/// orders. The range bucketed is the node's own observed lifetime — the full span from
/// its earliest to its latest successful order — since no `--since`/`--until` flag exists
/// yet (PR 10's job); granularity is auto-selected from that span per T129's evidence,
/// then every UTC calendar day/month/year in the range becomes one ordered, gap-free
/// bucket, including buckets with zero orders in them.
pub fn compute_activity_grid(orders: &[GridOrder]) -> ActivityGrid {
    if orders.is_empty() {
        return ActivityGrid {
            granularity: None,
            range_start: None,
            range_end: None,
            buckets: Vec::new(),
        };
    }

    let range_start = orders
        .iter()
        .map(|order| order.created_at)
        .min()
        .unwrap_or_default();
    let range_end = orders
        .iter()
        .map(|order| order.created_at)
        .max()
        .unwrap_or_default();

    let granularity = select_granularity(range_start, range_end);
    let boundaries = bucket_boundaries(granularity, range_start, range_end);

    let mut sorted_orders = orders.to_vec();
    sorted_orders.sort_by_key(|order| order.created_at);

    let mut buckets = Vec::with_capacity(boundaries.len().saturating_sub(1));
    let mut cursor = 0usize;
    for window in boundaries.windows(2) {
        let (bucket_start, next_start) = (window[0], window[1]);
        let mut successful_trades = 0usize;
        let mut volume_sats: u64 = 0;
        let mut amounts: Vec<u64> = Vec::new();

        while cursor < sorted_orders.len() && sorted_orders[cursor].created_at < next_start {
            successful_trades += 1;
            if let Some(amount) = sorted_orders[cursor].amount_sats {
                volume_sats = volume_sats.saturating_add(amount);
                amounts.push(amount);
            }
            cursor += 1;
        }

        buckets.push(GridBucket {
            bucket_start,
            successful_trades,
            volume_sats,
            median_trade_sats: compute_trade_stats(&amounts).median_trade_sats,
        });
    }

    ActivityGrid {
        granularity: Some(granularity),
        range_start: Some(range_start),
        range_end: Some(range_end),
        buckets,
    }
}

/// 002 FR-005a: warns when a daily grid is combined with a time range wide enough to
/// produce an unreasonably large number of rows. Reuses T129's own daily/monthly
/// switch-over boundary as the warning threshold, so the warning and the auto-selection
/// rule never disagree about what counts as "too wide" for daily buckets. In ordinary
/// operation `select_granularity` already switches away from daily past that same
/// boundary, so this can only fire in practice once PR 10's `--view` override lets a
/// caller force daily granularity over a range auto-selection would never pick on its
/// own; exercised here ahead of that override with a manually forced scenario. Stderr-only
/// diagnostic (002 FR-017), no JSON field — the caller decides where/whether to print it.
pub fn wide_range_warning_message(
    granularity: Granularity,
    range_start: i64,
    range_end: i64,
) -> Option<String> {
    if granularity != Granularity::Daily {
        return None;
    }

    let range_days = day_index(range_end) - day_index(range_start);
    if range_days <= DAILY_GRANULARITY_MAX_RANGE_DAYS {
        return None;
    }

    Some(format!(
        "Warning: a daily activity grid over a {range_days}-day range produces {} rows; consider a coarser view.",
        range_days + 1
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_activity_grid_with_zero_orders_reports_null_range_and_empty_buckets() {
        let grid = compute_activity_grid(&[]);

        assert_eq!(grid.granularity, None);
        assert_eq!(grid.range_start, None);
        assert_eq!(grid.range_end, None);
        assert!(grid.buckets.is_empty());
    }

    #[test]
    fn compute_activity_grid_daily_buckets_span_full_range_with_gap_filled() {
        let day0 = 100i64;
        let day2 = 2 * SECONDS_PER_DAY + 100;
        let orders = vec![
            GridOrder {
                created_at: day0,
                amount_sats: Some(1000),
            },
            GridOrder {
                created_at: day2,
                amount_sats: Some(3000),
            },
        ];

        let grid = compute_activity_grid(&orders);

        assert_eq!(grid.granularity, Some(Granularity::Daily));
        assert_eq!(grid.range_start, Some(day0));
        assert_eq!(grid.range_end, Some(day2));
        assert_eq!(grid.buckets.len(), 3);

        assert_eq!(grid.buckets[0].bucket_start, 0);
        assert_eq!(grid.buckets[0].successful_trades, 1);
        assert_eq!(grid.buckets[0].volume_sats, 1000);
        assert_eq!(grid.buckets[0].median_trade_sats, Some(1000.0));

        assert_eq!(grid.buckets[1].bucket_start, SECONDS_PER_DAY);
        assert_eq!(grid.buckets[1].successful_trades, 0);
        assert_eq!(grid.buckets[1].volume_sats, 0);
        assert_eq!(grid.buckets[1].median_trade_sats, None);

        assert_eq!(grid.buckets[2].bucket_start, 2 * SECONDS_PER_DAY);
        assert_eq!(grid.buckets[2].successful_trades, 1);
        assert_eq!(grid.buckets[2].volume_sats, 3000);
        assert_eq!(grid.buckets[2].median_trade_sats, Some(3000.0));
    }

    /// Edge Cases: a successful order whose `amt` never parsed still counts toward
    /// `successful_trades` but contributes nothing to `volume_sats`/`median_trade_sats`.
    #[test]
    fn compute_activity_grid_counts_a_trade_with_no_amount_toward_successful_trades_only() {
        let orders = vec![
            GridOrder {
                created_at: 100,
                amount_sats: None,
            },
            GridOrder {
                created_at: 200,
                amount_sats: Some(500),
            },
        ];

        let grid = compute_activity_grid(&orders);

        assert_eq!(grid.buckets.len(), 1);
        assert_eq!(grid.buckets[0].successful_trades, 2);
        assert_eq!(grid.buckets[0].volume_sats, 500);
        assert_eq!(grid.buckets[0].median_trade_sats, Some(500.0));
    }

    #[test]
    fn compute_activity_grid_stays_daily_at_exactly_the_ninety_day_boundary() {
        let orders = vec![
            GridOrder {
                created_at: 0,
                amount_sats: Some(1),
            },
            GridOrder {
                created_at: 90 * SECONDS_PER_DAY,
                amount_sats: Some(2),
            },
        ];

        let grid = compute_activity_grid(&orders);

        assert_eq!(grid.granularity, Some(Granularity::Daily));
        assert_eq!(grid.buckets.len(), 91);
    }

    #[test]
    fn compute_activity_grid_switches_to_monthly_one_day_past_the_daily_boundary() {
        let orders = vec![
            GridOrder {
                created_at: 0,
                amount_sats: Some(1),
            },
            GridOrder {
                created_at: 91 * SECONDS_PER_DAY,
                amount_sats: Some(2),
            },
        ];

        let grid = compute_activity_grid(&orders);

        assert_eq!(grid.granularity, Some(Granularity::Monthly));
    }

    #[test]
    fn compute_activity_grid_switches_to_monthly_beyond_the_daily_boundary() {
        let start = Utc
            .with_ymd_and_hms(2025, 1, 15, 0, 0, 0)
            .unwrap()
            .timestamp();
        let end = Utc
            .with_ymd_and_hms(2025, 4, 20, 0, 0, 0)
            .unwrap()
            .timestamp();
        let orders = vec![
            GridOrder {
                created_at: start,
                amount_sats: Some(100),
            },
            GridOrder {
                created_at: end,
                amount_sats: Some(200),
            },
        ];

        let grid = compute_activity_grid(&orders);

        assert_eq!(grid.granularity, Some(Granularity::Monthly));
        assert_eq!(grid.buckets.len(), 4);
        assert_eq!(
            grid.buckets[0].bucket_start,
            Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0)
                .unwrap()
                .timestamp()
        );
        assert_eq!(
            grid.buckets[3].bucket_start,
            Utc.with_ymd_and_hms(2025, 4, 1, 0, 0, 0)
                .unwrap()
                .timestamp()
        );
        assert_eq!(grid.buckets[0].successful_trades, 1);
        assert_eq!(grid.buckets[3].successful_trades, 1);
    }

    #[test]
    fn compute_activity_grid_switches_to_yearly_beyond_the_monthly_boundary() {
        let start = Utc
            .with_ymd_and_hms(2020, 6, 1, 0, 0, 0)
            .unwrap()
            .timestamp();
        let end = Utc
            .with_ymd_and_hms(2024, 3, 1, 0, 0, 0)
            .unwrap()
            .timestamp();
        let orders = vec![
            GridOrder {
                created_at: start,
                amount_sats: Some(10),
            },
            GridOrder {
                created_at: end,
                amount_sats: Some(20),
            },
        ];

        let grid = compute_activity_grid(&orders);

        assert_eq!(grid.granularity, Some(Granularity::Yearly));
        assert_eq!(grid.buckets.len(), 5);
        assert_eq!(
            grid.buckets[0].bucket_start,
            Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0)
                .unwrap()
                .timestamp()
        );
        assert_eq!(
            grid.buckets[4].bucket_start,
            Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0)
                .unwrap()
                .timestamp()
        );
    }

    #[test]
    fn wide_range_warning_message_fires_for_a_forced_daily_grid_over_a_wide_range() {
        let warning = wide_range_warning_message(Granularity::Daily, 0, 200 * SECONDS_PER_DAY);
        assert!(warning.is_some());
    }

    #[test]
    fn wide_range_warning_message_is_none_within_the_daily_boundary() {
        let warning = wide_range_warning_message(Granularity::Daily, 0, 30 * SECONDS_PER_DAY);
        assert_eq!(warning, None);
    }

    #[test]
    fn wide_range_warning_message_is_none_for_non_daily_granularity_regardless_of_range() {
        let warning = wide_range_warning_message(Granularity::Monthly, 0, 1000 * SECONDS_PER_DAY);
        assert_eq!(warning, None);
    }
}
