//! PR 9: binary-level tests for the CLI flags themselves (003 FR-001..FR-003,
//! FR-010..FR-013a) — precedence, well-formedness validation, `--quiet`, and
//! `--help`/`--version`. `src/cli/options.rs`'s own unit tests cover the pure
//! resolution/validation functions in isolation; these tests exercise the real `clap`
//! parsing and env-var behavior end to end, which only a subprocess can prove.

use mostro_score::fetch::client::{EventSource, RelayConnectFailure, RelayConnectionOutcome};
use mostro_score::report::render::{Format, RunOptions};
use nostr_sdk::prelude::*;

const TEST_PUBKEY_HEX: &str = "82fa8cb978b43c79b2156585bac2c011176a21d2aead6d9f7c575c005be88390";

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

/// T168/T169 (003 FR-001/FR-003): `MOSTRO_SCORE_PUBKEY` supplies `--pubkey` when the
/// flag itself is omitted.
#[test]
fn pubkey_falls_back_to_its_environment_variable_when_the_flag_is_omitted() {
    let output = assert_cmd::Command::cargo_bin("mostro-score")
        .unwrap()
        .env("MOSTRO_SCORE_PUBKEY", "not-a-valid-pubkey")
        .env_remove("MOSTRO_SCORE_RELAYS")
        .output()
        .expect("binary runs");

    // Reaching the invalid-pubkey exit code (5) instead of clap's missing-required-
    // argument exit code (2) proves the environment variable's value was used.
    assert_eq!(output.status.code(), Some(5));
}

/// T168/T169 (003 FR-003): an explicit `--pubkey` flag takes precedence over the
/// environment variable.
#[test]
fn an_explicit_pubkey_flag_takes_precedence_over_its_environment_variable() {
    let output = assert_cmd::Command::cargo_bin("mostro-score")
        .unwrap()
        .env("MOSTRO_SCORE_PUBKEY", "also-not-a-valid-pubkey")
        .args(["--pubkey", TEST_PUBKEY_HEX])
        .env_remove("MOSTRO_SCORE_RELAYS")
        .args(["--relays", "not-a-url"])
        .output()
        .expect("binary runs");

    // The flag's well-formed pubkey reaches relay validation (exit 2), never the
    // environment variable's malformed one (which would exit 5 first).
    assert_eq!(output.status.code(), Some(2));
}

/// 003 Edge Case, FR-013a: neither `--pubkey` nor `MOSTRO_SCORE_PUBKEY` present is
/// clap's own native missing-required-argument usage error, exit code `2`.
#[test]
fn missing_pubkey_and_its_environment_variable_is_a_usage_error() {
    let output = assert_cmd::Command::cargo_bin("mostro-score")
        .unwrap()
        .env_remove("MOSTRO_SCORE_PUBKEY")
        .env_remove("MOSTRO_SCORE_RELAYS")
        .output()
        .expect("binary runs");

    assert_eq!(output.status.code(), Some(2));
}

/// T168/T169 (003 FR-002/FR-003): `MOSTRO_SCORE_RELAYS` supplies `--relays` when the
/// flag itself is omitted.
#[test]
fn relays_falls_back_to_its_environment_variable_when_the_flag_is_omitted() {
    let output = assert_cmd::Command::cargo_bin("mostro-score")
        .unwrap()
        .args(["--pubkey", TEST_PUBKEY_HEX])
        .env("MOSTRO_SCORE_RELAYS", "not-a-url")
        .output()
        .expect("binary runs");

    // The malformed relay from the environment variable is validated the same way as
    // one from the flag, reaching exit code 2.
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not-a-url"));
}

/// T168/T169 (003 FR-003): an explicit `--relays` flag takes precedence over the
/// environment variable.
#[test]
fn an_explicit_relays_flag_takes_precedence_over_its_environment_variable() {
    let output = assert_cmd::Command::cargo_bin("mostro-score")
        .unwrap()
        .args(["--pubkey", TEST_PUBKEY_HEX, "--relays", "not-a-flag-url"])
        .env("MOSTRO_SCORE_RELAYS", "not-an-env-url")
        .output()
        .expect("binary runs");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not-a-flag-url"));
    assert!(!stderr.contains("not-an-env-url"));
}

