//! PR 2: small, synthetic-data tests for the error taxonomy, exit codes, and
//! diagnostic-routing behavior this PR adds. Deliberately not golden/characterization
//! tests against real relay captures (see `plan.md`'s PR 1 Amendment): PR 2 onward changes
//! behavior on purpose, so there is nothing to prove "stayed the same" — ordinary
//! Red-Green-Refactor tests against a handful of hand-built events are what apply here.

use mostro_score::error::exit_code::exit_code_for;
use mostro_score::error::AppError;
use mostro_score::fetch::client::{EventSource, RelayConnectFailure, RelayConnectionOutcome};
use mostro_score::report::content::SectionFilter;
use mostro_score::report::render::{Format, RunOptions};
use nostr_sdk::prelude::*;

const TEST_PUBKEY_HEX: &str = "82fa8cb978b43c79b2156585bac2c011176a21d2aead6d9f7c575c005be88390";

/// The default options every pre-PR-9 test in this file implicitly assumed: console
/// format, no quiet suppression, color forced off (these tests assert against
/// console-formatted output and were written before format/quiet/color existed as
/// flags).
fn default_test_options() -> RunOptions {
    RunOptions {
        format: Format::Console,
        quiet: false,
        color_override: Some(false),
        since: None,
        until: None,
        view: None,
        sections: SectionFilter::all(),
    }
}

