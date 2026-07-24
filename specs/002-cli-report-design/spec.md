# Feature Specification: CLI Report Structure and Output Format

**Feature Branch**: `002-cli-report-design`

**Created**: 2026-07-22

**Status**: Draft

**Input**: User description: "CLI interface design and report structure specification, Phase 2 of
the Summer of Bitcoin proposal. Defines the report's fixed sections, the activity grid, and the
available output formats (console, plain-text, JSON), scoped strictly to structure and format
selection, not to CLI flag names or argument parsing, which is Phase 3's concern."

## Clarifications

### Session 2026-07-22

- Q: How should the JSON output stay reliable for external consumers as the tool evolves? → A:
  Include an explicit schema version field (e.g. `schema_version`) in the JSON output, so
  consumers can detect a shape change rather than assuming stability implicitly.
- Q: What can the recommendations block (FR-008) legitimately claim about a metric being "high" or
  "low," given most Phase 1 metrics have no established cross-node baseline (explicitly called out
  as an open gap in `specs/reputation_system_v1.md` and the metrics spec)? → A: Only metrics with a
  genuine self-referential baseline (Premium Signal and Trade-Size Consistency, both defined
  against that same node's own history) may be described as higher/lower than normal. Metrics
  without a cross-node baseline (Dispute Signals, Longevity, Cumulative Performance) MUST be
  presented with informative context (e.g., sample size) but MUST NOT be labeled elevated/low/normal
  against an undefined baseline. Metrics that are self-explanatory in raw form MAY be shown as-is;
  metrics that need interpretation to be meaningful to a non-technical trader MUST get a
  plain-language translation that does not alter or overstate what the underlying data actually
  supports.
- Q: This spec was drafted before Phase 1's spec.md was finalized, which then discarded Rating
  Signals (FR-007 there) entirely, since kind `38384` events are keyed by a trader's pubkey, not
  the node's, and cannot support a node-level metric. Does this report-design spec need updating?
  → A: Yes. Every reference to "Rating Signals" as a metric this report displays is removed; the
  general statistics section (FR-006) now lists only the 11 metrics Phase 1 actually kept.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Read a node's report in the console (Priority: P1)

A trader runs the CLI against a node's pubkey and reads the default, human-readable console
report: who the node is, how the relay fetch went, an activity grid showing trends over time, a
full statistics section, and a plain-language recommendations block, so they can decide whether to
trade with this node without inspecting raw data themselves.

**Why this priority**: This is the default, most common way the tool is used; every other output
mode is secondary to getting this experience right.

**Independent Test**: Run the CLI against a node with a rich trade history in an interactive
terminal; verify all 5 sections appear in order, with the activity grid showing multiple time
buckets and the recommendations block giving a plain-language summary.

**Acceptance Scenarios**:

1. **Given** a node with several months of trade history, **When** the trader runs the CLI in an
   interactive terminal with no explicit format flag, **Then** the report renders as colored,
   sectioned console output containing all 5 sections in the defined order.
2. **Given** a node with no notable risk signals, **When** the trader reads the recommendations
   block, **Then** it explicitly states there is nothing notable to flag, rather than omitting the
   block or inventing a recommendation.
3. **Given** a trader unfamiliar with Mostro's reputation metrics, **When** they read any section
   of the report, **Then** each metric shown is labeled with enough context (what it measures,
   and, where relevant, which direction is favorable) that they understand it without leaving the
   tool, per FR-008b.

---

### User Story 2 - Consume the report programmatically (Priority: P2)

A developer building automation (a bot that screens nodes before suggesting a trade, or a
dashboard) runs the CLI with JSON output and parses the complete metric set without needing to
scrape human-readable text.

**Why this priority**: Machine consumption unlocks the tool's use beyond a single interactive
trader, but depends on the report structure from User Story 1 already being defined.

**Independent Test**: Run the CLI against the same node in JSON mode; verify every metric from the
Phase 1 spec (`specs/001-node-reputation-metrics/spec.md`) is present as a field, including nodes
with partial or missing data.

**Acceptance Scenarios**:

1. **Given** a node with complete trade history, **When** the JSON output is requested, **Then**
   the JSON contains every metric defined in the Phase 1 spec, structured consistently.
2. **Given** a node with zero successful trades, **When** the JSON output is requested, **Then**
   metrics that cannot be computed are represented explicitly (e.g., `null` or a not-applicable
   marker), not silently omitted from the JSON structure.

---

### User Story 3 - Pipe the report into another tool (Priority: P3)

A trader redirects the CLI's output to a file, or pipes it into another command-line tool, and
gets a clean, undecorated version of the same report structure, without color codes or formatting
that would corrupt the text.

**Why this priority**: A secondary convenience on top of the console experience, for users who
want the same human-readable content without terminal-specific decoration.

**Independent Test**: Run the CLI with output redirected to a file (not a terminal); verify the
report contains the same 5 sections and content as the console mode, with no ANSI color codes.

**Acceptance Scenarios**:

1. **Given** the CLI's output is redirected to a file rather than an interactive terminal, **When**
   no explicit format override is given, **Then** the tool automatically produces the plain-text
   format instead of the colored console format.
2. **Given** a user explicitly forces console format even while piping output, **When** the report
   is generated, **Then** the tool honors that explicit override rather than the automatic
   context-based default.

---

### Edge Cases

- What happens when a relay in the configured set fails to respond? The relay fetch summary
  section MUST show which relays succeeded and which failed, consistent with the project
  constitution's graceful-degradation principle; the report MUST still render using the relays
  that did succeed.
- What happens when a time bucket in the activity grid has zero activity? That bucket MUST still
  appear as a row with zero values, not be skipped, since a visible gap is itself meaningful
  activity-consistency information.
- What happens when the node has too little history to populate a metric (e.g., zero trades, no
  dev-fee events, zero disputes with zero trades)? The affected fields MUST be shown as explicitly
  not-applicable in every output format, never as a crash, a blank omission, or a misleading zero.
- What happens when output is redirected but the user wants the colored console format anyway
  (e.g., piping into a pager that supports color)? An explicit override MUST always be available
  regardless of what the automatic context-based default would choose.
- What happens when the terminal does not support color at all? The console format MUST still
  render its section structure and tables without color rather than emitting broken escape codes.
- What happens when the `NO_COLOR` environment variable is set, or `TERM` is `dumb`? Color output
  MUST be disabled in both cases, regardless of what the automatic terminal-detection default
  would otherwise choose.
- What happens when a relay is unreachable, the pubkey is invalid, or zero events are found? Each
  MUST map to its own distinct, documented exit code so calling scripts can distinguish them.

## Requirements *(mandatory)*

### Functional Requirements

**Report structure**

- **FR-001**: System MUST organize the report into 5 ordered sections: node identity header, relay
  fetch summary, activity grid, general statistics, and recommendations.
- **FR-002**: The node identity header MUST identify which node the report describes clearly
  enough that the trader can confirm they queried the intended pubkey.
- **FR-003**: The relay fetch summary MUST show which relays were queried and whether each
  succeeded or failed, plus totals for the events fetched across all relays: dev-fee-payment event
  count, order event count, and the deduplicated unique-order count after applying Phase 1's
  qualifying-order procedure, so a trader can sanity-check how much raw data backs the report.
- **FR-004**: The activity grid MUST show one row per time bucket, with successful order count and
  volume (from Cumulative Performance), median trade size (from Trade Statistics), and active days
  plus max inactive gap (from Activity Consistency) as columns.
- **FR-005**: The activity grid's time bucket granularity MUST be selectable (e.g., daily, monthly,
  yearly) rather than fixed to hardcoded day windows; the exact selection mechanism is out of scope
  for this spec.