/// T170/T171 (003 FR-002/FR-003, FR-013a Edge Case): a malformed `--relays` entry is
/// rejected as a usage error (exit `2`) before any connection attempt, naming the exact
/// malformed string. Superseded by PR 9: previously this reached `RelayEventSource::
/// connect()` and folded into `AppError::RelaysUnreachable` (exit `3`), since every
/// configured relay had failed to register; that generic-outage classification is no
/// longer reachable from the CLI for a syntactically malformed relay, since it is now
/// caught earlier with an actionable message.
#[test]
fn a_malformed_relays_flag_value_is_a_usage_error_naming_the_relay() {
    let output = assert_cmd::Command::cargo_bin("mostro-score")
        .unwrap()
        .args(["--pubkey", TEST_PUBKEY_HEX, "--relays", "not-a-url"])
        .output()
        .expect("binary runs");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not-a-url"));
}

/// 003 FR-011 Edge Case: `--color` and `--no-color` together is rejected as a
/// contradictory usage error, exit code `2`.
#[test]
fn color_and_no_color_together_is_a_usage_error() {
    let output = assert_cmd::Command::cargo_bin("mostro-score")
        .unwrap()
        .args(["--pubkey", TEST_PUBKEY_HEX, "--color", "--no-color"])
        .output()
        .expect("binary runs");

    assert_eq!(output.status.code(), Some(2));
}

/// 003 FR-010: `--format` accepts exactly the 3 documented values.
#[test]
fn an_unrecognized_format_value_is_claps_own_usage_error() {
    let output = assert_cmd::Command::cargo_bin("mostro-score")
        .unwrap()
        .args(["--pubkey", TEST_PUBKEY_HEX, "--format", "xml"])
        .output()
        .expect("binary runs");

    assert_eq!(output.status.code(), Some(2));
}

/// T176: `--help` prints usage information and exits `0`.
#[test]
fn help_flag_exits_0() {
    let output = assert_cmd::Command::cargo_bin("mostro-score")
        .unwrap()
        .arg("--help")
        .output()
        .expect("binary runs");

    assert_eq!(output.status.code(), Some(0));
    assert!(!output.stdout.is_empty());
}

/// T176: `--version` prints the tool's version and exits `0`.
#[test]
fn version_flag_exits_0() {
    let output = assert_cmd::Command::cargo_bin("mostro-score")
        .unwrap()
        .arg("--version")
        .output()
        .expect("binary runs");

    assert_eq!(output.status.code(), Some(0));
    assert!(!output.stdout.is_empty());
}

/// T174/T175 (003 FR-012): `--quiet` suppresses the two transient status lines while
/// every other diagnostic (a relay-failure warning) and all report content stay
/// unaffected. Exercised directly against `mostro_score::run()` — this behavior has no
/// dedicated `cli::options` resolution step of its own (`--quiet` threads straight
/// through into `RunOptions.quiet`), so a `run()`-level test proves the actual
/// suppression, which a pure-function unit test of the (trivial) flag-to-field mapping
/// alone could not.
#[tokio::test]
async fn quiet_suppresses_transient_status_lines_but_not_other_diagnostics_or_content() {
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
    let options = RunOptions {
        format: Format::Console,
        quiet: true,
        color_override: Some(false),
    };

    mostro_score::run(public_key, event_source, &now, &mut out, &mut err, &options)
        .await
        .expect("run succeeds");

    let actual_out = String::from_utf8(out).unwrap();
    let actual_err = String::from_utf8(err).unwrap();

    assert!(!actual_err.contains("Connected to relays"));
    assert!(!actual_err.contains("Fetched"));
    assert!(actual_err.contains("wss://unreachable.example"));
    assert!(actual_out.contains("=== NODE IDENTITY ==="));
    assert!(actual_out.contains("=== RELAY FETCH SUMMARY ==="));
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
