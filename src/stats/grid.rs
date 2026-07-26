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

/// 003 FR-004: an explicit caller-supplied range for the activity grid, or `Unbounded`
/// to keep this project's pre-PR-10 behavior (infer the range from the orders' own
/// min/max timestamp). By the time this reaches `compute_activity_grid`, both `since`/
/// `until` in the `Bounded` case are always already fully resolved concrete values —
/// `cli::options` resolves everything explicitly given, and `run()` resolves the one
/// data-dependent default (`since` defaulting to the node's earliest order) before
/// calling here, so this module never needs to look anything up itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridRange {
    Unbounded,
    Bounded { since: i64, until: i64 },
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

/// 002 FR-004/FR-005, 003 FR-004/FR-006: builds the activity grid from a node's
/// qualifying successful orders. `range` is authoritative when `Bounded` (003 FR-004):
/// it always wins even when it disagrees with what orders exist, so an explicit range
/// with zero orders inside it still renders a real grid with empty buckets spanning the
/// requested range, never the null/empty result reserved for the true zero-orders/
/// no-range case (002 FR-019). `range: GridRange::Unbounded` preserves this project's
/// pre-PR-10 behavior exactly: the range is the node's own observed lifetime, inferred
/// from the orders' own min/max timestamp.
///
/// `forced_granularity` overrides automatic selection (T129's evidence) when `Some` —
/// 003 FR-006's explicit `--view`, a configuration-sourced value, or any other caller
/// that already knows the desired granularity. When `range` is `Bounded`, a misaligned
/// `since`/`until` is snapped to the enclosing bucket's start/end once granularity is
/// known (003 FR-006); rejecting an explicit `--view`'s own misalignment instead of
/// snapping is `cli::options`'s job, upstream of this function — by the time a `Bounded`
/// range reaches here, snapping is always the correct behavior.
pub fn compute_activity_grid(
    orders: &[GridOrder],
    range: GridRange,
    forced_granularity: Option<Granularity>,
) -> ActivityGrid {
    match range {
        GridRange::Unbounded => compute_unbounded_activity_grid(orders, forced_granularity),
        GridRange::Bounded { since, until } => {
            compute_bounded_activity_grid(orders, since, until, forced_granularity)
        }
    }
}

/// 003 FR-006: a *defaulted* `--since`/`--until` (this function's entire reason for
/// existing: `GridRange::Unbounded` means neither flag was given at all) MUST snap to the
/// enclosing bucket's start/end when the granularity is forced — the reject-instead-of-
/// snap rule applies only to an *explicitly given* `--since`/`--until` combined with an
/// explicit `--view`, never to this inferred-from-orders, no-explicit-bound case. Reuses
/// `snap_range_to_granularity`, the same helper `GridRange::Bounded` uses, so the two
/// paths can never disagree about what "snapped" means for a given granularity.
fn compute_unbounded_activity_grid(
    orders: &[GridOrder],
    forced_granularity: Option<Granularity>,
) -> ActivityGrid {
    if orders.is_empty() {
        return ActivityGrid {
            granularity: None,
            range_start: None,
            range_end: None,
            buckets: Vec::new(),
        };
    }

    let inferred_start = orders
        .iter()
        .map(|order| order.created_at)
        .min()
        .unwrap_or_default();
    let inferred_end = orders
        .iter()
        .map(|order| order.created_at)
        .max()
        .unwrap_or_default();

    let granularity =
        forced_granularity.unwrap_or_else(|| select_granularity(inferred_start, inferred_end));
    let (range_start, range_end) = if forced_granularity.is_some() {
        snap_range_to_granularity(granularity, inferred_start, inferred_end)
    } else {
        (inferred_start, inferred_end)
    };
    let buckets = build_grid_buckets(orders, granularity, range_start, range_end, None);

    ActivityGrid {
        granularity: Some(granularity),
        range_start: Some(range_start),
        range_end: Some(range_end),
        buckets,
    }
}

