use nostr_sdk::prelude::*;

/// Selects the oldest dev-fee event (the longevity anchor), sorted by `created_at`
/// ascending.
pub fn select_oldest_dev_fee_event(mut dev_fee_events: Vec<Event>) -> Option<Event> {
    if dev_fee_events.is_empty() {
        return None;
    }

    dev_fee_events.sort_by_key(|e| e.created_at);
    Some(dev_fee_events[0].clone())
}

/// Per-kind dev-fee aggregation — the total fetched count plus the longevity anchor
/// timestamp (the oldest dev-fee event's `created_at`). Presentation (the "MOSTRO
/// TRADING ACTIVITY" block and the no-dev-fee warning) stays in
/// `report/render/console.rs`; this function only computes the values that block
/// prints.
pub struct DevFeeAggregate {
    pub count: usize,
    pub first_dev_fee_ts: Option<i64>,
}

pub fn aggregate_dev_fee_events(dev_fee_events: Vec<Event>) -> DevFeeAggregate {
    let count = dev_fee_events.len();
    let first_dev_fee_ts =
        select_oldest_dev_fee_event(dev_fee_events).map(|e| e.created_at.as_u64() as i64);

    DevFeeAggregate {
        count,
        first_dev_fee_ts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::test_support::make_event;

    #[test]
    fn select_oldest_dev_fee_event_empty_is_none() {
        assert!(select_oldest_dev_fee_event(vec![]).is_none());
    }

    /// A dev-fee event carrying no tags at all must not panic anywhere in this
    /// aggregation path — it is simply an event with unknown longevity-relevant
    /// content, still ordered on `created_at` alone.
    #[test]
    fn aggregate_dev_fee_events_handles_a_tagless_event_without_panicking() {
        let tagless = make_event(8383, 100, vec![]);

        let aggregate = aggregate_dev_fee_events(vec![tagless]);

        assert_eq!(aggregate.count, 1);
        assert_eq!(aggregate.first_dev_fee_ts, Some(100));
    }

    #[test]
    fn select_oldest_dev_fee_event_picks_earliest_created_at() {
        let older = make_event(8383, 100, vec![("z", "dev-fee-payment"), ("y", "mostro")]);
        let newer = make_event(8383, 200, vec![("z", "dev-fee-payment"), ("y", "mostro")]);

        let oldest = select_oldest_dev_fee_event(vec![newer, older.clone()]).unwrap();

        assert_eq!(oldest.id, older.id);
    }
}
