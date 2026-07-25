# Implementation Plan: Phase 4 — Modular Implementation of the Reputation CLI

**Branch**: `004-phase-4-implementation` (work lands as a series of feature branches off `development`)

**Date**: 2026-07-24

**Spec**: none — see "Why this phase has no spec.md" below

**Input**: The complete, already-ratified requirement set of three merged specs:

- `specs/001-node-reputation-metrics/spec.md` — the metric definitions (FR-001 through FR-015)
- `specs/002-cli-report-design/spec.md` — the report structure and output formats (FR-001 through FR-019)
- `specs/003-cli-parameters/spec.md` — the CLI flags, defaults, and persisted configuration (FR-001 through FR-019)

## Summary

Phase 4 turns three finished specifications into working code, closing the one outstanding
constitutional debt this repository carries: `src/main.rs` is still a single 547-line monolith that
predates Principle III.

The approach is two-staged. First, a single behavior-preserving refactor extracts today's monolith
into the seven single-purpose modules the constitution names (`cli`, `fetch`, `models`, `stats`,
`report`, `config`, `error`) plus a library target, guarded by characterization tests that pin the
existing output. Only then does new functionality land, one pull request per user story or
functional area from specs 001-003, each built on the modular structure with its tests written
alongside the code.

Splitting the refactor from the feature work is the central decision of this plan: a combined
"rewrite while adding features" pull request would be unreviewable, since a reviewer could not tell a
deliberate metric change from an accidental regression. Separating them makes the refactor
mechanically verifiable (same input, same output) and every subsequent pull request a small diff
against a stable structure.

### Why this phase has no spec.md

Phases 1 through 3 produced the requirements; Phase 4 produces the implementation. There is no new
user-facing requirement to specify, so a fourth `spec.md` would only restate specs 001-003 and create
a second source of truth that can drift from them.

This follows the constitution's phased-delivery allowance: the ratified sequence may be reordered
when documented in the artifact itself (Phase 1's checklist used the same allowance to run before its
`plan` step). The `specify` and `clarify` steps are not skipped — they were completed in Phases 1-3,
and their outputs are this plan's Input. Recorded in Complexity Tracking below.

## Technical Context

**Language/Version**: Rust, 2021 edition. CI pins the toolchain to 1.94.0 (`.github/workflows/ci.yml`),
matching `mostro-core`; that pin is the effective MSRV for this phase.

**Primary Dependencies**:

Already in `Cargo.toml` and kept: `nostr-sdk` 0.43 (relay access, event types), `mostro-core` 0.7.0
(shared Mostro domain types, notably `Status`), `tokio` 1 (async runtime), `clap` 4.4 with the derive
API (argument parsing), `chrono` 0.4 (UTC calendar-day arithmetic), `serde` + `serde_json` 1.0 (JSON
output), `log` + `env_logger` (diagnostics).

Added in this phase, each tied to a requirement that cannot be met without it:

| Crate | Requirement it serves | Alternative rejected |
|-------|----------------------|----------------------|
| `thiserror` | Typed error taxonomy behind 002 FR-019's six exit codes | `anyhow`: erases the variant, and the exit code must be derived from the variant |
| `toml` | 003 FR-016a's configuration file schema | none credible; TOML is fixed by 003's Assumptions |
| `directories` | 003 FR-014's platform-standard config path | hand-rolled `$XDG_CONFIG_HOME`/`%APPDATA%` logic: three platforms of edge cases for no gain |
| `comfy-table` | 002 FR-013's render-within-actual-terminal-width | `tabled`: dynamic width arrangement is a first-class feature in `comfy-table` |
| `indicatif` | 002 FR-014's progress indicator, suppressed off-tty | hand-rolled spinner: needs its own tty detection and redraw handling |
| `anstream` + `anstyle` | 002 FR-015's color policy including the `TERM=dumb` exception | `colored` 2 (currently used): honors `NO_COLOR` but has no `TERM=dumb` concept, and `clap` 4 already pulls `anstream` in, so keeping `colored` means shipping two color stacks |

`colored` stays in the dependency list until the renderer pull request replaces it, then is removed.

Deliberately not added: a `num-format`-style crate for 002 FR-018's thousands separators (a dozen
lines in `report/format.rs`, and the separator must never touch JSON), and any relative-duration
parsing crate for 003 FR-004 (`30d` means `N - 1` days back while `1mo`/`1y` are calendar units with
end-of-month clamping — bespoke semantics no general parser implements).

**Storage**: None for report data; every run is a read-only query against Nostr relays with no local
cache or database. The single persisted artifact is the optional TOML configuration file at the
platform config path (003 FR-014), read at startup and written only by `--init-config`.

**Testing**: `cargo test`, with unit tests in `#[cfg(test)]` modules beside the code they cover and
integration tests in `tests/` against the library target. `assert_cmd` + `predicates` for binary-level
behavior (exit codes, stdout/stderr separation). `insta` snapshots for the three renderers, so a
formatting change to any of the five sections shows up as a reviewable diff. `cargo llvm-cov` measures
the constitution's ≥50% coverage floor.

**Target Platform**: Cross-platform command-line binary — Linux, macOS, and Windows, as implied by
003 FR-014's three-platform config path requirement.

**Project Type**: Single Cargo package producing two targets: a library crate (`src/lib.rs`) holding
all seven modules, and a thin binary crate (`src/main.rs`) that only wires and dispatches. The library
target is not optional: Rust integration tests in `tests/` cannot import a binary-only crate, so
without it the coverage requirement could only be met by inline unit tests, and end-to-end behavior
would go untested.

**Performance Goals**: End-to-end runtime is dominated by the relay fetch, not computation. The
existing 10-second per-fetch timeout is preserved as a configurable constant. Post-fetch, every metric
is computed from a single pass over the deduplicated order set, with sorting (O(n log n)) only where a
median or percentile requires it; no metric may re-scan the full event set per output section. Target:
a node with ~10,000 events completes in well under a second on ordinary hardware.

**Constraints**:

- No panics or raw errors on any user-facing path (Principle VI, 002 FR-011). `unwrap`/`expect` are
  permitted only in tests.
- Report content to stdout, every diagnostic and warning to stderr (002 FR-017). The current code
  violates this: its `SAMPLE EVENTS` and `DEBUG INFORMATION` blocks print to stdout.
- Output must be deterministic for identical event input, independent of the host's local timezone —
  every date bucket is a UTC calendar day (001 FR-005, 002 FR-005).
- One failed relay among several that succeeded is a warning, not a failure; exit code `3` requires
  all relays to fail (002 FR-019).

**Scale/Scope**: One node analyzed per invocation. Event volumes observed in practice are in the
thousands per node; the design must not degrade badly at tens of thousands, but no paging,
streaming, or persistence layer is warranted at this scale.

## Constitution Check

*GATE: evaluated against `.specify/memory/constitution.md` v1.2.0.*

