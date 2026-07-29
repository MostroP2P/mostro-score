# Feature Specification: CLI Parameters

**Feature Branch**: `003-cli-parameters`

**Created**: 2026-07-24

**Status**: Ready for Planning

**Input**: User description: "CLI parameters specification, Phase 3 of the Summer of Bitcoin
proposal. Formalizes the CLI flags currently living only in an informal gist into a ratified
spec, defines their default values and constraints, and resolves known naming, precedence, and
completeness gaps rather than carrying the gist over as-is. Reuses, and does not redefine, the
output-format contract from specs/002-cli-report-design/spec.md and the metric set from
specs/001-node-reputation-metrics/spec.md."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Run the tool with no flags beyond the pubkey (Priority: P1)

A trader runs the CLI with only `--pubkey`, and gets a complete, useful default report: default
relay, default time range (full history), default output format chosen automatically for the
execution context, and a default activity-grid granularity, without needing to learn any other
flag first.

**Why this priority**: This is the tool's entry point; every other flag in this spec is an
optional refinement on top of this default path, and if the defaults are not sensible, the
flags below cannot compensate for it.

**Independent Test**: Run `mostro-score --pubkey <PUBKEY>` with no other flags and no relevant
environment variables set; verify the report renders using the compiled-in default relay,
covers full history, and picks console or plain-text format automatically based on whether
output is an interactive terminal, per specs/002-cli-report-design FR-010.

**Acceptance Scenarios**:

1. **Given** no `--relays` flag, no relay environment variable, and no relay value in a
   configuration file, **When** the tool runs, **Then** it queries the compiled-in default
   relay (`wss://relay.mostro.network`).
2. **Given** no `--since`/`--until` flags, **When** the tool runs, **Then** the report covers
   the node's full available history.
3. **Given** no `--view` flag and no `view` value in a configuration file, **When** the activity
   grid renders, **Then** it uses an automatically chosen granularity rather than failing or
   requiring the flag.

---

### User Story 2 - Scope the activity grid to a time range and grouping (Priority: P2)

A trader investigating recent node behavior sets `--since`, `--until`, and `--view` to look at,
for example, the last 30 days grouped by day, instead of the activity grid's default range and
granularity. The general statistics section's lifetime-anchored metrics (Longevity, Cumulative
Performance, Liveness, Dispute ratio, and the rest of Phase 1's metric set) are unaffected: they
remain computed over the node's full history, or their own fixed windows, exactly as Phase 1 and
specs/002-cli-report-design FR-006 already define, regardless of these flags.

**Why this priority**: Time scoping is the most common refinement on top of the default report,
and depends on User Story 1's default path already working.

**Independent Test**: Run the CLI with `--since 30d --view daily` against a node with a long
history; verify the activity grid only shows buckets within the last 30 days, bucketed by UTC
calendar day per specs/002-cli-report-design FR-005, while the general statistics section's
figures are identical to a run with no `--since`/`--until` at all.

**Acceptance Scenarios**:

1. **Given** `--since 30d`, **When** the tool runs, **Then** the activity grid only shows
   buckets from the last 30 days, computed relative to the current UTC time, while the general
   statistics section still reports the node's full-history figures, unaffected by this flag.
2. **Given** `--since` later than `--until`, **When** the tool parses the flags, **Then** it
   rejects the combination with an actionable validation error before querying any relay.
3. **Given** `--view daily` combined with a multi-year `--since`, **When** the tool runs,
   **Then** it surfaces the wide-range warning specs/002-cli-report-design FR-005a already
   defines, since this spec's flags are what produce that combination.

---

### User Story 3 - Run the tool non-interactively for automation (Priority: P3)

A developer scripting node audits sets `MOSTRO_SCORE_PUBKEY` and `MOSTRO_SCORE_RELAYS` as
environment variables, and runs the CLI with `--format json --quiet`, without passing
`--pubkey`/`--relays` as flags on every invocation.

**Why this priority**: Automation is a secondary consumption mode on top of the interactive
default, matching specs/002-cli-report-design's own User Story 2 priority ordering for JSON
output.

**Independent Test**: With `MOSTRO_SCORE_PUBKEY` and `MOSTRO_SCORE_RELAYS` set and no
corresponding flags passed, run the CLI with `--format json`; verify it queries the pubkey and
relays from the environment and emits valid JSON with no prose.