/// 003 FR-004/FR-005/FR-006: `since > until` (T190/191's empty/inverted-range case,
/// reachable from `run()`'s data-dependent earliest-history default when FR-005's own
/// explicit-`--since` check never applied) stays empty — checked, and returned, before
/// any snapping happens, so snapping can never turn an inverted range into a non-empty
/// one.
fn compute_bounded_activity_grid(
    orders: &[GridOrder],
    since: i64,
    until: i64,
    forced_granularity: Option<Granularity>,
) -> ActivityGrid {
    let granularity = forced_granularity.unwrap_or_else(|| select_granularity(since, until));

    if since > until {
        return ActivityGrid {
            granularity: Some(granularity),
            range_start: Some(since),
            range_end: Some(until),
            buckets: Vec::new(),
        };
    }

    let (snapped_since, snapped_until) = snap_range_to_granularity(granularity, since, until);
    // The filter must match what's actually displayed: once snapping widens the range
    // shown to the enclosing bucket boundary, every order inside that WIDENED range must
    // be counted too, not just the caller's originally (possibly narrower) requested
    // `[since, until]` -- otherwise the grid would claim to cover, say, all of March while
    // silently excluding orders from any day outside the caller's original sub-range.
    let buckets = build_grid_buckets(
        orders,
        granularity,
        snapped_since,
        snapped_until,
        Some((snapped_since, snapped_until)),
    );

    ActivityGrid {
        granularity: Some(granularity),
        range_start: Some(snapped_since),
        range_end: Some(snapped_until),
        buckets,
    }
}

/// 003 FR-006: snaps `since` down to the start of its enclosing bucket and `until` up to
/// the end of its enclosing bucket for the given `granularity`. A raw timestamp is not
/// itself a day boundary just because every calendar day is a valid daily bucket unit —
/// `since`/`until` still need rounding to `00:00:00`/`23:59:59` UTC on their respective
/// days, exactly like the monthly/yearly cases round to their own calendar boundaries.
fn snap_range_to_granularity(
    granularity: Granularity,
    range_start: i64,
    range_end: i64,
) -> (i64, i64) {
    match granularity {
        Granularity::Daily => {
            let snapped_start = day_index(range_start) * SECONDS_PER_DAY;
            let snapped_end = (day_index(range_end) + 1) * SECONDS_PER_DAY - 1;
            (snapped_start, snapped_end)
        }
        Granularity::Monthly => {
            let (start_year, start_month) = year_month(range_start);
            let snapped_start = month_start_epoch(start_year, start_month);
            let (end_year, end_month) = year_month(range_end);
            let (next_year, next_month_value) = next_month(end_year, end_month);
            let snapped_end = month_start_epoch(next_year, next_month_value) - 1;
            (snapped_start, snapped_end)
        }
        Granularity::Yearly => {
            let start_year = year_month(range_start).0;
            let snapped_start = year_start_epoch(start_year);
            let end_year = year_month(range_end).0;
            let snapped_end = year_start_epoch(end_year + 1) - 1;
            (snapped_start, snapped_end)
        }
    }
}