| Principle | Status | How this plan satisfies it |
|-----------|--------|---------------------------|
| **I. Evidence-Based Metrics** | Pass, with one flagged carry-over | Every metric implemented in `stats/` maps to a numbered requirement in spec 001, which already records its event kind, tag, and formula. This plan introduces no metric of its own. The one exception is the legacy `calculate_score` trust score in today's `main.rs`, which has no spec backing at all — see Complexity Tracking. |
| **II. Statistical Robustness** | Pass | `stats/trade_size.rs` computes the median as the primary typical-trade-size figure per 001 FR-003; the mean is retained as a secondary figure only. The report labels the median as the reference value, and the activity grid's typical-size column is the median (002 FR-004). |
| **III. Modular Architecture** | **Pass — this phase is what closes the gap** | See below. |
| **IV. Test-First Development** | Pass | Tests are written before or alongside the code, never deferred. Coverage floor ≥50% via `cargo llvm-cov`; CI enforcement of that floor is a deliberately deferred decision (see Testing Strategy). The refactor pull request is strictest: characterization tests must exist and pass against the current monolith *before* a single line is moved. |
| **V. Spec-Driven Development** | Pass, with a documented reordering | This plan is the `plan` step for specs 001-003 collectively, under the constitution's phased-delivery allowance. CI (fmt, clippy, tests) plus human pull request review remain the only verification gates. Recorded in Complexity Tracking. |
| **VI. Graceful Degradation** | Pass | The `error` module owns a typed taxonomy mapped to 002 FR-019's exit codes, with user-facing messages and no stack traces. `fetch` tracks per-relay outcomes and continues on partial failure. A malformed config file warns and is ignored entirely, never fatal (003 FR-015a). Malformed individual events are silently excluded, not reported as errors (001 FR-013, 002 FR-011). |
| **VII. English-Only Artifacts** | Pass | All identifiers, messages, and artifacts in English; comments reserved for non-obvious constraints. |

### Principle III in detail — the gate this phase exists to close

The constitution's transition note requires `src/main.rs` to be brought into compliance no later than
Phase 4. This is that phase.

Current state, measured: `src/main.rs` is the only file under `src/`, 547 lines holding `clap`
argument definitions, the `MostroStats` domain struct, relay client setup, Nostr filter construction,
event fetching, tag parsing, order deduplication, five statistical computations, all console
formatting and coloring, and an unspecified trust score — data fetching, domain modeling,
computation, and presentation interleaved line by line inside a single 385-line `main` function.

How this plan closes it:

1. The first pull request (see Delivery Strategy) is a pure extraction into exactly the seven modules
   the principle names, with no behavior change. After it merges, `main.rs` contains only argument
   parsing, a call into the library, and exit-code mapping.
2. The dependency direction is fixed and acyclic: `error` depends on nothing; `models` depends only
   on `error` and the Nostr event types; `stats` depends on `models` and is pure, with no I/O;
   `fetch` depends on `models` and `error` and is the only module performing network I/O; `report`
   depends on `models` and `stats` and is the only module performing terminal output; `config` and
   `cli` depend on `error`; the library root wires them together. No reverse edge is permitted.
3. This direction enforces the separation the principle asks for: `stats` cannot reach the network
   because it does not depend on `fetch`, and `report` cannot recompute a metric because the
   computation lives behind `stats`'s public surface. Both properties are testable without a relay,
   which is also what makes Principle IV's coverage floor reachable.
4. Every pull request after the first adds code *inside* these modules. None may reintroduce logic
   into `main.rs`.

The gate is met when the refactor pull request merges; it stays met by the dependency rule above.

## Project Structure

### Documentation (this feature)

```text
specs/004-phase-4-implementation/
├── plan.md              # This file
├── checklists/          # Phase-quality checklist(s)
└── tasks.md             # Next step, not created by this plan
```

No `spec.md`, `research.md`, `data-model.md`, or `contracts/` for this phase: the requirements live
in specs 001-003, the data model is spec 001's Key Entities section, and the CLI contract is spec
003's flag definitions. Duplicating them here would create a drift risk with no reader benefit.

### Source Code (repository root)

```text
src/
├── main.rs                      # Binary target: parse args, call run(), map error to exit code
├── lib.rs                       # Library target: module wiring, the run() entry point
├── cli/
│   ├── mod.rs
│   ├── args.rs                  # clap derive struct, value enums (Format, View, Section, ColorMode)
│   ├── duration.rs              # --since/--until: ISO 8601 dates and Nd/Nmo/Ny shorthand (003 FR-004)
│   └── options.rs               # precedence chain (flag > env > config > default) and cross-flag
│                                 # validation (since>until, --color+--no-color, --force alone) — one
│                                 # file, since resolving a flag's value and validating it happen together
├── config/
│   ├── mod.rs
│   ├── paths_defaults.rs        # compiled-in defaults and platform config directory (003 FR-014), both
│                                 # small, static, and looked up together at startup
│   ├── file.rs                  # TOML schema, load with warn-and-ignore degradation (003 FR-015a)
│   └── init.rs                  # --init-config scaffolding, overwrite refusal, --force (003 FR-017..019)
├── fetch/
│   ├── mod.rs
│   ├── client.rs                # relay connection, per-relay outcome tracking, partial-failure policy
│   └── filters_summary.rs       # per-kind filters (8383/38383/38385/38386) and the RelayFetchOutcome
│                                 # dedup-by-id counts they produce (002 FR-003) — paired since the
│                                 # summary is just the filters' fetch result, tallied
├── models/
│   ├── mod.rs
│   ├── core.rs                  # tag accessors (z/y/d/s/amt/f/pm/premium/bond_enabled), author/z/y
│                                 # scoping (001 FR-015) and future-timestamp exclusion (FR-014),
│                                 # MetricValue (Computed(T) | NotApplicable), and the ProgressReporter
│                                 # port (trait). Four small, no-logic-of-their-own types and helpers
│                                 # every other module in this tree depends on — grouped so `fetch` and
│                                 # `report` both reach ProgressReporter through the same shared
│                                 # dependency they already have, not through each other. The concrete
│                                 # reporter instance is bound into fetch::client's EventSource at
│                                 # construction in main()/lib.rs's wiring, not passed as its own run()
│                                 # parameter, so run()'s signature stays EventSource-generic.
│   ├── dedup.rs                 # highest created_at wins, ties by greatest event id (001 FR-002/006/012/014)
│   ├── dev_fee.rs                # kind 8383, the longevity anchor
│   ├── order.rs                  # kind 38383, qualifying-order selection and s=success filter
│   ├── dispute.rs                # kind 38386, dedup by d tag, resolved/active/unknown classification
│   └── instance_status.rs        # kind 38385, selection by d = node pubkey, tri-state bond_enabled
├── stats/
│   ├── mod.rs                   # the aggregate NodeMetrics assembled from the submodules below
│   ├── lifecycle.rs             # 001 FR-001/002/004/005: longevity, cumulative performance, liveness,
│                                 # activity consistency — one file, since all four answer "when and how
│                                 # much has this node traded," each a few lines of pure arithmetic
│   ├── trade_size.rs            # 001 FR-003 and FR-010: min/max/mean/median, population std dev, CV —
│                                 # kept on its own; substantial enough (four figures, two nullability
│                                 # rules) to earn a file the lifecycle group's smaller pieces don't need
│   ├── disputes.rs              # 001 FR-006: ratio per 100 trades, resolved/active/unknown counts
│   ├── context.rs               # 001 FR-008/FR-009/FR-011: fiat and payment-method breakdowns, premium
│                                 # baseline/dispersion — spec 001's own User Story 3 already groups
│                                 # these as descriptive due-diligence context, not core trust signals
│   └── grid.rs                  # 002 FR-004/FR-005: bucketing, granularity auto-selection, FR-005a
│                                 # warning — report/activity-grid logic, not a Phase 1 lifetime metric,
│                                 # kept apart from `lifecycle.rs` for that reason
├── report/
│   ├── mod.rs
│   ├── model.rs                 # the 5-section report model, serde-annotated, schema_version
│   ├── content.rs                # section filtering for console/plain only (003 FR-008) and
│                                 # recommendation synthesis with the baseline discipline of 002 FR-008a
│                                 # — both assemble what the report says before any renderer touches it
│   ├── format.rs                 # thousands separators, relative time, not-applicable rendering,
│                                 # tty/NO_COLOR/TERM=dumb color policy (002 FR-015) — the presentation
│                                 # details every renderer below needs, not specific to any one of them
│   ├── progress.rs              # terminal impl of models::core::ProgressReporter, suppressed off-tty and on --quiet
│   └── render/
│       ├── mod.rs               # Renderer trait, format auto-selection by context (002 FR-010)
│       ├── console.rs           # colored, tabular, width-adaptive (002 FR-013)
│       ├── plain.rs             # same content, one `label: value` line per metric, no decoration
│       └── json.rs              # complete stable structure, null for not-applicable, fatal-error envelope
└── error/
    ├── mod.rs                   # AppError taxonomy, user-facing messages
    └── exit_code.rs             # exit codes 0/1/2/3/4/5 (002 FR-019), JSON error envelope codes

tests/
├── cli_behavior.rs              # binary-level: exit codes, stdout/stderr split, flag validation
├── report_snapshots.rs          # insta snapshots for the console, plain, and JSON renderers
├── metrics_end_to_end.rs        # fixture events in, full metric set out, no network
└── fixtures/                    # serialized Nostr events per scenario (rich node, zero trades,
                                 # disputes, missing tags, future timestamps, multi-relay duplicates)
```