**Acceptance Scenarios**:

1. **Given** `MOSTRO_SCORE_PUBKEY` set and no `--pubkey` flag, **When** the tool runs, **Then**
   it uses the environment variable's value.
2. **Given** both `MOSTRO_SCORE_PUBKEY` and an explicit `--pubkey` flag, **When** the tool runs,
   **Then** the explicit flag value takes precedence.
3. **Given** `--quiet`, **When** the console or plain-text report renders, **Then** progress
   indicators and transient status narration are suppressed, while every metric value and every
   specs/002-cli-report-design-mandated piece of report content (recommendations text, per-metric
   inline explanations) still appears unchanged.

---

### User Story 4 - Filter which sections display (Priority: P4)

A trader who only cares about a node's activity trend runs the CLI with
`--sections activity,stats` to avoid scrolling past sections they don't need right now.

**Why this priority**: A display convenience on top of the already-complete report; lowest
priority because omitting it does not block any other story from delivering value.

**Independent Test**: Run the CLI with `--sections stats` against a node with rich
history; verify only the node identity header (always shown) and the general statistics
section render, with identical metric values to the same section in the unfiltered report.

**Acceptance Scenarios**:

1. **Given** `--sections` naming a valid subset of the 4 filterable sections (FR-008), **When**
   the tool runs, **Then** only those sections (plus the always-shown node identity header)
   render, with unchanged content.
2. **Given** `--sections` naming an unknown section, **When** the tool parses the flag, **Then**
   it rejects the value with an error listing the valid section names, before querying any
   relay.

---

### User Story 5 - Scaffold a config file to save preferred defaults (Priority: P5)

A trader who always uses the same relays and prefers plain-text output runs
`mostro-score --init-config` once, gets a starter TOML file at the platform-standard location
populated with the tool's current defaults, edits the values they want to change, and never has
to repeat those flags again.

**Why this priority**: A one-time convenience layered entirely on top of the already-optional
persisted-configuration feature (User Stories 1-4 all work with zero configuration); lowest
priority because omitting it does not block any report from being generated.

**Independent Test**: Run `mostro-score --init-config` with no existing configuration file
present; verify a TOML file is written at the resolved path setting `relays` to its compiled-in
default, with `pubkey`, `format`, `view`, `color`, and `sections` present only as commented-out
examples (not active values), and that the command exits `0` without attempting to fetch any
report data.

**Acceptance Scenarios**:

1. **Given** no configuration file exists yet at the resolved path, **When** the user runs
   `--init-config`, **Then** a starter file is written there with the tool's current defaults.
2. **Given** a configuration file already exists at the resolved path, **When** the user runs
   `--init-config` without `--force`, **Then** the tool refuses to overwrite it and reports the
   existing file's path.
3. **Given** `--init-config` is passed together with `--pubkey`, **When** the tool runs,
   **Then** it performs only the scaffold action and does not generate a report.

---

### Edge Cases

- What happens when the configuration file exists but is malformed (invalid TOML)? The tool
  MUST warn to standard error and ignore the file entirely, falling back to the
  environment-variable/compiled-default chain, per FR-015a — never a fatal error.
- What happens when `--init-config` targets a path that already has a configuration file? The
  tool MUST refuse to overwrite it, report the existing file's path, and require an explicit
  `--force` flag to proceed, per FR-018.
- What happens when `--init-config` is combined with `--pubkey` or another report-generating
  flag? `--init-config` MUST take precedence and short-circuit report generation entirely, per
  FR-019.
- What happens when `--pubkey` is omitted, its fallback environment variable is unset, and no
  configuration file supplies a `pubkey` value either (FR-016, amended 2026-07-26)? The tool
  MUST reject with a usage error (exit code `2`, per FR-013a) before attempting any relay
  connection; this is a missing required argument, distinct from specs/002-cli-report-design
  FR-019's exit code `5` for a syntactically-provided but invalid pubkey value.
- What happens when `--relays` or the relay environment variable contains a malformed URL? The
  tool MUST reject the malformed entry with an actionable message identifying which relay
  string failed to parse, before attempting to connect to any relay.
