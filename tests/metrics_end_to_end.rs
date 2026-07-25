use mostro_score::fetch::client::EventSource;
use nostr_sdk::prelude::*;
use std::fs;

/// Fixture `EventSource`: replays a canned event set (captured once from a real relay
/// round trip, Step -1) instead of querying relays. No network access.
struct FixtureEventSource {
    events: Vec<Event>,
}

impl EventSource for FixtureEventSource {
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

/// Normalizes the "Status distribution for order events (s tag):" block: its lines come
/// from `HashMap` iteration, so their order is nondeterministic across process runs,
/// independent of any code change. Both sides of a comparison must have that block's
/// lines sorted before the rest of the text is compared byte-for-byte.
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

/// PR 1 Step D: T011's golden-baseline unit test, moved here now that `run()` is part of
/// the library's public surface — able to live in `tests/` as a real integration test
/// against `mostro_score::run()` rather than a same-crate unit test. Asserts against
/// Step -1's scenario 1 capture byte-for-byte (`s_tag_distribution` sorted for
/// comparison only).
#[tokio::test]
async fn wrapped_run_matches_step_minus_1_golden_scenario_1() {
    let fixture_events = load_fixture_events("tests/fixtures/scenario1_events.ndjson");
    let expected_stdout = fs::read_to_string("tests/fixtures/scenario1_stdout.txt")
        .expect("read golden stdout capture");
    let expected_stderr = fs::read_to_string("tests/fixtures/scenario1_stderr.txt")
        .expect("read golden stderr capture");
    let now_pre: i64 = fs::read_to_string("tests/fixtures/scenario1_now_pre.txt")
        .expect("read golden now")
        .trim()
        .parse()
        .expect("valid now timestamp");

    let public_key =
        PublicKey::parse("82fa8cb978b43c79b2156585bac2c011176a21d2aead6d9f7c575c005be88390")
            .unwrap();
    let event_source = FixtureEventSource {
        events: fixture_events,
    };
    let frozen_now = move || chrono::DateTime::<chrono::Utc>::from_timestamp(now_pre, 0).unwrap();

    let mut out: Vec<u8> = Vec::new();
    let mut err: Vec<u8> = Vec::new();

    mostro_score::run(public_key, event_source, &frozen_now, &mut out, &mut err)
        .await
        .expect("run() succeeds against the fixture event source");

    let actual_stdout = String::from_utf8(out).expect("stdout is valid utf-8");
    let actual_stderr = String::from_utf8(err).expect("stderr is valid utf-8");

    assert_eq!(
        normalize_s_tag_distribution(&actual_stdout),
        normalize_s_tag_distribution(&expected_stdout),
        "wrapped run() stdout must match Step -1's golden scenario 1 capture, \
         modulo HashMap-ordered s_tag_distribution lines"
    );
    assert_eq!(
        actual_stderr, expected_stderr,
        "wrapped run() stderr must match Step -1's golden scenario 1 capture"
    );
}
