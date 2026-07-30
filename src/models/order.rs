use crate::models::core::{amt_tag, f_tag, pm_tag_values, premium_tag, s_tag};
use crate::models::dedup::dedup_events_by_d_tag;
use mostro_core::prelude::Status as OrderStatus;
use nostr_sdk::prelude::*;
use std::collections::HashMap;
use std::str::FromStr;

/// Per-kind order aggregation: raw time-range tracking and `s`-tag distribution over
/// every fetched order event, then `d`-tag dedup (via `models::dedup`) and the
/// `s=success` qualifying-order selection over the deduplicated set. Presentation (the
/// debug block and every report section) stays in `report/render/console.rs`; this
/// struct only holds the values those sections print.
pub struct OrderAggregate {
    pub total_order_count: usize,
    pub first_order_ts: i64,
    pub last_order_ts: i64,
    pub s_tag_distribution: HashMap<String, usize>,
    pub unique_order_count: usize,
    /// Count of deduplicated orders whose `s` tag parses as a recognized `OrderStatus`
    /// (pending, success, canceled, etc. — any known value, not just success). An order
    /// that survives `d`-tag dedup but carries no `s` tag at all is malformed: real
    /// Mostro order events always publish a status, so a missing one is incomplete
    /// data, not evidence of "an order this node placed."
    pub recognized_status_order_count: usize,
    pub successful_orders: usize,
    pub total_volume_sats: u64,
    pub trade_amounts: Vec<u64>,
    pub successful_trade_timestamps: Vec<i64>,
    /// The `f` tag's value for each qualifying successful order, omitting any order
    /// whose `f` value is empty. Excluded entirely (from both the fiat breakdown's
    /// numerator and denominator), not counted as an "(empty)" bucket.
    pub fiat_values: Vec<String>,
    /// Every `pm` tag mention across qualifying successful orders, flattened — one
    /// entry per tag value (`pm` is a multi-value Nostr tag, e.g. `["pm", "SEPA",
    /// "Cash"]`, not a single comma-joined string), so a single order can contribute
    /// zero, one, or many entries.
    pub payment_method_mentions: Vec<String>,
    /// The `premium` tag parsed as `i64` for each qualifying successful order, only when
    /// it parses successfully. A missing, empty, or unparseable value is excluded
    /// rather than treated as `0`.
    pub premium_values: Vec<i64>,
    /// One entry per qualifying successful order, pairing its `created_at` with its
    /// parsed `amt` (or `None` when `amt` did not parse) — the activity grid's own
    /// input shape (`stats::grid::GridOrder`). `trade_amounts` and
    /// `successful_trade_timestamps` above are two separately filtered lists (only
    /// amt-parseable orders vs. every successful order) and cannot be zipped together;
    /// this field is populated in the same loop as both, so the grid never needs a
    /// second scan over the aggregated orders.
    pub qualifying_orders: Vec<(i64, Option<u64>)>,
}

