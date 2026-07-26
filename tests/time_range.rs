//! PR 10 (003 FR-004..FR-007): `run()`-level tests proving the activity grid's
//! `--since`/`--until`/`--view` scoping actually reaches `stats::grid::compute_activity_grid`
//! from `RunOptions`, while the general statistics section stays unaffected — behavior
//! only a `run()`-level test can prove, matching this test suite's existing convention
//! for options that thread straight through `RunOptions` with no dedicated
//! `cli::options` resolution step of their own. Uses `--format json` throughout for
//! reliable, machine-parseable assertions on the exact figures involved.

use mostro_score::fetch::client::{EventSource, RelayConnectionOutcome};
use mostro_score::report::content::SectionFilter;
use mostro_score::report::render::{Format, RunOptions};
use mostro_score::stats::grid::Granularity;
use nostr_sdk::prelude::*;

const SECONDS_PER_DAY: i64 = 86400;

struct FixtureEventSource {
    connection: RelayConnectionOutcome,
    events: Vec<Event>,
}

impl EventSource for FixtureEventSource {
    async fn connect(&self) -> Result<RelayConnectionOutcome> {
        Ok(self.connection.clone())
    }

    async fn fetch(&self, _public_key: PublicKey) -> Result<Vec<Event>> {
        Ok(self.events.clone())
    }
}

fn make_order_event(keys: &Keys, order_id: &str, created_at: i64, amount_sats: u64) -> Event {
    let tags: Vec<Tag> = vec![
        Tag::parse(["z", "order"]).expect("valid tag"),
        Tag::parse(["y", "mostro"]).expect("valid tag"),
        Tag::parse(["d", order_id]).expect("valid tag"),
        Tag::parse(["s", "success"]).expect("valid tag"),
        Tag::parse(["amt", &amount_sats.to_string()]).expect("valid tag"),
    ];
    EventBuilder::new(Kind::Custom(38383), "")
        .tags(tags)
        .custom_created_at(Timestamp::from(created_at as u64))
        .sign_with_keys(keys)
        .expect("event signs")
}

fn connected_outcome() -> RelayConnectionOutcome {
    RelayConnectionOutcome {
        connected_count: 1,
        ordered: vec![],
        connected_urls: vec!["wss://connected.example".to_string()],
        failed: vec![],
    }
}

fn base_options() -> RunOptions {
    RunOptions {
        format: Format::Json,
        quiet: false,
        color_override: None,
        since: None,
        until: None,
        view: None,
        sections: SectionFilter::all(),
    }
}

fn total_grid_trades(json: &serde_json::Value) -> u64 {
    json["activity"]["buckets"]
        .as_array()
        .expect("buckets array")
        .iter()
        .map(|bucket| bucket["successful_trades"].as_u64().unwrap_or(0))
        .sum()
}

/// T194/195 (003 FR-004): scoping `--since`/`--until` restricts the activity grid to
/// only the orders inside that range, while the general statistics section's
/// lifetime-anchored figures (Cumulative Performance's total successful trades here)
/// stay identical to an unscoped run over the same fixture.
#[tokio::test]
async fn scoped_activity_grid_excludes_orders_outside_the_range_while_stats_stay_full_history() {
    let node_keys = Keys::generate();
    let public_key = node_keys.public_key();
    let events = vec![
        make_order_event(&node_keys, "order-1", 10 * SECONDS_PER_DAY, 100),
        make_order_event(&node_keys, "order-2", 50 * SECONDS_PER_DAY, 500),
        make_order_event(&node_keys, "order-3", 200 * SECONDS_PER_DAY, 900),
    ];
    let event_source = FixtureEventSource {
        connection: connected_outcome(),
        events,
    };
    let now = chrono::Utc::now;
    let mut out: Vec<u8> = Vec::new();
    let mut err: Vec<u8> = Vec::new();

    let options = RunOptions {
        since: Some(40 * SECONDS_PER_DAY),
        until: Some(60 * SECONDS_PER_DAY),
        ..base_options()
    };

    mostro_score::run(public_key, event_source, &now, &mut out, &mut err, &options)
        .await
        .expect("run succeeds");

    let json: serde_json::Value = serde_json::from_slice(&out).expect("valid json");

    // The general statistics section is unaffected: all 3 successful orders count.
    assert_eq!(json["stats"]["cumulative"]["total_successful_trades"], 3);

    // The activity grid only counts the one order inside [40d, 60d] (order-2).
    assert_eq!(total_grid_trades(&json), 1);
}

