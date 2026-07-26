# Tasks: Phase 4 — Modular Implementation of the Reputation CLI

**Input**: `specs/004-phase-4-implementation/plan.md` (the sole source of truth; specs 001-003 supply the requirement numbers referenced below)

**Tests**: Mandatory throughout (Principle IV, Strict TDD). PR 1 uses characterization tests against a golden baseline. PR 2 onward uses Red-Green-Refactor, tests written in the same PR as the code they cover.

**Organization**: Tasks are grouped **PR-by-PR**, mirroring the plan's own delivery table — not generic Setup/Foundational/User Story phases. Each PR section is one deliverable, independently reviewable, in the plan's own dependency order.

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | PR 1: High (mechanical move across all seven modules + 8-scenario golden baseline + characterization tests + binary-level scenario assertions). PR 3: Medium (four-kind fetch scoping added, wired end to end into the real `EventSource`). PR 7-8: Medium-High (grid/report/JSON contract). All others: Low-Medium. |
| 400-line budget risk | High (PR 1 only) |
| Chained PRs recommended | Yes — already the plan's own structure (12 PRs) |
| Suggested split | PR 1 → PR 2 → PR 3 → {PR 4} → {PR 5, PR 6} → PR 7 → PR 8 → PR 9 → {PR 10, PR 11} → PR 12 |
| Delivery strategy | ask-on-risk |
| Chain strategy | stacked-to-main |

Decision needed before apply: Yes
Chained PRs recommended: Yes
Chain strategy: stacked-to-main
400-line budget risk: High

PR 1 cannot be split further without breaking its own safety net: every step (wrap, extract, move, scenario assertion) is re-proved against one golden baseline captured in Step -1, and splitting the move across multiple PRs would mean re-deriving or re-sharing that baseline across diffs. PR 1 is a candidate for a maintainer-accepted `size:exception` on the same grounds the guard allows for generated/mechanical diffs — this decision is `sdd-apply`'s to confirm before PR 1 opens. PRs 2-12 are each independently sized to stay under budget.

### Suggested Work Units

| Unit | Goal | PR | Notes |
|------|------|----|-------|
| 1 | Modularize `main.rs`, no behavior change | PR 1 | base: `development`; size:exception candidate |
| 2 | Error taxonomy, exit codes, diagnostic routing | PR 2 | base: `development` after PR 1 merges; depends on 1 |
| 3 | Event scoping, dedup, four-kind fetch, exit code 4 | PR 3 | depends on 1, 2 |
| 4 | Core metrics (longevity/cumulative/trade-size/liveness/consistency) | PR 4 | depends on 3 |
| 5 | Dispute signals | PR 5 | depends on 3, 4 |
| 6 | Descriptive context (fiat/payment/premium/bond) | PR 6 | depends on 3, 4 |
| 7 | Report model, grid, recommendations, console renderer | PR 7 | depends on 4, 5, 6 |
| 8 | Plain/JSON renderers, fatal envelope | PR 8 | depends on 7 |
| 9 | CLI flags: identity, format, color, quiet, help | PR 9 | depends on 8 |
| 10 | Time range/grouping | PR 10 | depends on 9; independent of 11 |
| 11 | Section filtering | PR 11 | depends on 9; independent of 10 |
| 12 | Persisted config, `--init-config`, `--config-dir` | PR 12 | depends on 9, 10 |

---

## PR 1 — Modularization, no behavior change

**Goal**: Extract `src/main.rs` into the seven constitution modules plus `lib.rs`, with zero output change. **Depends on**: none.

### Step -1 — Golden baseline (before any source change)

- [x] T001 Build the current unmodified binary; capture baseline scenario 1 (happy path — dedicated settled test node, real relay round trip): serialize the fetched event set to `tests/fixtures/`, record the pre-launch `now` wall-clock second (verified against post-exit), and capture stdout/stderr/exit status byte-for-byte.
- [x] T002 [P] Capture baseline scenario 2 — malformed `--pubkey`.
- [x] T003 [P] Capture baseline scenario 3 — syntactically malformed `--relays` value.
- [x] T004 [P] Capture baseline scenario 4 — well-formed but unreachable `--relays` address: a closed local port on loopback (e.g. `ws://127.0.0.1:1`, or bind to port 0 for an OS-assigned free port and never listen on it) rather than a reserved/non-routable address, so the connection is refused immediately and deterministically, with no live internet dependency or OS-level timeout risk.
- [x] T005 [P] Capture baseline scenario 5 — fresh never-published keypair (zero fetched events).
- [x] T006 [P] Capture baseline scenario 6 — qualifying orders, no qualifying dev-fee event.
- [x] T007 [P] Capture baseline scenario 7 — qualifying dev-fee event, zero qualifying orders.
- [x] T008 [P] Capture baseline scenario 8 — two relays, one reachable, one not.

### Step 0 — Wrap `main()` before splitting

- [x] T009 In `src/main.rs`, extract `main()`'s body verbatim into a private `async fn` taking `public_key: PublicKey`, `event_source: E where E: EventSource`, `now: &dyn Fn() -> DateTime<Utc>`, `out: &mut impl Write`, `err: &mut impl Write`, returning `Result<()>` — the same `Result` alias current `main()` already uses via `nostr_sdk::prelude::*` (`Result<T, Box<dyn std::error::Error>>`); PR 1 must not change the error type.
- [x] T010 Shrink production `main()` in `src/main.rs` to arg/pubkey parsing, constructing the real relay-backed `EventSource`, wiring `chrono::Utc::now` and `io::stdout()`/`io::stderr()`, and awaiting the wrapped function.
- [x] T011 In `src/main.rs`'s `#[cfg(test)] mod tests`, call the wrapped function with a fixture `EventSource` (T001's events), T001's recorded `now`, and `Vec<u8>` writers; assert against T001's captured stdout/stderr (`s_tag_distribution` sorted by key for comparison only).

### Step A — Test the already-standalone functions

- [x] T012 [P] Characterization tests for `compute_trade_stats` against current `src/main.rs`.
- [x] T013 [P] Characterization tests for `compute_rolling_windows` against current `src/main.rs`.
- [x] T014 [P] Characterization tests for `compute_activity_consistency` against current `src/main.rs`.
- [x] T015 [P] Characterization tests for `format_relative_time`, every branch including future timestamps.
- [x] T016 [P] Characterization tests for `calculate_score`.

