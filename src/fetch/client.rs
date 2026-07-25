use crate::fetch::filters_summary::build_scoped_filters;
use nostr_sdk::prelude::*;
use std::time::Duration;
use tokio::sync::OnceCell;

/// Timeout for both the relay connection attempt and each fetch query.
const RELAY_TIMEOUT: Duration = Duration::from_secs(10);

/// PR 2 (T066-T069): the result of attempting to connect to every configured relay.
/// Principle VI's graceful-degradation rule and the Technical Context constraint ("one
/// failed relay among several that succeeded is a warning, not a failure; exit code 3
/// requires all relays to fail") both read off this struct: `run()` treats
/// `connected_count == 0` as fatal (`AppError::RelaysUnreachable`) and a non-empty
/// `failed` with `connected_count > 0` as warnings to print before continuing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayConnectionOutcome {
    pub connected_count: usize,
    pub failed: Vec<RelayConnectFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayConnectFailure {
    pub url: String,
    pub error: String,
}

/// Pure interpretation of `Client::try_connect`'s per-relay `Output`, kept separate from
/// the real network call so it stays unit-testable without a socket (Testing Strategy:
/// "No network in tests"). Failures are sorted by URL so a multi-relay warning listing is
/// deterministic, independent of the `HashMap` iteration order `Output::failed` uses.
fn interpret_connect_output(output: &Output<()>) -> RelayConnectionOutcome {
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
        failed,
    }
}

/// Seam introduced by PR 1 Step 0: the event-fetching surface `run()` depends on, so
/// production code and tests can supply different implementations (real relays vs. a
/// fixture replaying a captured event set). A generic bound, not `&dyn EventSource`,
/// since only two implementations exist and stable async-fn-in-traits needs no boxing
/// for static dispatch. `async fn` in a public trait is a deliberate, documented choice
/// (plan.md's Step 0 rationale): with only two call sites in this crate, the `Send`
/// bound the lint suggests adds nothing.
///
/// Two methods, not one: the original (pre-PR1) `main()` prints "Connected to relays"
/// only after relay setup (`add_relay`, which can fail) succeeds, and strictly before
/// issuing any fetch filters. A single `fetch()` call collapsing both phases
/// would either print that message too early (before a malformed relay's `add_relay`
/// failure surfaces) or require `run()` to reach into connection internals it has no
/// business owning. `connect()` isolates exactly the fallible relay-setup step so
/// `run()` can print its status line at the same logical point the original code did;
/// `fetch()` issues its filters afterward (PR 1's original two, expanded to PR 3's
/// four kind-scoped filters).
#[allow(async_fn_in_trait)]
pub trait EventSource {
    /// Establishes whatever connection this source needs before any event is fetched,
    /// reporting per-relay success/failure (PR 2, T067) so `run()` can distinguish a
    /// total outage from a partial one. A source with no real connection (e.g. a fixture
    /// replaying canned events for a test) reports every configured relay as connected.
    async fn connect(&self) -> Result<RelayConnectionOutcome>;

    async fn fetch(&self, public_key: PublicKey) -> Result<Vec<Event>>;
}

/// Production `EventSource`: connects to the configured relays and issues the four
/// kind-scoped filters from `filters_summary.rs` (PR 3), chaining every result set into
/// one `Vec<Event>`. The connected `Client` is cached in `connect()` and reused by
/// `fetch()`, since the original code builds the client and relay connection once, then
/// queries against that same connection.
pub struct RelayEventSource {
    pub relays: Vec<String>,
    client: OnceCell<Client>,
}

impl RelayEventSource {
    pub fn new(relays: Vec<String>) -> Self {
        Self {
            relays,
            client: OnceCell::new(),
        }
    }
}

impl EventSource for RelayEventSource {
    async fn connect(&self) -> Result<RelayConnectionOutcome> {
        let client = Client::new(Keys::generate());

        // A relay URL that fails to register (e.g. malformed) is a connection failure like
        // any other, not a distinct error class: it must feed the same graceful-degradation
        // classification as a relay that registers but fails to connect, so that "all relays
        // failed, for whatever reason" still maps to `RelaysUnreachable`, not `Other`.
        let mut registration_failures: Vec<RelayConnectFailure> = Vec::new();
        for relay in &self.relays {
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
        outcome.failed.sort_by(|a, b| a.url.cmp(&b.url));

        self.client
            .set(client)
            .map_err(|_| "RelayEventSource::connect called more than once")?;

        Ok(outcome)
    }

    async fn fetch(&self, public_key: PublicKey) -> Result<Vec<Event>> {
        let client = self
            .client
            .get()
            .ok_or("RelayEventSource::fetch called before connect()")?;

        // PR 3 (T097/T098): the four kind-scoped filters per 001 FR-015 — dev-fee
        // (8383), order (38383), instance-status (38385), and dispute (38386) —
        // replacing PR 1's original two-filter query.
        //
        // Each fetch runs in its own spawned task: `nostr-relay-pool` 0.43.1's
        // `fetch_events` constructs an internal `mpsc::channel(streams.len() * 512)`,
        // which Tokio panics on when every targeted relay's stream setup fails (e.g. a
        // relay disconnects between `connect()` succeeding and this call) — a real
        // transient-failure path, not a hypothetical one, that would otherwise abort the
        // whole process outside the `AppError` taxonomy (Principle VI). `tokio::spawn`
        // isolates a panic into a catchable `JoinError` instead of unwinding past it.
        let mut events: Vec<Event> = Vec::new();
        for filter in build_scoped_filters(public_key) {
            let client = client.clone();
            let fetched =
                tokio::spawn(async move { client.fetch_events(filter, RELAY_TIMEOUT).await })
                    .await
                    .map_err(|_| "relay fetch task panicked")??;
            events.extend(fetched);
        }

        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The plan's constraint ("`unwrap`/`expect` are permitted only in tests") rules out
    /// panicking when `fetch()` is called before `connect()` — an internal misuse that must
    /// still surface as an ordinary `AppError`, not abort the process.
    #[tokio::test]
    async fn fetch_before_connect_returns_an_error_instead_of_panicking() {
        let source = RelayEventSource::new(vec!["wss://relay.example".to_string()]);
        let public_key = Keys::generate().public_key();

        let result = source.fetch(public_key).await;

        assert!(result.is_err());
    }
}
