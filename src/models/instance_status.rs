//! Kind `38385` instance-status events: a NIP-33 replaceable event keyed by the node's
//! own pubkey as the `d` tag, republished on a timer. Selection restricts the candidate
//! set to events whose `d` tag equals the node's own pubkey, then picks the highest
//! `created_at` within that set (ties broken by the greatest event id).

use crate::models::core::{d_tag, tag_value};
use crate::models::dedup::select_current_event;
use nostr_sdk::prelude::*;

/// Tri-state read of the `bond_enabled` tag: `Unknown` covers both a missing tag and a
/// value that fails to parse as `true`/`false`, never defaulting to `false`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BondEnabled {
    True,
    False,
    Unknown,
}

impl BondEnabled {
    /// The report schema's three-valued bond-policy status string, distinct from
    /// `NodeMetrics`'s trade-history fields. A thin mapping of this existing tri-state;
    /// no new selection or parsing logic.
    pub fn as_bond_policy_status(&self) -> &'static str {
        match self {
            BondEnabled::True => "enabled",
            BondEnabled::False => "disabled",
            BondEnabled::Unknown => "unknown",
        }
    }
}

fn parse_bond_enabled(event: &Event) -> BondEnabled {
    match tag_value(event, "bond_enabled") {
        Some("true") => BondEnabled::True,
        Some("false") => BondEnabled::False,
        _ => BondEnabled::Unknown,
    }
}

/// The result of selecting a node's effective instance-status event (or the absence of
/// one).
pub struct InstanceStatusAggregate {
    pub found: bool,
    pub bond_enabled: BondEnabled,
}

/// Restricts `instance_status_events` to the ones whose `d` tag equals `node_pubkey`'s
/// hex encoding, then selects the current/final event among those candidates (highest
/// `created_at`, ties broken by the greatest event id). Events without a `d` tag, or
/// whose `d` tag does not match, are safely excluded.
pub fn select_instance_status_event(
    instance_status_events: Vec<Event>,
    node_pubkey: &PublicKey,
) -> Option<Event> {
    let node_pubkey_hex = node_pubkey.to_hex();
    let candidates: Vec<Event> = instance_status_events
        .into_iter()
        .filter(|event| d_tag(event) == Some(node_pubkey_hex.as_str()))
        .collect();

    select_current_event(candidates)
}

