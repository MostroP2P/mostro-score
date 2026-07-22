<!--
Sync Impact Report
Version change: template → 1.0.0 (initial ratification)
Modified principles: n/a (first fill of template)
Added sections: all Core Principles (I-VII), Technology Constraints, Development Workflow, Governance
Removed sections: none
Templates requiring updates:
  - .specify/templates/plan-template.md ⚠ pending (verify Constitution Check section references these principles)
  - .specify/templates/spec-template.md ⚠ pending (verify mandatory sections align with Principle I and V)
  - .specify/templates/tasks-template.md ⚠ pending (verify task categories include test-first and modular-architecture task types)
  - .claude/skills/speckit-*/SKILL.md ✅ reviewed, no outdated agent-specific references found
  - README.md ⚠ pending (does not yet mention spec-kit workflow or constitution)
Follow-up TODOs: none, all placeholders resolved from user-supplied input
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

### IV. Test-First Development (NON-NEGOTIABLE)
No implementation task is complete without accompanying unit tests written before or alongside
the code under test, covering at minimum event parsing, order deduplication, and statistical
calculations. The project MUST maintain at least 50% code coverage, per the Summer of Bitcoin
proposal commitment. Red-Green-Refactor is the expected cycle; tests are not optional polish.

### V. Spec-Driven Development
Every feature or metric change MUST go through spec → plan → tasks → implement → verify before
merging to `main`, using this repository's spec-kit artifacts (`specs/NNN-feature/`) as the
single externally-visible source of truth. Any internal reasoning or memory layer used to
support this process is a separate concern and is not part of this constitution.

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

**Version**: 1.0.0 | **Ratified**: 2026-07-22 | **Last Amended**: 2026-07-22