- **FR-005a**: When a fine granularity (e.g., daily) is combined with a very wide time range, the
  system MUST warn the user that this can produce a large number of rows, rather than silently
  rendering an unbounded grid.
- **FR-006**: The general statistics section MUST show Longevity, Cumulative Performance (the
  node's lifetime `total_successful_trades` and `total_volume_sats`, shown as a standalone figure
  and not just left implicit in the activity grid's per-bucket rows), the Liveness rolling windows,
  full Trade Statistics, Trade-Size Consistency, Dispute Signals (including its resolved-versus-
  active breakdown), Fiat Breakdown, Payment Method Breakdown, and Premium Signal (including its
  dispersion figure), all as defined in the Phase 1 metric spec. Rating Signals is not part of this
  list: Phase 1 discarded it entirely, since kind `38384` cannot support a node-level metric.
- **FR-007**: Bond Policy MUST be shown as its own distinctly labeled block, separate from
  trade-history statistics, since it describes a node policy setting rather than a historical
  metric.
- **FR-008**: The recommendations block MUST synthesize the metrics above into plain-language
  guidance, and MUST explicitly state that there is nothing notable to flag when no signal warrants
  one, rather than omitting the block or fabricating a recommendation.
- **FR-008a**: The recommendations block MUST NOT describe a metric as higher, lower, or more/less
  "normal" unless that comparison is against a genuine baseline the tool actually computes (a
  node's own historical value, as with Premium Signal and Trade-Size Consistency). Metrics without
  a cross-node baseline MUST be presented with informative context (e.g., sample size) instead of
  an unsupported elevated/low/normal label, and no metric's plain-language translation may overstate
  or alter what the underlying data actually supports.
- **FR-008b**: Every section and every metric shown in the report MUST carry enough labeling or
  inline explanation that a trader can understand what it means and how it was computed without
  leaving the tool or consulting external documentation, per the Summer of Bitcoin proposal's Phase
  2 commitment to "provide documentation explaining each section and the meaning of every metric."
  This is a property of the report's own structure and content (what this spec defines), not of a
  separate `--help` flag or user manual (Phase 3's concern, see Assumptions). A metric name alone
  (for example, a bare "Premium Signal" label) does not satisfy this requirement; the report MUST
  also convey, at minimum, what the number represents and, where relevant, what direction is
  favorable, consistent with each metric's "Meaning" and "Unit and direction" as defined in the
  Phase 1 metric spec and its companion decisions document.

**Output format**

- **FR-009**: System MUST support 3 output formats: a human-readable console format (sectioned,
  tabular, colored), a plain-text format (the same 5 sections and content as console, no color or
  decoration, with each metric rendered as one `label: value` line rather than in a decorative
  table, so the output stays easy to grep or parse line by line in scripts), and a JSON format
  (complete machine-readable structure).
- **FR-010**: System MUST select a sensible default output format based on execution context
  (e.g., whether output is going to an interactive terminal versus being redirected or piped),
  while always allowing an explicit override of that default.
- **FR-011**: Error conditions (unreachable relay, no data found, malformed events) MUST be
  presented as clear, actionable messages in every output format, never as raw stack traces, per
  the project constitution's graceful-degradation principle.
- **FR-012**: JSON output MUST include a stable, complete set of fields regardless of whether the
  underlying node has enough data to compute every metric, representing missing or not-applicable
  metrics explicitly rather than omitting keys.
- **FR-012a**: JSON output MUST include an explicit schema version field, so external consumers
  can detect a future shape change instead of assuming the structure is implicitly stable.

**Polish and accessibility**

- **FR-013**: Tabular sections (activity grid, general statistics) MUST render within the
  terminal's actual available width, wrapping or adapting content rather than assuming a fixed
  width.
- **FR-014**: Any relay fetch expected to take more than a couple of seconds MUST show a visible
  progress indicator, skipped entirely when output is not going to a terminal.
- **FR-015**: The console format MUST disable color automatically when standard output is not a
  terminal, when the `NO_COLOR` environment variable is set, or when `TERM` is `dumb`; an explicit
  override to force color off MUST also be available regardless of these automatic checks.
- **FR-016**: Color MUST be used with intent and MUST NOT be the only way a risk or warning signal
  (e.g., an elevated dispute ratio, a rating divergence) is conveyed, so the signal remains legible
  in plain-text mode and to users who cannot perceive color.
- **FR-017**: Error and warning messages MUST be written to standard error, while report content
  MUST be written to standard output, so redirecting the report does not also capture diagnostic
  noise.
- **FR-018**: Numeric values (sats, fiat amounts) MUST be formatted with thousands separators for
  readability.
- **FR-019**: The tool MUST return exit code `0` on success, and a distinct, documented exit code
  for each main failure case, so calling scripts can react to each case individually: `1` for a
  general error, `2` for an invalid public key, `3` for an unreachable relay (all configured relays
  failed; per the constitution's graceful-degradation principle, a single failed relay among
  several that succeeded is not this case), and `4` for zero events found across all successfully
  queried relays.

### Key Entities

- **Report**: The complete output for one node, composed of the 5 ordered sections; the artifact
  this feature produces.
- **Node Identity Header**: The section identifying which node the report describes.
- **Relay Fetch Summary**: The section reporting which relays were queried and their success or
  failure state.
- **Activity Bucket**: One row of the activity grid, covering one time bucket's worth of the
  relevant Phase 1 metrics.
- **General Statistics Section**: The section aggregating the lifetime and descriptive metrics
  from the Phase 1 spec.
- **Recommendations Block**: The section synthesizing the metrics into plain-language guidance.
- **Output Format**: One of console, plain-text, or JSON; determines how the same underlying report
  content is rendered.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A trader can determine, from the console report alone, whether a node is currently
  active, its typical trade size, and whether it carries any risk flags, without inspecting raw
  Nostr events.
- **SC-002**: A script consuming the JSON output can extract every metric defined in the Phase 1
  spec without parsing human-readable text.
- **SC-003**: When output is redirected to a file or another program without an explicit format
  override, the resulting content contains no ANSI color codes or decoration that would interfere
  with automated parsing.
- **SC-004**: A report generated for a node with zero trade history renders successfully in all 3
  output formats, with the affected metrics explicitly marked as not-applicable rather than the
  tool erroring or silently omitting sections.
- **SC-005**: A user who cannot perceive color, or who reads the plain-text format, can still
  identify every risk or warning signal from textual markers alone, without relying on color.
- **SC-006**: A trader with no prior knowledge of Mostro's reputation metrics can read any section
  of the report and understand what each metric measures and, where relevant, which direction is
  favorable, without leaving the tool or consulting a separate manual, per FR-008b.

## Assumptions

- The exact CLI flags or arguments used to choose bucket granularity or force a specific output
  format are Phase 3's concern (CLI parameters and user manual), not part of this spec. This also
  includes shell completion generation (bash/zsh/fish), the `--help` flag's own text, and the
  separate user manual document (setup, parameters, constraints, common use cases), all of which
  belong with Phase 3's CLI-parameter work per the Summer of Bitcoin proposal's own phase split, not
  this report-structure spec. FR-008b's requirement that the report explain its own sections and
  metrics is distinct from and does not substitute for that Phase 3 work.
- "Sensible default based on context" follows the common convention of detecting whether output is
  going to an interactive terminal versus being redirected or piped; the precise implementation
  mechanism is deferred to the planning phase.
- The recommendations block's judgment logic (what counts as "notable") is a presentation-level
  decision, following the same non-normative derived-indicator convention already established in
  `specs/reputation_system_v1.md` Section 5 (e.g., suggested safe trade size, activity status
  labels), not a hardcoded rule defined by this spec.
- JSON output is structured to mirror the same 5-section report structure (nested by section)
  rather than as one flat list of metrics, so a consumer can navigate it the same way a human reads
  the console report.
- This spec deliberately does not name specific rendering libraries (table layout, progress
  indication) for FR-013/FR-014, consistent with the constitution's separation between spec (what)
  and plan (how); library choices for these capabilities belong to the Phase 4 planning step.