- What happens when `--since`/`--until` use a relative shorthand with an unrecognized unit
  (e.g. `30x`)? The tool MUST reject it with an error naming the accepted units (`d`, `mo`,
  `y`), before querying any relay.
- What happens when `--no-color` and `--color` are both passed? The tool MUST reject the
  combination as contradictory, rather than silently letting one win.
- What happens when `--quiet` is combined with `--format json`? Quiet has no additional effect
  beyond suppressing progress indicators, since JSON output already has no transient narration or
  decoration to suppress; this MUST NOT alter the JSON completeness contract from
  specs/002-cli-report-design FR-012.
- What happens when `--quiet` is used against a node with no notable risk signals? The
  recommendations block's mandatory "nothing notable to flag" statement (specs/002-cli-report-design
  FR-008) MUST still appear: `--quiet` suppresses transient narration and progress indicators
  only, never content specs/002-cli-report-design requires, per FR-012.
- What happens when `--sections` is combined with `--format json`? `--sections` has no effect
  on JSON output: it is a console/plain-text display filter only, per FR-008. JSON always
  returns the complete, stable structure specs/002-cli-report-design FR-012 requires, so
  automated consumers get a predictable schema independent of any display preference.
- What happens when a configuration file is syntactically valid TOML but contains a
  semantically invalid value (a malformed `pubkey` or relay URL, an unrecognized
  `--format`/`--view` value, an unrecognized `--sections` name, or contradictory color
  settings)? The tool MUST treat this the same as a parse failure per FR-015a: warn to standard
  error identifying the file, the offending key, and the problem, then ignore the file entirely
  and fall back to the environment-variable/compiled-default chain, never a fatal error and
  never a partial application of the file's other, valid values.

## Requirements *(mandatory)*

### Functional Requirements

**Connection**

- **FR-001**: `-p`/`--pubkey` MUST identify the node to analyze, accepted as either npub or hex
  format, matching the tool's current behavior. It MUST be provided via the flag, via its
  environment-variable fallback (FR-003), or via a configuration-file value (FR-016, amended
  2026-07-26); the tool MUST reject with a usage error before any relay connection when none of
  the three is present.
- **FR-002**: `-r`/`--relays` MUST accept a comma-separated list of relay URLs. When omitted, it
  MUST fall back to its environment variable (FR-003), then to the compiled-in default
  `wss://relay.mostro.network`.
- **FR-003**: `--pubkey` and `--relays` MUST each support an environment-variable fallback
  (`MOSTRO_SCORE_PUBKEY`, `MOSTRO_SCORE_RELAYS`). Precedence, evaluated independently per flag,
  MUST be: explicit CLI flag, then environment variable, then compiled-in default when one
  exists. `--relays` has a compiled-in default and follows the full chain. `--pubkey` has no
  compiled-in default: per FR-001, when neither the flag, its environment variable, nor a
  configuration-file value (FR-016) is present, the tool MUST reject with a usage error rather
  than falling back further.

**Time range and grouping**

