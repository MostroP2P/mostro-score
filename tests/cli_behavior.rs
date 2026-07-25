//! PR 1 Step D2: assertions for Step -1's remaining golden-baseline scenarios (2-8;
//! scenario 1 is proved by `tests/metrics_end_to_end.rs`). Scenarios 2-4 exercise the
//! real, compiled binary via `assert_cmd`, since only a real process observes `main()`'s
//! own wiring (pubkey parsing, the real `EventSource`, and process exit status) — an
//! in-process `run()` call cannot verify any of that. Scenarios 5-8 depend on real
//! historical event data rather than deterministic connection-layer behavior, so they
//! stay in-process against `run()` with a fixture `EventSource` and a frozen clock,
//! matching `tests/metrics_end_to_end.rs`'s discipline. No live relay access anywhere in
//! this file.

use assert_cmd::Command;
use chrono::{DateTime, Utc};
use mostro_score::fetch::client::EventSource;
use nostr_sdk::prelude::*;
use std::fs;

const SCENARIO1_PUBKEY_HEX: &str =
    "82fa8cb978b43c79b2156585bac2c011176a21d2aead6d9f7c575c005be88390";

/// Fixture `EventSource`: replays a canned event set instead of querying relays.
struct FixtureEventSource {
    events: Vec<Event>,
}

impl EventSource for FixtureEventSource {
    async fn connect(&self) -> Result<()> {
        Ok(())
    }

    async fn fetch(&self, _public_key: PublicKey) -> Result<Vec<Event>> {
        Ok(self.events.clone())
    }
}

fn load_fixture_events(path: &str) -> Vec<Event> {
    let raw = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    raw.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| Event::from_json(l).unwrap_or_else(|e| panic!("parse event line: {e}")))
        .collect()
}

fn read_fixture(scenario: &str, stream: &str) -> String {
    fs::read_to_string(format!("tests/fixtures/{scenario}_{stream}.txt"))
        .unwrap_or_else(|e| panic!("read {scenario}_{stream}.txt: {e}"))
}

fn read_exit_code(scenario: &str) -> i32 {
    read_fixture(scenario, "exit")
        .trim()
        .parse()
        .unwrap_or_else(|e| panic!("parse {scenario}_exit.txt: {e}"))
}

fn read_now_pre(scenario: &str) -> i64 {
    fs::read_to_string(format!("tests/fixtures/{scenario}_now_pre.txt"))
        .unwrap_or_else(|e| panic!("read {scenario}_now_pre.txt: {e}"))
        .trim()
        .parse()
        .unwrap_or_else(|e| panic!("parse {scenario}_now_pre.txt: {e}"))
}

/// Normalizes the "Status distribution for order events (s tag):" block: its lines come
/// from `HashMap` iteration, so their order is nondeterministic across process runs,
/// independent of any code change. Both sides of a comparison must have that block's
/// lines sorted before the rest of the text is compared byte-for-byte. Mirrors
/// `tests/metrics_end_to_end.rs`'s helper of the same name.
fn normalize_s_tag_distribution(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut normalized: Vec<String> = Vec::with_capacity(lines.len());
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        normalized.push(line.to_string());
        if line == "Status distribution for order events (s tag):" {
            i += 1;
            let mut block: Vec<&str> = Vec::new();
            while i < lines.len() && lines[i].trim_start().starts_with("s=") {
                block.push(lines[i]);
                i += 1;
            }
            block.sort_unstable();
            normalized.extend(block.into_iter().map(|s| s.to_string()));
            continue;
        }
        i += 1;
    }
    normalized.join("\n")
}