**Structure Decision**: A single Cargo package with a library target plus a thin binary, where the
seven top-level directories under `src/` are exactly the seven module names Principle III mandates —
no more, no fewer, no renames.

Two clarifications about that mapping: the submodules shown inside each directory are internal to
their parent, not additional top-level modules — the principle constrains top-level decomposition,
not how many files each module uses internally, so files inside `stats` and the others are grouped by
what genuinely belongs together (each a few dozen lines answering one question) rather than split one
metric per file, which would be its own kind of fragmentation. And the library/binary split is a Rust
testability mechanism, not an eighth module: `main.rs` holds no logic, `lib.rs` holds only the wiring
and the `run()` entry point.

Files under `tests/` are integration tests against the library's public surface. Unit tests live in
`#[cfg(test)]` modules inside the file they cover, per Rust convention.

## Delivery Strategy

Implementation lands as granular, independently reviewable pull requests off `development`, never as
one large change. Every pull request must leave the tool working end to end, must pass CI (fmt,
clippy, tests), and must carry its own tests in the same diff.

### Pull request 1 — Modularization, no behavior change

This is a prerequisite for everything else and merges before any feature work starts.

Scope: extract today's `src/main.rs` into the seven modules and the library target. Move code; do not
improve it. Known deviations from specs 001-003 that exist in the current implementation are
preserved verbatim here and corrected in the later pull request that owns the relevant requirement.

Tests: characterization tests only, pinning current behavior so the extraction is provably
output-identical. The surface to pin is not uniformly testable as-is, so this pull request proceeds
in five ordered steps: capture a golden baseline from the untouched binary, wrap, two rounds of
extraction, then the module move — with the wrap and every step after it re-proved against that one
baseline.

**Step -1 — capture the golden baseline before a single line changes.** Steps A and B pin individual
functions, not section ordering, stdout/stderr routing, or top-level wiring — a wiring mistake can
pass every helper-function test while still changing what the binary prints. Only a golden-output
comparison against today's untouched code catches that, taken before Step 0's wrap: a baseline
captured after the wrap would only prove later steps preserved the *wrapped* code's behavior, leaving
the wrap itself — the step that touches every line of `main()` — unproven.

So this pull request's first action changes no source: build the current binary and run it once, as a
process, against a real relay round trip, with `--pubkey` set to a dedicated test node whose event
history is small and fully settled (older than the 90-day rolling window and the 30-day consistency
window, so no printed value sits near a boundary the clock could cross between runs). No fake relay is
used, since the point is to observe the shipped binary's real behavior. Three artifacts are recorded:

- **The exact event set the run fetched**, serialized into `tests/fixtures/` and replayed as canned
  input by every test from Step 0 onward, obtained by issuing the same two filters
  (`src/main.rs:68-78`) against the same relays in the same session from a throwaway capture harness
  that does not touch `main.rs`. Step 0's test passes only if replaying it reproduces the original
  run's output byte for byte.
- **The `now` value** to substitute for `src/main.rs:272`'s `chrono::Utc::now()`, recorded as the
  wall-clock second immediately before the process launched (the real call happens mid-run and cannot
  be observed without instrumenting the binary). Verified, not assumed: replays using the pre-launch
  and post-exit seconds must produce identical output, or the capture is discarded and redone.
- **The run's stdout and stderr, byte for byte, plus exit status**, captured across every distinct
  top-level outcome the binary has, since each can change exit status or output routing independently:
  (1) the settled-history node above, the primary capture the event set and `now` belong to; (2) a
  malformed `--pubkey`, its own invocation since that branch stays in `main()` and the in-process
  golden test cannot reach it; (3) a syntactically malformed `--relays` value, pinning
  `client.add_relay(relay).await?`'s parse-time failure (`src/main.rs:59-60`), a different code path
  than a well-formed but unreachable address; (4) `--relays` pointed at a well-formed but unreachable
  address (a reserved, non-routable address or closed local port), pinning `src/main.rs:63-86`'s
  connect/fetch-error path once relay setup succeeded; (5) a fresh, never-published keypair, pinning
  the zero-fetched-events path at `src/main.rs:282-284`; (6) a pubkey with qualifying orders but no
  qualifying dev-fee event, pinning the no-dev-fee fallback at `src/main.rs:142-160`; (7) a pubkey with
  a qualifying dev-fee event but zero qualifying orders — distinct from (6) and (5): `first_dev_fee_ts`
  is `Some`, so the early `"No events found."` return at `src/main.rs:283-284` is never reached, and
  the run falls through to a full report with every order-derived section empty; (8) two configured
  relays, one reachable and one not, distinct from (4): the constitution's graceful-degradation rule
  requires this to warn and still produce a successful report from the relay that succeeded, a path
  (4)'s all-relays-unreachable case does not exercise. Locating or constructing real nodes with
  properties (5)-(8) is part of this step's setup work.

These artifacts are the one and only golden reference for the rest of this pull request: Step 0,
Steps A through D, and the module move all compare against them; no later step takes a fresh capture
from already-modified code.

