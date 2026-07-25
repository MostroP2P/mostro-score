//! Per-kind Nostr filters for a node's four scoped event kinds (001 FR-015), and
//! `RelayFetchOutcome` — the fetch-result summary those four kinds produce (002 FR-003).
//! Each `RelayFetchOutcome` field follows its own semantic rule rather than a single
//! generic id-dedup helper: `dev_fee_events`/`order_events` are raw event-id dedup
//! counts, `unique_orders` applies `models::order`'s full qualifying-order procedure,
//! `dispute_events` applies `models::dispute`'s dedup-by-`d`-tag classification, and
//! `instance_status_found` reflects `models::instance_status`'s actual valid-instance
//! selection.

use crate::models::core::scope_events_to_node;
use crate::models::dispute::DisputeAggregate;
use crate::models::instance_status::aggregate_instance_status;
use crate::models::order::OrderAggregate;
use nostr_sdk::prelude::*;
use nostr_sdk::{Alphabet, SingleLetterTag};
use std::collections::HashSet;

/// Dev-fee payment event kind (not in `mostro-core`).
pub const DEV_FEE_EVENT_KIND: u16 = 8383;
/// Peer-to-peer order event kind.
pub const ORDER_EVENT_KIND: u16 = 38383;
/// Node instance-status event kind.
pub const INSTANCE_STATUS_EVENT_KIND: u16 = 38385;
/// Dispute event kind.
pub const DISPUTE_EVENT_KIND: u16 = 38386;

fn scoped_filter(public_key: PublicKey, kind: u16, expected_z: &str) -> Filter {
    Filter::new()
        .kind(Kind::Custom(kind))
        .author(public_key)
        .custom_tag(SingleLetterTag::lowercase(Alphabet::Z), expected_z)
        .custom_tag(SingleLetterTag::lowercase(Alphabet::Y), "mostro")
}

/// FR-015: the four kind-scoped filters a relay fetch issues for one node's report —
/// dev-fee (`8383`), order (`38383`), instance-status (`38385`), and dispute (`38386`),
/// each scoped to the node's own pubkey as author, its kind's expected `z` value, and
/// `y=mostro`.
pub fn build_scoped_filters(public_key: PublicKey) -> [Filter; 4] {
    [
        scoped_filter(public_key, DEV_FEE_EVENT_KIND, "dev-fee-payment"),
        scoped_filter(public_key, ORDER_EVENT_KIND, "order"),
        scoped_filter(public_key, INSTANCE_STATUS_EVENT_KIND, "info"),
        scoped_filter(public_key, DISPUTE_EVENT_KIND, "dispute"),
    ]
}

/// A node's raw fetched events (all four scoped kinds, concatenated), routed into
/// per-kind buckets and scoped to the node's own author/z/y per FR-015. Events of any
/// other kind, or that fail scoping for their kind, are silently excluded (FR-013).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PartitionedEvents {
    pub dev_fee_events: Vec<Event>,
    pub order_events: Vec<Event>,
    pub instance_status_events: Vec<Event>,
    pub dispute_events: Vec<Event>,
}

pub fn partition_scoped_events(events: Vec<Event>, node_pubkey: &PublicKey) -> PartitionedEvents {
    let mut raw_dev_fee: Vec<Event> = Vec::new();
    let mut raw_order: Vec<Event> = Vec::new();
    let mut raw_instance_status: Vec<Event> = Vec::new();
    let mut raw_dispute: Vec<Event> = Vec::new();

    for event in events {
        match event.kind.as_u16() {
            DEV_FEE_EVENT_KIND => raw_dev_fee.push(event),
            ORDER_EVENT_KIND => raw_order.push(event),
            INSTANCE_STATUS_EVENT_KIND => raw_instance_status.push(event),
            DISPUTE_EVENT_KIND => raw_dispute.push(event),
            _ => {}
        }
    }

    PartitionedEvents {
        dev_fee_events: scope_events_to_node(raw_dev_fee, node_pubkey, "dev-fee-payment"),
        order_events: scope_events_to_node(raw_order, node_pubkey, "order"),
        instance_status_events: scope_events_to_node(raw_instance_status, node_pubkey, "info"),
        dispute_events: scope_events_to_node(raw_dispute, node_pubkey, "dispute"),
    }
}

