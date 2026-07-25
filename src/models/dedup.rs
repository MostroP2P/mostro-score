use nostr_sdk::prelude::*;
use std::collections::HashMap;

/// PR 1 Step B seam: deduplicate order events by their `d` tag, keeping the event with
/// the greatest `created_at` for each order id. Extracted verbatim from the loop body
/// previously inline in the wrapped function; pure signature, no network, no I/O.
pub fn dedup_orders_by_d_tag(order_events: Vec<Event>) -> HashMap<String, Event> {
    let mut orders_map: HashMap<String, Event> = HashMap::new();

    for event in order_events {
        // If it's an order, map it by 'd' tag (Order ID) to get the final state
        if let Some(d_tag) = event
            .tags
            .iter()
            .find(|t| t.as_slice().first().map(|s| s.as_str()) == Some("d"))
        {
            if let Some(order_id) = d_tag.as_slice().get(1) {
                // Logic: Keep the event with the latest created_at for this Order ID
                match orders_map.get(order_id.as_str()) {
                    Some(existing) => {
                        if event.created_at > existing.created_at {
                            orders_map.insert(order_id.to_string(), event.clone());
                        }
                    }
                    None => {
                        orders_map.insert(order_id.to_string(), event.clone());
                    }
                }
            }
        }
    }

    orders_map
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::test_support::make_event;

    #[test]
    fn dedup_orders_by_d_tag_keeps_greatest_created_at_per_order_id() {
        let older = make_event(38383, 100, vec![("d", "order-1"), ("s", "pending")]);
        let newer = make_event(38383, 200, vec![("d", "order-1"), ("s", "success")]);
        let other = make_event(38383, 150, vec![("d", "order-2"), ("s", "success")]);

        let deduped = dedup_orders_by_d_tag(vec![older, newer.clone(), other.clone()]);

        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped.get("order-1").unwrap().id, newer.id);
        assert_eq!(deduped.get("order-2").unwrap().id, other.id);
    }

    #[test]
    fn dedup_orders_by_d_tag_ignores_events_without_a_d_tag() {
        let no_d_tag = make_event(38383, 100, vec![("s", "success")]);
        let deduped = dedup_orders_by_d_tag(vec![no_d_tag]);
        assert!(deduped.is_empty());
    }
}
