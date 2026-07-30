use crate::fetch::filters_summary::build_scoped_filters;
use crate::models::core::{NoOpProgressReporter, ProgressReporter};
use nostr_sdk::prelude::*;
use std::time::Duration;
use tokio::sync::OnceCell;

/// Timeout for both the relay connection attempt and each fetch query.
const RELAY_TIMEOUT: Duration = Duration::from_secs(10);

/// Measured against the real default relay `wss://relay.mostro.network` with a known
/// real pubkey: 3 runs of a full connect+fetch cycle timed at 2.06s/1.96s/1.69s wall time
/// -- normal single-relay operation sits right around 2 seconds. 3 seconds is
/// comfortably above that normal variance while still low enough to catch a genuinely
/// slow fetch.
pub const PROGRESS_INDICATOR_THRESHOLD: Duration = Duration::from_secs(3);

/// The result of attempting to connect to every configured relay. `run()` treats
/// `connected_count == 0` as fatal (`AppError::RelaysUnreachable`) and a non-empty
/// `failed` with `connected_count > 0` as warnings to print before continuing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayConnectionOutcome {
    pub connected_count: usize,
    pub connected_urls: Vec<String>,
    pub failed: Vec<RelayConnectFailure>,
    /// One entry per configured relay, in the user's originally configured `--relays`
    /// order -- `connected_urls` and `failed` alone cannot reconstruct this once
    /// combined, since each is independently populated from `Output`'s unordered
    /// success/failure sets and merging two separately-sorted lists still groups every
    /// success before every failure regardless of where each relay actually sat in
    /// `--relays`.
    pub ordered: Vec<RelayOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayConnectFailure {
    pub url: String,
    pub error: String,
}

/// One configured relay's outcome, preserving its position in `--relays`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayOutcome {
    pub url: String,
    pub succeeded: bool,
    pub error: Option<String>,
}

/// Pure interpretation of `Client::try_connect`'s per-relay `Output`, kept separate from
/// the real network call so it stays unit-testable without a socket.
/// `connected_urls`/`failed` are alphabetical here (`Output`'s own sets/maps have no
/// meaningful order of their own); `RelayEventSource::connect()` separately builds
/// `ordered` in the user's configured order once it has the original `--relays` list to
/// consult.
fn interpret_connect_output(output: &Output<()>) -> RelayConnectionOutcome {
    let mut connected_urls: Vec<String> =
        output.success.iter().map(|url| url.to_string()).collect();
    connected_urls.sort();

    let mut failed: Vec<RelayConnectFailure> = output
        .failed
        .iter()
        .map(|(url, error)| RelayConnectFailure {
            url: url.to_string(),
            error: error.clone(),
        })
        .collect();
    failed.sort_by(|a, b| a.url.cmp(&b.url));

    RelayConnectionOutcome {
        connected_count: output.success.len(),
        connected_urls,
        failed,
        ordered: Vec::new(),
    }
}

/// Builds `RelayConnectionOutcome::ordered` -- one `RelayOutcome` per entry in
/// `configured`, in that exact order. Every configured relay ends up in exactly one of
/// `connected_urls` or `failed` (registration failures are added to `failed` for every
/// configured relay that never even reached `try_connect`), so the final `else` arm
/// below is unreachable in practice; it reports an unknown-outcome relay rather than
/// panicking or silently dropping it, in case that invariant is ever violated by a
/// future change.
fn build_ordered_outcomes(
    configured: &[String],
    connected_urls: &[String],
    failed: &[RelayConnectFailure],
) -> Vec<RelayOutcome> {
    configured
        .iter()
        .map(|url| {
            if connected_urls.iter().any(|connected| connected == url) {
                RelayOutcome {
                    url: url.clone(),
                    succeeded: true,
                    error: None,
                }
            } else if let Some(failure) = failed.iter().find(|failure| &failure.url == url) {
                RelayOutcome {
                    url: url.clone(),
                    succeeded: false,
                    error: Some(failure.error.clone()),
                }
            } else {
                RelayOutcome {
                    url: url.clone(),
                    succeeded: false,
                    error: Some(
                        "unknown outcome: relay was neither connected nor failed".to_string(),
                    ),
                }
            }
        })
        .collect()
}

/// The event-fetching surface `run()` depends on, so production code and tests can
/// supply different implementations (real relays vs. a fixture replaying a captured
/// event set). A generic bound, not `&dyn EventSource`, since only two implementations
/// exist and stable async-fn-in-traits needs no boxing for static dispatch.
///
/// Two methods, not one: `run()` prints "Connected to relays" only after relay setup
/// (`add_relay`, which can fail) succeeds, and strictly before issuing any fetch
/// filters. A single `fetch()` call collapsing both phases would either print that
/// message too early (before a malformed relay's `add_relay` failure surfaces) or
/// require `run()` to reach into connection internals it has no business owning.
/// `connect()` isolates exactly the fallible relay-setup step; `fetch()` issues its
/// filters afterward.
#[allow(async_fn_in_trait)]
pub trait EventSource {
    /// Reports per-relay success/failure so `run()` can distinguish a total outage from
    /// a partial one. A source with no real connection (e.g. a fixture replaying canned
    /// events for a test) reports every configured relay as connected.
    async fn connect(&self) -> Result<RelayConnectionOutcome>;