/// Runs `run()` in-process against a fixture event set and a frozen clock, matching
/// `tests/metrics_end_to_end.rs`'s discipline: no live relay access.
async fn run_against_fixture(
    public_key_hex: &str,
    events: Vec<Event>,
    now_pre: i64,
) -> (String, String) {
    let public_key = PublicKey::parse(public_key_hex).expect("valid golden-scenario pubkey");
    let event_source = FixtureEventSource { events };
    let frozen_now = move || DateTime::<Utc>::from_timestamp(now_pre, 0).unwrap();

    let mut out: Vec<u8> = Vec::new();
    let mut err: Vec<u8> = Vec::new();

    mostro_score::run(public_key, event_source, &frozen_now, &mut out, &mut err)
        .await
        .expect("run() succeeds against the fixture event source");

    (
        String::from_utf8(out).expect("stdout is valid utf-8"),
        String::from_utf8(err).expect("stderr is valid utf-8"),
    )
}

// T044-T045: scenario 2 — malformed `--pubkey`, binary-level. Pubkey parsing stays in
// `main()` itself, out of `run()` entirely, so this must run the real compiled binary.
#[test]
fn scenario_2_malformed_pubkey_matches_golden_capture() {
    let expected_stdout = read_fixture("scenario2", "stdout");
    let expected_stderr = read_fixture("scenario2", "stderr");
    let expected_exit = read_exit_code("scenario2");

    let mut cmd = Command::cargo_bin("mostro-score").expect("locate compiled binary");
    let assert = cmd.args(["--pubkey", "not-a-valid-pubkey"]).assert();
    let output = assert.get_output();

    assert_eq!(String::from_utf8_lossy(&output.stdout), expected_stdout);
    assert_eq!(String::from_utf8_lossy(&output.stderr), expected_stderr);
    assert_eq!(output.status.code(), Some(expected_exit));
}

// T046-T047: scenario 3 — syntactically malformed `--relays`, binary-level. Pins
// `client.add_relay(...)`'s parse-time failure through the real `EventSource`.
#[test]
fn scenario_3_malformed_relays_matches_golden_capture() {
    let expected_stdout = read_fixture("scenario3", "stdout");
    let expected_stderr = read_fixture("scenario3", "stderr");
    let expected_exit = read_exit_code("scenario3");

    let mut cmd = Command::cargo_bin("mostro-score").expect("locate compiled binary");
    let assert = cmd
        .args([
            "--pubkey",
            SCENARIO1_PUBKEY_HEX,
            "--relays",
            "not a valid relay url",
        ])
        .assert();
    let output = assert.get_output();

    assert_eq!(String::from_utf8_lossy(&output.stdout), expected_stdout);
    assert_eq!(String::from_utf8_lossy(&output.stderr), expected_stderr);
    assert_eq!(output.status.code(), Some(expected_exit));
}

// T048-T049: scenario 4 — well-formed but unreachable `--relays` (closed local port),
// binary-level. Pins the connect/fetch-error path through the real `EventSource`.
#[test]
fn scenario_4_unreachable_relay_matches_golden_capture() {
    let expected_stdout = read_fixture("scenario4", "stdout");
    let expected_stderr = read_fixture("scenario4", "stderr");
    let expected_exit = read_exit_code("scenario4");

    let mut cmd = Command::cargo_bin("mostro-score").expect("locate compiled binary");
    let assert = cmd
        .args([
            "--pubkey",
            SCENARIO1_PUBKEY_HEX,
            "--relays",
            "ws://127.0.0.1:1",
        ])
        .assert();
    let output = assert.get_output();

    assert_eq!(String::from_utf8_lossy(&output.stdout), expected_stdout);
    assert_eq!(String::from_utf8_lossy(&output.stderr), expected_stderr);
    assert_eq!(output.status.code(), Some(expected_exit));
}

// T050-T051: scenario 5 — fresh never-published keypair, zero fetched events. No
// `scenario5_events.ndjson` fixture exists because the real capture returned none.
#[tokio::test]
async fn scenario_5_fresh_keypair_matches_golden_capture() {
    let expected_stdout = read_fixture("scenario5", "stdout");
    let expected_stderr = read_fixture("scenario5", "stderr");
    let now_pre = read_now_pre("scenario5");

    let pubkey_hex = "a9b1ee37f17b4fa8b514804b03d92774ff8250923091d6e08ac3172334588449";
    let (actual_stdout, actual_stderr) = run_against_fixture(pubkey_hex, vec![], now_pre).await;

    assert_eq!(
        normalize_s_tag_distribution(&actual_stdout),
        normalize_s_tag_distribution(&expected_stdout)
    );
    assert_eq!(actual_stderr, expected_stderr);
}

