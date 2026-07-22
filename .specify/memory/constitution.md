<!--
Sync Impact Report
Version change: 1.0.0 → 1.0.2
Modified principles:
  - V. Spec-Driven Development (fixed an internal inconsistency found by a Codex audit: this
    principle previously said "spec → plan → tasks → implement → verify", a sequence that does
    not match the Development Workflow section's actual ratified sequence and references a
    "verify" step with no corresponding spec-kit command or workflow.yml step. Reworded to point
    at the real sequence and ground "verified" in CI + human PR review, the two gates that
    actually exist.)
  - III. Modular Architecture (added a transition note acknowledging that src/main.rs is still
    monolithic as of ratification, deferring compliance to Phase 4 rather than silently ratifying
    a principle the repo already violates with no stated exception, per a follow-up review
    finding.)
Added sections: none
Removed sections: none
Templates requiring updates:
  - .specify/templates/plan-template.md ✅ verified, Constitution Check section reads the
    constitution file dynamically at plan time rather than hardcoding principle names, no edit
    needed
  - .specify/templates/spec-template.md ✅ verified, generic mandatory sections do not conflict
    with Principle I or V; domain-specific evidence requirements are satisfied per-spec content,
    not by the shared template
  - .specify/templates/tasks-template.md ✅ updated, test sections changed from "OPTIONAL - only
    if tests requested" to mandatory, aligning with Principle IV (Test-First Development,
    NON-NEGOTIABLE)
  - .claude/skills/speckit-*/SKILL.md ✅ updated after a CodeRabbit review on PR #3: tests now
    mandatory in speckit-tasks/speckit-implement per Principle IV, hook auto-execution now
    requires an explicit trust gate, task-ID regex and remote-URL handling fixed in
    speckit-taskstoissues, prerequisite gates strengthened in speckit-analyze/speckit-converge
  - README.md ✅ updated, Contributing section now references the constitution and the spec-kit
    workflow
Follow-up TODOs: none, all previously pending items resolved
-->
# mostro-score Constitution

## Core Principles

### I. Evidence-Based Metrics
Every reputation metric MUST be traceable to a specific Nostr event kind and tag (currently
kind 8383 dev-fee-payment and kind 38383 order events), and MUST have a documented,
deterministic computation method recorded in `specs/`. Heuristic or subjective scoring without
a written formula is prohibited. Rationale: the tool's entire value proposition is transparency;
an unauditable metric undermines user trust in the reputation it produces.

### II. Statistical Robustness (Median Over Mean)
Metrics describing typical trade size or typical value MUST use the median, not the mean, as
the primary reference statistic. Rationale: medians resist manipulation by outlier or fake
trades far better than averages, and this is an already-established convention in
`specs/reputation_system_v1.md` and the project changelog.

### III. Modular Architecture
The codebase MUST be organized into single-purpose modules (`cli`, `fetch`, `models`, `stats`,
`report`, `config`, `error`), never as a single monolithic `main.rs`. Modules MUST exhibit low
coupling and high cohesion, with a clear separation between data fetching, domain models,
statistical computation, and reporting/presentation.

**Transition note**: `src/main.rs` predates this constitution and is still monolithic as of
ratification. This principle does not retroactively fail the repository; the modular refactor is
explicitly scheduled as Phase 4 of the project's Summer of Bitcoin proposal. New code added after
ratification MUST follow this principle; the existing monolith MUST be brought into compliance no
later than Phase 4, not left indefinitely as a silent exception.

### IV. Test-First Development (NON-NEGOTIABLE)
No implementation task is complete without accompanying unit tests written before or alongside
the code under test, covering at minimum event parsing, order deduplication, and statistical
calculations. The project MUST maintain at least 50% code coverage, per the Summer of Bitcoin
proposal commitment. Red-Green-Refactor is the expected cycle; tests are not optional polish.

### V. Spec-Driven Development
Every feature or metric change MUST go through the full ratified sequence defined in
Development Workflow (`constitution → specify → clarify → plan → checklist → tasks → analyze →
implement → converge`) before merging to `main`, using this repository's spec-kit artifacts
(`specs/NNN-feature/`) as the single externally-visible source of truth. A change is only
considered verified once CI passes (formatting, linting, tests) and it has been reviewed by a
human via pull request; there is no separate automated "verify" step distinct from these two
gates, and this principle MUST NOT reference one. Any internal reasoning or memory layer used
to support this process is a separate concern and is not part of this constitution.

### VI. Graceful Degradation & User-Facing Errors
Error messages MUST be clear and actionable for end users, never raw stack traces or unhandled
panics. When a configured relay is unreachable, the tool MUST warn and continue with the
remaining relays rather than crash, and MUST only exit with an error when no relay succeeds.

### VII. English-Only Artifacts
All code, identifiers, comments, specs, and documentation MUST be written in English. Comments
MUST be minimal; code is expected to be self-documenting, and a comment is only justified when
it explains a non-obvious constraint or rationale that the code itself cannot convey.

## Technology Constraints

- Rust, 2021 edition, built with Cargo.
- Nostr protocol access via `nostr-sdk`; shared domain types via `mostro-core` rather than
  redefining Mostro-specific structures locally.
- Async runtime: `tokio`. CLI argument parsing: `clap` (derive API).
- Dependency upgrades that change public behavior MUST be reflected in the relevant spec before
  being merged.

## Development Workflow

- This repository follows spec-kit's production workflow, not the quick-experiment shortcut:
  `constitution → specify → clarify → plan → checklist → tasks → analyze → implement → converge`.
- Every step in that sequence requires an explicit review gate (approve/reject) before moving to
  the next step; skipping a step requires a documented justification in the feature's spec
  directory.
- CI (formatting, linting, tests) MUST pass before any implementation task is considered merged,
  per the project's Phase 5 commitment in the Summer of Bitcoin proposal.

## Governance

This constitution supersedes any conflicting ad hoc practice in this repository. Amendments are
made by editing this file directly, incrementing the version per semantic versioning (MAJOR for
backward-incompatible principle removals or redefinitions, MINOR for new or materially expanded
principles, PATCH for wording/clarity fixes), and prepending an updated Sync Impact Report.
Every pull request MUST be checked against these principles before merge; any deviation MUST be
justified in the PR description and, where it recurs, MUST trigger an amendment to this document
rather than a silent exception.

**Version**: 1.0.2 | **Ratified**: 2026-07-22 | **Last Amended**: 2026-07-22