### Step B — Extract a seam, test, then move

- [x] T017 Extract order dedup-by-`d`-tag into a named function in `src/main.rs`; re-run T011's golden test.
- [x] T018 Characterization test for the extracted dedup function.
- [x] T019 Extract dev-fee event selection into a named function in `src/main.rs`; re-run T011's golden test.
- [x] T020 Characterization test for the extracted dev-fee selection function.
- [x] T021 Extract `z`/`y` tag partitioning into a named function in `src/main.rs`; re-run T011's golden test.
- [x] T022 Characterization test for the extracted `z`/`y` partitioning function.
- [x] T023 Move the dedup function + test to `src/models/dedup.rs` (pure file move).
- [x] T024 Move the dev-fee selection function + test to `src/models/dev_fee.rs` (pure file move).
- [x] T025 Move the `z`/`y` partitioning function + test to `src/models/core.rs` (pure file move).
- [x] T026 Re-run T011's golden test after the three moves.

### Step C — Decompose the remaining body

- [x] T027 Create `src/fetch/mod.rs` and `src/fetch/client.rs`; move relay client setup and the two-filter query verbatim into the real `EventSource` implementation.
- [x] T028 Move per-kind dev-fee aggregation verbatim into `src/models/dev_fee.rs`.
- [x] T029 Move per-kind order aggregation (qualifying-order selection, `s=success` filter) verbatim into `src/models/order.rs`.
- [x] T030 Create empty scaffolding for `src/models/dispute.rs` (stub types only, no aggregation logic) — base `src/main.rs` has no dispute aggregation to move verbatim; the real dispute dedup-by-`d`-tag and resolved/active/unknown classification logic is implemented in PR 3 (T086-T087).
- [x] T031 Create empty scaffolding for `src/models/instance_status.rs` (stub types only, no aggregation logic) — base `src/main.rs` has no instance-status aggregation to move verbatim; the real instance-status selection logic is implemented in PR 3 (T088-T089).
- [x] T032 Extract shared tag accessors (`z`/`y`/`d`/`s`/`amt`/`f`/`pm`/`premium`/`bond_enabled`) into `src/models/core.rs`.
- [x] T033 Move `compute_trade_stats` verbatim into `src/stats/trade_size.rs`.
- [x] T034 Move `compute_rolling_windows` and `compute_activity_consistency` verbatim into `src/stats/lifecycle.rs`.
- [x] T035 Move `format_relative_time` (plus its Step A characterization test, T015) verbatim into `src/report/format.rs` — rendering/presentation logic, not a stats computation, per the plan's module tree; must land before T037 moves the console formatting calls that consume it.
- [x] T036 Move `calculate_score` (plus its Step A characterization test, T016) verbatim into `src/stats/mod.rs` as a private function still called by the wrapped function — PR 1 is behavior-preserving, so it is relocated here rather than deleted; removed later in PR 7's T124 once the report model no longer needs it, per the plan's Complexity Tracking decision.
- [x] T037 Move every remaining formatting/coloring `writeln!` call verbatim into `src/report/render/console.rs`; add minimal `src/report/mod.rs` and `src/report/render/mod.rs` wiring for the console-only path.
- [x] T038 Create empty scaffolding for `src/error/mod.rs` and `src/error/exit_code.rs` with stub types only, no behavior — at this point in history `main()` returns `Result<()>` and relies on the runtime's implicit termination on error, with no distinct exit-code mapping logic to move; the actual exit-code mapping is implemented in PR 2 (T062-T063).
- [x] T039 Create stub `src/cli/mod.rs` and `src/config/mod.rs` (empty modules, no logic yet); wire both into `src/lib.rs` so all seven constitution modules exist under `src/` by the end of this PR — `cli` is populated in PR 9, `config` in PR 12.
- [x] T040 Re-run T001's full baseline comparison after the Step C moves and module scaffolding; confirm the wrapped function now contains only calls into `fetch`, `models`, `stats`, and `report`.

### Step D — Final module move