// T052-T053: scenario 6 — qualifying orders, no qualifying dev-fee event.
#[tokio::test]
async fn scenario_6_orders_without_dev_fee_matches_golden_capture() {
    let expected_stdout = read_fixture("scenario6", "stdout");
    let expected_stderr = read_fixture("scenario6", "stderr");
    let now_pre = read_now_pre("scenario6");

    let pubkey_hex = "c6e5e031989223dd63e6ed49f0905a19a92ed86e0754721d6071133a9340bf7e";
    let events = load_fixture_events("tests/fixtures/scenario6_events.ndjson");
    let (actual_stdout, actual_stderr) = run_against_fixture(pubkey_hex, events, now_pre).await;

    assert_eq!(
        normalize_s_tag_distribution(&actual_stdout),
        normalize_s_tag_distribution(&expected_stdout)
    );
    assert_eq!(actual_stderr, expected_stderr);
}

// T054-T055: scenario 7 — qualifying dev-fee event, zero qualifying orders.
#[tokio::test]
async fn scenario_7_dev_fee_without_orders_matches_golden_capture() {
    let expected_stdout = read_fixture("scenario7", "stdout");
    let expected_stderr = read_fixture("scenario7", "stderr");
    let now_pre = read_now_pre("scenario7");

    let pubkey_hex = "00000235a3e904cfe1213a8a54d6f1ec1bef7cc6bfaabd6193e82931ccf1366a";
    let events = load_fixture_events("tests/fixtures/scenario7_events.ndjson");
    let (actual_stdout, actual_stderr) = run_against_fixture(pubkey_hex, events, now_pre).await;

    assert_eq!(
        normalize_s_tag_distribution(&actual_stdout),
        normalize_s_tag_distribution(&expected_stdout)
    );
    assert_eq!(actual_stderr, expected_stderr);
}

// T056-T057: scenario 8 — two relays, one reachable and one not. The current (pre-PR1)
// binary has no partial-relay-failure handling, so the pre-PR1 `EventSource` simply
// chains whatever each relay returns; a closed second relay contributes nothing, and
// this scenario's captured output is byte-identical to scenario 7's (documented in
// `tests/fixtures/MANIFEST.md`). `scenario8_events.ndjson` is NOT replayed here: it
// contains 15 extra kind-38383 order events beyond the 8 kind-8383 dev-fee events, from
// a separate, later capture-harness query of the same relay (the relay's documented
// flaky delivery — MANIFEST.md's "Learned" note) — not what the official binary run that
// produced `scenario8_stdout.txt` actually saw. Verified: the 8 dev-fee event ids in
// `scenario8_events.ndjson` are byte-identical to `scenario7_events.ndjson`'s, matching
// the captured stdout/stderr exactly. The two-relay outcome is therefore simulated with
// scenario 7's verified-consistent event set, per this scenario's own documented
// identical-to-scenario-7 behavior.
#[tokio::test]
async fn scenario_8_two_relays_one_unreachable_matches_golden_capture() {
    let expected_stdout = read_fixture("scenario8", "stdout");
    let expected_stderr = read_fixture("scenario8", "stderr");
    let now_pre = read_now_pre("scenario8");

    let pubkey_hex = "00000235a3e904cfe1213a8a54d6f1ec1bef7cc6bfaabd6193e82931ccf1366a";
    let events = load_fixture_events("tests/fixtures/scenario7_events.ndjson");
    let (actual_stdout, actual_stderr) = run_against_fixture(pubkey_hex, events, now_pre).await;

    assert_eq!(
        normalize_s_tag_distribution(&actual_stdout),
        normalize_s_tag_distribution(&expected_stdout)
    );
    assert_eq!(actual_stderr, expected_stderr);
}