- **FR-004**: `--since`/`--until` MUST each accept either an ISO 8601 calendar date or a
  relative duration shorthand (`30d`, `6mo`, `1y`, using `d`/`mo`/`y` units, `N` MUST be a
  positive integer — `0` or negative is rejected as invalid), and scope the
  **activity grid's requested range only** — the range specs/002-cli-report-design FR-005
  refers to without defining how it is supplied. They MUST NOT affect the general statistics
  section: every metric there remains computed over the node's full history or its own fixed
  window, exactly as Phase 1 and specs/002-cli-report-design FR-006 already define. Both flags
  resolve to UTC calendar-day boundaries, matching the convention specs/002-cli-report-design
  FR-005 already establishes for bucket boundaries: `--since` resolves to `00:00:00` UTC on the
  resolved day (inclusive), `--until` resolves to `23:59:59.999` UTC on the resolved day
  (inclusive), with `--since` applying that day's start-of-day boundary and `--until` its
  end-of-day boundary. `Nd` subtracts `N - 1` days from the current UTC date, so `Nd` means
  exactly N calendar days including today (matching User Story 2's "last 30 days"), not `N + 1`.
  `Nmo`/`Ny` subtract the full `N` calendar months/years instead (no `- 1` adjustment: `1mo`
  means one month back, not today), using calendar units rather than fixed 30-day/365-day
  durations; when the resulting month has no day matching the original day-of-month (e.g.,
  subtracting `1mo` from March 31, or `1y` from February 29 in a leap year), the result MUST
  clamp to the last valid day of that resulting month (February 28 or 29, as applicable), never
  overflow into the next month or error. When only `--since` is given, `--until` defaults to the
  exact report-generation instant, not the end-of-day boundary that an explicit or shorthand
  `--until` would resolve to, so the grid never reaches into activity that hasn't happened yet;
  when only `--until` is given, `--since` defaults to the node's earliest available history.
  When omitted, the activity grid MUST cover the node's full available history, matching current
  tool behavior.
- **FR-005**: When `--since` is explicitly given (flag or shorthand, not defaulted) and resolves
  to a point later than `--until`, the tool MUST reject the combination with an actionable
  validation error before querying any relay. This check does not apply when `--since` is
  defaulted to the node's earliest history (FR-004), since that value is unknown before
  querying; a defaulted `--since` later than `--until` is not an error, just an ordinary, empty
  activity grid. Whether a given range actually contains any events is likewise unknowable
  before querying and is not part of this requirement.
- **FR-006**: `--view` MUST select the activity grid's time-bucket granularity (`daily`,
  `monthly`, `yearly`), providing the selection mechanism specs/002-cli-report-design FR-005
  leaves unspecified. This alignment rule applies only when `--view` is passed as an explicit
  `--view monthly`/`--view yearly` CLI flag — never when the granularity instead comes from a
  configuration file (FR-016) or automatic selection, both of which MUST always snap rather than
  reject (see below), since the user isn't actively choosing that granularity in this
  invocation. With an explicit `--view` flag, an explicitly given `--since` MUST resolve to the
  first day of a calendar month/year and an explicitly given `--until` MUST resolve to the last
  day of one; a value resolving anywhere else MUST be rejected with a validation error before
  querying any relay. In every other case — a *defaulted* `--since`/`--until` (FR-004: earliest
  history, or now), a configuration-sourced `--view`, or automatic selection — `--since`/`--until`
  MUST instead snap to the enclosing bucket's start/end respectively, never rejected. An empty
  or inverted range (FR-005) stays empty regardless of snapping: snapping MUST NOT turn it into
  a non-empty one. It MUST
  choose a granularity automatically based on
  the requested range, so a user who never passes `--view` still gets a usable grid; the exact
  automatic-selection thresholds are a planning-level decision (see Assumptions), not fixed
  here, consistent with specs/002-cli-report-design's own qualitative, no-invented-threshold
  precedent for FR-005a.
- **FR-007**: The wide-range warning specs/002-cli-report-design FR-005a defines MUST fire when
  this spec's `--view`/`--since`/`--until` flags combine to request a fine granularity over a
  wide range; the warning's own trigger condition and text remain defined by FR-005a, not
  redefined here.

**Section filtering**

- **FR-008**: `--sections` MUST accept a comma-separated subset of the 4 *filterable*
  specs/002-cli-report-design FR-001 sections, addressed by the exact, case-sensitive tokens
  `fetch`, `activity`, `stats`, `recommendations` (in that same order). The node identity header
  is not a valid token and is not affected by this flag: it always renders, per the Assumptions
  below. When `--sections` is omitted and no configuration-file `sections` value is present
  either (FR-016, amended 2026-07-26), all 5 sections render, matching current tool behavior.
  This flag controls **console and plain-text display only** and MUST NOT alter
  section content or computation; it has no effect on `--format json`, whose complete structure
  MUST always be returned regardless of `--sections`, preserving specs/002-cli-report-design
  FR-012's stable, complete schema contract. This spec's `--sections` filter is a documented,
  Phase 3-scoped exception to specs/002-cli-report-design FR-001's and FR-009's fixed 5-section
  structure for console and plain-text output specifically: when `--sections` narrows the set,
  the reduced set of sections is the complete, valid report for that invocation, not a violation
  of those requirements.
- **FR-009**: An unrecognized name passed to `--sections` MUST be rejected with a validation
  error listing the valid section names, before querying any relay.

**Output and display**

- **FR-010**: `--format` MUST accept exactly the 3 values specs/002-cli-report-design FR-009
  defines (`console`, `plain`, `json`), selecting between them explicitly. This spec adds the
  flag name and syntax only; format behavior itself remains defined by specs/002-cli-report-design.
