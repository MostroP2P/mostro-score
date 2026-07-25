use nostr_sdk::prelude::*;

/// PR 1 Step B seam: select the oldest dev-fee event (the longevity anchor), sorted by
/// `created_at` ascending. Extracted verbatim from the wrapped function body; pure
/// signature, no network, no I/O.
pub fn select_oldest_dev_fee_event(mut dev_fee_events: Vec<Event>) -> Option<Event> {
    if dev_fee_events.is_empty() {
        return None;
    }

    dev_fee_events.sort_by_key(|e| e.created_at);
    Some(dev_fee_events[0].clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::test_support::make_event;

    #[test]
    fn select_oldest_dev_fee_event_empty_is_none() {
        assert!(select_oldest_dev_fee_event(vec![]).is_none());
    }

    #[test]
    fn select_oldest_dev_fee_event_picks_earliest_created_at() {
        let older = make_event(8383, 100, vec![("z", "dev-fee-payment"), ("y", "mostro")]);
        let newer = make_event(8383, 200, vec![("z", "dev-fee-payment"), ("y", "mostro")]);

        let oldest = select_oldest_dev_fee_event(vec![newer, older.clone()]).unwrap();

        assert_eq!(oldest.id, older.id);
    }
}