/// 002 FR-003's per-kind fetch-result summary. Each field follows its own semantic rule,
/// not one generic id-dedup helper:
/// - `dev_fee_events`/`order_events`: raw event-id dedup count, guarding against the same
///   relay-delivered event being double-counted when multiple relays return it.
/// - `unique_orders`: `models::order`'s full qualifying-order procedure (001 FR-002).
/// - `has_valid_orders`: whether at least one deduplicated order carries a recognized
///   `s` status, independent of whether that status is `success` — an order missing its
///   `d` tag, or missing/carrying an unrecognized `s` value entirely, is malformed
///   (FR-013) and must not count as usable data on its own, even though it still
///   increments the raw `order_events` count above.
/// - `dispute_events`: `models::dispute`'s dedup-by-`d`-tag count (001 FR-006).
/// - `instance_status_found`: whether `models::instance_status`'s actual valid-instance
///   selection succeeded (001 FR-012), not merely whether a kind-`38385` event was
///   fetched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayFetchOutcome {
    pub dev_fee_events: usize,
    pub order_events: usize,
    pub unique_orders: usize,
    pub has_valid_orders: bool,
    pub dispute_events: usize,
    pub instance_status_found: bool,
}

impl RelayFetchOutcome {
    /// 002 FR-019 exit code `4`'s gate: true only when none of the four scoped kinds
    /// yielded any usable data at all. Orders are gated on `has_valid_orders`, not the
    /// raw `order_events` count: an order fetched but missing its `d` tag is discarded
    /// entirely by `models::order`'s qualifying-order procedure and must not, on its
    /// own, count as "this node has usable order data."
    pub fn has_no_usable_events(&self) -> bool {
        self.dev_fee_events == 0
            && !self.has_valid_orders
            && self.dispute_events == 0
            && !self.instance_status_found
    }
}

pub(crate) fn dedup_by_event_id_count(events: &[Event]) -> usize {
    events
        .iter()
        .map(|event| event.id)
        .collect::<HashSet<_>>()
        .len()
}

