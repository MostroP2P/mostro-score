//! PR 2: small, synthetic-data tests for the error taxonomy, exit codes, and
//! diagnostic-routing behavior this PR adds. Deliberately not golden/characterization
//! tests against real relay captures (see `plan.md`'s PR 1 Amendment): PR 2 onward changes
//! behavior on purpose, so there is nothing to prove "stayed the same" — ordinary
//! Red-Green-Refactor tests against a handful of hand-built events are what apply here.

use mostro_score::error::exit_code::exit_code_for;
use mostro_score::error::AppError;
use mostro_score::fetch::client::{EventSource, RelayConnectFailure, RelayConnectionOutcome};
use nostr_sdk::prelude::*;

const TEST_PUBKEY_HEX: &str = "82fa8cb978b43c79b2156585bac2c011176a21d2aead6d9f7c575c005be88390";

fn make_event(kind: u16, created_at: u64, tags: Vec<(&str, &str)>) -> Event {
    let keys = Keys::generate();
    let parsed_tags: Vec<Tag> = tags
        .into_iter()
        .map(|(name, value)| Tag::parse([name, value]).expect("valid tag"))
        .collect();
    EventBuilder::new(Kind::Custom(kind), "")
        .tags(parsed_tags)
        .custom_created_at(Timestamp::from(created_at))
        .sign_with_keys(&keys)
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
#[test]
fn invalid_pubkey_exits_5() {
    let output = assert_cmd::Command::cargo_bin("mostro-score")
        .unwrap()
        .args(["--pubkey", "not-a-valid-pubkey"])
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

    let result = mostro_score::run(public_key, event_source, &now, &mut out, &mut err).await;

    let actual_err = result.expect_err("zero connected relays must be fatal");
    assert!(
        matches!(actual_err, AppError::RelaysUnreachable),
        "must be the RelaysUnreachable variant specifically, not any error: {actual_err:?}"
    );
    assert_eq!(exit_code_for(&actual_err), 3);
}

/// Regression: `RelayEventSource::connect()` must classify a relay URL that fails to
/// register (e.g. malformed) the same way as one that registers but fails to connect —
/// not abort before a `RelayConnectionOutcome` even exists. Otherwise "every relay
/// unreachable" collapses to exit `1` (`AppError::Other`) instead of `3` whenever the
/// cause is a bad URL rather than a network failure.
#[test]
fn all_relay_urls_malformed_is_relays_unreachable_not_general_error() {
    let output = assert_cmd::Command::cargo_bin("mostro-score")
        .unwrap()
        .args(["--pubkey", TEST_PUBKEY_HEX, "--relays", "not-a-url"])
        .output()
        .expect("binary runs");

    assert_eq!(output.status.code(), Some(3));
    assert_eq!(
        String::from_utf8_lossy(&output.stderr).trim(),
        "Error: None of the configured relays could be reached."
    );
}

/// T066/T067: one relay failing among several that connected is a warning on `err`, not a
/// failure — the report still succeeds (Technical Context's graceful-degradation rule).
#[tokio::test]
async fn one_failed_relay_among_several_is_a_warning_not_a_failure() {
    let public_key = PublicKey::parse(TEST_PUBKEY_HEX).unwrap();
    let event_source = FixtureEventSource {
        connection: RelayConnectionOutcome {
            connected_count: 1,
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

    let result = mostro_score::run(public_key, event_source, &now, &mut out, &mut err).await;

    assert!(
        result.is_ok(),
        "one failed relay among several that connected must not fail run()"
    );
    let actual_err = String::from_utf8(err).unwrap();
    assert!(actual_err.contains("wss://unreachable.example"));
    assert!(actual_err.contains("connection refused"));
}

/// T070-T073: diagnostic/transient-status content (the sample-event dump, the debug
/// block, the connecting/fetched-count lines) writes to `err`, never `out` — `out` carries
/// only report content.
#[tokio::test]
async fn diagnostics_route_to_err_not_out() {
    let public_key = PublicKey::parse(TEST_PUBKEY_HEX).unwrap();
    let events = vec![make_event(
        8383,
        1_700_000_000,
        vec![("z", "dev-fee-payment"), ("y", "mostro")],
    )];
    let event_source = FixtureEventSource {
        connection: RelayConnectionOutcome {
            connected_count: 1,
            failed: vec![],
        },
        events,
    };
    let now = chrono::Utc::now;
    let mut out: Vec<u8> = Vec::new();
    let mut err: Vec<u8> = Vec::new();

    mostro_score::run(public_key, event_source, &now, &mut out, &mut err)
        .await
        .expect("run succeeds");

    let actual_out = String::from_utf8(out).unwrap();
    let actual_err = String::from_utf8(err).unwrap();

    assert!(!actual_out.contains("SAMPLE EVENTS"));
    assert!(!actual_out.contains("DEBUG INFORMATION"));
    assert!(!actual_out.contains("Connected to relays"));
    assert!(!actual_out.contains("Fetched"));
    assert!(actual_err.contains("SAMPLE EVENTS"));
    assert!(actual_err.contains("Connected to relays"));
    assert!(actual_err.contains("Fetched"));
    assert!(actual_err.contains("DEBUG INFORMATION"));
}

/// T070/T071: the no-dev-fee-events branch is a diagnostic warning about data
/// availability (`err`), not report content — distinct from `diagnostics_route_to_err_not_out`
/// above, which only exercises the success (dev-fee-events-present) branch of the same
/// function.
#[tokio::test]
async fn no_dev_fee_events_warns_on_err_and_falls_back_to_order_timestamps() {
    let public_key = PublicKey::parse(TEST_PUBKEY_HEX).unwrap();
    let events = vec![make_event(
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
            failed: vec![],
        },
        events,
    };
    let now = chrono::Utc::now;
    let mut out: Vec<u8> = Vec::new();
    let mut err: Vec<u8> = Vec::new();

    mostro_score::run(public_key, event_source, &now, &mut out, &mut err)
        .await
        .expect("run succeeds");

    let actual_out = String::from_utf8(out).unwrap();
    let actual_err = String::from_utf8(err).unwrap();

    assert!(!actual_out.contains("No dev fee events found"));
    assert!(actual_err.contains("No dev fee events found (z=dev-fee-payment, y=mostro)"));
    assert!(actual_err.contains("Falling back to order timestamps"));
}

/// T074/T075: the `s`-tag distribution block prints sorted by key, deterministic across
/// runs, instead of following `HashMap` iteration order.
#[tokio::test]
async fn s_tag_distribution_prints_sorted_by_key() {
    let public_key = PublicKey::parse(TEST_PUBKEY_HEX).unwrap();
    let events = vec![
        make_event(
            38383,
            1_700_000_001,
            vec![
                ("z", "order"),
                ("y", "mostro"),
                ("d", "order-1"),
                ("s", "success"),
            ],
        ),
        make_event(
            38383,
            1_700_000_002,
            vec![
                ("z", "order"),
                ("y", "mostro"),
                ("d", "order-2"),
                ("s", "canceled"),
            ],
        ),
        make_event(
            38383,
            1_700_000_003,
            vec![
                ("z", "order"),
                ("y", "mostro"),
                ("d", "order-3"),
                ("s", "pending"),
            ],
        ),
    ];
    let event_source = FixtureEventSource {
        connection: RelayConnectionOutcome {
            connected_count: 1,
            failed: vec![],
        },
        events,
    };
    let now = chrono::Utc::now;
    let mut out: Vec<u8> = Vec::new();
    let mut err: Vec<u8> = Vec::new();

    mostro_score::run(public_key, event_source, &now, &mut out, &mut err)
        .await
        .expect("run succeeds");

    let actual_err = String::from_utf8(err).unwrap();
    let canceled_pos = actual_err.find("s='canceled'").expect("canceled present");
    let pending_pos = actual_err.find("s='pending'").expect("pending present");
    let success_pos = actual_err.find("s='success'").expect("success present");

    assert!(
        canceled_pos < pending_pos && pending_pos < success_pos,
        "status lines must be alphabetically sorted: canceled, pending, success"
    );
}
