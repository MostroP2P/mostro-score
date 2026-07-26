//! T137: proves the real `fetch::client::RelayEventSource`'s `ProgressReporter` binding
//! actually fires once a fetch runs past `PROGRESS_INDICATOR_THRESHOLD` (002 FR-014),
//! using `tokio::time::pause`/`advance` (simulated virtual time) through the test-only
//! `with_test_fetch_delay` seam — never a real sleep, never a real relay connection. This
//! exercises the production `EventSource::fetch()` method itself (its real
//! `tokio::select!` racing logic and its real progress-reporter binding), not a fake
//! substitute standing in for it.

use mostro_score::fetch::client::{EventSource, RelayEventSource, PROGRESS_INDICATOR_THRESHOLD};
use mostro_score::models::core::ProgressReporter;
use nostr_sdk::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

const TEST_PUBKEY_HEX: &str = "82fa8cb978b43c79b2156585bac2c011176a21d2aead6d9f7c575c005be88390";

#[derive(Clone, Default)]
struct SpyProgressReporter {
    calls: Arc<AtomicUsize>,
}

impl SpyProgressReporter {
    fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl ProgressReporter for SpyProgressReporter {
    fn report_slow_fetch(&self) {
        self.calls.fetch_add(1, Ordering::SeqCst);
    }
}

/// The real `EventSource::fetch()` racing logic invokes the bound `ProgressReporter`
/// once `PROGRESS_INDICATOR_THRESHOLD` elapses before the (simulated) fetch resolves.
/// `fetch()` issues 4 kind-scoped filters sequentially (`build_scoped_filters`), each
/// independently raced against the threshold, so a delay applied uniformly to every
/// filter's fetch fires the reporter once per filter — 4 times by the time the whole
/// `fetch()` call resolves, not once overall.
#[tokio::test(start_paused = true)]
async fn fetch_reports_slow_progress_once_it_exceeds_the_threshold() {
    let public_key = PublicKey::parse(TEST_PUBKEY_HEX).unwrap();
    let spy = SpyProgressReporter::default();
    let source = RelayEventSource::with_progress_reporter(vec![], spy.clone())
        .with_test_fetch_delay(PROGRESS_INDICATOR_THRESHOLD + Duration::from_secs(1));

    // `source.fetch(...)`'s error type is `Box<dyn std::error::Error>`, which is not
    // `Send`; mapped to a `String` here purely so this spawned task's output satisfies
    // `tokio::spawn`'s `Send` bound. This doesn't touch the real `fetch()` logic under
    // test, only how its already-resolved `Result` is carried across the spawn boundary.
    let fetch_task =
        tokio::spawn(async move { source.fetch(public_key).await.map_err(|e| e.to_string()) });

    // Let the spawned task reach its first await point (registering both the
    // progress-threshold timer and the simulated-delay timer) before advancing time.
    tokio::task::yield_now().await;

    tokio::time::advance(PROGRESS_INDICATOR_THRESHOLD).await;
    tokio::task::yield_now().await;
    assert_eq!(
        spy.call_count(),
        1,
        "progress reporter should have fired for the first filter's fetch past the threshold"
    );

    // Paused-clock auto-advance carries the remaining 3 filters (each with the same
    // injected delay) to completion once nothing else can make progress.
    let events = fetch_task.await.unwrap().unwrap();

    assert!(events.is_empty());
    assert_eq!(
        spy.call_count(),
        4,
        "one report per kind-scoped filter that individually exceeded the threshold"
    );
}

/// A fetch that resolves before the threshold never invokes the progress reporter.
#[tokio::test(start_paused = true)]
async fn fetch_does_not_report_progress_when_it_completes_before_the_threshold() {
    let public_key = PublicKey::parse(TEST_PUBKEY_HEX).unwrap();
    let spy = SpyProgressReporter::default();
    let source = RelayEventSource::with_progress_reporter(vec![], spy.clone())
        .with_test_fetch_delay(Duration::from_millis(100));

    let events = source.fetch(public_key).await.unwrap();

    assert!(events.is_empty());
    assert_eq!(spy.call_count(), 0);
}