**Step 0 — wrap before splitting, to get a callable seam for replaying the baseline.** Today's
`main()` cannot be replayed against the golden reference: it is 385 lines of inline logic, not a
function a test can invoke with substituted inputs, and its three side-effecting dependencies — the
relay query, the clock read, and the `println!`/`eprintln!` calls — are hardcoded into the body.

So, before any extraction, the first code change is a single mechanical wrap: extract today's
`main()` body verbatim into a new, ordinary (not `#[cfg(test)]`-gated) private function at module
scope in `main.rs`, callable by production `main()` in a normal `cargo build` — only its *test*, in a
`#[cfg(test)] mod tests` block in the same file, is test-only. This stays a same-crate unit test
rather than a `tests/` integration test because the wrapped function is still local to the binary
crate. Five parameters replace its one caller-supplied input and its three hidden dependencies (the
writers count as two, one per stream):

- The node's public key (`public_key: PublicKey`, the `nostr_sdk` type `main.rs` already parses
  `--pubkey` into at `src/main.rs:44`), used to print the identity header (`src/main.rs:52-53`) and
  author-scope both Nostr filters (`src/main.rs:70,77`). Parsing stays in `main()`, so the
  invalid-pubkey branch (`src/main.rs:44-50`) stays out of the wrapped function and is pinned by
  Step -1's second capture at binary level instead.
- An event source (`event_source: E where E: EventSource`, matching the fetch surface
  `fetch::client` will later implement) in place of the in-line relay client and query. `main()`'s
  fetch calls are all `.await`ed (`src/main.rs:59-85`), so `EventSource::fetch` is `async fn` and the
  wrapped function becomes `async fn` too, called from `main()`'s existing `#[tokio::main]` runtime —
  the one place this step is not a purely mechanical unindent. `EventSource` is a generic bound, not a
  `&dyn EventSource` trait object, since stable Rust's async-fn-in-traits needs boxing or `async-trait`
  for dynamic dispatch, and with only two implementations, static dispatch needs neither.
- A clock closure (`now: &dyn Fn() -> DateTime<Utc>`) in place of the in-line `chrono::Utc::now()`
  call, invoked at *exactly the same point* the original call occupies today (`src/main.rs:272`,
  after fetching and the per-event aggregation loop). Sampling any earlier would be a real behavior
  change: a fetch straddling a rolling-window or relative-time boundary would compute differently.
- Two writer parameters (`out: &mut impl std::io::Write`, `err: &mut impl std::io::Write`) in place
  of the direct `println!`/`eprintln!` calls, which become `writeln!(out, ...)`/`writeln!(err, ...)`.
  An injected buffer is what gives a test something to capture, since `println!`/`eprintln!` write
  straight to the process's real stdout/stderr.

`main()` itself shrinks to argument parsing (including `--pubkey` into `PublicKey`), constructing the
real relay-backed `EventSource` implementation, wiring `chrono::Utc::now` as the clock closure, and
`.await`ing the wrapped async function with `io::stdout()`/`io::stderr()` as the writers — already
the shape the Structure Decision requires of the final `main.rs`, reached one pull request early,
with the wrapped function still physically inside `main.rs`. This wrap changes zero logic: it is the
existing code, unindented, with its three side-effecting calls replaced by parameters.

The wrap is proved immediately, before Steps A and B touch anything. A same-crate unit test calls the
wrapped function with the test node's `PublicKey`, Step -1's fixture event set behind a fixture
`EventSource` (returns canned events instead of querying relays), a clock closure returning Step -1's
recorded `now`, and two `Vec<u8>` buffers as the writers, then asserts the buffers match Step -1's
captured stdout and stderr. One block needs normalizing before comparison: `s_tag_distribution`
(`src/main.rs:167,228-232`) is a `HashMap`, so its printed order varies between runs from Rust's
randomized hash seed, independent of any code change — Step -1's capture and every later comparison
sort that block's entries by key first. This is the only proof that the wrap itself is
behavior-preserving; no later test can retroactively cover it.

