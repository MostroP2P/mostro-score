use crate::models::core::d_tag;
use nostr_sdk::prelude::*;
use std::collections::{HashMap, HashSet};

/// Deduplicates events by event id (not by `d` tag), keeping the first occurrence — for
/// kinds like dev-fee payments that have no NIP-33 replaceable-event semantics of their
/// own, this is the only dedup axis that matters, guarding against the same
/// relay-delivered event being independently returned, and double-counted, by more than
/// one relay.
pub fn dedup_events_by_id(events: Vec<Event>) -> Vec<Event> {
    let mut seen_ids: HashSet<EventId> = HashSet::new();
    events
        .into_iter()
        .filter(|event| seen_ids.insert(event.id))
        .collect()
}

/// Shared tie-break rule: the event with the highest `created_at` wins; when two
/// candidates tie on `created_at`, the one with the lexicographically greatest event id
/// wins instead. `EventId`'s `Ord` impl compares its raw fixed-length bytes, which
/// agrees with lexicographic comparison of its hex encoding.
fn is_more_current(candidate: &Event, existing: &Event) -> bool {
    match candidate.created_at.cmp(&existing.created_at) {
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Less => false,
        std::cmp::Ordering::Equal => candidate.id > existing.id,
    }
}

/// Deduplicates events by their `d` tag, keeping the current/final event for each key
/// per the shared `is_more_current` tie-break rule. Used for order events and dispute
/// events alike — both are NIP-33 replaceable events keyed by `d`, republished at each
/// status change.
pub fn dedup_events_by_d_tag(order_events: Vec<Event>) -> HashMap<String, Event> {
    let mut orders_map: HashMap<String, Event> = HashMap::new();

    for event in order_events {
        if let Some(order_id) = d_tag(&event) {
            match orders_map.get(order_id) {
                Some(existing) => {
                    if is_more_current(&event, existing) {
                        orders_map.insert(order_id.to_string(), event.clone());
                    }
                }
                None => {
                    orders_map.insert(order_id.to_string(), event.clone());
                }
            }
        }
    }

    orders_map
}

/// Selects the single current/final event from a homogeneous candidate set (e.g. all
/// instance-status events already filtered to one node's own `d` tag), applying the
/// same tie-break rule as `dedup_events_by_d_tag`.
pub fn select_current_event(events: Vec<Event>) -> Option<Event> {
    events
        .into_iter()
        .fold(None, |current, candidate| match current {
            None => Some(candidate),
            Some(existing) => {
                if is_more_current(&candidate, &existing) {
                    Some(candidate)
                } else {
                    Some(existing)
                }
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::test_support::make_event;

    #[test]
    fn dedup_events_by_d_tag_keeps_greatest_created_at_per_order_id() {
        let older = make_event(38383, 100, vec![("d", "order-1"), ("s", "pending")]);
        let newer = make_event(38383, 200, vec![("d", "order-1"), ("s", "success")]);
        let other = make_event(38383, 150, vec![("d", "order-2"), ("s", "success")]);

        let deduped = dedup_events_by_d_tag(vec![older, newer.clone(), other.clone()]);

        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped.get("order-1").unwrap().id, newer.id);
        assert_eq!(deduped.get("order-2").unwrap().id, other.id);
    }

    #[test]
    fn dedup_events_by_d_tag_ignores_events_without_a_d_tag() {
        let no_d_tag = make_event(38383, 100, vec![("s", "success")]);
        let deduped = dedup_events_by_d_tag(vec![no_d_tag]);
        assert!(deduped.is_empty());
    }

    #[test]
    fn dedup_events_by_d_tag_breaks_created_at_ties_by_greatest_event_id_regardless_of_order() {
        let one = make_event(38383, 100, vec![("d", "order-1"), ("s", "pending")]);
        let other = make_event(38383, 100, vec![("d", "order-1"), ("s", "success")]);
        let expected_winner_id = std::cmp::max(one.id, other.id);

        // The winner must not depend on which event happened to be processed first —
        // both insertion orders must agree on the greatest event id.
        let deduped_forward = dedup_events_by_d_tag(vec![one.clone(), other.clone()]);
        let deduped_backward = dedup_events_by_d_tag(vec![other.clone(), one.clone()]);

        assert_eq!(
            deduped_forward.get("order-1").unwrap().id,
            expected_winner_id
        );
        assert_eq!(
            deduped_backward.get("order-1").unwrap().id,
            expected_winner_id
        );
    }

    #[test]
    fn dedup_events_by_id_collapses_the_same_event_delivered_by_multiple_relays() {
        let event = make_event(8383, 100, vec![("z", "dev-fee-payment"), ("y", "mostro")]);

        // The same event object, as if two relays independently returned it.
        let deduped = dedup_events_by_id(vec![event.clone(), event.clone()]);

        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0].id, event.id);
    }

    #[test]
    fn dedup_events_by_id_keeps_genuinely_distinct_events() {
        let first = make_event(8383, 100, vec![("z", "dev-fee-payment"), ("y", "mostro")]);
        let second = make_event(8383, 200, vec![("z", "dev-fee-payment"), ("y", "mostro")]);

        let deduped = dedup_events_by_id(vec![first, second]);

        assert_eq!(deduped.len(), 2);
    }
}