/// Computes a `RelayFetchOutcome` from a node's already-partitioned, already-scoped
/// event buckets. Takes already-computed dev-fee/order counts, the `OrderAggregate`,
/// and the `DisputeAggregate` rather than raw event vectors for those: `run()` needs
/// the deduplicated dev-fee count, the full order aggregate, and the full dispute
/// aggregate for its own reporting regardless (stats included, PR 5), so recomputing
/// any of them here would rescan the same event set for no new information.
pub fn compute_relay_fetch_outcome(
    dev_fee_event_count: usize,
    order_event_count: usize,
    order_aggregate: &OrderAggregate,
    instance_status_events: Vec<Event>,
    dispute_aggregate: &DisputeAggregate,
    node_pubkey: &PublicKey,
) -> RelayFetchOutcome {
    let instance_status_aggregate = aggregate_instance_status(instance_status_events, node_pubkey);

    RelayFetchOutcome {
        dev_fee_events: dev_fee_event_count,
        order_events: order_event_count,
        unique_orders: order_aggregate.successful_orders,
        has_valid_orders: order_aggregate.recognized_status_order_count > 0,
        dispute_events: dispute_aggregate.total_disputes,
        instance_status_found: instance_status_aggregate.found,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::dispute::aggregate_dispute_events;
    use crate::models::order::aggregate_order_events;
    use std::collections::BTreeSet;

    /// Test-only helper mirroring how `run()` derives the two order-related arguments
    /// `compute_relay_fetch_outcome` needs, without computing the aggregate twice.
    fn order_parts(order_events: Vec<Event>) -> (usize, OrderAggregate) {
        let count = dedup_by_event_id_count(&order_events);
        (count, aggregate_order_events(order_events))
    }

    fn make_event_with_keys(
        keys: &Keys,
        kind: u16,
        created_at: u64,
        tags: Vec<(&str, &str)>,
    ) -> Event {
        let parsed_tags: Vec<Tag> = tags
            .into_iter()
            .map(|(name, value)| Tag::parse([name, value]).expect("valid tag"))
            .collect();
        EventBuilder::new(Kind::Custom(kind), "")
            .tags(parsed_tags)
            .custom_created_at(Timestamp::from(created_at))
            .sign_with_keys(keys)
            .expect("event signs")
    }

    #[test]
    fn build_scoped_filters_covers_all_four_kinds() {
        let public_key = Keys::generate().public_key();

        let filters = build_scoped_filters(public_key);

        assert_eq!(filters.len(), 4);
    }

    /// Regression: a filter-count assertion alone would still pass for four wrong or
    /// duplicate filters. Assert each filter's actual kind, author, and expected `z`
    /// value, per FR-015.
    #[test]
    fn build_scoped_filters_each_filter_has_the_expected_kind_author_and_z_value() {
        let public_key = Keys::generate().public_key();
        let z_tag = SingleLetterTag::lowercase(Alphabet::Z);
        let y_tag = SingleLetterTag::lowercase(Alphabet::Y);

        let expected = [
            (DEV_FEE_EVENT_KIND, "dev-fee-payment"),
            (ORDER_EVENT_KIND, "order"),
            (INSTANCE_STATUS_EVENT_KIND, "info"),
            (DISPUTE_EVENT_KIND, "dispute"),
        ];

        for (filter, (expected_kind, expected_z)) in
            build_scoped_filters(public_key).into_iter().zip(expected)
        {
            assert_eq!(
                filter.kinds,
                Some(BTreeSet::from([Kind::Custom(expected_kind)]))
            );
            assert_eq!(filter.authors, Some(BTreeSet::from([public_key])));
            assert_eq!(
                filter.generic_tags.get(&z_tag),
                Some(&BTreeSet::from([expected_z.to_string()]))
            );
            assert_eq!(
                filter.generic_tags.get(&y_tag),
                Some(&BTreeSet::from(["mostro".to_string()]))
            );
        }
    }

    #[test]
    fn compute_relay_fetch_outcome_dev_fee_events_dedups_by_event_id() {
        let node_keys = Keys::generate();
        let node_pubkey = node_keys.public_key();
        let event = make_event_with_keys(
            &node_keys,
            8383,
            100,
            vec![("z", "dev-fee-payment"), ("y", "mostro")],
        );

        // Two relays independently delivering the same event id must count once.
        let dev_fee_event_count = dedup_by_event_id_count(&[event.clone(), event.clone()]);
        let (order_event_count, order_aggregate) = order_parts(vec![]);
        let outcome = compute_relay_fetch_outcome(
            dev_fee_event_count,
            order_event_count,
            &order_aggregate,
            vec![],
            &aggregate_dispute_events(vec![]),
            &node_pubkey,
        );

        assert_eq!(outcome.dev_fee_events, 1);
    }

    #[test]
    fn compute_relay_fetch_outcome_unique_orders_uses_the_qualifying_order_procedure() {
        let node_pubkey = Keys::generate().public_key();
        let success = make_event_with_keys(
            &Keys::generate(),
            38383,
            100,
            vec![("d", "order-1"), ("s", "success")],
        );
        let pending = make_event_with_keys(
            &Keys::generate(),
            38383,
            100,
            vec![("d", "order-2"), ("s", "pending")],
        );

        let (order_event_count, order_aggregate) = order_parts(vec![success, pending]);
        let outcome = compute_relay_fetch_outcome(
            0,
            order_event_count,
            &order_aggregate,
            vec![],
            &aggregate_dispute_events(vec![]),
            &node_pubkey,
        );

        assert_eq!(outcome.order_events, 2);
        assert_eq!(outcome.unique_orders, 1);
        assert!(outcome.has_valid_orders);
    }

    /// Regression: an order missing its `d` tag is discarded entirely by the qualifying-
    /// order procedure (FR-013), so it must not count as usable order data even though it
    /// still increments the raw `order_events` count.
    #[test]
    fn compute_relay_fetch_outcome_has_valid_orders_is_false_for_a_d_tagless_order() {
        let node_pubkey = Keys::generate().public_key();
        let no_d_tag = make_event_with_keys(&Keys::generate(), 38383, 100, vec![("s", "success")]);

        let (order_event_count, order_aggregate) = order_parts(vec![no_d_tag]);
        let outcome = compute_relay_fetch_outcome(
            0,
            order_event_count,
            &order_aggregate,
            vec![],
            &aggregate_dispute_events(vec![]),
            &node_pubkey,
        );

        assert_eq!(outcome.order_events, 1);
        assert!(!outcome.has_valid_orders);
        assert!(outcome.has_no_usable_events());
    }

    /// Regression: an order with a `d` tag but no `s` tag at all survives `d`-tag dedup
    /// (it has a valid dedup key), but real Mostro order events always publish a status
    /// — a missing one is malformed data (FR-013), not evidence of order history, and
    /// must not count toward `has_valid_orders`.
    #[test]
    fn compute_relay_fetch_outcome_has_valid_orders_is_false_for_an_order_missing_its_s_tag() {
        let node_pubkey = Keys::generate().public_key();
        let missing_s_tag =
            make_event_with_keys(&Keys::generate(), 38383, 100, vec![("d", "order-1")]);

        let (order_event_count, order_aggregate) = order_parts(vec![missing_s_tag]);
        let outcome = compute_relay_fetch_outcome(
            0,
            order_event_count,
            &order_aggregate,
            vec![],
            &aggregate_dispute_events(vec![]),
            &node_pubkey,
        );

        assert_eq!(outcome.order_events, 1);
        assert_eq!(order_aggregate.unique_order_count, 1);
        assert!(!outcome.has_valid_orders);
        assert!(outcome.has_no_usable_events());
    }

    #[test]
    fn compute_relay_fetch_outcome_dispute_events_uses_d_tag_dedup() {
        let node_pubkey = Keys::generate().public_key();
        let older = make_event_with_keys(
            &Keys::generate(),
            38386,
            100,
            vec![("d", "dispute-1"), ("s", "initiated")],
        );
        let newer = make_event_with_keys(
            &Keys::generate(),
            38386,
            200,
            vec![("d", "dispute-1"), ("s", "settled")],
        );

        let (order_event_count, order_aggregate) = order_parts(vec![]);
        let outcome = compute_relay_fetch_outcome(
            0,
            order_event_count,
            &order_aggregate,
            vec![],
            &aggregate_dispute_events(vec![older, newer]),
            &node_pubkey,
        );

        assert_eq!(outcome.dispute_events, 1);
    }

    #[test]
    fn compute_relay_fetch_outcome_instance_status_found_reflects_valid_selection() {
        let node_keys = Keys::generate();
        let node_pubkey = node_keys.public_key();
        let unrelated_d_tag =
            make_event_with_keys(&node_keys, 38385, 100, vec![("d", "not-the-node-pubkey")]);

        let (order_event_count, order_aggregate) = order_parts(vec![]);
        let outcome_without_match = compute_relay_fetch_outcome(
            0,
            order_event_count,
            &order_aggregate,
            vec![unrelated_d_tag],
            &aggregate_dispute_events(vec![]),
            &node_pubkey,
        );
        assert!(!outcome_without_match.instance_status_found);

        let matching =
            make_event_with_keys(&node_keys, 38385, 100, vec![("d", &node_pubkey.to_hex())]);
        let (order_event_count, order_aggregate) = order_parts(vec![]);
        let outcome_with_match = compute_relay_fetch_outcome(
            0,
            order_event_count,
            &order_aggregate,
            vec![matching],
            &aggregate_dispute_events(vec![]),
            &node_pubkey,
        );
        assert!(outcome_with_match.instance_status_found);
    }

    #[test]
    fn has_no_usable_events_is_true_only_when_all_four_kinds_are_empty() {
        let empty = RelayFetchOutcome {
            dev_fee_events: 0,
            order_events: 0,
            unique_orders: 0,
            has_valid_orders: false,
            dispute_events: 0,
            instance_status_found: false,
        };
        assert!(empty.has_no_usable_events());

        let has_dev_fee = RelayFetchOutcome {
            dev_fee_events: 1,
            ..empty.clone()
        };
        assert!(!has_dev_fee.has_no_usable_events());

        let has_valid_orders = RelayFetchOutcome {
            has_valid_orders: true,
            ..empty.clone()
        };
        assert!(!has_valid_orders.has_no_usable_events());
    }

    #[test]
    fn partition_scoped_events_routes_each_kind_and_drops_unscoped_events() {
        let node_keys = Keys::generate();
        let node_pubkey = node_keys.public_key();
        let dev_fee = make_event_with_keys(
            &node_keys,
            8383,
            100,
            vec![("z", "dev-fee-payment"), ("y", "mostro")],
        );
        let order = make_event_with_keys(
            &node_keys,
            38383,
            100,
            vec![
                ("z", "order"),
                ("y", "mostro"),
                ("d", "order-1"),
                ("s", "success"),
            ],
        );
        let instance_status = make_event_with_keys(
            &node_keys,
            38385,
            100,
            vec![("z", "info"), ("y", "mostro"), ("d", &node_pubkey.to_hex())],
        );
        let dispute = make_event_with_keys(
            &node_keys,
            38386,
            100,
            vec![("z", "dispute"), ("y", "mostro"), ("d", "dispute-1")],
        );
        let wrong_author = make_event_with_keys(
            &Keys::generate(),
            8383,
            100,
            vec![("z", "dev-fee-payment"), ("y", "mostro")],
        );

        let partitioned = partition_scoped_events(
            vec![
                dev_fee.clone(),
                order.clone(),
                instance_status.clone(),
                dispute.clone(),
                wrong_author,
            ],
            &node_pubkey,
        );

        assert_eq!(partitioned.dev_fee_events.len(), 1);
        assert_eq!(partitioned.dev_fee_events[0].id, dev_fee.id);
        assert_eq!(partitioned.order_events.len(), 1);
        assert_eq!(partitioned.order_events[0].id, order.id);
        assert_eq!(partitioned.instance_status_events.len(), 1);
        assert_eq!(partitioned.instance_status_events[0].id, instance_status.id);
        assert_eq!(partitioned.dispute_events.len(), 1);
        assert_eq!(partitioned.dispute_events[0].id, dispute.id);
    }
}
