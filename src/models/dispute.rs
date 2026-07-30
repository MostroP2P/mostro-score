//! Kind `38386` dispute events: dedup by the dispute's `d` tag to each dispute's latest
//! reported status — a NIP-33 replaceable event, republished at each status change —
//! then classify that deduplicated set into resolved, active, or unknown counts.

use crate::models::core::s_tag;
use crate::models::dedup::dedup_events_by_d_tag;
use nostr_sdk::prelude::*;

/// A deduplicated dispute's classified status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisputeStatus {
    /// `s` = `settled`, `seller-refunded`, or `released`.
    Resolved,
    /// `s` = `initiated` or `in-progress`.
    Active,
    /// `s` is missing or does not match any of the five known values. Still counts once
    /// toward the total, since a valid dispute event exists for it regardless of status.
    Unknown,
}

/// Per-dispute classification, applied to a single deduplicated dispute's latest `s`
/// value.
pub fn classify_dispute_status(s_value: Option<&str>) -> DisputeStatus {
    match s_value {
        Some("settled") | Some("seller-refunded") | Some("released") => DisputeStatus::Resolved,
        Some("initiated") | Some("in-progress") => DisputeStatus::Active,
        _ => DisputeStatus::Unknown,
    }
}

/// The result of deduplicating a node's fetched dispute events and classifying each
/// unique dispute's latest status. `resolved + active + unknown` always equals
/// `total_disputes`.
pub struct DisputeAggregate {
    pub total_disputes: usize,
    pub resolved: usize,
    pub active: usize,
    pub unknown: usize,
}

/// Dedups fetched dispute events by their `d` tag (highest `created_at`, ties broken by
/// the greatest event id), then classifies each deduplicated dispute's latest `s`
/// value. Events without a `d` tag are safely excluded, never counted.
pub fn aggregate_dispute_events(dispute_events: Vec<Event>) -> DisputeAggregate {
    let deduped = dedup_events_by_d_tag(dispute_events);

    let mut resolved = 0usize;
    let mut active = 0usize;
    let mut unknown = 0usize;

    for event in deduped.values() {
        match classify_dispute_status(s_tag(event)) {
            DisputeStatus::Resolved => resolved += 1,
            DisputeStatus::Active => active += 1,
            DisputeStatus::Unknown => unknown += 1,
        }
    }

    DisputeAggregate {
        total_disputes: deduped.len(),
        resolved,
        active,
        unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::test_support::make_event;

    #[test]
    fn classify_dispute_status_recognizes_every_resolved_value() {
        assert_eq!(
            classify_dispute_status(Some("settled")),
            DisputeStatus::Resolved
        );
        assert_eq!(
            classify_dispute_status(Some("seller-refunded")),
            DisputeStatus::Resolved
        );
        assert_eq!(
            classify_dispute_status(Some("released")),
            DisputeStatus::Resolved
        );
    }

    #[test]
    fn classify_dispute_status_recognizes_every_active_value() {
        assert_eq!(
            classify_dispute_status(Some("initiated")),
            DisputeStatus::Active
        );
        assert_eq!(
            classify_dispute_status(Some("in-progress")),
            DisputeStatus::Active
        );
    }

    #[test]
    fn classify_dispute_status_treats_unrecognized_and_missing_values_as_unknown() {
        assert_eq!(
            classify_dispute_status(Some("something-else")),
            DisputeStatus::Unknown
        );
        assert_eq!(classify_dispute_status(None), DisputeStatus::Unknown);
    }

    #[test]
    fn aggregate_dispute_events_dedups_by_d_tag_and_classifies_the_latest_status() {
        let older_initiated = make_event(38386, 100, vec![("d", "dispute-1"), ("s", "initiated")]);
        let newer_settled = make_event(38386, 200, vec![("d", "dispute-1"), ("s", "settled")]);
        let another_active = make_event(38386, 150, vec![("d", "dispute-2"), ("s", "in-progress")]);
        let unknown_status = make_event(38386, 150, vec![("d", "dispute-3"), ("s", "weird")]);

        let aggregate = aggregate_dispute_events(vec![
            older_initiated,
            newer_settled,
            another_active,
            unknown_status,
        ]);

        assert_eq!(aggregate.total_disputes, 3);
        assert_eq!(aggregate.resolved, 1);
        assert_eq!(aggregate.active, 1);
        assert_eq!(aggregate.unknown, 1);
    }

    #[test]
    fn aggregate_dispute_events_ignores_events_without_a_d_tag() {
        let no_d_tag = make_event(38386, 100, vec![("s", "settled")]);

        let aggregate = aggregate_dispute_events(vec![no_d_tag]);

        assert_eq!(aggregate.total_disputes, 0);
        assert_eq!(aggregate.resolved, 0);
        assert_eq!(aggregate.active, 0);
        assert_eq!(aggregate.unknown, 0);
    }

    /// A dispute event carrying no tags at all must be safely excluded, never panic.
    #[test]
    fn aggregate_dispute_events_handles_a_tagless_event_without_panicking() {
        let tagless = make_event(38386, 100, vec![]);

        let aggregate = aggregate_dispute_events(vec![tagless]);

        assert_eq!(aggregate.total_disputes, 0);
    }

    #[test]
    fn aggregate_dispute_events_resolved_plus_active_plus_unknown_equals_total() {
        let events = vec![
            make_event(38386, 100, vec![("d", "d-1"), ("s", "settled")]),
            make_event(38386, 100, vec![("d", "d-2"), ("s", "initiated")]),
            make_event(38386, 100, vec![("d", "d-3")]),
        ];

        let aggregate = aggregate_dispute_events(events);

        assert_eq!(
            aggregate.resolved + aggregate.active + aggregate.unknown,
            aggregate.total_disputes
        );
    }
}
