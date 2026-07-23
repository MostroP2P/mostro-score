# Specification Quality Checklist: Node Reputation Metric System

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-07-22
**Feature**: [spec.md](../spec.md)

**Documented workflow deviation**: this repository's ratified sequence
(`constitution → specify → clarify → plan → checklist → tasks → analyze → implement → converge`,
per `.specify/workflows/speckit/workflow.yml` and the constitution's Development Workflow section)
runs `checklist` after `plan`. This feature intentionally runs it before `plan` instead: no
`plan.md` exists yet in `specs/001-node-reputation-metrics/`, and `spec.md`'s own `Status` field
says `Ready for Planning`, not `Planned`. This PR (Phase 1 of the Summer of Bitcoin proposal) is
scoped to the metric specification only; the technical plan is deferred to a later phase and PR, so
this checklist validates spec quality on its own before that plan exists, per the constitution's
Development Workflow section (v1.2.0), which explicitly permits this exact reordering for
phased, multi-PR features when documented here, not merely the general allowance that "skipping a
step requires a documented justification in the feature's spec directory."

## Content Quality

- [x] No implementation details (languages, frameworks, APIs)
- [x] Focused on user value and business needs
- [x] Written for non-technical stakeholders
- [x] All mandatory sections completed

## Requirement Completeness

- [x] No [NEEDS CLARIFICATION] markers remain
- [x] Requirements are testable and unambiguous
- [x] Success criteria are measurable
- [x] Success criteria are technology-agnostic (no implementation details)
- [x] All acceptance scenarios are defined
- [x] Edge cases are identified
- [x] Scope is clearly bounded
- [x] Dependencies and assumptions identified

## Feature Readiness

- [x] All functional requirements have clear acceptance criteria
- [x] User scenarios cover primary flows
- [x] Feature meets measurable outcomes defined in Success Criteria
- [x] No implementation details leak into specification

## Notes

**Current status** (read this first; the entries below are a chronological log, not all still
accurate on their own): FR-007 (Rating Signals) is discarded and does not exist as an active
requirement in `spec.md`. Where earlier notes below mention "review-coverage formula (FR-007)" or
similar, they describe an earlier point in this session, before FR-007 was found unfixable and
removed; the final, authoritative state is described in the last two entries of this log and in
FR-007's own rejection entry in `spec.md`.

- All checklist items pass. FR-012 (Bond Policy data source) was resolved: `bond_enabled` tag on
  the node's kind `38385` instance status event, confirmed against the source gist analysis.
- Clarification session 2026-07-22 resolved 3 further ambiguities without changing checklist
  state (already 16/16 passing): Premium Signal definition (FR-011), trade-size consistency
  formula (FR-010), and review-coverage formula (FR-007). See `## Clarifications` in spec.md.
- CodeRabbit review round (2026-07-23) flagged FR-004 through FR-012 for missing acceptance
  criteria: population/sample semantics and the 2-sample floor for FR-010, timestamp field and
  rolling-window-vs-UTC-calendar-day boundaries for FR-004/FR-005, dispute deduplication and a
  resolved/active breakdown for FR-006, `total_rating`'s non-mean incremental formula for FR-007,
  the exact tags plus the partition-vs-ranking distinction between FR-008 and FR-009, the general
  order-deduplication procedure underlying FR-002, and the multi-relay tie-break plus "unknown"
  fallback for FR-012. All were resolved with a new Clarifications session verified directly
  against the real `mostro`/`mostro-core` source (not just the source gist), keeping checklist
  state at 16/16 passing. See `## Clarifications` in spec.md for the full session.
- Independent Codex CLI audit (`codex exec review --uncommitted`, iterated across 5 rounds until
  clean, 2026-07-23) of that same round caught five more gaps the human+CodeRabbit pass missed:
  (1) FR-007 was missing the multi-relay tie-break rule for kind `38384` (also NIP-33 replaceable,
  confirmed in `mostro/README.md`); (2) User Story 2's Independent Test still said "average rating"
  after FR-007's body was corrected to "self-reported rating figure"; (3) User Story 2's first
  acceptance scenario said "3 dispute events," ambiguous against FR-006's dedup-by-`d`-tag rule
  since republished status transitions could make 3 events resolve to fewer than 3 unique disputes;
  (4) FR-011 required `premium_deviation` for every order in a set that could have exactly 1 member,
  where a leave-one-out baseline is impossible to satisfy; (5) FR-008's denominator
  (`qualifying_orders`) was inconsistent with also excluding empty-`f` orders, which would make its
  own percentages sum to less than 100%. All five fixed; checklist state unchanged at 16/16.
- Further review rounds (a second independent reviewer plus repeated Codex passes, 2026-07-23) on
  the squashed commit found: missing event author/protocol scoping (new FR-015, `y`/`z` tag and
  pubkey checks), a stale `Status: Draft` header inconsistent with 16/16 passing, an acceptance
  scenario claiming a "flagged as outlier" behavior the spec's own Assumptions place out of scope,
  a relay-access assumption missing kind `38385`, zero-denominator handling missing from
  FR-008/FR-009, a missing dispute tie-break in FR-006, a missing `last_successful_trade_at` /
  `days_since_last_trade` metric that User Story 1 already promised, a stale constitution amendment
  date and an over-broad Sync Impact Report, an untracked-but-committed `.specify/feature.json`
  that would have silently pinned future spec-kit features to this one, a dev-fee-fallback formula
  that copied a prototype inconsistency (spanning first-to-last order instead of first-to-now), and
  finally, most significant, **FR-007 (Rating Signals) discarded entirely**: verification against
  `mostro/src/app/rate_user.rs` showed kind `38384` is keyed by the rater's own one-time trade
  pubkey, not the node's, and not even the rated party's, so it cannot support a node-level rating
  metric at all. All fixed or
  resolved by removing the affected requirement; checklist state unchanged at 16/16 (FR-007's
  removal does not add a new gap, since User Story 2 and SC-002 were updated to no longer depend on
  it).
- A review round (2026-07-23) fixed four smaller inconsistencies: User Story 2 said "unresolved
  disputes" while FR-006 counts all disputes regardless of resolution; FR-006 and FR-001 were
  missing explicit precedence rules for their double-zero edge cases (zero disputes with zero
  trades, and zero dev-fee events with zero orders); and FR-014 still listed the discarded FR-007
  in its tie-break reference. That same round also changed this file's own Purpose line to say the
  checklist runs after `plan`, matching `.specify/workflows/speckit/workflow.yml`'s literal step
  order, without checking whether a `plan.md` actually existed for this feature. A later round
  found it did not, and reverted that change: see "Documented workflow deviation" at the top of
  this file for why this checklist intentionally runs before `plan` for this feature. All fixed;
  checklist state unchanged at 16/16.
- On "No implementation details" and "Written for non-technical stakeholders" above: `spec.md`'s
  Clarifications and FR-007's rejection entry deliberately cite file paths, function names, and
  worked traces (for example, `mostro/src/app/rate_user.rs`, `User::update_rating`). This is
  intentional, not a leak of implementation design into the requirements themselves: it is
  verification evidence proving each requirement's traceability, per the constitution's
  Evidence-Based Metrics principle, which specifically requires a "documented, deterministic
  computation method" rather than an unverified claim. The functional requirements (FR-001 through
  FR-015) and User Stories remain phrased in terms of computed values and outcomes, not code
  structure; the code citations live in the supporting rationale, not in the requirements' own
  wording.
- CodeRabbit's review of the pushed commit (2026-07-23) raised two points. First, it did not
  accept a per-file note alone as sufficient grounds to run this checklist before `plan`, and asked
  for either a `plan.md` or a constitutional amendment permitting the reordering; the constitution
  was amended to 1.2.0 with an explicit Development Workflow bullet for phased, multi-PR features,
  rather than writing a `plan.md` out of scope for Phase 1. Second, FR-003 restricted its
  successful-order set to orders with a parseable `amt` but never said what to report when that
  restricted set is empty while successful orders still exist; FR-003 now explicitly reports
  `min_trade_sats`, `max_trade_sats`, `mean_trade_sats`, and `median_trade_sats` as not applicable
  in that case. Both fixed; checklist state unchanged at 16/16.
