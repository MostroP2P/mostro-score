# Feature Specification: Node Reputation Metric System

**Feature Branch**: `001-node-reputation-metrics`

**Created**: 2026-07-22

**Status**: Draft

**Input**: User description: "Complete node reputation metric system for mostro-score, Phase 1 of
the Summer of Bitcoin proposal. Formalizes and extends the existing `specs/reputation_system_v1.md`
(v1.1) with new metrics researched in a preliminary gist analysis: dispute signals, rating signals,
fiat/payment method breakdown, trade-size consistency, premium signal, and bond policy."

## Clarifications

### Session 2026-07-22

- Q: What does the Premium Signal actually compare? → A: The `premium` tag already published on each
  order event (kind `38383`), a percentage markup/discount over market price, compared against that
  same node's own historical average/median `premium`. Not a comparison of trade size in sats.
- Q: How should trade-size consistency (FR-010) be expressed so it stays comparable across nodes
  of very different volume? → A: Expose both the raw standard deviation in sats and the
  coefficient of variation (`std_dev / median`), with a note that the ratio is the value to use
  when comparing nodes of different scale; the ratio has no upper bound and a higher value means
  less consistent trade sizing.
- Q: What does "review coverage" mean in FR-007? The source gist's example wording references
  "frequent traders," a cohort not derivable from the documented kind `38384` tags. → A:
  `review_coverage = total_reviews / total_successful_trades`, i.e. the share of the node's
  successful trades that received a rating, since that is the only version computable from real
  tag data.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Assess a node's track record and current health (Priority: P1)

A trader considering an order with a specific Mostro node runs the CLI against that node's pubkey
and sees its historical track record (how long it has operated, how much volume and how many
successful trades it has completed, the typical trade size) together with its current operational
health (how recently it traded, and whether its activity has been steady or erratic).

**Why this priority**: This is the existing, already-partially-implemented core of the tool. Without
it there is no reputation signal at all, so every other story builds on top of it.

**Independent Test**: Run the CLI against a node pubkey with a mix of old and recent successful
orders; verify longevity, cumulative performance, trade statistics (median as primary reference),
liveness windows, and activity consistency are all reported and internally consistent with the raw
order events fetched from the relay.

**Acceptance Scenarios**:

1. **Given** a node with dev-fee-payment events and successful orders spanning several months,
   **When** the trader runs the CLI against that node's pubkey, **Then** the report shows
   `days_active` derived from the oldest dev-fee event, `total_successful_trades`,
   `total_volume_sats`, and `median_trade_sats` as the primary typical-trade-size figure.
2. **Given** a node with no successful orders in the last 90 days, **When** the trader runs the
   CLI, **Then** the report clearly shows zero recent activity across the 7/30/90-day windows
   without erroring.

---

### User Story 2 - Surface risk signals before trading (Priority: P2)

A trader wants to know, beyond raw volume and longevity, whether a node has a track record of
unresolved disputes or poor user ratings, so they can avoid nodes that look healthy on paper but
have real trust problems.

**Why this priority**: Longevity and volume alone can be gamed or can mask a node that mistreats
counterparties; dispute and rating data is the next most important signal after basic activity.

**Independent Test**: Run the CLI against a node with a known dispute event and rating events;
verify the disputes-per-100-trades ratio and the average rating with review coverage are both
reported, independently of the Phase 1 (P1) metrics.

**Acceptance Scenarios**:

1. **Given** a node with 3 dispute events and 150 successful trades, **When** the trader runs the
   CLI, **Then** the report shows a disputes-per-100-successful-trades ratio of 2.0.
2. **Given** a node with zero rating events, **When** the trader runs the CLI, **Then** the report
   shows the rating section as having no review coverage rather than failing or showing a
   misleading zero rating.

---

### User Story 3 - Deeper due diligence context (Priority: P3)

A trader doing deeper due diligence before a large trade wants descriptive context: which fiat
currencies and payment methods a node's counterparties actually use, how consistent the node's
trade sizes are, whether any individual order looks like an outlier in price relative to that
node's own history, and whether the node enforces anti-abuse bonds.

**Why this priority**: This is supporting, descriptive context rather than a core trust signal; it
is valuable for power users but not required for a baseline trust assessment.

**Independent Test**: Run the CLI against a node with varied fiat currencies, payment methods, and
at least one price outlier order; verify the breakdowns and premium signal are reported
independently of the P1/P2 metrics, and do not block the report if any single piece of this context
is unavailable.

**Acceptance Scenarios**:

1. **Given** a node with orders in 3 different fiat currencies, **When** the trader runs the CLI,
   **Then** the report shows the relative distribution of currencies and payment methods used.
2. **Given** a node whose historical average `premium` is X% and one order published with a
   `premium` far above X%, **When** the trader runs the CLI, **Then** that order is flagged as an
   outlier relative to the node's own history, not against a global benchmark.

---

### Edge Cases

- What happens when a node has dev-fee-payment events but zero successful orders? The report MUST
  show longevity as computable while explicitly stating there are no completed trades, not error out.
- What happens when a configured relay is unreachable? Per the project constitution, the tool MUST
  warn and continue with the remaining relays, only failing if none succeed.
- How does the system handle duplicate order events for the same order ID (state updates)? Orders
  MUST be deduplicated by the order's `d` tag, keeping only the final successful state.
- How does the system handle malformed or incomplete events (missing required tags)? They MUST be
  ignored safely without crashing the report.
- What happens when a node has zero rating events but nonzero dispute events, or vice versa? Each
  signal MUST be computed and reported independently; the absence of one MUST NOT suppress the other.
- What happens when total successful trades is zero? Ratios that divide by successful trade count
  (e.g., disputes per 100 trades) MUST be reported as not applicable rather than as a division error.

