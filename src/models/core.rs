use nostr_sdk::prelude::*;

/// PR 1 Step C: shared single-letter tag accessor, extracted from the repeated
/// `event.tags.iter().find(...)` pattern duplicated across dedup, dev-fee, and order
/// aggregation. Returns the tag's first value (index 1) as a borrowed `&str`.
pub fn tag_value<'e>(event: &'e Event, name: &str) -> Option<&'e str> {
    event
        .tags
        .iter()
        .find(|t| t.as_slice().first().map(|s| s.as_str()) == Some(name))
        .and_then(|t| t.as_slice().get(1))
        .map(|s| s.as_str())
}

/// `z` tag accessor (order/dev-fee-payment discriminator).
pub fn z_tag(event: &Event) -> Option<&str> {
    tag_value(event, "z")
}

/// `y` tag accessor (dev-fee event's `mostro` marker).
pub fn y_tag(event: &Event) -> Option<&str> {
    tag_value(event, "y")
}

/// `d` tag accessor (order id / replaceable-event identifier).
pub fn d_tag(event: &Event) -> Option<&str> {
    tag_value(event, "d")
}

/// `s` tag accessor (order status).
pub fn s_tag(event: &Event) -> Option<&str> {
    tag_value(event, "s")
}

/// `amt` tag accessor (trade amount, sats).
pub fn amt_tag(event: &Event) -> Option<&str> {
    tag_value(event, "amt")
}

// `f` (fiat currency), `pm` (payment method), `premium`, and `bond_enabled` accessors
// are not extracted here: base `src/main.rs` never reads those tags, so adding them now
// would be new behavior, not a mechanical move. They land in PR 3 (event scoping) and
// PR 6 (descriptive context), each alongside the aggregation logic that first uses them.

/// PR 1 Step B seam: partition fetched events into dev-fee events (z=dev-fee-payment,
/// y=mostro) and order events (z=order). Extracted verbatim from the wrapped function
/// body; pure signature, no network, no I/O.
pub fn partition_by_z_y_tag(events: Vec<Event>) -> (Vec<Event>, Vec<Event>) {
    let mut dev_fee_events: Vec<Event> = Vec::new();
    let mut order_events: Vec<Event> = Vec::new();

    for event in events {
        match (z_tag(&event), y_tag(&event)) {
            (Some("dev-fee-payment"), Some("mostro")) => dev_fee_events.push(event),
            (Some("order"), _) => order_events.push(event),
            _ => {}
        }
    }

    (dev_fee_events, order_events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::test_support::make_event;

    #[test]
    fn partition_by_z_y_tag_splits_dev_fee_and_order_events() {
        let dev_fee = make_event(8383, 100, vec![("z", "dev-fee-payment"), ("y", "mostro")]);
        let order = make_event(38383, 100, vec![("z", "order")]);
        let unrelated = make_event(1, 100, vec![("z", "something-else")]);

        let (dev_fee_events, order_events) =
            partition_by_z_y_tag(vec![dev_fee.clone(), order.clone(), unrelated]);

        assert_eq!(dev_fee_events.len(), 1);
        assert_eq!(dev_fee_events[0].id, dev_fee.id);
        assert_eq!(order_events.len(), 1);
        assert_eq!(order_events[0].id, order.id);
    }

    #[test]
    fn partition_by_z_y_tag_requires_y_mostro_for_dev_fee_events() {
        let wrong_y = make_event(8383, 100, vec![("z", "dev-fee-payment"), ("y", "other")]);
        let (dev_fee_events, order_events) = partition_by_z_y_tag(vec![wrong_y]);
        assert!(dev_fee_events.is_empty());
        assert!(order_events.is_empty());
    }
}
