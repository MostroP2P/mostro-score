use nostr_sdk::prelude::*;

/// 002 FR-014's progress-indicator port: a single method the fetch layer invokes once a
/// relay fetch has run past FR-014's latency threshold. `fetch` and `report` both depend
/// on this trait through their shared dependency on `models`, rather than `fetch`
/// depending on `report` directly — the concrete terminal implementation
/// (`report::progress::TerminalProgressReporter`) is bound into
/// `fetch::client::RelayEventSource` only at construction time, in the library/binary
/// wiring root, so `fetch` itself never depends on `report`.
pub trait ProgressReporter {
    fn report_slow_fetch(&self);
}

/// The default reporter for any `RelayEventSource` that has nothing to report progress
/// to (e.g. a caller that never bound a terminal implementation). Does nothing.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoOpProgressReporter;

impl ProgressReporter for NoOpProgressReporter {
    fn report_slow_fetch(&self) {}
}

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

/// `f` tag accessor (fiat currency, 001 FR-008). An order always carries exactly one
/// `f` value, so this is a single-value accessor, matching `amt_tag`/`s_tag`'s pattern.
pub fn f_tag(event: &Event) -> Option<&str> {
    tag_value(event, "f")
}

/// `premium` tag accessor (001 FR-011). The raw string value; parsing it as a signed
/// integer is the caller's job (`models::order::aggregate_order_events`), since an empty
/// or unparseable value is excluded per FR-013 rather than treated as an error here.
pub fn premium_tag(event: &Event) -> Option<&str> {
    tag_value(event, "premium")
}

/// `pm` tag accessor (payment method, 001 FR-009). Unlike every other single-value tag
/// accessor in this module, `pm` is a multi-value Nostr tag: `order.payment_method` is
/// split on commas server-side (`mostro/src/nip33.rs`'s `order_to_tags`), but the
/// resulting `Vec<String>` is passed directly as the tag's content, so each split token
/// becomes its own array element on the wire (`["pm", "SEPA", "Cash"]`), not one
/// comma-joined value — reading only index 1 (`tag_value`'s single-value convention)
/// would silently drop every method after the first. This returns every value from
/// index 1 onward, filtering empty ones defensively since relay data is untrusted
/// (FR-013), even though `order_to_tags` already filters them before publishing. Values
/// are never trimmed, re-split, or case-normalized: FR-009 requires byte-for-byte
/// comparison of method labels exactly as published.
pub fn pm_tag_values(event: &Event) -> Vec<String> {
    event
        .tags
        .iter()
        .find(|t| t.as_slice().first().map(|s| s.as_str()) == Some("pm"))
        .map(|t| {
            t.as_slice()[1..]
                .iter()
                .filter(|value| !value.is_empty())
                .map(|value| value.to_string())
                .collect()
        })
        .unwrap_or_default()
}

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
    fn no_op_progress_reporter_does_nothing_without_panicking() {
        NoOpProgressReporter.report_slow_fetch();
    }

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

    #[test]
    fn f_tag_returns_the_single_fiat_currency_value() {
        let event = make_event(38383, 100, vec![("f", "USD")]);
        assert_eq!(f_tag(&event), Some("USD"));
    }

    #[test]
    fn f_tag_is_none_when_missing() {
        let event = make_event(38383, 100, vec![]);
        assert_eq!(f_tag(&event), None);
    }

    #[test]
    fn premium_tag_returns_the_raw_string_value() {
        let event = make_event(38383, 100, vec![("premium", "-5")]);
        assert_eq!(premium_tag(&event), Some("-5"));
    }

    /// Builds an event with a multi-value tag, e.g. `["pm", "SEPA", "Cash"]` — matching
    /// the actual Nostr wire format for `pm` (`mostro/src/nip33.rs` passes the
    /// already-comma-split `Vec<String>` directly as the tag's content, so each method
    /// is its own array element, not a single comma-joined string). The shared
    /// `test_support::make_event` helper only builds single-value `(name, value)` tags,
    /// which cannot represent this.
    fn make_event_with_multi_value_tag(kind: u16, created_at: u64, values: Vec<&str>) -> Event {
        let keys = Keys::generate();
        EventBuilder::new(Kind::Custom(kind), "")
            .tags(vec![Tag::parse(values).expect("valid tag")])
            .custom_created_at(Timestamp::from(created_at))
            .sign_with_keys(&keys)
            .expect("event signs")
    }

    #[test]
    fn pm_tag_values_reads_every_value_not_just_the_first() {
        let event = make_event_with_multi_value_tag(38383, 100, vec!["pm", "SEPA", "Cash"]);
        assert_eq!(
            pm_tag_values(&event),
            vec!["SEPA".to_string(), "Cash".to_string()]
        );
    }

    /// Defensive only: `order_to_tags` never publishes an empty value, but this is
    /// untrusted relay data (FR-013), so an empty array element must still be excluded.
    #[test]
    fn pm_tag_values_defensively_filters_empty_values() {
        let event = make_event_with_multi_value_tag(38383, 100, vec!["pm", "SEPA", "", "Cash"]);
        assert_eq!(
            pm_tag_values(&event),
            vec!["SEPA".to_string(), "Cash".to_string()]
        );
    }

    #[test]
    fn pm_tag_values_never_trims_or_normalizes_case() {
        let event = make_event_with_multi_value_tag(38383, 100, vec!["pm", "SEPA", " Cash"]);
        assert_eq!(
            pm_tag_values(&event),
            vec!["SEPA".to_string(), " Cash".to_string()]
        );
    }

    #[test]
    fn pm_tag_values_is_empty_when_missing() {
        let event = make_event(38383, 100, vec![]);
        assert!(pm_tag_values(&event).is_empty());
    }
}