/// T192/193 (003 FR-005a/FR-007): the wide-range warning fires once `--view`/`--since`/
/// `--until` combine to force a daily grid over a range wide enough to trigger it —
/// wired here into `run()`'s real resolved values, not merely unit-tested against
/// `stats::grid::wide_range_warning_message` in isolation.
#[tokio::test]
async fn wide_range_warning_fires_when_view_forces_a_daily_grid_over_a_wide_range() {
    let node_keys = Keys::generate();
    let public_key = node_keys.public_key();
    let events = vec![make_order_event(&node_keys, "order-1", 0, 100)];
    let event_source = FixtureEventSource {
        connection: connected_outcome(),
        events,
    };
    let now = chrono::Utc::now;
    let mut out: Vec<u8> = Vec::new();
    let mut err: Vec<u8> = Vec::new();

    let options = RunOptions {
        since: Some(0),
        until: Some(300 * SECONDS_PER_DAY),
        view: Some(Granularity::Daily),
        ..base_options()
    };

    mostro_score::run(public_key, event_source, &now, &mut out, &mut err, &options)
        .await
        .expect("run succeeds");

    let actual_err = String::from_utf8(err).unwrap();
    assert!(actual_err.contains("Warning: a daily activity grid"));
}

/// T192/193: within the daily boundary, no wide-range warning fires, matching the
/// existing threshold `stats::grid` already defines (no new one invented for PR 10).
#[tokio::test]
async fn no_wide_range_warning_within_the_daily_boundary() {
    let node_keys = Keys::generate();
    let public_key = node_keys.public_key();
    let events = vec![make_order_event(&node_keys, "order-1", 0, 100)];
    let event_source = FixtureEventSource {
        connection: connected_outcome(),
        events,
    };
    let now = chrono::Utc::now;
    let mut out: Vec<u8> = Vec::new();
    let mut err: Vec<u8> = Vec::new();

    let options = RunOptions {
        since: Some(0),
        until: Some(10 * SECONDS_PER_DAY),
        view: Some(Granularity::Daily),
        ..base_options()
    };

    mostro_score::run(public_key, event_source, &now, &mut out, &mut err, &options)
        .await
        .expect("run succeeds");

    let actual_err = String::from_utf8(err).unwrap();
    assert!(!actual_err.contains("Warning: a daily activity grid"));
}

/// 003 FR-004: `--until` alone defaults `--since` to the node's earliest available
/// history, resolved once `run()` has fetched the qualifying orders.
#[tokio::test]
async fn until_alone_defaults_since_to_the_earliest_qualifying_order() {
    let node_keys = Keys::generate();
    let public_key = node_keys.public_key();
    let events = vec![
        make_order_event(&node_keys, "order-1", 10 * SECONDS_PER_DAY, 100),
        make_order_event(&node_keys, "order-2", 50 * SECONDS_PER_DAY, 500),
    ];
    let event_source = FixtureEventSource {
        connection: connected_outcome(),
        events,
    };
    let now = chrono::Utc::now;
    let mut out: Vec<u8> = Vec::new();
    let mut err: Vec<u8> = Vec::new();

    let options = RunOptions {
        since: None,
        until: Some(20 * SECONDS_PER_DAY),
        ..base_options()
    };

    mostro_score::run(public_key, event_source, &now, &mut out, &mut err, &options)
        .await
        .expect("run succeeds");

    let json: serde_json::Value = serde_json::from_slice(&out).expect("valid json");

    // Both orders exist lifetime-wide, but only order-1 (day 10) falls inside
    // [earliest = day 10, day 20] — order-2 (day 50) is excluded from the grid.
    assert_eq!(total_grid_trades(&json), 1);
    assert_eq!(json["stats"]["cumulative"]["total_successful_trades"], 2);
}