- **FR-011**: `--no-color` MUST force color off, satisfying specs/002-cli-report-design FR-015's
  requirement for an explicit force-off override. `--color` MUST force color on, satisfying
  that same FR-015's requirement for an explicit force-on override (e.g., piping into a
  color-aware pager), **scoped to console output only**, matching FR-015's own console-only
  scope, including that same requirement's `TERM=dumb` exception: `--color` MUST NOT be honored
  when `TERM=dumb`, since that signals a technical incapability to render ANSI codes, not a
  preference. `--color` MUST imply console format only when format resolution reaches FR-010's
  fully automatic step — no `--format` flag and no configuration-file value (FR-016) — overriding
  that automatic redirected-output default so piping into a color-aware pager works as intended.
  A configuration-sourced `format` value is a saved preference and takes precedence over this
  implication, same as an explicit `--format` flag. `--color` combined with a `--format plain` or
  `--format json` that came from either the CLI flag or the configuration file MUST have no
  effect: plain text stays undecorated per
  specs/002-cli-report-design FR-009, and JSON stays valid, colorless JSON per the same
  requirement, regardless of `--color`. Passing both `--no-color`
  and `--color` together MUST be rejected as a contradictory combination.
- **FR-012**: `-q`/`--quiet` MUST suppress progress indicators (specs/002-cli-report-design
  FR-014) and the tool's own transient status narration (e.g., "Fetching history..."), while
  every requirement specs/002-cli-report-design mandates as report *content* MUST still appear
  unchanged: every metric value, the recommendations block's mandatory statement including its
  explicit "nothing notable to flag" case (FR-008), and each metric's minimum inline explanation
  (FR-008b). `--quiet` trims transient and decorative output, never content specs/002-cli-report-design
  requires. In `json` format, `--quiet` MUST have no effect beyond suppressing progress
  indicators, since JSON output carries no transient narration or decoration to begin with, and
  MUST NOT alter specs/002-cli-report-design FR-012's completeness contract.

**Standard flags**

- **FR-013**: `-h`/`--help` and `-V`/`--version` MUST behave per standard CLI convention,
  printing usage information or the tool's version and exiting `0`.