/// Builds every ordered, gap-free bucket in `[range_start, range_end]` for
/// `granularity`, counting only orders that fall inside `filter_range` when given
/// (003 FR-004's `Bounded` case). `filter_range` is always the same `(range_start,
/// range_end)` the buckets themselves span — once snapping has widened what's
/// displayed, every order inside that widened range must count too, not just the
/// orders inside the caller's original, possibly narrower, request. `filter_range: None`
/// counts every order, matching `GridRange::Unbounded`'s pre-PR-10 behavior.
fn build_grid_buckets(
    orders: &[GridOrder],
    granularity: Granularity,
    range_start: i64,
    range_end: i64,
    filter_range: Option<(i64, i64)>,
) -> Vec<GridBucket> {
    let boundaries = bucket_boundaries(granularity, range_start, range_end);

    let mut relevant_orders: Vec<GridOrder> = match filter_range {
        Some((since, until)) => orders
            .iter()
            .copied()
            .filter(|order| order.created_at >= since && order.created_at <= until)
            .collect(),
        None => orders.to_vec(),
    };
    relevant_orders.sort_by_key(|order| order.created_at);

    let mut buckets = Vec::with_capacity(boundaries.len().saturating_sub(1));
    let mut cursor = 0usize;
    for window in boundaries.windows(2) {
        let (bucket_start, next_start) = (window[0], window[1]);
        let mut successful_trades = 0usize;
        let mut volume_sats: u64 = 0;
        let mut amounts: Vec<u64> = Vec::new();

        while cursor < relevant_orders.len() && relevant_orders[cursor].created_at < next_start {
            successful_trades += 1;
            if let Some(amount) = relevant_orders[cursor].amount_sats {
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

    buckets
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

    /// `report::model::ReportActivity` depends on this exact lowercase representation,
    /// mirroring `RelayStatus`'s equivalent coverage in `report::model`.
    #[test]
    fn granularity_serializes_as_a_lowercase_json_string() {
        assert_eq!(
            serde_json::to_string(&Granularity::Daily).unwrap(),
            "\"daily\""
        );
        assert_eq!(
            serde_json::to_string(&Granularity::Monthly).unwrap(),
            "\"monthly\""
        );
        assert_eq!(
            serde_json::to_string(&Granularity::Yearly).unwrap(),
            "\"yearly\""
        );
    }

    #[test]
    fn compute_activity_grid_with_zero_orders_reports_null_range_and_empty_buckets() {
        let grid = compute_activity_grid(&[], GridRange::Unbounded, None);

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

        let grid = compute_activity_grid(&orders, GridRange::Unbounded, None);

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

        let grid = compute_activity_grid(&orders, GridRange::Unbounded, None);

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

        let grid = compute_activity_grid(&orders, GridRange::Unbounded, None);

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

        let grid = compute_activity_grid(&orders, GridRange::Unbounded, None);

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

        let grid = compute_activity_grid(&orders, GridRange::Unbounded, None);

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

        let grid = compute_activity_grid(&orders, GridRange::Unbounded, None);

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

    /// 003 FR-006: an explicit `--view` with no `--since`/`--until` at all is still a
    /// *defaulted* range in FR-006's own terms, so it MUST snap to the enclosing bucket
    /// boundary, exactly like the config-sourced/automatic-selection cases -- the
    /// reject-instead-of-snap rule applies only when `--since`/`--until` are *also*
    /// explicitly given. `range_start`/`range_end` must reflect the snapped calendar-month
    /// boundary, not the orders' own raw min/max timestamps.
    #[test]
    fn compute_activity_grid_unbounded_with_forced_granularity_snaps_the_inferred_range() {
        let mid_march = Utc
            .with_ymd_and_hms(2026, 3, 15, 12, 0, 0)
            .unwrap()
            .timestamp();
        let mid_april = Utc
            .with_ymd_and_hms(2026, 4, 10, 8, 0, 0)
            .unwrap()
            .timestamp();
        let orders = vec![
            GridOrder {
                created_at: mid_march,
                amount_sats: Some(100),
            },
            GridOrder {
                created_at: mid_april,
                amount_sats: Some(200),
            },
        ];

        let grid = compute_activity_grid(&orders, GridRange::Unbounded, Some(Granularity::Monthly));

        assert_eq!(
            grid.range_start,
            Some(
                Utc.with_ymd_and_hms(2026, 3, 1, 0, 0, 0)
                    .unwrap()
                    .timestamp()
            )
        );
        assert_eq!(
            grid.range_end,
            Some(
                Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0)
                    .unwrap()
                    .timestamp()
                    - 1
            )
        );
    }

    /// The pre-PR-10 fully-automatic path (`Unbounded`, no forced granularity) must stay
    /// exactly as it was: `range_start`/`range_end` are the orders' own raw min/max, never
    /// snapped -- only an explicit `--view` (or a config-sourced value, once PR 12 lands)
    /// triggers snapping for an otherwise-defaulted range.
    #[test]
    fn compute_activity_grid_unbounded_with_no_forced_granularity_never_snaps() {
        let mid_march = Utc
            .with_ymd_and_hms(2026, 3, 15, 12, 0, 0)
            .unwrap()
            .timestamp();
        let orders = vec![GridOrder {
            created_at: mid_march,
            amount_sats: Some(100),
        }];

        let grid = compute_activity_grid(&orders, GridRange::Unbounded, None);

        assert_eq!(grid.range_start, Some(mid_march));
        assert_eq!(grid.range_end, Some(mid_march));
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

    // ---- 003 FR-004/FR-006: `GridRange::Bounded` ----

    /// FR-004: an explicit range with zero orders inside it renders a real grid with
    /// empty buckets spanning the requested range, never the null result reserved for
    /// the true zero-orders/no-range case.
    #[test]
    fn compute_activity_grid_bounded_range_with_zero_orders_still_renders_empty_buckets() {
        let since = 0;
        let until = 2 * SECONDS_PER_DAY;

        let grid = compute_activity_grid(&[], GridRange::Bounded { since, until }, None);

        assert_eq!(grid.granularity, Some(Granularity::Daily));
        assert_eq!(grid.range_start, Some(since));
        // `until` sits exactly on a day boundary (the very start of day index 2); Daily
        // snapping rounds it up to the last whole second of that same day.
        assert_eq!(grid.range_end, Some(3 * SECONDS_PER_DAY - 1));
        assert_eq!(grid.buckets.len(), 3);
        assert!(grid
            .buckets
            .iter()
            .all(|bucket| bucket.successful_trades == 0));
    }

    /// FR-004: the bounded range wins even when orders exist outside it — only orders
    /// inside `[since, until]` are counted.
    #[test]
    fn compute_activity_grid_bounded_range_filters_out_orders_outside_the_range() {
        let since = SECONDS_PER_DAY;
        let until = 2 * SECONDS_PER_DAY;
        let orders = vec![
            GridOrder {
                created_at: 0,
                amount_sats: Some(999),
            },
            GridOrder {
                created_at: SECONDS_PER_DAY + 100,
                amount_sats: Some(500),
            },
            GridOrder {
                created_at: 10 * SECONDS_PER_DAY,
                amount_sats: Some(999),
            },
        ];

        let grid = compute_activity_grid(&orders, GridRange::Bounded { since, until }, None);

        assert_eq!(grid.range_start, Some(since));
        // `until` sits exactly on a day boundary; Daily snapping rounds it up to the
        // last whole second of that day (see the sibling zero-orders test above).
        assert_eq!(grid.range_end, Some(3 * SECONDS_PER_DAY - 1));
        let total_trades: usize = grid
            .buckets
            .iter()
            .map(|bucket| bucket.successful_trades)
            .sum();
        assert_eq!(total_trades, 1);
        let total_volume: u64 = grid.buckets.iter().map(|bucket| bucket.volume_sats).sum();
        assert_eq!(total_volume, 500);
    }

    /// FR-006: an explicit forced granularity is used directly, with no automatic
    /// selection, even over a range automatic selection would never pick on its own.
    #[test]
    fn compute_activity_grid_bounded_range_uses_forced_granularity_directly() {
        let since = 0;
        let until = 10 * SECONDS_PER_DAY;

        let grid = compute_activity_grid(
            &[],
            GridRange::Bounded { since, until },
            Some(Granularity::Monthly),
        );

        assert_eq!(grid.granularity, Some(Granularity::Monthly));
    }

    /// FR-006: a misaligned `since`/`until` is snapped to the enclosing calendar-month
    /// boundary when the monthly granularity comes from config/automatic selection
    /// rather than an explicit `--view` (already rejected upstream in `cli::options` in
    /// that case).
    #[test]
    fn compute_activity_grid_snaps_misaligned_bounds_to_the_enclosing_month_boundary() {
        let since = Utc
            .with_ymd_and_hms(2026, 3, 15, 0, 0, 0)
            .unwrap()
            .timestamp();
        let until = Utc
            .with_ymd_and_hms(2026, 3, 20, 23, 59, 59)
            .unwrap()
            .timestamp();

        let grid = compute_activity_grid(
            &[],
            GridRange::Bounded { since, until },
            Some(Granularity::Monthly),
        );

        assert_eq!(
            grid.range_start,
            Some(
                Utc.with_ymd_and_hms(2026, 3, 1, 0, 0, 0)
                    .unwrap()
                    .timestamp()
            )
        );
        assert_eq!(
            grid.range_end,
            Some(
                Utc.with_ymd_and_hms(2026, 4, 1, 0, 0, 0)
                    .unwrap()
                    .timestamp()
                    - 1
            )
        );
    }

    /// FR-006: an empty or inverted range (`since > until`) stays empty regardless of
    /// snapping — proven here by choosing bounds that would land in *different* calendar
    /// months once snapped, so snapping could only ever widen, never repair, the
    /// inversion.
    #[test]
    fn compute_activity_grid_empty_inverted_range_stays_empty_after_snapping() {
        let since = Utc
            .with_ymd_and_hms(2026, 6, 15, 0, 0, 0)
            .unwrap()
            .timestamp();
        let until = Utc
            .with_ymd_and_hms(2026, 1, 15, 0, 0, 0)
            .unwrap()
            .timestamp();

        let grid = compute_activity_grid(
            &[],
            GridRange::Bounded { since, until },
            Some(Granularity::Monthly),
        );

        assert!(grid.buckets.is_empty());
        assert_eq!(grid.range_start, Some(since));
        assert_eq!(grid.range_end, Some(until));
    }

    /// FR-006: with no forced granularity, a `Bounded` range auto-selects granularity
    /// from its own span, not from any order's own min/max.
    #[test]
    fn compute_activity_grid_bounded_range_auto_selects_granularity_from_the_range_span() {
        let since = 0;
        let until = 91 * SECONDS_PER_DAY;

        let grid = compute_activity_grid(&[], GridRange::Bounded { since, until }, None);

        assert_eq!(grid.granularity, Some(Granularity::Monthly));
    }

    /// FR-006: once snapping widens the displayed range to the enclosing calendar month,
    /// an order that falls inside that widened month but OUTSIDE the caller's originally
    /// requested (narrower) `[since, until]` must still be counted -- the grid claims to
    /// cover the whole month, so it must actually count the whole month, not silently
    /// exclude days the snap itself introduced.
    #[test]
    fn compute_activity_grid_snapping_widens_the_counted_range_not_just_the_label() {
        let since = Utc
            .with_ymd_and_hms(2026, 3, 15, 0, 0, 0)
            .unwrap()
            .timestamp();
        let until = Utc
            .with_ymd_and_hms(2026, 3, 20, 23, 59, 59)
            .unwrap()
            .timestamp();
        // Outside the caller's requested [15th, 20th] but inside the snapped March 1-31.
        let order_outside_requested_range = Utc
            .with_ymd_and_hms(2026, 3, 5, 0, 0, 0)
            .unwrap()
            .timestamp();
        let orders = vec![GridOrder {
            created_at: order_outside_requested_range,
            amount_sats: Some(100),
        }];

        let grid = compute_activity_grid(
            &orders,
            GridRange::Bounded { since, until },
            Some(Granularity::Monthly),
        );

        let total_trades: usize = grid
            .buckets
            .iter()
            .map(|bucket| bucket.successful_trades)
            .sum();
        assert_eq!(
            total_trades, 1,
            "an order inside the snapped (widened) month must be counted"
        );
    }

    /// 003 FR-006: a defaulted daily range (e.g. `--view daily` alone, or any other
    /// non-explicit-`--view` daily case) still snaps to UTC day boundaries -- a raw
    /// mid-day timestamp is not itself a day boundary just because a day is a valid
    /// bucket unit.
    #[test]
    fn compute_activity_grid_snaps_a_bounded_daily_range_to_day_boundaries() {
        let since = Utc
            .with_ymd_and_hms(2026, 3, 15, 14, 30, 0)
            .unwrap()
            .timestamp();
        let until = Utc
            .with_ymd_and_hms(2026, 3, 15, 20, 0, 0)
            .unwrap()
            .timestamp();

        let grid = compute_activity_grid(
            &[],
            GridRange::Bounded { since, until },
            Some(Granularity::Daily),
        );

        assert_eq!(
            grid.range_start,
            Some(
                Utc.with_ymd_and_hms(2026, 3, 15, 0, 0, 0)
                    .unwrap()
                    .timestamp()
            )
        );
        assert_eq!(
            grid.range_end,
            Some(
                Utc.with_ymd_and_hms(2026, 3, 16, 0, 0, 0)
                    .unwrap()
                    .timestamp()
                    - 1
            )
        );
    }
}