/// Selects the node's effective instance-status event and reads its `bond_enabled`
/// tri-state. `found` reflects whether a valid instance-status event exists for this
/// node — not merely whether a kind-`38385` event was fetched at all.
pub fn aggregate_instance_status(
    instance_status_events: Vec<Event>,
    node_pubkey: &PublicKey,
) -> InstanceStatusAggregate {
    match select_instance_status_event(instance_status_events, node_pubkey) {
        Some(event) => InstanceStatusAggregate {
            found: true,
            bond_enabled: parse_bond_enabled(&event),
        },
        None => InstanceStatusAggregate {
            found: false,
            bond_enabled: BondEnabled::Unknown,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::test_support::make_event_with_keys;

    #[test]
    fn select_instance_status_event_restricts_candidates_to_the_node_pubkey_d_tag() {
        let node_keys = Keys::generate();
        let node_pubkey = node_keys.public_key();
        let matching = make_event_with_keys(
            &node_keys,
            38385,
            100,
            vec![("d", &node_pubkey.to_hex()), ("z", "info"), ("y", "mostro")],
        );
        let other_node = make_event_with_keys(
            &Keys::generate(),
            38385,
            200,
            vec![("d", "some-other-pubkey"), ("z", "info"), ("y", "mostro")],
        );

        let selected =
            select_instance_status_event(vec![matching.clone(), other_node], &node_pubkey);

        assert_eq!(selected.unwrap().id, matching.id);
    }

    #[test]
    fn select_instance_status_event_picks_the_highest_created_at() {
        let node_keys = Keys::generate();
        let node_pubkey = node_keys.public_key();
        let older = make_event_with_keys(
            &node_keys,
            38385,
            100,
            vec![("d", &node_pubkey.to_hex()), ("bond_enabled", "false")],
        );
        let newer = make_event_with_keys(
            &node_keys,
            38385,
            200,
            vec![("d", &node_pubkey.to_hex()), ("bond_enabled", "true")],
        );

        let selected = select_instance_status_event(vec![older, newer.clone()], &node_pubkey);

        assert_eq!(selected.unwrap().id, newer.id);
    }

    #[test]
    fn select_instance_status_event_breaks_created_at_ties_by_greatest_event_id() {
        let node_keys = Keys::generate();
        let node_pubkey = node_keys.public_key();
        let one = make_event_with_keys(
            &node_keys,
            38385,
            100,
            vec![("d", &node_pubkey.to_hex()), ("bond_enabled", "true")],
        );
        let other = make_event_with_keys(
            &node_keys,
            38385,
            100,
            vec![("d", &node_pubkey.to_hex()), ("bond_enabled", "false")],
        );
        let expected_winner_id = std::cmp::max(one.id, other.id);

        let selected = select_instance_status_event(vec![one, other], &node_pubkey);

        assert_eq!(selected.unwrap().id, expected_winner_id);
    }

    #[test]
    fn select_instance_status_event_returns_none_when_no_candidate_matches() {
        let node_pubkey = Keys::generate().public_key();
        let unrelated = make_event_with_keys(
            &Keys::generate(),
            38385,
            100,
            vec![("d", "not-the-node-pubkey")],
        );

        let selected = select_instance_status_event(vec![unrelated], &node_pubkey);

        assert!(selected.is_none());
    }

    #[test]
    fn aggregate_instance_status_reports_not_found_when_no_valid_event_exists() {
        let node_pubkey = Keys::generate().public_key();

        let aggregate = aggregate_instance_status(vec![], &node_pubkey);

        assert!(!aggregate.found);
        assert_eq!(aggregate.bond_enabled, BondEnabled::Unknown);
    }

    #[test]
    fn aggregate_instance_status_reads_bond_enabled_true() {
        let node_keys = Keys::generate();
        let node_pubkey = node_keys.public_key();
        let event = make_event_with_keys(
            &node_keys,
            38385,
            100,
            vec![("d", &node_pubkey.to_hex()), ("bond_enabled", "true")],
        );

        let aggregate = aggregate_instance_status(vec![event], &node_pubkey);

        assert!(aggregate.found);
        assert_eq!(aggregate.bond_enabled, BondEnabled::True);
    }

    #[test]
    fn aggregate_instance_status_reads_bond_enabled_false() {
        let node_keys = Keys::generate();
        let node_pubkey = node_keys.public_key();
        let event = make_event_with_keys(
            &node_keys,
            38385,
            100,
            vec![("d", &node_pubkey.to_hex()), ("bond_enabled", "false")],
        );

        let aggregate = aggregate_instance_status(vec![event], &node_pubkey);

        assert!(aggregate.found);
        assert_eq!(aggregate.bond_enabled, BondEnabled::False);
    }

    #[test]
    fn aggregate_instance_status_treats_a_missing_or_unparseable_bond_enabled_as_unknown() {
        let node_keys = Keys::generate();
        let node_pubkey = node_keys.public_key();
        let missing_tag =
            make_event_with_keys(&node_keys, 38385, 100, vec![("d", &node_pubkey.to_hex())]);
        let unparseable_tag = make_event_with_keys(
            &node_keys,
            38385,
            100,
            vec![("d", &node_pubkey.to_hex()), ("bond_enabled", "maybe")],
        );

        let aggregate = aggregate_instance_status(vec![missing_tag], &node_pubkey);
        assert!(aggregate.found);
        assert_eq!(aggregate.bond_enabled, BondEnabled::Unknown);

        let aggregate = aggregate_instance_status(vec![unparseable_tag], &node_pubkey);
        assert!(aggregate.found);
        assert_eq!(aggregate.bond_enabled, BondEnabled::Unknown);
    }

    /// An instance-status event carrying no tags at all (no `d` tag either) must never
    /// match any node's pubkey and must never panic.
    #[test]
    fn select_instance_status_event_handles_a_tagless_event_without_panicking() {
        let node_pubkey = Keys::generate().public_key();
        let tagless = make_event_with_keys(&Keys::generate(), 38385, 100, vec![]);

        let selected = select_instance_status_event(vec![tagless], &node_pubkey);

        assert!(selected.is_none());
    }

    #[test]
    fn bond_enabled_maps_to_the_report_schemas_three_valued_status_string() {
        assert_eq!(BondEnabled::True.as_bond_policy_status(), "enabled");
        assert_eq!(BondEnabled::False.as_bond_policy_status(), "disabled");
        assert_eq!(BondEnabled::Unknown.as_bond_policy_status(), "unknown");
    }
}