**Step A — write tests directly against what is already a standalone function.**
`compute_trade_stats` (min/max/mean/median), `compute_rolling_windows` (7/30/90-day counts),
`compute_activity_consistency` (active days and max gap), `format_relative_time` (every branch,
including the future-timestamp case), and `calculate_score` are already free functions in `main.rs`
(no network dependency, unaffected by Step 0's wrap), so their characterization tests are written
directly against the current code, before any module file is touched.

**Step B — extract a seam, then test, then move.** Order deduplication by `d` tag, dev-fee event
selection, and the `z`/`y` tag partitioning are inline in Step 0's wrapped function body, with no
function boundary a test can call. For each of the three: extract it verbatim into its own named
function within `main.rs` (pure signature: fetched events in, selected/partitioned events out — no
network, no I/O), re-run Step 0's golden test to confirm the output is unchanged, then write its
characterization test against the new function. Only once Steps A and B's tests are green does the
module move (to `models/dedup.rs`, `models/dev_fee.rs`, `models/core.rs` for the `z`/`y` partitioning) happen — a pure file move
of already-tested code, not an extraction under test.

**Step C — decompose the rest of the body, not just the three named pieces.** Everything else in
Step 0's wrapped function — relay client setup and the two-filter query, the per-event loop that
dispatches on kind and builds `MostroStats`, and every remaining `writeln!` formatting and coloring
call — is still one large body, leaving `lib.rs`'s `run()` doing far more than its assigned wiring.
This step closes that gap: the relay setup and query become the real `EventSource` implementation in
`fetch/client.rs` (the trait Step 0 already introduced, now with a body); the per-kind aggregation
moves verbatim into whichever of `models/dev_fee.rs`, `models/order.rs`, `models/dispute.rs`,
`models/instance_status.rs` matches each branch; and every formatting/coloring call moves verbatim
into `report/render/console.rs`. Each move is mechanical relocation of already-golden-tested code, so
Step -1's baseline (re-run after every move) is the safety net rather than new per-piece unit tests.
Only once this step is complete does Step 0's wrapped function contain nothing but calls into
`fetch`, `models`, `stats`, and `report` — the wiring-only shape `run()` requires.

**Step D — the module move, re-proved against the same baseline.** Only once Steps A, B, and C are
done does the final module move happen: the wiring-only wrapped function relocates to become
`lib.rs`'s `pub async fn run<E: EventSource>(public_key: PublicKey, event_source: E, now: &dyn Fn() ->
DateTime<Utc>, out: &mut impl std::io::Write, err: &mut impl std::io::Write)`, keeping the clock call
at the same logical point. Step 0's test — now able to live in `tests/` since `run()` is part of the
library's public surface — calls that function with the same public key, fixture event set, recorded
`now`, and buffer writers, asserting the output still matches Step -1's capture exactly
(`s_tag_distribution` sorted, everything else byte-for-byte). `main.rs` ends this pull request as thin
as Step 0 already made it — parse the arguments and pubkey, construct the real `EventSource`, wire the
real clock and `io::stdout()`/`io::stderr()`, call `run()` — with the binary-level invalid-pubkey pin
from Step -1 still guarding the one branch that stays behind. The comparison against Step -1's capture
is what proves output-identical end to end across all four steps; Steps A and B's unit tests only
cover the individual pieces they pin.

No new functionality, no new metric, no output change anywhere in this pull request.

Deviations knowingly carried forward by this pull request, each with the pull request that fixes it:

| Current behavior | Conflicts with | Fixed in |
|------------------|----------------|----------|
| `days_active` fallback spans first order to *last* order | 001 FR-001 (must span first order to now) | PR 4 |
| Invalid pubkey prints to stderr and returns success | 002 FR-019 (must exit `5`) | PR 2 |
| Debug/sample-event dumps and the no-dev-fee-events warning write to `out` (PR 1 turned every original `println!` into a `writeln!(out, ...)`; these three never distinguished report content from diagnostics) | 002 FR-017 (diagnostics belong on `err`) | PR 2 |
| `calculate_score` trust score is reported | No spec defines it (see Complexity Tracking) | Removed in PR 7 |
| `s_tag_distribution`'s printed order follows `HashMap` iteration (nondeterministic across process runs) | Technical Context's determinism constraint | PR 2 (already touches this block to route it to `err`; sort by key there too) |

### Pull requests 2 onward — one per user story or functional area

Each builds on the modular structure, with tests written alongside. Ordering follows dependency, then
the specs' own priority, so later pull requests only layer explicit overrides on already-working
automatic defaults.

| PR | Scope | Requirements | Depends on |
|----|-------|-------------|------------|
| 2 | Error taxonomy; exit codes `0`/`1`/`2`/`3`/`5` (none need domain scoping to detect); graceful relay degradation; diagnostic routing — rewrites every diagnostic `writeln!(out, ...)` call PR 1 produced (see the PR 1 deviations table) to `writeln!(err, ...)` via the injected writer, never a direct `eprintln!` — closing FR-017 completely | 002 FR-011, FR-017; part of FR-019 (all exit codes except `4`); Principle VI | 1 |
| 3 | Event scoping, deduplication, domain models, exit code `4` (whether any of the four scoped kinds is usable is only knowable once this PR's scoping exists, so it cannot be claimed in PR 2) | 001 FR-002, FR-013, FR-014, FR-015; remainder of 002 FR-019 (exit code `4`) | 1, 2 |
| 4 | Core metrics: longevity, cumulative, trade statistics including std dev/CV, liveness, activity consistency | 001 FR-001..FR-005, FR-010 (spec 001 US1, P1) | 3 |
| 5 | Dispute signals (depends on 4, not just 3: FR-006's disputes-per-100-trades ratio needs PR 4's `total_successful_trades` as its denominator) | 001 FR-006 (spec 001 US2, P2) | 3, 4 |
| 6 | Descriptive context: fiat and payment-method breakdowns, premium signal, bond policy | 001 FR-008, FR-009, FR-011, FR-012 (spec 001 US3, P3) | 3, 4 |
| 7 | Report model, 5 sections, activity grid with automatic granularity, recommendations, console renderer | 002 FR-001..FR-008b, FR-013..FR-018 (spec 002 US1, P1) | 4, 5, 6 |
| 8 | Plain-text and JSON renderers, context-based format default, schema version, JSON fatal envelope | 002 FR-009..FR-012a (spec 002 US2/US3) | 7 |
| 9 | CLI flags: pubkey and relays with environment fallbacks, format, color, quiet, help, version | 003 FR-001..FR-003, FR-010..FR-013a (spec 003 US1, US3) | 8 |
| 10 | Time range and grouping: `--since`, `--until`, `--view`, boundary snapping, wide-range warning | 003 FR-004..FR-007 (spec 003 US2, P2) | 9 |
| 11 | Section filtering | 003 FR-008, FR-009 (spec 003 US4, P4) | 9 |
| 12 | Persisted configuration and `--init-config` | 003 FR-014..FR-019 (spec 003 US5, P5) | 9, 10 |

Pull requests 10 and 11 are independent of one another and may be reviewed in any order once 9 has
merged. Pull request 12 additionally depends on 10: FR-016 requires the configuration file to parse,
validate, and honor a persisted `view` value, which needs the `--view` model and grid integration
PR 10 introduces — persisting a setting before the code that interprets it exists would leave it
unvalidatable.

### Resolving the deferred grid and warning thresholds (PR 7)

Spec 002's checklist is explicit that the exact numbers for FR-005a's "very wide time range" warning
and FR-014's "more than a couple of seconds" progress trigger are a planning-level decision to be set
after usability testing, not invented without evidence. Spec 003 FR-006 likewise delegates the
automatic daily/monthly/yearly granularity boundaries to planning. All three belong to spec 002's
grid/report work and are scoped to PR 7 (002 FR-013 through FR-018, which includes FR-014). This plan
does not invent the numbers either; PR 7 MUST resolve each through the documented procedure below
rather than leaving the trigger undefined.

This evidence gathering does not need the `--since`/`--until`/`--view` CLI flags, which are not wired
in until PR 10 (003 FR-004..FR-007): PR 7 operates on the internal grid and report functions directly,
driven by fixture event sets.

1. Before implementing any of the three triggers, construct fixture event sets spanning
   real-world-representative durations (weeks, several months, several years) and feed them directly
   to the grid-bucketing and report-assembly functions under test — no CLI invocation involved. For
   FR-014's fetch-latency trigger, measure actual relay round-trip time against the project's default
   relay set (this one genuinely needs a network call). Record the observed behavior — grid row counts
   and granularity choices per fixture range, actual fetch latency — in the pull request's description.
2. Pick each boundary from that observed evidence, not an assumed round number: the wide-range warning
   threshold (FR-005a), the progress-indicator latency threshold (FR-014), and the automatic
   granularity switch-over points between daily/monthly/yearly buckets (003 FR-006).
3. The pull request's tests assert each mechanism fires at its chosen boundary (a grid with more rows
   than the boundary shows the warning; a fetch exceeding the recorded latency shows the indicator; a
   fixture range past the switch-over point buckets by month or year instead of day) — not a specific
   number this plan asserts in advance.

PR 10 only wires the explicit `--view`/`--since`/`--until` CLI flags to the grid, warning, and
automatic-granularity mechanism PR 7 already built and chose boundaries for; it introduces no new
threshold of its own.

### The JSON output contract (PR 8)

Spec 002 fixes the JSON structure (five sections, Bond Policy as a sub-object inside general
statistics) and the completeness contract (FR-012, FR-012a), but its Assumptions leave the exact field
names and key casing to this planning step. This subsection resolves it, so PR 8 implements a
contract already reviewed rather than inventing names mid-implementation.

Every name below is `snake_case`, matching the constitution's Rust conventions and letting
`report/model.rs` derive `serde::Serialize` with no per-field renames. Units are carried in the field
name (`_sats`, `_percent`, `days_`) rather than left to documentation, since a consumer reading a bare
number has no other way to learn them.

#### Top-level shape

```json
{
  "schema_version": "1.0.0",
  "generated_at": "2026-07-24T10:15:00Z",
  "node": {},
  "fetch": {},
  "activity": {},
  "stats": {},
  "recommendations": {},
  "metric_definitions": {}
}
```

The five section keys are the four `--sections` tokens spec 003 FR-008 already fixes (`fetch`,
`activity`, `stats`, `recommendations`) plus `node` for the identity header, which has no token
because it is never filterable — reusing that vocabulary means the CLI and JSON name sections
identically. All five keys are always present regardless of `--sections`, per FR-008's statement that
section filtering has no effect on JSON. `metric_definitions` sits alongside them, not inside any one
section, since it explains fields across several sections at once; it is unaffected by `--sections`
for the same reason `schema_version` and `generated_at` are.

`metric_definitions` is what keeps this contract conformant with FR-008b, which requires every shown
metric to carry enough labeling or inline explanation that a trader understands it "without leaving
the tool" — worded as "every section and every metric," not limited to the human-readable formats, so
a JSON consumer is not exempt. Wrapping each metric value in an object (`{"value": 5.2, "label":
"..."}`) was rejected: FR-012 fixes a computed metric's JSON type as a bare number (or `null`), and
turning every one of the 28 `stats` fields into an object would violate that type contract. Instead,
`metric_definitions` is a single object, keyed by the same dotted path
`recommendations.items[].metric` already uses (for example `stats.trade_size.coefficient_of_variation`),
where each entry is `{ "label": string, "meaning": string, "unit_and_direction": string }`. The key
set is exhaustive and mechanically checkable, not judgment-based: every row already listed in the
`stats` table above (all ten sub-objects' fields), plus `activity.granularity` and
`buckets[].median_trade_sats`, plus `fetch.relays[].status` and `fetch.relays[].error`, gets an entry
— nothing else does. That excludes `node`'s identity fields, the error envelope's mechanical fields,
and `fetch`'s bookkeeping counts (`dev_fee_events`, `order_events`, `unique_orders`,
`dispute_events`, `instance_status_found`), none of which need interpretation. PR 8's test asserts
this key set matches exactly: one entry per field named above, no more, no fewer. FR-008b cites a
companion decisions document
that does not exist in the repository; spec 001 covers each metric's meaning only in its FR prose. PR
8 therefore authors the three string fields per metric, grounded in that FR prose — for example
`trade_size.coefficient_of_variation`'s `meaning` restates FR-010's explanation of the ratio, and its
`unit_and_direction` states it is unitless with no fixed upper bound, a lower value meaning more
consistent trade sizing. `metric_definitions` is static per build, so `report/model.rs` generates it
once from a fixed table PR 8 writes out explicitly, rather than recomputing it per report.

`schema_version` (002 FR-012a) is a semantic-version string rather than an integer, so a consumer can
tell an additive change (minor bump) from a breaking one (major bump) — an integer counter can only
say "different", forcing a full re-verification on any change. `generated_at` is RFC 3339 UTC and is
not a metric: `days_active`, `days_since_last_trade`, and the rolling windows are all measured against
report-generation time (001 FR-001, FR-004), so a stored JSON document cannot be interpreted later
without it. It is also the single field the renderer snapshot tests freeze.

Timestamps throughout are RFC 3339 UTC strings, not epoch integers. Every date in this tool is a UTC
calendar-day quantity (001 FR-005, 002 FR-005), and an RFC 3339 string says so in the value itself
where a bare integer does not; `null` remains distinguishable from a string, so FR-012's
not-applicable contract is unaffected.

#### `node` — identity header (002 FR-002)

| Field | Type | Source |
|-------|------|--------|
| `pubkey_hex` | string | 002 FR-002 |
| `pubkey_npub` | string | 002 FR-002 |

Neither is ever `null`: a report document only exists once the key parsed, and a key that failed to
parse produces the error envelope below instead. Both encodings are emitted since the console report
shows both and a consumer may hold either form.

#### `fetch` — relay fetch summary (002 FR-003)

| Field | Type | Source |
|-------|------|--------|
| `relays` | array of `{ "url": string, "status": "success" \| "failed", "error": string \| null }` | 002 FR-003 |
| `dev_fee_events` | number | 002 FR-003 (backs Longevity) |
| `order_events` | number | 002 FR-003 (unique by event id, before `d`-tag dedup) |
| `unique_orders` | number | 002 FR-003 (after 001 FR-002's qualifying-order procedure) |
| `dispute_events` | number | 002 FR-003 (unique by `d` tag) |
| `instance_status_found` | boolean | 002 FR-003 (backs Bond Policy) |

No count here is ever `null`. Zero fetched events of a kind is a real, measured value, not missing
data — that is exactly what this section exists to let a trader sanity-check. `relays` preserves the
configured order so the array is deterministic, and it stays an array of objects rather than a map
keyed by URL because a per-relay `error` string belongs next to its relay.

#### `activity` — activity grid (002 FR-004, FR-005)

| Field | Type | Source |
|-------|------|--------|
| `granularity` | `"daily" \| "monthly" \| "yearly"` \| null | 002 FR-005, 003 FR-006 |
| `range_start` | RFC 3339 UTC string \| null | 002 FR-005 |
| `range_end` | RFC 3339 UTC string \| null | 002 FR-005 |
| `buckets` | array of bucket objects, chronological (empty when the range is null) | 002 FR-004, FR-005 |
| `buckets[].bucket_start` | RFC 3339 UTC string | 002 FR-005 |
| `buckets[].successful_trades` | number | 002 FR-004 |
| `buckets[].volume_sats` | number | 002 FR-004 |
| `buckets[].median_trade_sats` | number \| null | 002 FR-004, Edge Cases |

An empty bucket is emitted as a row with `successful_trades` and `volume_sats` of `0` and
`median_trade_sats` of `null` — not an inconsistency: spec 002's Edge Cases state zero orders and
zero volume are real values for an empty bucket, while a median over zero orders is undefined, not
zero, per 001 FR-003. `buckets` never skips a bucket, so consumers can index it as a contiguous
series. FR-005a's wide-range warning is a stderr diagnostic (002 FR-017) with no JSON field; the grid
it warns about is rendered in full either way.

One more case: 002 FR-019 requires a successful, non-error report for a node whose only usable data
is a dev-fee, dispute, or instance-status event — zero orders. With no order timestamp to anchor a
default range and no explicit `--since`/`--until`, there is no value to put in `range_start`/
`range_end` that would not be invented, so `range_start`, `range_end`, and `granularity` are `null`
and `buckets` is `[]` — the block stays present, per FR-012's "every key always present" rule, simply
empty. The console and plain-text renderers show this as an explicit "no order history to build an
activity grid from" line rather than an empty table, consistent with FR-008b.

#### `stats` — general statistics (002 FR-006, FR-007)

Sub-objects mirror the `stats/` module split in the Project Structure above, one key per module, so a
field's owner is readable from its path.

| Path | Type | Source |
|------|------|--------|
| `longevity.first_seen_at` | RFC 3339 UTC string \| null | 001 FR-001 |
| `longevity.days_active` | number \| null | 001 FR-001 |
| `cumulative.total_successful_trades` | number | 001 FR-002 |
| `cumulative.total_volume_sats` | number | 001 FR-002 |
| `trade_size.min_trade_sats` | number \| null | 001 FR-003 |
| `trade_size.max_trade_sats` | number \| null | 001 FR-003 |
| `trade_size.mean_trade_sats` | number \| null | 001 FR-003 |
| `trade_size.median_trade_sats` | number \| null | 001 FR-003 |
| `trade_size.std_dev_trade_sats` | number \| null | 001 FR-010 |
| `trade_size.coefficient_of_variation` | number \| null | 001 FR-010 |
| `liveness.last_successful_trade_at` | RFC 3339 UTC string \| null | 001 FR-004 |
| `liveness.days_since_last_trade` | number \| null | 001 FR-004 |
| `liveness.successful_trades_last_7d` | number | 001 FR-004 |
| `liveness.successful_trades_last_30d` | number | 001 FR-004 |
| `liveness.successful_trades_last_90d` | number | 001 FR-004 |
| `consistency.active_days_last_30d` | number | 001 FR-005 |
| `consistency.max_consecutive_inactive_days_last_30d` | number | 001 FR-005 |
| `disputes.disputes_per_100_trades` | number \| null | 001 FR-006 |
| `disputes.total_disputes` | number | 001 FR-006 |
| `disputes.resolved_disputes` | number | 001 FR-006 |
| `disputes.active_disputes` | number | 001 FR-006 |
| `disputes.unknown_status_disputes` | number | 001 FR-006 |
| `fiat_breakdown.orders_considered` | number | 001 FR-008 |
| `fiat_breakdown.distribution` | array of `{ "currency": string, "orders": number, "share_percent": number }` \| null | 001 FR-008 |
| `payment_method_breakdown.total_mentions` | number | 001 FR-009 |
| `payment_method_breakdown.distribution` | array of `{ "method": string, "mentions": number, "share_percent": number }` \| null | 001 FR-009 |
| `premium.premium_baseline_percent` | number \| null | 001 FR-011 |
| `premium.premium_dispersion_percent` | number \| null | 001 FR-011 |
| `bond_policy.status` | `"enabled" \| "disabled" \| "unknown"` | 002 FR-007, 001 FR-012 |

Which fields may be `null` is decided by the specs, not convenience. The rolling-window counts, the
two Activity Consistency figures, and the four dispute counts are never `null`: 001 FR-004, FR-005,
and FR-006 define real values for the empty case — most explicitly FR-005's `active_days_last_30d = 0`
with `max_consecutive_inactive_days_last_30d = 30`, which 002's Edge Cases call out as data that must
not be overridden with a not-applicable marker. Everything else in the table is `null` exactly under
the condition its own requirement names: no dev-fee anchor and no qualifying order for Longevity, an
empty `amt`-restricted set for the four Trade Statistics figures, fewer than two such orders (or a
zero median) for the coefficient of variation, zero successful trades for `disputes_per_100_trades`
and Liveness's last-trade pair, a zero denominator for either breakdown, and fewer than two valid
`premium` tags for both Premium Signal figures.

`null` must mean not-applicable and nothing else, which is a real hazard in Rust: `serde_json`
serializes an `f64` NaN or infinity as `null` without erroring, so a division that slipped through
would be indistinguishable from a deliberate not-applicable value. `models/core.rs`'s
`MetricValue::Computed(T) | NotApplicable` is therefore the only thing allowed to produce a `null`
here — every `stats/` computation returns `NotApplicable` at its guard rather than letting a
degenerate float reach the serializer.

Both breakdowns are arrays of records rather than objects keyed by currency or method: 001 FR-009
requires payment-method values compared byte for byte with no trimming, so `"Bank transfer"` and
`" Bank transfer"` are distinct methods, and as object keys they would be near-identical entries with
no guaranteed order, while an array preserves the ranking order the requirement defines — descending
`share_percent`, ties broken by label ascending, deterministic even though the underlying tally is a
`HashMap`. `share_percent` is `[0, 100]` rather than `[0, 1]` because both requirements state their
distributions sum to 100%; a different scale than the spec describes invites off-by-100 errors.

Bond Policy is its own sub-object with a three-valued string, per 002 FR-007's requirement that it be
distinctly named, not merged into the trade-history metrics. A `true | false | null` boolean would be
more idiomatic, but `null` is already this schema's marker for "could not be computed", and 001
FR-012 requires unknown to be *reported* — a missing signal is neither evidence of a disabled policy
nor the same as a metric with no data. A string enum keeps the three states distinguishable to a
consumer that treats `null` uniformly; FR-012's numeric rule does not apply here, since it exempts
non-numeric metrics that define their own representation.

All numeric fields carry full computed precision with no thousands separators, per 002 FR-018;
rounding to one decimal place is `report/format.rs`'s job for the console and plain-text renderers
only. Consumers must not assume integers: `mean_trade_sats`, and `median_trade_sats` over an
even-sized set, are legitimately fractional.

#### `recommendations` (002 FR-008, FR-008a)

| Field | Type | Source |
|-------|------|--------|
| `nothing_notable` | boolean | 002 FR-008 |
| `items` | array of `{ "id": string, "metric": string \| null, "message": string }` | 002 FR-008, FR-008a |

`id` is a stable `snake_case` key so automation can branch on a recommendation without parsing
English, and `metric` is the dotted path of the field the guidance refers to (for example
`stats.premium.premium_dispersion_percent`), or `null` for guidance that synthesizes several. This is
also where FR-008a's discipline becomes checkable: only a recommendation pointing at Premium Signal or
Trade-Size Consistency may use comparative language, since those are the only two metrics with a
baseline the tool actually computes.

When nothing warrants a flag, `nothing_notable` is `true` and `items` is empty. FR-008's mandatory
"nothing notable to flag" statement is carried by the boolean rather than a canned English sentence —
the block stays present and explicit, and a fixed sentence in a machine-readable field would only
invite consumers to string-match it. The console and plain-text renderers turn the boolean into the
sentence FR-008 requires for human readers.

##### Recommendation trigger rules (PR 7)

FR-008 requires the block to "synthesize the metrics ... into plain-language guidance" without naming
which conditions qualify, and FR-008a restricts *how* a triggered recommendation may be worded, not
*when* one fires. Left unresolved, PR 7's tests could only bless whatever it happened to implement,
not verify a pre-reviewed outcome. This plan resolves the "when" two different ways, matching each
condition's own nature.

**Deterministic, existence-based triggers** — these need no invented magnitude, so they are fixed
here rather than deferred:

| Condition | `metric` | Message (factual, no comparison — no cross-node baseline exists for any of these) |
|-----------|----------|----|
| `cumulative.total_successful_trades == 0` | `stats.cumulative.total_successful_trades` | States the node has no completed trade history yet |
| `disputes.total_disputes > 0` | `stats.disputes.disputes_per_100_trades` | States the exact dispute and trade counts, per FR-008a's "informative context instead of an unsupported label" |
| `bond_policy.status != "enabled"` | `stats.bond_policy.status` | States the exact status (`disabled` or `unknown`), never implying which is safer, since 001 FR-012 requires reporting Bond Policy neutrally |

Each condition is independently evaluated; more than one may produce an item in the same report, and
`nothing_notable` is `true` only when none of them, nor the two threshold-based triggers below, fire.

**Threshold-based triggers** — `premium.premium_dispersion_percent` (relative to
`premium_baseline_percent`) and `trade_size.coefficient_of_variation` are the only two metrics FR-008a
permits comparative language for, since both compare a node against its own historical baseline. But
"how much dispersion is worth flagging" is the same kind of number the checklist reserves for
evidence, not invention — the same rule already applied to the grid and warning thresholds above. PR
7's evidence-gathering procedure is extended to cover these two: before implementing either trigger,
compute both metrics across the same fixture event sets already gathered for the grid work, observe
the range of values a real node history produces, and pick each boundary from that distribution —
recorded in PR 7's description, tested at the chosen boundary, not asserted here in advance.

#### Fatal error envelope (002 FR-011)

A fatal error is a different document, not a report with fields nulled out:

```json
{
  "schema_version": "1.0.0",
  "error": {
    "code": "relays_unreachable",
    "message": "None of the configured relays could be reached.",
    "relays": [
      { "url": "wss://relay.mostro.network", "status": "failed", "error": "connection timed out" }
    ]
  }
}
```

A consumer distinguishes the two with one key check: a report never carries `error`, and an error
document never carries the five section keys. `schema_version` is repeated here because the envelope
is versioned on the same schedule as the report.

| `code` | Exit code | Condition |
|--------|-----------|-----------|
| `general_error` | `1` | Any failure not covered by another code (002 FR-019) |
| `usage_error` | `2` | A usage error detected after argument parsing (003 FR-013a, FR-018) |
| `relays_unreachable` | `3` | Every configured relay failed (002 FR-019) |
| `no_usable_events` | `4` | Zero dev-fee, order, dispute, or info events for the pubkey (002 FR-019) |
| `invalid_pubkey` | `5` | A syntactically supplied pubkey that fails format validation (002 FR-019) |

The code strings live in `error/exit_code.rs` beside the exit codes themselves, so a new error variant
cannot be added without naming its JSON code in the same place. `relays_unreachable` is plural on
purpose: 002 FR-019 defines exit code `3` as *all* configured relays failing, while a single failure
among several successes is a stderr warning inside a normal report, not this envelope.

`error.relays` uses the exact same three-field record shape as `fetch.relays`
(`{ url, status, error }`), so a consumer's relay-parsing code works unmodified against both
documents; `status` is always `"failed"` here, since every relay in this array is one that failed.
The `relays` key is always present in the envelope object; its value is the array above only when
`code` is `relays_unreachable`, and `null` for every other code — the same presence rule the rest of
this schema uses for not-applicable, rather than a special case where the key disappears.

One caveat on `usage_error`: per 003 FR-013a, a usage error raised by the argument parser itself
prints the library's own plain usage text to stderr and exits `2` before `--format` is interpreted, so
it never reaches this envelope. The code exists for usage errors application code detects after
parsing succeeds — `--force` without `--init-config` (003 FR-018), or `--since` later than `--until`
— where the requested format is already known.

## Testing Strategy

Test-First Development is non-negotiable (Principle IV), so this section is a constraint on how each
pull request above is built, not a later phase.

- **Written alongside, never deferred.** A pull request that adds a metric adds that metric's tests in
  the same diff; Phase 5's testing commitment is absorbed into each pull request here.
- **Red-Green-Refactor.** Each requirement starts as a failing test derived from its spec's Acceptance
  Scenarios and Edge Cases, already written in given/when/then form, translating directly into test
  names.
- **Coverage floor.** ≥50% overall, measured with `cargo llvm-cov`. The pure modules (`stats`,
  `models`, `cli::duration`) should sit far above that, since they need no fixtures or network.
  Enforcing this floor as a CI gate is out of scope for this plan; it can be added once there is real
  coverage data to calibrate against.
- **No network in tests.** `fetch` is the only module performing I/O, and nothing downstream depends
  on it. Every metric and renderer test runs against fixture events loaded from `tests/fixtures/`.
- **Priority test cases**, per the constitution's list plus the specs' edge cases: event parsing and
  tag extraction; order deduplication including the tie-break by greatest event id and the exclusion
  of future-dated events; every statistical calculation including its not-applicable branch; the
  zero-successful-trades node (which must render, not error); all-relays-failed versus
  one-relay-failed; and each of the six exit codes.
- **Snapshot discipline.** Renderer output is snapshot-tested with `insta`, so any change to the five
  sections' wording or layout appears as an explicit, reviewable diff rather than passing silently.

## Out of Scope

- **The mdBook documentation.** The `book/` deliverable is sequenced after Phases 4 and 5 so it can be
  written against real implemented behavior rather than rewritten if the implementation refines a spec
  detail — deferring what the Summer of Bitcoin proposal labels a Phase 3 milestone, flagged and
  accepted when the sequencing was decided.
- **Shell completion generation** (bash/zsh/fish) and the standalone user manual, both explicitly
  assigned to the documentation track by spec 003's Assumptions.
- **Kind 38384 rating events.** Discarded outright by spec 001 FR-007; no code in this phase reads,
  fetches, or filters them, and per 002 FR-019 their presence alone does not save a pubkey from exit
  code `4`.
- **Any new metric.** This phase implements the specified metric set and adds nothing to it. A new
  metric requires its own `specify` cycle.
- **`tasks.md`.** The next step, produced separately.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| A `plan` step with no matching `spec.md` (Principle V's sequence) | Phase 4 implements three already-ratified specs and introduces no new requirement; its `specify` and `clarify` steps were completed in Phases 1-3 | Writing a fourth spec that restates specs 001-003 would create a second source of truth for the same requirements, with a permanent drift risk and no reader benefit. The constitution's Development Workflow already permits documented reordering for phased, multi-pull-request delivery |
| Dual crate targets (`lib.rs` alongside `main.rs`) where the constitution names seven modules | Rust integration tests under `tests/` cannot import a binary-only crate; without a library target, Principle IV's coverage floor could only be approached with inline unit tests and the CLI's end-to-end behavior would be untestable | A binary-only crate keeps the file count lower but makes the test requirement unreachable. The split adds no logic: `main.rs` only parses, calls, and maps an exit code |
| Legacy `calculate_score` trust score carried into the refactor with no spec backing | Pull request 1 is behavior-preserving by definition; deleting the function there would make the characterization tests unable to prove output equivalence | Deleting it during the refactor would conflate two decisions. **Resolved: removed in pull request 7, not amended into spec 002** — 002 FR-006 and FR-001 fix the general statistics section's five-section content with no trust score, and a sound composite score would need cross-node relative normalization, out of this tool's single-node scope; deferred to a future `sdd-explore` cycle rather than shipped unspecified, per Principle I |
| Submodules nested inside the seven mandated top-level modules | `stats` alone implements fourteen distinct requirements; grouping related ones (e.g. longevity, cumulative, liveness, and consistency into one `lifecycle.rs`) keeps files cohesive without a file per individual FR | A flat single file per top-level module would recreate the monolith at a smaller scale — a 600-line `stats.rs` fails the same cohesion test `main.rs` fails today. A file per individual metric was the other extreme, rejected as unnecessary fragmentation for pieces this small. Principle III constrains the top-level decomposition, which this layout matches exactly |