/// The full qualifying-order procedure — dedup by `d` tag to the highest `created_at`
/// (ties broken by the greatest event id), then filter to only the deduplicated events
/// whose selected state carries `s=success`.
pub fn aggregate_order_events(order_events: Vec<Event>) -> OrderAggregate {
    let total_order_count = order_events.len();
    let mut first_order_ts = i64::MAX;
    let mut last_order_ts = 0i64;
    let mut s_tag_distribution: HashMap<String, usize> = HashMap::new();
    let mut pending_dedup_events: Vec<Event> = Vec::new();

    for event in order_events {
        if (event.created_at.as_u64() as i64) < first_order_ts {
            first_order_ts = event.created_at.as_u64() as i64;
        }
        if (event.created_at.as_u64() as i64) > last_order_ts {
            last_order_ts = event.created_at.as_u64() as i64;
        }

        let s_value = s_tag(&event).map(|s| s.to_string());
        match &s_value {
            Some(val) => {
                *s_tag_distribution.entry(val.clone()).or_insert(0) += 1;
            }
            None => {
                *s_tag_distribution
                    .entry("(missing)".to_string())
                    .or_insert(0) += 1;
            }
        }

        pending_dedup_events.push(event);
    }

    let orders_map = dedup_events_by_d_tag(pending_dedup_events);
    let unique_order_count = orders_map.len();

    let mut recognized_status_order_count = 0usize;
    let mut successful_orders = 0usize;
    let mut total_volume_sats = 0u64;
    let mut trade_amounts: Vec<u64> = Vec::new();
    let mut successful_trade_timestamps: Vec<i64> = Vec::new();
    let mut fiat_values: Vec<String> = Vec::new();
    let mut payment_method_mentions: Vec<String> = Vec::new();
    let mut premium_values: Vec<i64> = Vec::new();
    let mut qualifying_orders: Vec<(i64, Option<u64>)> = Vec::new();

    for (_order_id, event) in orders_map {
        let status_str = s_tag(&event).unwrap_or("unknown");
        let parsed_status = OrderStatus::from_str(status_str);

        if parsed_status.is_ok() {
            recognized_status_order_count += 1;
        }

        if parsed_status == Ok(OrderStatus::Success) {
            successful_orders += 1;
            let event_ts = event.created_at.as_u64() as i64;
            successful_trade_timestamps.push(event_ts);

            // Get Amount 'amt' (sats). Parsed once and reused for both `trade_amounts`
            // (the amt-restricted set) and `qualifying_orders` (paired with this same
            // order's timestamp for the activity grid), rather than re-parsing it twice.
            let amount_sats: Option<u64> =
                amt_tag(&event).and_then(|amt_str| amt_str.parse::<u64>().ok());
            if let Some(amount) = amount_sats {
                // `amt` comes from an untrusted relay event; saturating_add avoids a
                // panic (debug) or silent wraparound (release) on a crafted extreme
                // value. No observable difference for any realistic sat amount
                // (bounded by the 21M BTC supply, far below u64::MAX).
                total_volume_sats = total_volume_sats.saturating_add(amount);
                trade_amounts.push(amount);
            }
            qualifying_orders.push((event_ts, amount_sats));

            if let Some(fiat) = f_tag(&event) {
                if !fiat.is_empty() {
                    fiat_values.push(fiat.to_string());
                }
            }

            payment_method_mentions.extend(pm_tag_values(&event));

            if let Some(premium_str) = premium_tag(&event) {
                if let Ok(premium) = premium_str.parse::<i64>() {
                    premium_values.push(premium);
                }
            }
        }
    }

    OrderAggregate {
        total_order_count,
        first_order_ts,
        last_order_ts,
        s_tag_distribution,
        unique_order_count,
        recognized_status_order_count,
        successful_orders,
        total_volume_sats,
        trade_amounts,
        successful_trade_timestamps,
        fiat_values,
        payment_method_mentions,
        premium_values,
        qualifying_orders,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::test_support::make_event;

    /// The full procedure is dedup-then-filter, in that order: whichever event wins the
    /// `d`-tag tie-break (highest `created_at`, ties broken by greatest event id) is the
    /// one whose `s` value decides whether the order counts as successful — never the
    /// other candidate's status, and never both.
    #[test]
    fn aggregate_order_events_qualifying_selection_follows_the_tie_break_winner() {
        let success_candidate = make_event(
            38383,
            100,
            vec![("d", "order-1"), ("s", "success"), ("z", "order")],
        );
        let pending_candidate = make_event(
            38383,
            100,
            vec![("d", "order-1"), ("s", "pending"), ("z", "order")],
        );
        let winner_is_success = success_candidate.id > pending_candidate.id;

        let aggregate =
            aggregate_order_events(vec![success_candidate.clone(), pending_candidate.clone()]);

        assert_eq!(aggregate.unique_order_count, 1);
        assert_eq!(aggregate.successful_orders, usize::from(winner_is_success));
    }

    /// A malformed or missing `amt` value is unusable data on that order specifically,
    /// not evidence the trade did not happen — the order still counts toward
    /// `successful_orders`, but is safely excluded from `total_volume_sats` and
    /// `trade_amounts` rather than panicking on the unparseable value.
    #[test]
    fn aggregate_order_events_excludes_a_malformed_amt_from_volume_but_still_counts_the_trade() {
        let malformed_amt = make_event(
            38383,
            100,
            vec![("d", "order-1"), ("s", "success"), ("amt", "not-a-number")],
        );
        let missing_amt = make_event(38383, 200, vec![("d", "order-2"), ("s", "success")]);

        let aggregate = aggregate_order_events(vec![malformed_amt, missing_amt]);

        assert_eq!(aggregate.successful_orders, 2);
        assert_eq!(aggregate.total_volume_sats, 0);
        assert!(aggregate.trade_amounts.is_empty());
    }

    #[test]
    fn aggregate_order_events_ignores_an_order_event_missing_its_d_tag_without_panicking() {
        let no_d_tag = make_event(38383, 100, vec![("s", "success")]);

        let aggregate = aggregate_order_events(vec![no_d_tag]);

        assert_eq!(aggregate.unique_order_count, 0);
        assert_eq!(aggregate.successful_orders, 0);
    }

    /// Only successful orders carrying a non-empty `f` value contribute to
    /// `fiat_values`; an empty value is excluded entirely.
    #[test]
    fn aggregate_order_events_collects_fiat_values_from_successful_orders_excluding_empty() {
        let usd = make_event(
            38383,
            100,
            vec![("d", "order-1"), ("s", "success"), ("f", "USD")],
        );
        let empty_f = make_event(
            38383,
            200,
            vec![("d", "order-2"), ("s", "success"), ("f", "")],
        );
        let not_successful = make_event(
            38383,
            300,
            vec![("d", "order-3"), ("s", "pending"), ("f", "EUR")],
        );

        let aggregate = aggregate_order_events(vec![usd, empty_f, not_successful]);

        assert_eq!(aggregate.fiat_values, vec!["USD".to_string()]);
    }

    /// Builds an order event with a multi-value `pm` tag alongside ordinary
    /// single-value tags — matching the actual Nostr wire format, e.g. `["pm", "SEPA",
    /// "Cash"]`. The shared `test_support::make_event` helper only builds single-value
    /// `(name, value)` tags, which cannot represent this.
    fn make_order_event_with_pm_values(
        created_at: u64,
        d: &str,
        s: &str,
        pm_values: Vec<&str>,
    ) -> Event {
        let keys = Keys::generate();
        let mut tags = vec![
            Tag::parse(["d", d]).expect("valid tag"),
            Tag::parse(["s", s]).expect("valid tag"),
        ];
        let mut pm_tag = vec!["pm"];
        pm_tag.extend(pm_values);
        tags.push(Tag::parse(pm_tag).expect("valid tag"));

        EventBuilder::new(Kind::Custom(38383), "")
            .tags(tags)
            .custom_created_at(Timestamp::from(created_at))
            .sign_with_keys(&keys)
            .expect("event signs")
    }

    /// `pm` mentions are flattened across every successful order, one entry per tag
    /// value. Different orders are keyed by distinct `d` tags and processed from a
    /// `HashMap` with no guaranteed iteration order, so this compares the flattened
    /// mentions as a sorted multiset rather than asserting a specific cross-order
    /// sequence; the within-order value order is exercised separately below.
    #[test]
    fn aggregate_order_events_flattens_payment_method_mentions_across_successful_orders() {
        let multi = make_order_event_with_pm_values(
            100,
            "order-1",
            "success",
            vec!["SEPA", "Bank transfer"],
        );
        let single = make_order_event_with_pm_values(200, "order-2", "success", vec!["Cash"]);

        let aggregate = aggregate_order_events(vec![multi, single]);

        let mut mentions = aggregate.payment_method_mentions.clone();
        mentions.sort();
        assert_eq!(
            mentions,
            vec![
                "Bank transfer".to_string(),
                "Cash".to_string(),
                "SEPA".to_string()
            ]
        );
    }

    /// Within a single order, `pm` mentions preserve the tag's value order.
    #[test]
    fn aggregate_order_events_preserves_the_tag_value_order_within_one_order() {
        let multi = make_order_event_with_pm_values(
            100,
            "order-1",
            "success",
            vec!["SEPA", "Bank transfer"],
        );

        let aggregate = aggregate_order_events(vec![multi]);

        assert_eq!(
            aggregate.payment_method_mentions,
            vec!["SEPA".to_string(), "Bank transfer".to_string()]
        );
    }

    /// `qualifying_orders` pairs each qualifying successful order's timestamp with its
    /// parsed `amt` (or `None` when unparseable), one entry per successful order — the
    /// same set `successful_trade_timestamps` covers, not the smaller
    /// `trade_amounts`-only set.
    #[test]
    fn aggregate_order_events_pairs_each_successful_orders_timestamp_with_its_parsed_amount() {
        let with_amount = make_event(
            38383,
            100,
            vec![("d", "order-1"), ("s", "success"), ("amt", "500")],
        );
        let malformed_amount = make_event(
            38383,
            200,
            vec![("d", "order-2"), ("s", "success"), ("amt", "not-a-number")],
        );
        let missing_amount = make_event(38383, 300, vec![("d", "order-3"), ("s", "success")]);
        let not_successful = make_event(
            38383,
            400,
            vec![("d", "order-4"), ("s", "pending"), ("amt", "999")],
        );

        let aggregate = aggregate_order_events(vec![
            with_amount,
            malformed_amount,
            missing_amount,
            not_successful,
        ]);

        let mut qualifying_orders = aggregate.qualifying_orders.clone();
        qualifying_orders.sort_by_key(|(created_at, _)| *created_at);

        assert_eq!(
            qualifying_orders,
            vec![(100, Some(500)), (200, None), (300, None)]
        );
    }

    /// A `premium` tag that fails to parse as `i64`, or is missing, is excluded from
    /// `premium_values` without panicking.
    #[test]
    fn aggregate_order_events_excludes_a_malformed_premium_but_keeps_a_valid_one() {
        let valid = make_event(
            38383,
            100,
            vec![("d", "order-1"), ("s", "success"), ("premium", "-3")],
        );
        let malformed = make_event(
            38383,
            200,
            vec![
                ("d", "order-2"),
                ("s", "success"),
                ("premium", "not-a-number"),
            ],
        );
        let missing = make_event(38383, 300, vec![("d", "order-3"), ("s", "success")]);

        let aggregate = aggregate_order_events(vec![valid, malformed, missing]);

        assert_eq!(aggregate.premium_values, vec![-3i64]);
    }
}
