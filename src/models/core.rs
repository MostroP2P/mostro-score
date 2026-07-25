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

/// PR 3 (T077): FR-015's full scoping rule for a single event — the event's author
/// (signer) must be the node's own pubkey, its `z` tag must match the expected subtype
/// for its kind, and its `y` tag's first value must be `mostro`. Shared across all four
/// scoped kinds (`8383`/`38383`/`38385`/`38386`) so a relay fetch never mixes another
/// node's events, or another application's/subtype's same-kind events, into this node's
/// report.
pub fn is_scoped_event(event: &Event, node_pubkey: &PublicKey, expected_z: &str) -> bool {
    event.pubkey == *node_pubkey
        && z_tag(event) == Some(expected_z)
        && y_tag(event) == Some("mostro")
}

/// Filters a batch of events down to the ones scoped to `node_pubkey` for `expected_z`,
/// per FR-015.
pub fn scope_events_to_node(
    events: Vec<Event>,
    node_pubkey: &PublicKey,
    expected_z: &str,
) -> Vec<Event> {
    events
        .into_iter()
        .filter(|event| is_scoped_event(event, node_pubkey, expected_z))
        .collect()
}

/// PR 3 (T079): FR-014's future-timestamp exclusion — any event whose `created_at` is
/// later than report-generation time cannot be a legitimate signing time relative to the
/// report and MUST be excluded from consideration entirely, not merely deprioritized.
pub fn exclude_future_events(events: Vec<Event>, report_generated_at: Timestamp) -> Vec<Event> {
    events
        .into_iter()
        .filter(|event| event.created_at <= report_generated_at)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::test_support::{make_event, make_event_with_keys};

    #[test]
    fn is_scoped_event_accepts_matching_author_z_and_y() {
        let keys = Keys::generate();
        let event = make_event_with_keys(
            &keys,
            8383,
            100,
            vec![("z", "dev-fee-payment"), ("y", "mostro")],
        );

        assert!(is_scoped_event(
            &event,
            &keys.public_key(),
            "dev-fee-payment"
        ));
    }

    #[test]
    fn is_scoped_event_rejects_a_different_author() {
        let node_keys = Keys::generate();
        let other_keys = Keys::generate();
        let event = make_event_with_keys(
            &other_keys,
            8383,
            100,
            vec![("z", "dev-fee-payment"), ("y", "mostro")],
        );

        assert!(!is_scoped_event(
            &event,
            &node_keys.public_key(),
            "dev-fee-payment"
        ));
    }

    #[test]
    fn is_scoped_event_rejects_a_mismatched_z_value() {
        let keys = Keys::generate();
        let event = make_event_with_keys(&keys, 38383, 100, vec![("z", "order"), ("y", "mostro")]);

        assert!(!is_scoped_event(&event, &keys.public_key(), "dispute"));
    }

    #[test]
    fn is_scoped_event_rejects_a_missing_y_mostro_marker() {
        let keys = Keys::generate();
        let event = make_event_with_keys(&keys, 8383, 100, vec![("z", "dev-fee-payment")]);

        assert!(!is_scoped_event(
            &event,
            &keys.public_key(),
            "dev-fee-payment"
        ));
    }

    #[test]
    fn scope_events_to_node_keeps_only_matching_events() {
        let node_keys = Keys::generate();
        let other_keys = Keys::generate();
        let matching = make_event_with_keys(
            &node_keys,
            8383,
            100,
            vec![("z", "dev-fee-payment"), ("y", "mostro")],
        );
        let wrong_author = make_event_with_keys(
            &other_keys,
            8383,
            100,
            vec![("z", "dev-fee-payment"), ("y", "mostro")],
        );

        let scoped = scope_events_to_node(
            vec![matching.clone(), wrong_author],
            &node_keys.public_key(),
            "dev-fee-payment",
        );

        assert_eq!(scoped.len(), 1);
        assert_eq!(scoped[0].id, matching.id);
    }

    #[test]
    fn exclude_future_events_drops_events_after_report_generation_time() {
        let past = make_event(8383, 100, vec![("z", "dev-fee-payment"), ("y", "mostro")]);
        let future = make_event(8383, 300, vec![("z", "dev-fee-payment"), ("y", "mostro")]);

        let kept = exclude_future_events(vec![past.clone(), future], Timestamp::from(200));

        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].id, past.id);
    }

    #[test]
    fn is_scoped_event_rejects_an_event_with_no_tags_at_all_without_panicking() {
        let keys = Keys::generate();
        let event = make_event_with_keys(&keys, 8383, 100, vec![]);

        assert!(!is_scoped_event(
            &event,
            &keys.public_key(),
            "dev-fee-payment"
        ));
    }

    #[test]
    fn exclude_future_events_keeps_an_event_exactly_at_report_generation_time() {
        let at_boundary = make_event(8383, 200, vec![("z", "dev-fee-payment"), ("y", "mostro")]);

        let kept = exclude_future_events(vec![at_boundary.clone()], Timestamp::from(200));

        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].id, at_boundary.id);
    }

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