    async fn fetch(&self, public_key: PublicKey) -> Result<Vec<Event>>;
}

/// Production `EventSource`: connects to the configured relays and issues the four
/// kind-scoped filters from `filters_summary.rs`, chaining every result set into one
/// `Vec<Event>`. The connected `Client` is cached in `connect()` and reused by `fetch()`.
///
/// Generic over `R: ProgressReporter` (defaulting to `NoOpProgressReporter`) rather than
/// depending on `report::progress::TerminalProgressReporter` directly: `fetch` depends
/// only on `models` and `error`, so the concrete terminal reporter is bound here only
/// from the library/binary wiring root (`main.rs`), never from within this module.
pub struct RelayEventSource<R = NoOpProgressReporter> {
    pub relays: Vec<String>,
    client: OnceCell<Client>,
    progress_reporter: R,
    /// Test-only seam: when set, `fetch()` substitutes this controllable async delay for
    /// the real relay-client call entirely, so `tests/metrics_end_to_end.rs` can drive
    /// the real `fetch()` method's progress-reporter-racing logic under
    /// `tokio::time::pause`/`advance` without a real relay connection or any real sleep.
    /// Not `#[cfg(test)]`-gated: that only applies when this crate itself is compiled in
    /// test mode, which does not cover `tests/`'s separate compilation of this crate as
    /// an ordinary dependency, so a literal `#[cfg(test)]` field would be invisible
    /// there. No production call site (`main.rs`) ever sets this.
    test_fetch_delay: Option<Duration>,
}

impl RelayEventSource<NoOpProgressReporter> {
    pub fn new(relays: Vec<String>) -> Self {
        Self::with_progress_reporter(relays, NoOpProgressReporter)
    }
}

impl<R> RelayEventSource<R> {
    pub fn with_progress_reporter(relays: Vec<String>, progress_reporter: R) -> Self {
        Self {
            relays,
            client: OnceCell::new(),
            progress_reporter,
            test_fetch_delay: None,
        }
    }

    /// Test-only builder: see `test_fetch_delay`'s field doc for why this cannot be
    /// `#[cfg(test)]`-gated instead.
    pub fn with_test_fetch_delay(mut self, delay: Duration) -> Self {
        self.test_fetch_delay = Some(delay);
        self
    }
}

impl<R: ProgressReporter> RelayEventSource<R> {
    /// Races `task` against `PROGRESS_INDICATOR_THRESHOLD`, invoking the bound
    /// `ProgressReporter` at most once if the threshold elapses before `task` resolves,
    /// then continuing to await `task` itself. Shared by both the real relay-client path
    /// and the test-only simulated-delay path.
    async fn await_with_progress<T, E>(
        &self,
        mut task: tokio::task::JoinHandle<std::result::Result<T, E>>,
    ) -> Result<T>
    where
        E: std::error::Error + 'static,
    {
        let mut reported = false;
        loop {
            // `biased` removes one source of nondeterminism: without it, `select!`
            // polls ready branches in a pseudo-random order, so a task and the threshold
            // timer becoming ready in the same poll could non-reproducibly report
            // progress for a fetch that has, in that same instant, already finished.
            tokio::select! {
                biased;
                result = &mut task => {
                    return Ok(result.map_err(|_| "relay fetch task panicked")??);
                }
                _ = tokio::time::sleep(PROGRESS_INDICATOR_THRESHOLD), if !reported => {
                    self.progress_reporter.report_slow_fetch();
                    reported = true;
                }
            }
        }
    }

    async fn fetch_one_real(&self, filter: Filter) -> Result<Vec<Event>> {
        let client = self
            .client
            .get()
            .ok_or("RelayEventSource::fetch called before connect()")?
            .clone();

        // Spawned in its own task: `nostr-relay-pool` 0.43.1's `fetch_events` constructs
        // an internal `mpsc::channel` that Tokio panics on when every targeted relay's
        // stream setup fails, and `tokio::spawn` isolates that panic into a catchable
        // `JoinError` instead of unwinding past it.
        let task = tokio::spawn(async move { client.fetch_events(filter, RELAY_TIMEOUT).await });
        let fetched = self.await_with_progress(task).await?;
        Ok(fetched.into_iter().collect())
    }

    /// Simulated path: substitutes `tokio::time::sleep(delay)` for the real
    /// `client.fetch_events(...)` call entirely, so the test needs no `Client` at all
    /// (and therefore no `connect()`, no real network) while still exercising the real
    /// `await_with_progress` racing logic above.
    async fn fetch_one_simulated(&self, delay: Duration) -> Result<Vec<Event>> {
        let task = tokio::spawn(async move {
            tokio::time::sleep(delay).await;
            Ok::<Vec<Event>, nostr_sdk::client::Error>(Vec::new())
        });
        self.await_with_progress(task).await
    }
}

