use crate::models::core::{amt_tag, s_tag};
use crate::models::dedup::dedup_events_by_d_tag;
use mostro_core::prelude::Status as OrderStatus;
use nostr_sdk::prelude::*;
use std::collections::HashMap;
use std::str::FromStr;

/// PR 1 Step C: per-kind order aggregation, verbatim from the wrapped function body's
/// order-handling loop — raw time-range tracking and `s`-tag distribution over every
/// fetched order event, then `d`-tag dedup (via `models::dedup`) and the `s=success`
/// qualifying-order selection over the deduplicated set. Presentation (the debug block
/// and every report section) stays in `report/render/console.rs` (T037); this function
/// only computes the values those sections print.
pub struct OrderAggregate {
    pub total_order_count: usize,
    pub first_order_ts: i64,
    pub last_order_ts: i64,
    pub s_tag_distribution: HashMap<String, usize>,
    pub unique_order_count: usize,
    /// PR 3: count of deduplicated orders whose `s` tag parses as a recognized
    /// `OrderStatus` (pending, success, canceled, etc. — any known value, not just
    /// success). An order that survives `d`-tag dedup but carries no `s` tag at all is
    /// malformed (FR-013): real Mostro order events always publish a status, so a
    /// missing one is incomplete data, not evidence of "an order this node placed."
    pub recognized_status_order_count: usize,
    pub successful_orders: usize,
    pub total_volume_sats: u64,
    pub trade_amounts: Vec<u64>,
    pub successful_trade_timestamps: Vec<i64>,
}

/// PR 3 (T084/T085): FR-002's full qualifying-order procedure — dedup by `d` tag to the
/// highest `created_at` (ties broken by the greatest event id), then filter to only the
/// deduplicated events whose selected state carries `s=success`.
pub fn aggregate_order_events(order_events: Vec<Event>) -> OrderAggregate {
    let total_order_count = order_events.len();
    let mut first_order_ts = i64::MAX;
    let mut last_order_ts = 0i64;
    let mut s_tag_distribution: HashMap<String, usize> = HashMap::new();
    let mut pending_dedup_events: Vec<Event> = Vec::new();

    for event in order_events {
        // Track order time range
        if (event.created_at.as_u64() as i64) < first_order_ts {
            first_order_ts = event.created_at.as_u64() as i64;
        }
        if (event.created_at.as_u64() as i64) > last_order_ts {
            last_order_ts = event.created_at.as_u64() as i64;
        }

        // Track status distribution for all fetched events (all are orders now)
        let s_value = s_tag(&event).map(|s| s.to_string());
        match &s_value {
            Some(val) => {
                *s_tag_distribution.entry(val.clone()).or_insert(0) += 1;
            }
            None => {
                *s_tag_distribution
                    .entry("(missing)".to_string())
                    .or_insert(0) += 1;
            }
        }

        pending_dedup_events.push(event);
    }

    let orders_map = dedup_events_by_d_tag(pending_dedup_events);
    let unique_order_count = orders_map.len();

    let mut recognized_status_order_count = 0usize;
    let mut successful_orders = 0usize;
    let mut total_volume_sats = 0u64;
    let mut trade_amounts: Vec<u64> = Vec::new();
    let mut successful_trade_timestamps: Vec<i64> = Vec::new();

    // Process the final state of unique orders
    for (_order_id, event) in orders_map {
        // Check Status 's'
        let status_str = s_tag(&event).unwrap_or("unknown");
        let parsed_status = OrderStatus::from_str(status_str);

        if parsed_status.is_ok() {
            recognized_status_order_count += 1;
        }

        if parsed_status == Ok(OrderStatus::Success) {
            successful_orders += 1;
            let event_ts = event.created_at.as_u64() as i64;
            successful_trade_timestamps.push(event_ts);

            // Get Amount 'amt' (sats)
            if let Some(amt_str) = amt_tag(&event) {
                if let Ok(amount) = amt_str.parse::<u64>() {
                    // `amt` comes from an untrusted relay event; saturating_add avoids a
                    // panic (debug) or silent wraparound (release) on a crafted extreme
                    // value, per Principle VI (no panics on user-facing paths). No
                    // observable difference for any realistic sat amount (bounded by the
                    // 21M BTC supply, far below u64::MAX).
                    total_volume_sats = total_volume_sats.saturating_add(amount);
                    trade_amounts.push(amount);
                }
            }
        }
    }

    OrderAggregate {
        total_order_count,
        first_order_ts,
        last_order_ts,
        s_tag_distribution,
        unique_order_count,
        recognized_status_order_count,
        successful_orders,
        total_volume_sats,
        trade_amounts,
        successful_trade_timestamps,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::test_support::make_event;

    /// FR-002's full procedure is dedup-then-filter, in that order: whichever event wins
    /// the `d`-tag tie-break (highest `created_at`, ties broken by greatest event id) is
    /// the one whose `s` value decides whether the order counts as successful — never the
    /// other candidate's status, and never both.
    #[test]
    fn aggregate_order_events_qualifying_selection_follows_the_tie_break_winner() {
        let success_candidate = make_event(
            38383,
            100,
            vec![("d", "order-1"), ("s", "success"), ("z", "order")],
        );
        let pending_candidate = make_event(
            38383,
            100,
            vec![("d", "order-1"), ("s", "pending"), ("z", "order")],
        );
        let winner_is_success = success_candidate.id > pending_candidate.id;

        let aggregate =
            aggregate_order_events(vec![success_candidate.clone(), pending_candidate.clone()]);

        assert_eq!(aggregate.unique_order_count, 1);
        assert_eq!(aggregate.successful_orders, usize::from(winner_is_success));
    }

    /// FR-013: a malformed or missing `amt` value is unusable data on that order
    /// specifically, not evidence the trade did not happen — the order still counts
    /// toward `successful_orders`, but is safely excluded from `total_volume_sats` and
    /// `trade_amounts` rather than panicking on the unparseable value.
    #[test]
    fn aggregate_order_events_excludes_a_malformed_amt_from_volume_but_still_counts_the_trade() {
        let malformed_amt = make_event(
            38383,
            100,
            vec![("d", "order-1"), ("s", "success"), ("amt", "not-a-number")],
        );
        let missing_amt = make_event(38383, 200, vec![("d", "order-2"), ("s", "success")]);

        let aggregate = aggregate_order_events(vec![malformed_amt, missing_amt]);

        assert_eq!(aggregate.successful_orders, 2);
        assert_eq!(aggregate.total_volume_sats, 0);
        assert!(aggregate.trade_amounts.is_empty());
    }

    #[test]
    fn aggregate_order_events_ignores_an_order_event_missing_its_d_tag_without_panicking() {
        let no_d_tag = make_event(38383, 100, vec![("s", "success")]);

        let aggregate = aggregate_order_events(vec![no_d_tag]);

        assert_eq!(aggregate.unique_order_count, 0);
        assert_eq!(aggregate.successful_orders, 0);
    }
}
