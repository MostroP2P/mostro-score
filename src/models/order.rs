use crate::models::dedup::dedup_orders_by_d_tag;
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
    pub successful_orders: usize,
    pub total_volume_sats: u64,
    pub trade_amounts: Vec<u64>,
    pub successful_trade_timestamps: Vec<i64>,
}

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
        let s_tag = event
            .tags
            .iter()
            .find(|t| t.as_slice().first().map(|s| s.as_str()) == Some("s"));
        let s_value = s_tag
            .and_then(|t| t.as_slice().get(1))
            .map(|s| s.to_string());
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

    let orders_map = dedup_orders_by_d_tag(pending_dedup_events);
    let unique_order_count = orders_map.len();

    let mut successful_orders = 0usize;
    let mut total_volume_sats = 0u64;
    let mut trade_amounts: Vec<u64> = Vec::new();
    let mut successful_trade_timestamps: Vec<i64> = Vec::new();

    // Process the final state of unique orders
    for (_order_id, event) in orders_map {
        // Check Status 's'
        let s_tag = event
            .tags
            .iter()
            .find(|t| t.as_slice().first().map(|s| s.as_str()) == Some("s"));
        let status_str = s_tag
            .and_then(|t| t.as_slice().get(1))
            .map(|s| s.as_str())
            .unwrap_or("unknown");

        if OrderStatus::from_str(status_str) == Ok(OrderStatus::Success) {
            successful_orders += 1;
            let event_ts = event.created_at.as_u64() as i64;
            successful_trade_timestamps.push(event_ts);

            // Get Amount 'amt' (sats)
            if let Some(amt_tag) = event
                .tags
                .iter()
                .find(|t| t.as_slice().first().map(|s| s.as_str()) == Some("amt"))
            {
                if let Some(amt_str) = amt_tag.as_slice().get(1) {
                    if let Ok(amount) = amt_str.parse::<u64>() {
                        total_volume_sats += amount;
                        trade_amounts.push(amount);
                    }
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
        successful_orders,
        total_volume_sats,
        trade_amounts,
        successful_trade_timestamps,
    }
}