impl<R: ProgressReporter> EventSource for RelayEventSource<R> {
    async fn connect(&self) -> Result<RelayConnectionOutcome> {
        let client = Client::new(Keys::generate());

        // `add_relay` parses its argument into a canonical `RelayUrl` internally before
        // ever touching the pool, so `output.success`/`output.failed` always report
        // relays in that canonical form (e.g. normalized trailing slash), not the user's
        // raw `--relays` string. Canonicalizing here too, once, up front, keeps every
        // later string comparison against `connected_urls`/`failed` correct even when a
        // relay's raw and canonical forms differ syntactically but name the same relay
        // -- a URL that fails to parse at all falls back to its raw string, which is
        // exactly what `add_relay` itself would have failed on too.
        let configured_relays: Vec<String> = self
            .relays
            .iter()
            .map(|relay| {
                RelayUrl::parse(relay)
                    .map(|parsed| parsed.to_string())
                    .unwrap_or_else(|_| relay.clone())
            })
            .collect();

        // A relay URL that fails to register (e.g. malformed) is a connection failure
        // like any other, not a distinct error class: it must feed the same
        // classification as a relay that registers but fails to connect, so that "all
        // relays failed, for whatever reason" still maps to `RelaysUnreachable`, not
        // `Other`.
        let mut registration_failures: Vec<RelayConnectFailure> = Vec::new();
        for relay in &configured_relays {
            if let Err(error) = client.add_relay(relay.as_str()).await {
                registration_failures.push(RelayConnectFailure {
                    url: relay.clone(),
                    error: error.to_string(),
                });
            }
        }

        let output = client.try_connect(RELAY_TIMEOUT).await;
        let mut outcome = interpret_connect_output(&output);
        outcome.failed.extend(registration_failures);
        outcome.ordered =
            build_ordered_outcomes(&configured_relays, &outcome.connected_urls, &outcome.failed);

        self.client
            .set(client)
            .map_err(|_| "RelayEventSource::connect called more than once")?;

        Ok(outcome)
    }

    async fn fetch(&self, public_key: PublicKey) -> Result<Vec<Event>> {
        // Each filter's fetch races against `PROGRESS_INDICATOR_THRESHOLD` in
        // `await_with_progress`, invoking the bound `ProgressReporter` at most once if a
        // fetch runs past it. `test_fetch_delay`, when set, substitutes a controllable
        // delay for the real relay-client call entirely (see its field doc).
        let mut events: Vec<Event> = Vec::new();
        for filter in build_scoped_filters(public_key) {
            let fetched = if let Some(delay) = self.test_fetch_delay {
                self.fetch_one_simulated(delay).await?
            } else {
                self.fetch_one_real(filter).await?
            };
            events.extend(fetched);
        }

        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Documents the exact canonicalization `RelayEventSource::connect()` relies on to
    /// avoid falsely reporting a successfully-connected relay as failed: `RelayUrl`
    /// wraps the standard `url` crate, which lowercases the scheme and host per the
    /// WHATWG URL spec. A raw `--relays` string differing only in case from the pool's
    /// canonical form must still match `build_ordered_outcomes`'s lookup.
    #[test]
    fn relay_url_canonicalizes_scheme_and_host_case() {
        let canonical = RelayUrl::parse("WSS://Relay.Example").unwrap().to_string();
        assert_eq!(canonical, "wss://relay.example");
    }

    #[test]
    fn build_ordered_outcomes_matches_the_configured_relays_order_across_success_and_failure() {
        // Interleaved on purpose: success, failure, success — proves the merge follows
        // `configured`'s true order rather than grouping every success before every
        // failure (which independently-sorted `connected_urls`/`failed` lists cannot
        // avoid on their own).
        let configured = vec![
            "wss://z-relay.example".to_string(),
            "wss://m-relay.example".to_string(),
            "wss://a-relay.example".to_string(),
        ];
        let connected_urls = vec![
            "wss://a-relay.example".to_string(),
            "wss://z-relay.example".to_string(),
        ];
        let failed = vec![RelayConnectFailure {
            url: "wss://m-relay.example".to_string(),
            error: "connection refused".to_string(),
        }];

        let ordered = build_ordered_outcomes(&configured, &connected_urls, &failed);

        assert_eq!(
            ordered
                .iter()
                .map(|outcome| outcome.url.as_str())
                .collect::<Vec<_>>(),
            vec![
                "wss://z-relay.example",
                "wss://m-relay.example",
                "wss://a-relay.example",
            ]
        );
        assert!(ordered[0].succeeded);
        assert!(!ordered[1].succeeded);
        assert_eq!(ordered[1].error, Some("connection refused".to_string()));
        assert!(ordered[2].succeeded);
    }

    /// Calling `fetch()` before `connect()` is an internal misuse that must still
    /// surface as an ordinary `AppError`, not abort the process.
    #[tokio::test]
    async fn fetch_before_connect_returns_an_error_instead_of_panicking() {
        let source = RelayEventSource::new(vec!["wss://relay.example".to_string()]);
        let public_key = Keys::generate().public_key();

        let result = source.fetch(public_key).await;

        assert!(result.is_err());
    }
}