- [x] T041 Relocate the wrapped function to `src/lib.rs` as `pub async fn run<E: EventSource>(public_key: PublicKey, event_source: E, now: &dyn Fn() -> DateTime<Utc>, out: &mut impl std::io::Write, err: &mut impl std::io::Write) -> Result<()>` (same `Result` alias as T009; PR 2's T061/T069 later swap it for `AppError`), clock call at the same logical point.
- [x] T042 Move T011's test to `tests/metrics_end_to_end.rs` as an integration test against `run()`; assert against T001's capture byte-for-byte.
- [x] T043 Confirm `src/main.rs` ends this PR as: parse args/pubkey, construct real `EventSource`, wire real clock and stdio, call `run()` — invalid-pubkey branch still pinned by T002.

### Step D2 — Assert the remaining golden baseline scenarios

Only scenario 1 (T001) is proved by T042's integration test, which calls `run()` in-process. Scenarios 2, 3, and 4 (T002, T003, T004) are pinned at binary level via `assert_cmd`, running the actual compiled binary as a subprocess: an in-process `run()` call only observes the library's `Result` and injected writers, so it cannot verify the real `main()`/tokio wrapper, the real process exit status, or error rendering at the process boundary, and a regression in that wiring layer could still pass an in-process test. Scenario 2 (malformed `--pubkey`) stays out of `run()` entirely, since pubkey parsing lives in `main()` itself. Scenarios 3 and 4 depend only on deterministic connection-layer behavior against a fixed bad target — a syntactically invalid `--relays` value for scenario 3, and a closed local port on loopback for scenario 4 (T004; not a reserved/non-routable address, which risks a slow OS-level connection timeout and platform-dependent behavior) — not on any relay's real event history, so both are asserted against the compiled binary's real stdout/stderr/exit code, exercising the real, production `fetch::client::EventSource` through the real process. Scenarios 5 through 8 (T005-T008) depend on real historical event data, so they stay on a fixture `EventSource` called in-process against `run()`, plus a frozen `now`, matching the same fixture-plus-frozen-clock discipline as T042 — no live relay access in any of these four.

- [x] T044 [RED] Write an `assert_cmd` binary-level test asserting scenario 2 (malformed `--pubkey`) matches T002's capture (stdout/stderr/exit status) in `tests/cli_behavior.rs`.
- [x] T045 [GREEN] Run the scenario 2 test against the fully refactored binary; confirm it passes, fixing any wiring regression found.
- [x] T046 [RED] Write an `assert_cmd` binary-level test in `tests/cli_behavior.rs` asserting scenario 3 (T003's syntactically malformed `--relays` value) matches T003's capture (stdout/stderr/exit status), running the actual compiled binary.
- [x] T047 [GREEN] Run the scenario 3 test against the fully refactored binary; confirm it passes, fixing any wiring regression found — found and fixed a real regression: Step 0's original wrap (previous batch) had moved the "Connected to relays" print to before relay setup, so a malformed `--relays` value printed one extra line the pre-PR1 binary never did. Fixed by splitting `EventSource` into `connect()` (fallible relay setup) and `fetch()` (unchanged), called in the original order.
- [x] T048 [RED] Write an `assert_cmd` binary-level test in `tests/cli_behavior.rs` asserting scenario 4 (T004's closed local port on loopback) matches T004's capture (stdout/stderr/exit status), running the actual compiled binary.
- [x] T049 [GREEN] Run the scenario 4 test against the fully refactored binary; confirm it passes, fixing any wiring regression found.
- [x] T050 [RED] Write a test in `tests/cli_behavior.rs` calling `run()` with a fixture `EventSource` returning T005's empty event set and a frozen `now`; assert output matches T005's capture, no live relay access.
- [x] T051 [GREEN] Run the scenario 5 test against the fully refactored `run()`; confirm it passes, fixing any wiring regression found.
- [x] T052 [RED] Write a test in `tests/cli_behavior.rs` calling `run()` with a fixture `EventSource` returning T006's captured events (qualifying orders, no qualifying dev-fee event) and a frozen `now`; assert output matches T006's capture, no live relay access.
- [x] T053 [GREEN] Run the scenario 6 test against the fully refactored `run()`; confirm it passes, fixing any wiring regression found.
- [x] T054 [RED] Write a test in `tests/cli_behavior.rs` calling `run()` with a fixture `EventSource` returning T007's captured events (qualifying dev-fee event, zero qualifying orders) and a frozen `now`; assert output matches T007's capture, no live relay access.
- [x] T055 [GREEN] Run the scenario 7 test against the fully refactored `run()`; confirm it passes, fixing any wiring regression found.
- [x] T056 [RED] Write a test in `tests/cli_behavior.rs` asserting output for scenario 8's two-relay configuration (one reachable, one not) matches T008's capture, no live relay access. The pre-PR1 binary has no partial-relay-failure handling (that lands in PR 2/3), so this asserts output/event-set correspondence against the same captured behavior as scenario 7, not connection-failure handling — uses scenario 7's event set through a fixture `EventSource` whose `connect()` is a no-op, not `scenario8_events.ndjson`: that fixture's extra 15 order events come from a later, separate capture-harness query (the relay's documented flaky delivery per `MANIFEST.md`), not what the official binary run that produced `scenario8_stdout.txt` saw. Verified the 8 dev-fee event ids in both fixtures are identical.
- [x] T057 [GREEN] Run the scenario 8 test against the fully refactored `run()`; confirm it passes, fixing any wiring regression found.

---

## PR 2 — Error taxonomy, exit codes, diagnostic routing

**Depends on**: 1. Requirements: 002 FR-011, FR-017; FR-019 except exit `4`; Principle VI.

**Amendment**: `tests/fixtures/` and the fixture-only test files (`tests/cli_behavior.rs`,
`tests/metrics_end_to_end.rs`) are deleted in this PR, not retired — see `plan.md`'s PR 1 Amendment.
Committing real relay captures to the repository is reversed; behavior-preservation is verified by
independent review of the diff instead. T058/T059 below (originally "retire the golden assertion
for scenario N") are superseded by this single deletion; the specific per-scenario fixture-amendment
work they described no longer applies.

- [x] T058 Delete `tests/fixtures/` and `tests/cli_behavior.rs`/`tests/metrics_end_to_end.rs` entirely (supersedes the original per-scenario fixture-retirement plan).
- [x] T059 Confirm `cargo build --all-targets`, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo fmt --check` all pass with the fixture-dependent tests gone.
- [x] T060 [RED] Failing tests for `AppError` variants in `src/error/mod.rs`.
- [x] T061 [GREEN] Implement `AppError` with `thiserror` in `src/error/mod.rs`.
- [x] T062 [RED] Failing tests for exit code mapping (`0`/`1`/`2`/`3`/`5`) in `src/error/exit_code.rs`.
- [x] T063 [GREEN] Implement exit code mapping in `src/error/exit_code.rs`.
- [x] T064 [RED] Failing test: invalid `--pubkey` exits `5` (fixes PR 1 deviation) — `tests/error_handling.rs::invalid_pubkey_exits_5`.
- [x] T065 [GREEN] Wire invalid-pubkey branch in `src/main.rs` to `AppError::InvalidPubkey`.
- [x] T066 [RED] Failing test: one relay failing among several succeeding still yields a successful report + warning — `tests/error_handling.rs::one_failed_relay_among_several_is_a_warning_not_a_failure`.
- [x] T067 [GREEN] Implement graceful partial-relay-failure handling in `src/fetch/client.rs`.
- [x] T068 [RED] Failing test: all relays failing exits `3` — `tests/error_handling.rs::all_relays_unreachable_is_fatal`.
- [x] T069 [GREEN] Wire all-relays-failed to `AppError::RelaysUnreachable` in `src/lib.rs`'s `run()`.
- [x] T070 [RED] Failing tests: debug/sample-event dumps and no-dev-fee warning route to `err`, not `out` (fixes PR 1 deviation) — `tests/error_handling.rs::diagnostics_route_to_err_not_out`.
- [x] T071 [GREEN] Rewrite those diagnostic `writeln!(out, ...)` calls to `writeln!(err, ...)` in `src/report/render/console.rs`.
- [x] T072 [RED] Failing tests: the two transient status lines (`"Connected to relays. Fetching history... (this might take a moment)"` and `"Fetched {N} events. Analyzing..."`) route to `err`, not `out` — same test as T070, both lines asserted.
- [x] T073 [GREEN] Rewrite those two transient-status `writeln!(out, ...)` calls to `writeln!(err, ...)` in `src/report/render/console.rs` — stderr is the correct interim destination since `ProgressReporter`'s concrete implementation is not wired in until PR 7's T138.
- [x] T074 [RED] Failing test: `s_tag_distribution` prints sorted by key, deterministic across runs — `tests/error_handling.rs::s_tag_distribution_prints_sorted_by_key`.
- [x] T075 [GREEN] Sort `s_tag_distribution` by key in `src/report/render/console.rs`.

---

## PR 3 — Event scoping, dedup, four-kind fetch, domain models, exit code 4

**Depends on**: 1, 2. Requirements: 001 FR-002, FR-006, FR-012, FR-013, FR-014, FR-015; 002 FR-003 (fetch scoping), FR-019 (exit `4`).

- [x] T076 [RED] Failing tests for author/`z`/`y` scoping in `src/models/core.rs`.
- [x] T077 [GREEN] Implement scoping per 001 FR-015 in `src/models/core.rs`.
- [x] T078 [RED] Failing tests for future-timestamp exclusion in `src/models/core.rs`.
- [x] T079 [GREEN] Implement future-timestamp exclusion per 001 FR-014 in `src/models/core.rs`.
- [x] T080 [RED] Failing tests for the dedup tie-break by greatest event id on equal `created_at` in `src/models/dedup.rs`.
- [x] T081 [GREEN] Implement the tie-break rule per 001 FR-002/006/012/014 in `src/models/dedup.rs`.
- [x] T082 [RED] Failing tests for the four-kind fetch filter expansion (`8383`/`38383`/`38385`/`38386`) in `src/fetch/filters_summary.rs`.
- [x] T083 [GREEN] Implement the four-kind filter query per 001 FR-015, 002 FR-003 in `src/fetch/filters_summary.rs`; `RelayFetchOutcome`'s per-field counts are implemented separately below, each against its own semantic rule.
- [x] T084 [RED] Failing tests for qualifying-order selection (`d`-tag dedup to the highest `created_at`, then the `s=success` filter) in `src/models/order.rs`.
- [x] T085 [GREEN] Implement qualifying-order selection per 001 FR-002 in `src/models/order.rs`.
- [x] T086 [RED] Failing tests for dispute event dedup-by-`d`-tag and resolved/active/unknown classification in `src/models/dispute.rs`.
- [x] T087 [GREEN] Implement dispute dedup and classification per 001 FR-006, FR-015 in `src/models/dispute.rs`.
- [x] T088 [RED] Failing tests for instance-status kind-`38385` selection (`d` = node pubkey, highest `created_at`, tie-break by event id) in `src/models/instance_status.rs`.
- [x] T089 [GREEN] Implement instance-status selection per 001 FR-012, FR-015 in `src/models/instance_status.rs`.
- [x] T090 [RED] Failing tests asserting that events with malformed or missing required tags across all four scoped kinds (`8383` dev-fee, `38383` order, `38385` instance-status, `38386` dispute) are safely excluded from aggregation without panicking, in `src/models/core.rs`.
- [x] T091 [GREEN] Implement malformed/incomplete-tag exclusion per 001 FR-013 across `src/models/dev_fee.rs`, `src/models/order.rs`, `src/models/dispute.rs`, `src/models/instance_status.rs`, routed through the shared tag accessors in `src/models/core.rs`.
- [x] T092 [RED] Failing tests asserting each `RelayFetchOutcome`/fetch-summary count follows its own semantic rule, not a single generic id-dedup helper: `dev_fee_events` deduplicated by event id only (raw fetch count); `order_events` deduplicated by event id only (raw fetch count); `unique_orders` applying `src/models/order.rs`'s full qualifying-order procedure (`d`-tag dedup to the highest `created_at`, then the `s=success` filter, per 001 FR-002); `dispute_events` deduplicated by `d` tag via `src/models/dispute.rs`'s classification (NIP-33 replaceable event, latest status wins); `instance_status_found` reflecting `src/models/instance_status.rs`'s actual valid-instance selection (highest `created_at` kind-`38385` event for the node's own pubkey, per 001's Clarifications), not merely whether a kind-`38385` event was fetched — in `src/fetch/filters_summary.rs`.
- [x] T093 [RED] Failing test: two relays both returning the same kind-`8383` dev-fee event id yield `dev_fee_events == 1`, not `2`, per 002 FR-003's event-id dedup rule, in `src/fetch/filters_summary.rs`.
- [x] T094 [GREEN] Implement each `RelayFetchOutcome` count against its own semantic rule per 002 FR-003, 001 FR-002/FR-006/FR-012 in `src/fetch/filters_summary.rs`, consuming `src/models/order.rs`, `src/models/dispute.rs`, and `src/models/instance_status.rs` rather than a single generic id-dedup helper.
- [x] T095 [RED] Failing test: exit `4` when zero dev-fee/order/dispute/instance-status events are usable.
- [x] T096 [GREEN] Wire `AppError::NoUsableEvents` (exit `4`) into `src/lib.rs`'s `run()`.
- [x] T097 [RED] Failing tests: (1) `src/fetch/client.rs`'s real `EventSource` implementation issues all four filters (`8383`/`38383`/`38385`/`38386`) from `src/fetch/filters_summary.rs`, replacing PR 1's original two-filter query; (2) an integration-level test proving a node whose only usable data is a dispute or instance-status event (no dev-fee event, no orders) is fetched successfully and does not trigger `AppError::NoUsableEvents` (exit code `4`).
- [x] T098 [GREEN] Wire `src/fetch/client.rs`'s real `EventSource` implementation to issue all four filters from `src/fetch/filters_summary.rs`, replacing PR 1's original two-filter query.

---

## PR 4 — Core metrics: longevity, cumulative, trade size, liveness, consistency

**Depends on**: 3. Requirements: 001 FR-001..FR-005, FR-010.

- [x] T099 No golden fixture to retire (PR 2 removed `tests/fixtures/` and the golden/characterization test files — see `plan.md`'s PR 1 Amendment). Instead: failing test with 2-3 hand-built synthetic order events (no dev-fee event) asserting `days_active` spans first order to *now*, not first order to *last* order, per 001 FR-001.
- [x] T100 [RED] Failing tests for longevity, `days_active` spanning first order to **now** (fixes PR 1 deviation), in `src/stats/lifecycle.rs`.
- [x] T101 [GREEN] Implement longevity per 001 FR-001 in `src/stats/lifecycle.rs`.
- [x] T102 [RED] Failing tests for cumulative performance in `src/stats/lifecycle.rs`.
- [x] T103 [GREEN] Implement cumulative performance per 001 FR-002 in `src/stats/lifecycle.rs`.
- [x] T104 [RED] Failing tests for trade-size stats (min/max/mean/median, std dev, CV, not-applicable branches) in `src/stats/trade_size.rs`.
- [x] T105 [GREEN] Implement trade-size stats per 001 FR-003, FR-010 in `src/stats/trade_size.rs`.
- [x] T106 [RED] Failing tests for liveness (last trade, 7/30/90-day counts) in `src/stats/lifecycle.rs`.
- [x] T107 [GREEN] Implement liveness per 001 FR-004 in `src/stats/lifecycle.rs`.
- [x] T108 [RED] Failing tests for activity consistency, including the all-zero-activity edge case, in `src/stats/lifecycle.rs`.
- [x] T109 [GREEN] Implement activity consistency per 001 FR-005 in `src/stats/lifecycle.rs`.
- [x] T110 Introduce the `NodeMetrics` struct in `src/stats/mod.rs` with `longevity`, `cumulative`, `trade_size`, `liveness`, and `consistency` fields, assembled from this PR's computations in `src/stats/lifecycle.rs` and `src/stats/trade_size.rs` (pure struct assembly, no new logic to characterize).

---

## PR 5 — Dispute signals

**Depends on**: 3, 4. Requirements: 001 FR-006.

- [x] T111 [RED] Failing tests for dispute counts and `disputes_per_100_trades` (denominator from PR 4, zero-trades not-applicable) in `src/stats/disputes.rs`.
- [x] T112 [GREEN] Implement dispute signals per 001 FR-006 in `src/stats/disputes.rs`, consuming PR 3's dispute dedup/classification.
- [x] T113 Extend `NodeMetrics` in `src/stats/mod.rs` with a `disputes` field, assembled from this PR's dispute signals in `src/stats/disputes.rs` (pure struct assembly, no new logic to characterize).

---

## PR 6 — Descriptive context

**Depends on**: 3, 4. Requirements: 001 FR-008, FR-009, FR-011, FR-012.

- [x] T114 [RED] Failing tests for fiat breakdown (byte-for-byte comparison, ranking, zero-denominator) in `src/stats/context.rs`.
- [x] T115 [GREEN] Implement fiat breakdown per 001 FR-008 in `src/stats/context.rs`.
- [x] T116 [RED] Failing tests for payment-method breakdown in `src/stats/context.rs`.
- [x] T117 [GREEN] Implement payment-method breakdown per 001 FR-009 in `src/stats/context.rs`.
- [x] T118 [RED] Failing tests for premium baseline/dispersion (fewer-than-two-tags not-applicable) in `src/stats/context.rs`.
- [x] T119 [GREEN] Implement premium signal per 001 FR-011 in `src/stats/context.rs`.
- [x] T120 [RED] Failing tests for tri-state bond policy status in `src/models/instance_status.rs`.
- [x] T121 [GREEN] Implement bond policy per 001 FR-012, 002 FR-007 in `src/models/instance_status.rs`, consuming PR 3's instance-status selection.
- [x] T122 Extend `NodeMetrics` in `src/stats/mod.rs` with `fiat_breakdown`, `payment_method_breakdown`, `premium`, and `bond_policy` fields, assembled from this PR's computations in `src/stats/context.rs` and `src/models/instance_status.rs` (pure struct assembly, no new logic to characterize).

---

## PR 7 — Report model, activity grid, recommendations, console renderer

**Depends on**: 4, 5, 6. Requirements: 002 FR-001..FR-008b, FR-013..FR-018.

- [x] T123 No golden fixture to retire (see T099's note). Instead: failing test that renders a hand-built synthetic event set through the console renderer (`src/report/render/console.rs`) and asserts its actual output no longer contains a `TRUST SCORE` line — not just that the report model has no score field, since the model could drop the field while the renderer still prints the old line — per the plan's Complexity Tracking decision to remove it entirely rather than migrate it.
- [x] T124 Remove `calculate_score` and its output entirely (no replacement; relocated to `src/stats/mod.rs` by PR 1's T036; see Complexity Tracking).
- [x] T125 [RED] Failing tests for the 5-section report model + `schema_version`, consuming PR 6's now-complete `NodeMetrics` as the `stats` section's source, in `src/report/model.rs`.
- [x] T126 [GREEN] Implement the report model per 002 FR-001/FR-002/FR-006 in `src/report/model.rs`, populating the `stats` section from PR 6's now-complete `NodeMetrics` (longevity, cumulative, trade_size, liveness, consistency, disputes, fiat_breakdown, payment_method_breakdown, premium, bond_policy).
- [x] T127 [RED] Failing tests for the `fetch` section fields (`relays[]` outcomes, `dev_fee_events`, `order_events`, `unique_orders`, `dispute_events`, `instance_status_found`) in `src/report/model.rs`.
- [x] T128 [GREEN] Assemble the `fetch` summary section per 002 FR-003 in `src/report/model.rs`, sourced from PR 3's `src/fetch/filters_summary.rs::RelayFetchOutcome`.
- [x] T129 Construct fixture event sets spanning weeks/months/years; record observed grid row counts and granularity choices per range in the PR description (evidence for FR-005a, FR-014, 003 FR-006).
- [x] T130 Measure actual relay round-trip latency against the default relay set; record in the PR description (evidence for FR-014).
- [x] T131 [RED] Failing tests for grid bucketing and daily/monthly/yearly auto-granularity at the boundary chosen from T129, in `src/stats/grid.rs`.
- [x] T132 [GREEN] Implement grid bucketing + granularity selection per 002 FR-004/FR-005, 003 FR-006 in `src/stats/grid.rs`, using T129's chosen boundaries.
- [x] T133 [RED] Failing test: FR-005a wide-range warning fires at the chosen boundary.
- [x] T134 [GREEN] Implement the FR-005a warning (stderr only, no JSON field) using the chosen boundary.
- [x] T135 [RED] Failing test: progress indicator appears past the chosen latency threshold, suppressed off-tty/`--quiet`.
- [x] T136 [GREEN] Implement `models/core.rs`'s `ProgressReporter` trait and `report/progress.rs`'s terminal impl per 002 FR-014, using T130's threshold.
- [x] T137 [RED] Failing integration test asserting the bound `ProgressReporter` is invoked when a fetch exceeds T130's threshold, exercising the REAL `fetch::client::EventSource` (not a fake replacing it) with a test-only injectable delay seam beneath its relay-client call (e.g. a `#[cfg(test)]` constructor parameter wrapping the underlying fetch future with a controllable async delay), advanced via `tokio::time::pause`/`advance` (simulated virtual time, no real sleep, no network) — this tests the actual production `EventSource` and its real `ProgressReporter` binding, not a substitute's own reporting behavior, in `tests/metrics_end_to_end.rs`.
- [x] T138 [GREEN] Bind the concrete `report/progress.rs` terminal `ProgressReporter` implementation into `fetch::client`'s real `EventSource` at construction in `main()`/`lib.rs`'s production wiring, per the plan's `models/core.rs` port design; confirm T137's integration test passes through the real `EventSource` with its injected delay seam.
- [x] T139 [RED] Failing tests for the empty-activity-grid case (zero orders) in `src/stats/grid.rs`.
- [x] T140 [GREEN] Implement the empty-grid case per 002 FR-005 Edge Cases.
- [ ] T141 **DEFERRED**: Compute `premium_dispersion_percent` and trade-size CV across T129's fixtures; pick each recommendation boundary from the observed distribution; record in the PR description. Deferred because a defensible boundary needs either real historical data from an active Mostro node or a carefully-justified constructed comparison; premium dispersion in particular has no unit-less statistical convention to lean on (it is a raw population standard deviation in percentage points, not a ratio, per spec 001's own clarification), unlike trade-size CV which can at least start from general coefficient-of-variation conventions. Pick this back up once real-node evidence is available, tracked separately from the rest of PR 7c.
- [x] T142 [RED] Failing tests for the 3 deterministic recommendation triggers (zero trades, disputes present, bond policy not enabled) in `src/report/content.rs`.
- [x] T143 [GREEN] Implement the 3 fixed triggers per the plan's table in `src/report/content.rs`.
- [ ] T144 **DEFERRED** (depends on T141): Failing tests for the 2 threshold-based triggers (premium dispersion, trade-size CV) at T141's boundaries, asserting FR-008a's comparative-language restriction.
- [ ] T145 **DEFERRED** (depends on T141): Implement the 2 threshold-based triggers in `src/report/content.rs`, using T141's boundaries.
- [x] T146 [RED] Failing test: `nothing_notable` true only when none of the 3 currently-implemented triggers fire (zero trades, disputes present, bond policy not enabled); extending this to the 2 deferred threshold-based triggers is follow-up work once T141/T144/T145 land.
- [x] T147 [GREEN] Implement `nothing_notable`/`items` assembly per 002 FR-008 in `src/report/content.rs`, scoped to the 3 implemented triggers.
- [x] T148 [RED] Failing `insta` snapshot tests for the console renderer's 5 sections in `tests/report_snapshots.rs`. **Later removed** (PR 8): the maintainer decided against committing generated `insta` snapshot files to the repository; `insta` and every snapshot-based test (console and plain-text) were removed, keeping only the non-snapshot assertions that survive on their own merits.
- [x] T149 [GREEN] Implement the width-adaptive, colored console renderer (`comfy-table`) per 002 FR-013 in `src/report/render/console.rs`.
- [x] T150 [RED] Failing test for the tty/`NO_COLOR`/`TERM=dumb` color policy in `src/report/format.rs`.
- [x] T151 [GREEN] Implement the color policy per 002 FR-015 (`anstream`/`anstyle`) in `src/report/format.rs`; remove `colored` from `Cargo.toml`.
- [x] T152 [RED] Failing test for thousands separators and relative-time/not-applicable rendering in `src/report/format.rs`.
- [x] T153 [GREEN] Implement formatting helpers per 002 FR-018 in `src/report/format.rs`.

---

## PR 8 — Plain-text and JSON renderers, fatal envelope

**Depends on**: 7. Requirements: 002 FR-009..FR-012a.

- [x] T154 [RED] Failing `insta` snapshot tests for the plain-text renderer in `tests/report_snapshots.rs`. **Later removed**, see T148's note: no generated snapshot files are committed to this repository.
- [x] T155 [GREEN] Implement the plain-text renderer per 002 FR-009 in `src/report/render/plain.rs`.
- [x] T156 [RED] Failing tests for context-based format auto-selection (tty vs. piped) in `src/report/render/mod.rs`.
- [x] T157 [GREEN] Implement the `Renderer` trait + format auto-selection per 002 FR-010 in `src/report/render/mod.rs`.
- [x] T158 [RED] Failing tests for the JSON top-level shape (`schema_version`, `generated_at`, `node`, `fetch`, `activity`, `stats`, `recommendations`, `metric_definitions`, all keys always present) in `src/report/render/json.rs`.
- [x] T159 [GREEN] Implement the JSON top-level shape per the plan's contract in `src/report/render/json.rs`.
- [x] T160 [RED] Failing test: `metric_definitions`'s key set matches the plan's exhaustive list exactly — all `stats` sub-object fields, `activity.granularity`, `buckets[].median_trade_sats`, `fetch.relays[].status`, `fetch.relays[].error` — no more, no fewer.
- [x] T161 [GREEN] Author the static `metric_definitions` table (`label`/`meaning`/`unit_and_direction`) in `src/report/model.rs`.
- [x] T162 [RED] Failing tests for `MetricValue::Computed(T) | NotApplicable` `null` serialization, guarding against NaN/infinity, in `src/models/core.rs`.
- [x] T163 [GREEN] Ensure every `stats/` computation returns `NotApplicable` at its guard rather than a degenerate float.
- [x] T164 [RED] Failing test for the fatal-error envelope shape and the 5 `code`/exit-code pairs in `src/error/exit_code.rs`.
- [x] T165 [GREEN] Implement the fatal-error envelope in `src/report/render/json.rs`, `error.relays` populated only for `relays_unreachable`.
- [x] T166 [RED] Failing test: explicit `--format` overrides the context-based default.
- [x] T167 [GREEN] Wire explicit-format override ahead of the default in `src/cli/options.rs` (call-site skeleton; full flag wiring in PR 9).

---

## PR 9 — CLI flags: identity, format, color, quiet, help, version

**Depends on**: 8. Requirements: 003 FR-001..FR-003, FR-010..FR-013a.

- [x] T168 [RED] Failing tests for `--pubkey`/`MOSTRO_SCORE_PUBKEY` and `--relays`/`MOSTRO_SCORE_RELAYS` precedence in `src/cli/options.rs`. **Implemented as binary-level tests in `tests/cli_flags.rs`** instead: precedence here is entirely native `clap` `env`/`default_value` behavior (no custom resolution code to unit-test in isolation), so the RED/GREEN cycle is proven end to end through the real binary.
- [x] T169 [GREEN] Implement the precedence chain per 003 FR-001..FR-003 in `src/cli/options.rs`. **Implemented as `env = "MOSTRO_SCORE_PUBKEY"`/`env = "MOSTRO_SCORE_RELAYS"` attributes on `Args` in `src/cli/args.rs`** — clap's own flag > env > default precedence, zero custom code.
- [x] T170 [RED] Failing tests validating `--relays`/`MOSTRO_SCORE_RELAYS` well-formedness before any connection attempt, covering both the flag and the environment-variable path, expecting an actionable message naming the malformed relay string and exit code `2` (003's `--relays`/environment-variable Edge Case, FR-002/FR-003, FR-013a) — in `src/cli/options.rs`. No golden fixture to retire (see T099's note); this intentionally replaces the malformed-relay case's current behavior of folding into `AppError::RelaysUnreachable` (exit `3`, via `RelayEventSource::connect()`'s `add_relay`-failure handling) with the actionable usage-error/exit-`2` behavior. Covered by unit tests in `src/cli/options.rs` plus binary-level tests (both flag and env path) in `tests/cli_flags.rs`; `tests/error_handling.rs`'s pre-existing malformed-relay regression test updated to assert the new exit-`2` behavior.
- [x] T171 [GREEN] Implement pre-connection well-formedness validation for `--relays`/`MOSTRO_SCORE_RELAYS` in `src/cli/options.rs`, rejecting a malformed value with an actionable message and exit code `2` before `Client::add_relay` is ever called.
- [x] T172 [RED] Failing tests for `--format`, `--color`/`--no-color` mutual exclusion, `--quiet` validation in `src/cli/options.rs`.
- [x] T173 [GREEN] Implement resolution/validation per 003 FR-010..FR-013a in `src/cli/options.rs`.
- [x] T174 [RED] Failing test: `--quiet` suppresses the two transient status lines (`"Connected to relays. Fetching history..."` and `"Fetched {N} events. Analyzing..."`, routed to `err` in T072/T073) while required report content on `out` is unaffected, in `src/cli/options.rs`. **Implemented as an integration test in `tests/cli_flags.rs`** exercising `mostro_score::run()` directly, since the suppression itself lives in `run()` (`src/lib.rs`), not in `cli::options`.
- [x] T175 [GREEN] Wire `--quiet` to suppress those two transient status lines per 003 FR-012 in `src/report/render/console.rs`. **Implemented in `src/lib.rs`'s `run()`**, where those two `writeln!` calls actually live — `console.rs` never contained them.
- [x] T176 [RED] Failing binary-level test: `--help`/`--version` exit `0`.
- [x] T177 [GREEN] Wire `--help`/`--version` via clap derive in `src/cli/args.rs`.

---

## PR 10 — Time range and grouping

**Depends on**: 9 (independent of PR 11). Requirements: 003 FR-004..FR-007.

- [ ] T178 [RED] Failing tests for ISO 8601 and `Nd`/`Nmo`/`Ny` shorthand (end-of-month clamping) in `src/cli/duration.rs`.
- [ ] T179 [GREEN] Implement `--since`/`--until` parsing per 003 FR-004 in `src/cli/duration.rs`.
- [ ] T180 [RED] Failing test: `--since` later than `--until` is a usage error (exit `2`).
- [ ] T181 [GREEN] Wire the cross-validation into `src/cli/options.rs`.
- [ ] T182 [RED] Failing tests for one-sided `--since`/`--until` range resolution: `--since` alone defaults `--until` to the report-generation instant, `--until` alone defaults `--since` to earliest history, and omitting both resolves to full history, in `src/cli/options.rs`.
- [ ] T183 [GREEN] Implement one-sided `--since`/`--until` default resolution per 003 FR-004 in `src/cli/options.rs`.
- [ ] T184 [RED] Failing tests for `--view` overriding PR 7's granularity mechanism in `src/cli/options.rs`.
- [ ] T185 [GREEN] Wire `--view` per 003 FR-006/FR-007 into `src/stats/grid.rs` (no new threshold).
- [ ] T186 [RED] Failing test: an explicitly-supplied `--since`/`--until` that does not align to a calendar month/year boundary is rejected as a usage error when `--view monthly`/`--view yearly` is explicit, in `src/cli/options.rs`.
- [ ] T187 [GREEN] Implement the explicit-`--view` boundary-rejection rule per 003 FR-006 in `src/cli/options.rs`.
- [ ] T188 [RED] Failing test: a misaligned range is snapped, not rejected, to the calendar month/year boundary when the monthly/yearly granularity comes from config or automatic selection rather than an explicit `--view`, in `src/stats/grid.rs`.
- [ ] T189 [GREEN] Implement boundary snapping for the non-explicit-`--view` path per 003 FR-006 in `src/stats/grid.rs`.
- [ ] T190 [RED] Failing test: an empty or inverted `--since`/`--until` range stays empty after monthly/yearly boundary snapping, in `src/stats/grid.rs`.
- [ ] T191 [GREEN] Preserve the empty range through snapping per 003 FR-006 in `src/stats/grid.rs`.
- [ ] T192 [RED] Failing test: FR-005a warning fires when `--since`/`--until` exceed PR 7's chosen boundary.
- [ ] T193 [GREEN] Wire `--since`/`--until` into PR 7's existing warning mechanism.
- [ ] T194 [RED] Failing test comparing a `--since`/`--until`-scoped activity grid against an unscoped grid over the same fixture event set: the scoped grid includes only buckets within range, while spec 001's lifetime metrics (longevity, cumulative, trade size, liveness) remain unaffected by the flag.
- [ ] T195 [GREEN] Wire the resolved `--since`/`--until` range into `src/stats/grid.rs`'s bucket construction so only in-range buckets are built; confirm `src/stats/lifecycle.rs` and `src/stats/trade_size.rs` are untouched by this wiring.

---

## PR 11 — Section filtering

**Depends on**: 9 (independent of PR 10). Requirements: 003 FR-008, FR-009.

- [ ] T196 [RED] Failing tests for `--sections` token parsing/validation in `src/cli/options.rs`.
- [ ] T197 [GREEN] Implement `--sections` per 003 FR-008/FR-009 in `src/cli/options.rs`.
- [ ] T198 [RED] Failing tests: console/plain honor `--sections`, JSON always emits all 5 keys.
- [ ] T199 [GREEN] Implement console/plain-only filtering in `src/report/content.rs`.

---

## PR 12 — Persisted configuration, `--init-config`, `--config-dir`

**Depends on**: 9, 10. Requirements: 003 FR-014..FR-019.

- [ ] T200 [RED] Failing tests for the TOML schema, including a persisted `view` value, in `src/config/file.rs`.
- [ ] T201 [GREEN] Implement the TOML schema/parsing in `src/config/file.rs`.
- [ ] T202 [RED] Failing tests for warn-and-ignore degradation on a malformed config file — parse failure — (003 FR-015a).
- [ ] T203 [GREEN] Implement warn-and-ignore degradation for parse failures in `src/config/file.rs`.
- [ ] T204 [RED] Failing test: an unreadable config file (I/O failure, e.g. permission denied) is warned about and the entire file is ignored, falling back to compiled defaults, in `src/config/file.rs`.
- [ ] T205 [GREEN] Implement warn-and-ignore degradation for I/O failures per 003 FR-015a in `src/config/file.rs`.
- [ ] T206 [RED] Failing test: a `config.toml` that simply does not exist (the normal first-run case) produces zero warnings on `err` and falls through silently to the env-var/compiled-default chain, per 003 FR-015 — distinct from T202/T204's malformed-parse and unreadable-file cases (FR-015a), which do warn — in `src/config/file.rs`.
- [ ] T207 [GREEN] Implement the missing-file-is-silent branch per 003 FR-015 in `src/config/file.rs`, so a `FileNotFound` result alone never reaches the warn-and-ignore path T203/T205 implement for malformed or unreadable files.
- [ ] T208 [RED] Failing test: a config file with valid TOML syntax but a semantically-invalid value warns and ignores the entire file, never applying valid keys alongside the invalid one, in `src/config/file.rs`.
- [ ] T209 [GREEN] Implement the all-or-nothing fallback for semantically-invalid values per 003 FR-015a in `src/config/file.rs`.
- [ ] T210 [RED] Failing tests for platform config-directory resolution (Linux/macOS/Windows) in `src/config/paths_defaults.rs`.
- [ ] T211 [GREEN] Implement path resolution per 003 FR-014 (`directories`), alongside compiled defaults, in `src/config/paths_defaults.rs`.
- [ ] T212 [RED] Failing tests for `-d`/`--config-dir` overriding the platform-default config-directory path in `src/config/paths_defaults.rs`.
- [ ] T213 [GREEN] Implement the `--config-dir` override per 003 FR-014 in `src/config/paths_defaults.rs`, taking precedence over T211's platform-default resolution.
- [ ] T214 [RED] Failing test: `--init-config --config-dir <path>` writes the config file to the overridden location, not the platform default.
- [ ] T215 [GREEN] Wire `--config-dir` into `src/config/init.rs`'s `--init-config` scaffolding so it writes to the overridden path.
- [ ] T216 [RED] Failing binary-level test: `--force` without `--init-config` is validated after `--format` resolution, so `--format json --force` produces a JSON fatal envelope (exit `2`), not a plain-text usage error.
- [ ] T217 [GREEN] Wire the `--force`-without-`--init-config` check into `src/cli/options.rs` to run after format resolution, so `AppError::UsageError`'s JSON rendering respects the resolved `--format` (per 003 FR-013a's caveat that only the parser's own errors bypass `--format`).
- [ ] T218 [RED] Failing tests for `--init-config` scaffolding, overwrite refusal, `--force` in `src/config/init.rs`.
- [ ] T219 [GREEN] Implement `--init-config`/`--force` per 003 FR-017..FR-019 in `src/config/init.rs`.
- [ ] T220 [RED] Failing test: `--init-config` takes precedence over and short-circuits report generation entirely when combined with report-generating flags (e.g. `--init-config --pubkey <x>`, or `--init-config` alongside an otherwise-invalid report-scoped flag), in `src/cli/options.rs`.
- [ ] T221 [GREEN] Implement `--init-config` precedence per 003 FR-019 in `src/cli/options.rs`, short-circuiting before report-flag validation.
- [ ] T222 [RED] Failing test: config-file precedence sits between env and compiled defaults.
- [ ] T223 [GREEN] Wire config-file values into `src/cli/options.rs`'s precedence chain.

---

## Dependencies & Execution Order

Matches the plan's delivery table exactly:

```
PR 1 → PR 2 → PR 3 → PR 4 → PR 5 ┐
                          └→ PR 6 ┘→ PR 7 → PR 8 → PR 9 → PR 10 ┐
                                                          └→ PR 11 ┘
                                                   PR 9, PR 10 → PR 12
```

- PR 2 depends on 1. PR 3 depends on 1, 2. PR 4 depends on 3. PR 5 depends on 3, 4. PR 6 depends on 3, 4. PR 7 depends on 4, 5, 6. PR 8 depends on 7. PR 9 depends on 8. PR 10 depends on 9. PR 11 depends on 9. PR 12 depends on 9, 10.
- PR 5 and PR 6 may be reviewed in either order once PR 4 merges. PR 10 and PR 11 may be reviewed in either order once PR 9 merges.
- Within PR 1, Steps -1 → 0 → A → B → C → D → D2 are strictly sequential; T012-T016 (Step A) and T002-T008 (Step -1 scenarios) are internally parallelizable, and Step D2's seven scenario pairs (T044-T057) are internally parallelizable once Step D's T041-T043 are green.
- Within every PR 2-12, RED precedes its paired GREEN task; GREEN tasks across different files may run in parallel once their RED counterpart is red.

## Not in scope for this task list

- CI coverage gate (coverage is measured, never CI-enforced, per the plan's Testing Strategy).
- Any numeric threshold value not yet chosen — PR 7's tasks gather evidence and implement at the chosen boundary; no number is fixed here.
- `book/` documentation, shell completions, the user manual, kind 38384 rating events, and any new metric — all explicitly out of scope per the plan.