fn make_event_with_keys(keys: &Keys, kind: u16, created_at: u64, tags: Vec<(&str, &str)>) -> Event {
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

/// T064/T065: a syntactically invalid `--pubkey` exits `5` (002 FR-019), never reaching a
/// relay. No network access — the pubkey parse failure short-circuits `main()` before
/// `RelayEventSource` is even constructed.
///
/// PR 12: since every invocation now loads a real persisted configuration file (003
/// FR-015/FR-016), `--config-dir` points at a directory that does not exist so this
/// test's outcome never silently depends on whatever configuration file happens to
/// exist on the machine or CI runner actually running the suite (FR-015 treats a
/// missing file the same as a genuinely absent one, silently). `--config-dir`, not
/// `XDG_CONFIG_HOME`, since this codebase's own path resolution only consults that
/// environment variable on Linux -- `--config-dir` is checked first on every platform.
#[test]
fn invalid_pubkey_exits_5() {
    let output = assert_cmd::Command::cargo_bin("mostro-score")
        .unwrap()
        .args([
            "--pubkey",
            "not-a-valid-pubkey",
            "--config-dir",
            "/nonexistent-mostro-score-test-isolation-dir",
        ])
        .output()
        .expect("binary runs");

    assert_eq!(output.status.code(), Some(5));
    assert_eq!(
        String::from_utf8_lossy(&output.stderr).trim(),
        "Error: Invalid public key format."
    );
    assert!(output.stdout.is_empty());
}

/// T068/T069: every configured relay failing to connect exits `3` (002 FR-019), distinct
/// from a partial failure (below), which is a warning, not a fatal error.
#[tokio::test]
async fn all_relays_unreachable_is_fatal() {
    let public_key = PublicKey::parse(TEST_PUBKEY_HEX).unwrap();
    let event_source = FixtureEventSource {
        connection: RelayConnectionOutcome {
            connected_count: 0,
            ordered: vec![],
            connected_urls: vec![],
            failed: vec![RelayConnectFailure {
                url: "wss://unreachable.example".to_string(),
                error: "connection refused".to_string(),
            }],
        },
        events: vec![],
    };
    let now = chrono::Utc::now;
    let mut out: Vec<u8> = Vec::new();
    let mut err: Vec<u8> = Vec::new();

    let options = default_test_options();
    let result =
        mostro_score::run(public_key, event_source, &now, &mut out, &mut err, &options).await;

    let actual_err = result.expect_err("zero connected relays must be fatal");
    assert!(
        matches!(actual_err, AppError::RelaysUnreachable(_)),
        "must be the RelaysUnreachable variant specifically, not any error: {actual_err:?}"
    );
    assert_eq!(exit_code_for(&actual_err), 3);
}

/// T066/T067: one relay failing among several that connected is a warning on `err`, not a
/// failure — the report still succeeds (Technical Context's graceful-degradation rule).
#[tokio::test]
async fn one_failed_relay_among_several_is_a_warning_not_a_failure() {
    let node_keys = Keys::generate();
    let public_key = node_keys.public_key();
    // At least one usable event is required so this test exercises relay-warning
    // routing independently from the zero-usable-events gate (T095/T096).
    let events = vec![make_event_with_keys(
        &node_keys,
        8383,
        1_700_000_000,
        vec![("z", "dev-fee-payment"), ("y", "mostro")],
    )];
    let event_source = FixtureEventSource {
        connection: RelayConnectionOutcome {
            connected_count: 1,
            ordered: vec![],
            connected_urls: vec!["wss://connected.example".to_string()],
            failed: vec![RelayConnectFailure {
                url: "wss://unreachable.example".to_string(),
                error: "connection refused".to_string(),
            }],
        },
        events,
    };
    let now = chrono::Utc::now;
    let mut out: Vec<u8> = Vec::new();
    let mut err: Vec<u8> = Vec::new();

    let options = default_test_options();
    let result =
        mostro_score::run(public_key, event_source, &now, &mut out, &mut err, &options).await;

    assert!(
        result.is_ok(),
        "one failed relay among several that connected must not fail run()"
    );
    let actual_err = String::from_utf8(err).unwrap();
    assert!(actual_err.contains("wss://unreachable.example"));
    assert!(actual_err.contains("connection refused"));
}

/// T070-T073 (superseded by PR 7d): the transient connecting/fetched-count status lines
/// write to `err`, never `out` — `out` carries only the rendered `Report`. PR 1's
/// "SAMPLE EVENTS"/"DEBUG INFORMATION" debug dumps are gone entirely (PR 7d): they were
/// PR1-era debugging aids, superseded by the report's own fetch section (002 FR-003).
#[tokio::test]
async fn diagnostics_route_to_err_not_out() {
    let node_keys = Keys::generate();
    let public_key = node_keys.public_key();
    let events = vec![make_event_with_keys(
        &node_keys,
        8383,
        1_700_000_000,
        vec![("z", "dev-fee-payment"), ("y", "mostro")],
    )];
    let event_source = FixtureEventSource {
        connection: RelayConnectionOutcome {
            connected_count: 1,
            ordered: vec![],
            connected_urls: vec!["wss://connected.example".to_string()],
            failed: vec![],
        },
        events,
    };
    let now = chrono::Utc::now;
    let mut out: Vec<u8> = Vec::new();
    let mut err: Vec<u8> = Vec::new();

    let options = default_test_options();
    mostro_score::run(public_key, event_source, &now, &mut out, &mut err, &options)
        .await
        .expect("run succeeds");

    let actual_out = String::from_utf8(out).unwrap();
    let actual_err = String::from_utf8(err).unwrap();

    assert!(!actual_out.contains("Connected to relays"));
    assert!(!actual_out.contains("Fetched"));
    assert!(actual_err.contains("Connected to relays"));
    assert!(actual_err.contains("Fetched"));
    assert!(actual_out.contains("=== NODE IDENTITY ==="));
    assert!(actual_out.contains("=== RELAY FETCH SUMMARY ==="));
}

/// T070/T071 (superseded by PR 7d): the no-dev-fee-anchor fallback is a diagnostic
/// warning about data availability (`err`), not report content — distinct from
/// `diagnostics_route_to_err_not_out` above, which only exercises the
/// dev-fee-event-present branch.
#[tokio::test]
async fn no_dev_fee_events_warns_on_err_and_falls_back_to_order_timestamps() {
    let node_keys = Keys::generate();
    let public_key = node_keys.public_key();
    let events = vec![make_event_with_keys(
        &node_keys,
        38383,
        1_700_000_000,
        vec![
            ("z", "order"),
            ("y", "mostro"),
            ("d", "order-1"),
            ("s", "success"),
        ],
    )];
    let event_source = FixtureEventSource {
        connection: RelayConnectionOutcome {
            connected_count: 1,
            ordered: vec![],
            connected_urls: vec!["wss://connected.example".to_string()],
            failed: vec![],
        },
        events,
    };
    let now = chrono::Utc::now;
    let mut out: Vec<u8> = Vec::new();
    let mut err: Vec<u8> = Vec::new();

    let options = default_test_options();
    mostro_score::run(public_key, event_source, &now, &mut out, &mut err, &options)
        .await
        .expect("run succeeds");

    let actual_out = String::from_utf8(out).unwrap();
    let actual_err = String::from_utf8(err).unwrap();

    assert!(!actual_out.contains("no dev-fee anchor found"));
    assert!(actual_err.contains(
        "Warning: no dev-fee anchor found; falling back to order timestamps for days_active."
    ));
}

/// T095/T096: zero usable events across all four scoped kinds (dev-fee, order, dispute,
/// instance-status) is fatal (002 FR-019 exit code `4`) — nothing about this node can be
/// reported.
#[tokio::test]
async fn zero_usable_events_across_all_four_kinds_exits_4() {
    let node_keys = Keys::generate();
    let public_key = node_keys.public_key();
    let event_source = FixtureEventSource {
        connection: RelayConnectionOutcome {
            connected_count: 1,
            ordered: vec![],
            connected_urls: vec!["wss://connected.example".to_string()],
            failed: vec![],
        },
        events: vec![],
    };
    let now = chrono::Utc::now;
    let mut out: Vec<u8> = Vec::new();
    let mut err: Vec<u8> = Vec::new();

    let options = default_test_options();
    let result =
        mostro_score::run(public_key, event_source, &now, &mut out, &mut err, &options).await;

    let actual_err = result.expect_err("zero usable events across all four kinds must be fatal");
    assert!(
        matches!(actual_err, AppError::NoUsableEvents),
        "must be the NoUsableEvents variant specifically, not any error: {actual_err:?}"
    );
    assert_eq!(exit_code_for(&actual_err), 4);
}

/// T097: a node whose only usable data is a dispute or instance-status event (no
/// dev-fee event, no orders) must still be fetched successfully — it must NOT trigger
/// `AppError::NoUsableEvents`.
#[tokio::test]
async fn a_node_with_only_a_dispute_event_does_not_trigger_no_usable_events() {
    let node_keys = Keys::generate();
    let public_key = node_keys.public_key();
    let dispute_event = make_event_with_keys(
        &node_keys,
        38386,
        1_700_000_000,
        vec![
            ("z", "dispute"),
            ("y", "mostro"),
            ("d", "dispute-1"),
            ("s", "initiated"),
        ],
    );
    let event_source = FixtureEventSource {
        connection: RelayConnectionOutcome {
            connected_count: 1,
            ordered: vec![],
            connected_urls: vec!["wss://connected.example".to_string()],
            failed: vec![],
        },
        events: vec![dispute_event],
    };
    let now = chrono::Utc::now;
    let mut out: Vec<u8> = Vec::new();
    let mut err: Vec<u8> = Vec::new();

    let options = default_test_options();
    mostro_score::run(public_key, event_source, &now, &mut out, &mut err, &options)
        .await
        .expect("a node with a dispute event but no dev-fee/order events must not exit 4");

    // The report must render in full with not-applicable markers, not be silently
    // truncated: the removed early-return branch also returned Ok, so asserting only
    // success cannot prove the rest of the report actually rendered.
    let actual_out = String::from_utf8(out).unwrap();
    assert!(actual_out.contains("-- Longevity"));
    assert!(actual_out.contains("-- Liveness"));
    assert!(actual_out.contains("N/A"));
}

/// T097: same as above, using an instance-status event as the node's only usable data.
#[tokio::test]
async fn a_node_with_only_an_instance_status_event_does_not_trigger_no_usable_events() {
    let node_keys = Keys::generate();
    let public_key = node_keys.public_key();
    let instance_status_event = make_event_with_keys(
        &node_keys,
        38385,
        1_700_000_000,
        vec![
            ("z", "info"),
            ("y", "mostro"),
            ("d", &public_key.to_hex()),
            ("bond_enabled", "true"),
        ],
    );
    let event_source = FixtureEventSource {
        connection: RelayConnectionOutcome {
            connected_count: 1,
            ordered: vec![],
            connected_urls: vec!["wss://connected.example".to_string()],
            failed: vec![],
        },
        events: vec![instance_status_event],
    };
    let now = chrono::Utc::now;
    let mut out: Vec<u8> = Vec::new();
    let mut err: Vec<u8> = Vec::new();

    let options = default_test_options();
    mostro_score::run(public_key, event_source, &now, &mut out, &mut err, &options)
        .await
        .expect("a node with an instance-status event but no dev-fee/order events must not exit 4");

    let actual_out = String::from_utf8(out).unwrap();
    assert!(actual_out.contains("-- Longevity"));
    assert!(actual_out.contains("-- Liveness"));
    assert!(actual_out.contains("N/A"));
}

/// FR-014 regression: an event whose `created_at` is later than report-generation time
/// must be excluded from the pipeline entirely, not merely deprioritized. A node whose
/// only event is dated far in the future must therefore still exit `4` — proving
/// `exclude_future_events` is actually wired into `run()`, not just unit-tested in
/// isolation in `models::core`.
#[tokio::test]
async fn a_future_dated_event_is_excluded_and_does_not_count_as_usable() {
    let node_keys = Keys::generate();
    let public_key = node_keys.public_key();
    // Year 2100: always "future" relative to the real clock `now` calls, for the
    // foreseeable lifetime of this test.
    let far_future_dispute = make_event_with_keys(
        &node_keys,
        38386,
        4_102_444_800,
        vec![
            ("z", "dispute"),
            ("y", "mostro"),
            ("d", "dispute-1"),
            ("s", "initiated"),
        ],
    );
    let event_source = FixtureEventSource {
        connection: RelayConnectionOutcome {
            connected_count: 1,
            ordered: vec![],
            connected_urls: vec!["wss://connected.example".to_string()],
            failed: vec![],
        },
        events: vec![far_future_dispute],
    };
    let now = chrono::Utc::now;
    let mut out: Vec<u8> = Vec::new();
    let mut err: Vec<u8> = Vec::new();

    let options = default_test_options();
    let result =
        mostro_score::run(public_key, event_source, &now, &mut out, &mut err, &options).await;

    let actual_err =
        result.expect_err("a future-dated event must be excluded, leaving nothing usable");
    assert!(
        matches!(actual_err, AppError::NoUsableEvents),
        "must be NoUsableEvents, not any error: {actual_err:?}"
    );
}

/// T123/T124: the legacy trust-score line is removed entirely (no replacement, per the
/// plan's Complexity Tracking decision). Asserted through `run()`'s real console-render
/// call path, not merely that `NodeMetrics` carries no score field: a renderer could
/// still print stale text even after the underlying model dropped it.
#[tokio::test]
async fn console_report_never_prints_a_trust_score_line() {
    let node_keys = Keys::generate();
    let public_key = node_keys.public_key();
    let events = vec![make_event_with_keys(
        &node_keys,
        8383,
        1_700_000_000,
        vec![("z", "dev-fee-payment"), ("y", "mostro")],
    )];
    let event_source = FixtureEventSource {
        connection: RelayConnectionOutcome {
            connected_count: 1,
            ordered: vec![],
            connected_urls: vec!["wss://connected.example".to_string()],
            failed: vec![],
        },
        events,
    };
    let now = chrono::Utc::now;
    let mut out: Vec<u8> = Vec::new();
    let mut err: Vec<u8> = Vec::new();

    let options = default_test_options();
    mostro_score::run(public_key, event_source, &now, &mut out, &mut err, &options)
        .await
        .expect("run succeeds");

    let actual_out = String::from_utf8(out).unwrap();
    assert!(!actual_out.contains("TRUST SCORE"));
}

/// 002 FR-003 regression: the same dev-fee event independently delivered by more than
/// one relay must be counted once in the displayed "Found N dev fee events" line, not
/// once per relay that happened to return it.
#[tokio::test]
async fn duplicate_dev_fee_event_from_multiple_relays_is_counted_once_in_the_report() {
    let node_keys = Keys::generate();
    let public_key = node_keys.public_key();
    let dev_fee_event = make_event_with_keys(
        &node_keys,
        8383,
        1_700_000_000,
        vec![("z", "dev-fee-payment"), ("y", "mostro")],
    );
    let event_source = FixtureEventSource {
        connection: RelayConnectionOutcome {
            connected_count: 1,
            ordered: vec![],
            connected_urls: vec!["wss://connected.example".to_string()],
            failed: vec![],
        },
        // The same event, as if two relays independently returned it.
        events: vec![dev_fee_event.clone(), dev_fee_event],
    };
    let now = chrono::Utc::now;
    let mut out: Vec<u8> = Vec::new();
    let mut err: Vec<u8> = Vec::new();

    let options = default_test_options();
    mostro_score::run(public_key, event_source, &now, &mut out, &mut err, &options)
        .await
        .expect("run succeeds");

    let actual_out = String::from_utf8(out).unwrap();
    assert!(
        actual_out.contains("Dev-fee events (backs Longevity):        1"),
        "duplicate delivery of the same event must not inflate the displayed count: {actual_out}"
    );
}