## Requirements *(mandatory)*

### Functional Requirements

**Historical reputation (Keep, from spec v1.1)**

- **FR-001**: System MUST compute `first_seen_at` and `days_active` from the oldest kind `8383`
  dev-fee-payment event published by the node.
- **FR-002**: System MUST compute `total_successful_trades` and `total_volume_sats` from kind
  `38383` order events carrying `s=success`, deduplicated by the order's `d` tag.
- **FR-003**: System MUST compute `min_trade_sats`, `max_trade_sats`, `mean_trade_sats`, and
  `median_trade_sats` from the same successful-order set, and MUST treat `median_trade_sats` as the
  primary reference for typical trade size in any user-facing risk assessment, per the project
  constitution's statistical-robustness principle.
- **FR-004**: System MUST compute rolling `successful_trades_last_7d`, `successful_trades_last_30d`,
  and `successful_trades_last_90d` liveness windows.
- **FR-005**: System MUST compute `active_days_last_30d` and
  `max_consecutive_inactive_days_last_30d` to expose activity consistency.

**Risk signals (New)**

- **FR-006**: System MUST compute a disputes-per-100-successful-trades ratio from kind `38386`
  dispute events, reported as not applicable when successful trade count is zero.
- **FR-007**: System MUST compute an average rating from kind `38384` rating event tags
  (`total_rating`, `last_rating`, `max_rate`, `min_rate`), and a review-coverage figure defined as
  `total_reviews / total_successful_trades`, i.e. the share of the node's successful trades that
  received a rating. Both MUST be reported together and independently of dispute data, since a
  high average rating from very low coverage is weaker evidence than a slightly lower average from
  high coverage.

**Descriptive context (New)**

- **FR-008**: System MUST compute the relative distribution of fiat currencies used across a
  node's successful orders.
- **FR-009**: System MUST compute the relative distribution of payment methods used across a
  node's successful orders.
- **FR-010**: System MUST compute both the raw standard deviation of trade size in sats and the
  coefficient of variation (`std_dev_trade_sats / median_trade_sats`) to express trade-size
  consistency. The coefficient of variation, not the raw standard deviation, MUST be the value
  used when comparing consistency across nodes of different trade volume; it has no fixed upper
  bound, and a higher value indicates less consistent trade sizing. This remains a raw statistic
  rather than a pass/fail judgment, consistent with the project's existing "derived indicators are
  presentation-level, non-normative" convention.
- **FR-011**: System MUST read the `premium` tag (a market-price markup/discount percentage)
  already published on each of the node's successful order events, and compare it against that
  same node's own historical average/median `premium`, exposing the deviation as a raw signal per
  order rather than a hardcoded threshold-based flag. This MUST NOT be computed from trade size in
  sats, which is a distinct metric already covered by FR-003 and FR-010.
- **FR-012**: System MUST report whether a node enforces anti-abuse bonds, read from the
  `bond_enabled` tag (`true`/`false`) on the node's kind `38385` instance status event. This value
  MUST NOT be derived from individual kind `38383` order events, which never carry bond data.

**Data integrity (Keep, from spec v1.1)**

- **FR-013**: System MUST ignore malformed or incomplete events safely without crashing.
- **FR-014**: System MUST process events ordered by `created_at`.

### Key Entities

- **Node**: A Mostro instance identified by its Nostr pubkey; the subject of the report. Owns all
  metrics in this spec.
- **Dev-Fee-Payment Event**: Kind `8383` event anchoring the node's earliest known trading
  activity (longevity anchor).
- **Order Event**: Kind `38383` event representing a trade; only the final `s=success` state per
  order `d` tag counts toward historical and liveness metrics. Carries a `premium` tag (percentage
  markup/discount over market price) used by the Premium Signal, distinct from trade size in sats.
- **Rating Event**: Kind `38384` event carrying aggregate user-satisfaction tags for the node.
- **Dispute Event**: Kind `38386` event indicating an unresolved or resolved trade dispute
  involving the node.
- **Instance Status Event**: Kind `38385` event carrying node-level policy tags, including
  `bond_enabled`; distinct from individual order events and never present on kind `38383`.
- **Metric Report**: The complete set of computed values for a node, the artifact this feature
  produces; consumed by the CLI's reporting/presentation layer (out of scope for this spec).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: For any node with at least one dev-fee-payment event, the tool produces a complete
  historical reputation section (longevity, cumulative performance, trade statistics) without
  manual interpretation of raw events.
- **SC-002**: For any node with dispute or rating events, the tool surfaces both risk signals
  independently, so a trader can identify a node with disputes or poor ratings without cross
  referencing raw Nostr events themselves.
- **SC-003**: A trader can determine, from a single report, whether a node's typical trade size,
  currency/payment mix, and price behavior are consistent with their intended trade, without
  needing to inspect the underlying event data.
- **SC-004**: Every metric in this spec is traceable to a documented Nostr event kind and tag, and
  every computation method is written down, satisfying the project's evidence-based-metrics
  principle with no undocumented or heuristic scoring.

## Assumptions

- Fiat currency and payment method breakdowns are computed over the node's full lifetime of
  successful orders, not a rolling window, since the gist source describes them as descriptive
  context rather than a liveness signal.
- The premium signal's threshold for what a report layer chooses to visually flag as "abnormal" is
  a presentation-level decision (per the existing non-normative derived-indicators convention in
  `specs/reputation_system_v1.md`), not part of this spec's computation requirements.
- Percentile-distribution style breakdowns are explicitly out of scope for this spec: the source
  gist analysis classifies that as report/CLI design, not a metric.
- This spec assumes access to kind `38384` and kind `38386` events is available on the same
  relays already used for kind `8383` and `38383`, with no additional relay configuration required.