- **FR-013a**: A CLI-level usage error (missing required flag, unparseable value, unrecognized
  flag or value) MUST exit with code `2`, matching both specs/002-cli-report-design FR-019's
  definition of that code (amended 2026-07-24 to mean CLI usage error) and the argument-parsing
  library's own native default behavior for its plain entry point — no custom exit-code handling
  is required for this case. This is distinct from a syntactically-provided but invalid
  `--pubkey` value (specs/002-cli-report-design FR-019's exit code `5`): the library cannot
  detect that during argument parsing, since it accepts `--pubkey` as a plain string; that
  validation happens in application code after parsing succeeds, and exits `5` explicitly. A
  usage error occurring during argument parsing itself MUST print the library's own plain,
  human-readable usage text to standard error regardless of `--format`, since the parser has not
  yet handed control back to the application to know which format was requested; this case is
  explicitly outside specs/002-cli-report-design FR-011's JSON fatal-error envelope, which
  applies only to post-parse fatal errors where `--format` is already known.

**Persisted configuration**

- **FR-014**: `-d`/`--config-dir` (renamed from the gist's `--dirsettings` for naming
  consistency with this project's kebab-case flags) MUST override the directory the tool reads
  a configuration file from. When omitted, the tool MUST use the platform-standard user
  configuration directory: `$XDG_CONFIG_HOME/mostro-score/config.toml` on Linux, falling back
  to `~/.config/mostro-score/config.toml` when `XDG_CONFIG_HOME` is unset; `~/Library/Application
  Support/mostro-score/config.toml` on macOS; `%APPDATA%\mostro-score\config.toml` on Windows —
  matching standard Rust CLI tool convention (e.g. the `directories` crate's project-directory
  resolution).
- **FR-015**: The configuration file MUST be entirely optional; its absence MUST NOT be an
  error, and nothing in this spec creates it automatically — not tool installation, and not an
  ordinary report-generating invocation. With no file present, the tool falls back fully to the
  flag/environment-variable/compiled-default chain FR-003 already defines. The only way the file
  comes into existence is the explicit `--init-config` action (FR-017).
- **FR-015a**: When a configuration file is present at the resolved path but cannot be read (a
  permissions error, a broken symlink, or another I/O error), fails to parse (malformed TOML), or
  contains a value that would fail the same validation the equivalent CLI flag requires (a
  malformed relay URL, an unrecognized `--format`/`--view` value, a malformed `--pubkey` value,
  an unrecognized `--sections` name, or contradictory color settings), the tool MUST warn to
  standard error identifying the file's path and the specific problem, then ignore the file
  **entirely** (never applying only its valid keys) and fall back fully to the
  flag/environment-variable/compiled-default chain, consistent with the project constitution's
  graceful-degradation principle (Principle VI) already applied to relay failures. This MUST NOT
  be treated as a fatal error: a config-sourced `--pubkey` value is validated at this point using
  the same check as an explicit `--pubkey` flag, so an invalid one is caught and ignored here,
  before it could otherwise surface later as specs/002-cli-report-design FR-019's exit-`5`
  invalid-pubkey error; if no `--pubkey`/`MOSTRO_SCORE_PUBKEY`/config value is present at all,
  that remains FR-001's existing missing-value usage error, unaffected by this amendment.
- **FR-016** *(amended 2026-07-26 to add `--pubkey` and `--sections` — see Clarifications)*: A
  present configuration value for `--pubkey`, `--relays`, `--format`, `--view`,
  `--no-color`/`--color`, or `--sections` MUST be honored, extending FR-003's chain to: CLI flag,
  then environment variable (`--pubkey` and `--relays` only), then configuration file. When
  absent from the file, `--pubkey` falls back to FR-001's existing requiredness rule; `--relays`
  falls back to its compiled-in default; `--format`, `--view`, and color mode fall back to their
  own automatic resolution (specs/002-cli-report-design FR-010, this spec's FR-006,
  specs/002-cli-report-design FR-015); `--sections` falls back to FR-008's unfiltered default
  (every section renders).
- **FR-016a**: The configuration file's top-level keys MUST be: `pubkey` (string, npub or hex,
  validated the same way an explicit `--pubkey` flag is), `relays` (array of strings, MUST
  be non-empty when present — an empty array MUST be treated as absent, falling back through
  FR-016's chain, not as "no relays to query"), `format` (string: `console`/`plain`/`json`),
  `view` (string: `daily`/`monthly`/`yearly`), `color` (string: `always`/`never`; absent means
  automatic), `sections` (array of strings, each one of FR-008's 4 filterable section names —
  same case-sensitive validation as `--sections`; an empty array MUST be treated as absent,
  same rule as `relays`, falling back to FR-008's unfiltered default rather than "show nothing").
  Any other value is a semantic validation failure under FR-015a.
- **FR-017**: `--init-config` MUST write a starter configuration file to the resolved config
  path (creating the directory if needed) and exit `0` without generating a report. It MUST set
  `relays` to its compiled-in default, and MUST include `pubkey`, `format`, `view`, `color`, and
  `sections` as commented-out examples, not active values, since none of the five has a fixed
  default to freeze. Each key in the written file MUST be preceded by a short comment explaining
  what it does and which values it accepts. `--pubkey` is not required for `--init-config`.
- **FR-018**: `--init-config` MUST NOT silently overwrite an existing configuration file at the
  resolved path; when one already exists, the tool MUST refuse, report the existing file's
  path, and exit non-zero, requiring an explicit `--force` companion flag to overwrite it
  intentionally. `--force` MUST be rejected as a usage error (exit code `2`, per FR-013a) when
  passed without `--init-config`, since it has no meaning on its own.
- **FR-019**: When `--init-config` is combined with any report-generating flag (e.g.
  `--pubkey`), `--init-config` MUST take precedence: the tool performs only the scaffold action
  and exits, never generating a report in the same invocation. Report-scoped semantic
  validation that only matters for report generation (FR-005's range check, FR-009's
  section-name check, FR-011's contradictory-color check) MUST NOT be evaluated against other
  flags passed alongside `--init-config` in the same invocation, since no report is generated;
  only `--init-config`'s own overwrite check (FR-018) applies. Basic argument-syntax parsing
  (e.g., rejecting an unparseable value type) still applies to every flag regardless.

**Report output destination** *(added 2026-07-26 — see Clarifications)*

- **FR-020**: `-o`/`--output <PATH>` MUST write the rendered report to the given file instead of
  standard output. Valid only when the resolved format is `plain` or `json`: whenever format
  resolution lands on `console` — whether from an explicit `--format console` flag or from a
  configuration-sourced `format = "console"` value (FR-016, which gives that value the same
  precedence as an explicit flag) — combining it with `--output` MUST be rejected as a usage
  error before querying any relay. When `--output` is present and neither an explicit flag nor a
  configuration-sourced value selects a format, resolution MUST default to `plain` rather than
  performing specs/002-cli-report-design FR-010's terminal-based automatic detection, since
  writing colored console escape codes to a file has no benefit. On
  success, the tool MUST print a confirmation message naming the written path to standard
  error; this message is a diagnostic fact, not transient progress narration, so it MUST NOT be
  suppressed by `--quiet` (FR-012). The report's content MUST be written only to the specified
  file, never additionally to standard output.
- **FR-021**: A successfully rendered `--format json` **report** MUST be pretty-printed
  (multi-line, indented) rather than a single-line/minified document, whether written to standard
  output or to an `--output` file. This applies to the report body only: it does not alter
  specs/002-cli-report-design FR-012's completeness contract, this spec's own FR-013a's separate
  parser-level usage-error text (which remains plain and unaffected), nor
  specs/002-cli-report-design FR-011's fatal-error envelope, which MUST remain a single-line
  document — one JSON object per line is a deliberate, machine-log-friendly convention for a
  fatal condition, distinct from the multi-line report a successful run produces.

### Key Entities

- **CLI Flag**: A named, user-supplied parameter controlling one aspect of report generation
  (connection target, time scope, grouping, section selection, output format, or display).
- **Environment Variable Fallback**: A named environment variable supplying a flag's value when
  the flag itself is omitted, applicable to `--pubkey` and `--relays` per FR-003.
- **Report Section Name**: One of the 5 fixed section identifiers from
  specs/002-cli-report-design FR-001, used as the vocabulary `--sections` filters against.
- **Configuration File**: An optional, platform-located TOML file supplying persisted user
  preferences for `--pubkey`, `--relays`, `--format`, `--view`, color mode, and `--sections`, per
  FR-014 through FR-016a, created via `--init-config` per FR-017 through FR-019.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A trader gets a complete, useful report by supplying only a pubkey, with no other
  flag required.
- **SC-002**: A user can scope the activity grid to any time window and grouping without
  needing to know the tool's internal date-handling or bucketing conventions beyond the
  documented shorthand, while the rest of the report's lifetime-anchored metrics stay
  unaffected.
- **SC-003**: A script can run the tool fully non-interactively, using only environment
  variables, with no interactive flag required.
- **SC-004**: In a report-generating invocation (`--init-config` absent), every invalid flag
  value (malformed date range, unknown section name, malformed relay URL, contradictory color
  flags) is rejected with an actionable message before any relay is queried, never after a
  partial or failed fetch. `--init-config` invocations skip these report-scoped validations
  entirely, per FR-019.
- **SC-005**: A report filtered to a subset of sections via `--sections` shows metric values
  identical to the same sections in the unfiltered report.

## Assumptions

- specs/002-cli-report-design FR-019's exit codes were amended 2026-07-24 (see that spec's
  Clarifications) so `2` matches the mandated argument-parsing library's own native default for
  CLI usage errors (FR-013a), moving "invalid public key" to `5`; this is a surgical, documented
  cross-spec change, not a new exit code this spec introduces independently.
- The node identity header always renders regardless of `--sections`, since every other section
  depends on the trader first confirming which node the report describes; `--sections` is
  documented here as filtering the other 4 sections only.
- `--view`'s automatic-granularity default thresholds are a planning-level "how" decision, to be
  set after reviewing typical report ranges, not invented here without that evidence — the same
  posture specs/002-cli-report-design already takes for FR-005a's qualitative trigger.
- Environment variable names use a `MOSTRO_SCORE_` prefix to avoid collision with unrelated
  environment variables, following common CLI convention.
- Shell completion generation, the full `--help` text content, and the standalone user manual
  (setup, common use cases) remain a separate deliverable from this spec: this spec defines
  each flag's behavior and constraints; the reader-facing explanation of what each flag means
  belongs to the project's documentation book (a parallel Phase 3 deliverable), not to this spec.
- The configuration file's format (TOML) follows the same technology-appropriate default as the
  rest of the Technology Constraints section of the project constitution (Rust/Cargo ecosystem
  convention); this spec does not name a specific parsing library, consistent with the
  constitution's separation between spec (what) and plan (how).

## Clarifications

### Session 2026-07-24

- Q: The source gist lists a `-d`/`--dirsettings` flag ("custom settings folder path instead of
  default") with no further detail. What does this flag actually configure, and does the tool
  need a persisted-settings concept at all for Phase 3? → A: It points to a directory holding an
  optional configuration file that persists preferences for `--relays`, `--format`, `--view`, and
  color mode, not `--pubkey`. Explicit flags and environment variables always override it; its
  absence is not an error. See FR-014 through FR-016.
- Q: Where does that configuration file live by default, before any `--config-dir` override? →
  A: The platform-standard user configuration directory, per standard Rust CLI convention
  (`$XDG_CONFIG_HOME/mostro-score/config.toml` on Linux, falling back to
  `~/.config/mostro-score/config.toml`, and the equivalent standard location on macOS/Windows).
  See FR-014.
- Q: Nothing in the spec explained how the configuration file actually comes into existence —
  does tool installation create it, or does something else? → A: Installation (`cargo install`/
  `cargo build`) creates nothing beyond the binary; the file was originally scoped as entirely
  hand-authored, which turned out to be a real gap since the tool offered no way to help a user
  create one. Resolved by adding a scaffolding mechanism: `--init-config` writes a starter file
  populated with current defaults, refuses to overwrite an existing one without `--force`, and
  is a standalone action that takes precedence over report generation when combined with other
  flags. See FR-017 through FR-019 and User Story 5.
- Q: When a configuration file exists but fails to parse (malformed TOML), what should the tool
  do? → A: Warn to standard error identifying the file and the problem, ignore it entirely, and
  fall back fully to the environment-variable/compiled-default chain — never a fatal error,
  matching the constitution's graceful-degradation precedent for relay failures. See FR-015a.

### Session 2026-07-26

- Q: FR-016 originally excluded `--pubkey` from the configuration file specifically to guarantee
  that a broken or stale config file could never leave a trader analyzing the wrong node
  silently. Should that protection be removed? → A: Yes, surgically amended: `--pubkey` may now
  be sourced from the configuration file, validated the same way an explicit `--pubkey` flag is,
  at config-load time. The original protection's goal (never let an invalid config value produce
  a wrong-node analysis) is preserved by that same-time validation, not by excluding the field
  outright. `--sections` gains the same config-sourced precedence at the same time, for
  consistency with every other filterable/display flag already in FR-016. See FR-016, FR-016a,
  FR-017.
- Q: Should the persisted configuration file's generated content stay a bare list of keys, or
  say more? → A: Each key in the file `--init-config` writes MUST be preceded by a short comment
  explaining what it does and which values it accepts, without introducing nested TOML tables —
  the file stays a flat list of commented, individually-documented keys. See FR-017.
- Q: The report always printed to standard output; is there a way to send it to a file instead?
  → A: Added `-o`/`--output`, valid for `plain`/`json` only (not `console`, since colored escape
  codes in a file provide no benefit); omitting `--format` alongside it resolves to `plain`
  rather than performing terminal detection. See FR-020.
- Q: Should `--format json`'s output stay single-line? → A: No, changed to pretty-printed
  (multi-line, indented) unconditionally, for both standard output and `--output` files. See
  FR-021.
- Q: Now that pretty-printed JSON is actually readable, is the `metric_definitions` table (a
  static, 34-entry block explaining every metric's label/meaning/unit) still worth carrying in
  every JSON document? → A: No, removed. specs/002-cli-report-design FR-008b's underlying
  documentation commitment is satisfied by console/plain-text's own inline explanations; JSON's
  typical consumer is a script or another program, not a trader reading the raw output, so it
  does not need a parallel documentation table to satisfy that same commitment. See the amended
  specs/002-cli-report-design FR-008b.
