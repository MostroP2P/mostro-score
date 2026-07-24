# Feature Specification: Node Reputation Metric System

**Feature Branch**: `001-node-reputation-metrics`

**Created**: 2026-07-22

**Status**: Ready for Planning

**Input**: User description: "Complete node reputation metric system for mostro-score, Phase 1 of
the Summer of Bitcoin proposal. Formalizes and extends the existing `specs/reputation_system_v1.md`
(v1.1) with new metrics researched in a preliminary gist analysis: dispute signals, rating signals,
fiat/payment method breakdown, trade-size consistency, premium signal, and bond policy."

## Clarifications

### Session 2026-07-22

- Q: What does the Premium Signal actually compare, and at what level (per-maker or per-node)? → A:
  Per-node, not per-maker. The `premium` tag on each order event (kind `38383`) is chosen by the
  order's maker (`creator_pubkey`), not by the node, but `creator_pubkey` is a one-time trade key
  that mostro-core rotates per order, and even the buyer/seller master identity fields
  (`master_buyer_pubkey`/`master_seller_pubkey`) are unset before matching and collapse to the trade
  key itself under full-privacy mode — so no maker can be reliably tracked across multiple orders
  from public data alone. The signal therefore compares each order's `premium` against the median
  `premium` of that same node's other successful orders (the order being evaluated is excluded from
  its own baseline), i.e. it measures the price consistency of the order flow published through that
  node, not an individual maker's pricing behavior.
- Q: How is node-level premium dispersion expressed — same coefficient-of-variation approach as
  Trade-Size Consistency (FR-010)? → A: No. Unlike trade size in sats (always positive), `premium`
  can be zero or negative, so dividing by the median baseline is undefined or unstable. `premium` is
  already expressed as a percentage and needs no further normalization: node-level dispersion is the
  raw standard deviation of the node's historical premiums, in percentage points.
- Q: How should trade-size consistency (FR-010) be expressed so it stays comparable across nodes
  of very different volume? → A: Expose both the raw standard deviation in sats and the
  coefficient of variation (`std_dev / median`), with a note that the ratio is the value to use
  when comparing nodes of different scale; the ratio has no upper bound and a higher value means
  less consistent trade sizing.
- Q: Should that standard deviation use population or sample (Bessel-corrected) semantics, and what
  happens when there are too few trades for the coefficient of variation to mean anything? → A:
  Population standard deviation (divide by `N`, not `N-1`), since the qualifying order set is the
  node's complete historical record, not a sample drawn from a larger population. The coefficient
  of variation is reported as not applicable when fewer than 2 successful trades exist: a single
  trade has no spread to measure, and zero trades leaves no median to divide by.
- Q: What exact timestamp powers the 7/30/90-day liveness windows (FR-004) and the 30-day activity
  window (FR-005), and where do the window boundaries fall? → A: The Nostr event `created_at` of
  each order's final deduplicated `s=success` state, the same field FR-014 already orders events
  by. FR-004's windows are a rolling exact window: a trade counts toward `last_Nd` when
  `created_at` falls within the last `N × 24` hours counted back from the moment the report runs,
  both ends inclusive. FR-005 counts in UTC calendar days instead, since it measures days with
  activity and gaps in days: each order is bucketed into a UTC calendar day (midnight-to-midnight
  UTC) derived from its `created_at`, and the window is the 30 UTC calendar days ending on and
  including today. A node with zero successful trades in that window MUST report
  `active_days_last_30d = 0` and `max_consecutive_inactive_days_last_30d = 30` — the whole window
  counted as inactive — rather than an error or a not-applicable value, since absence of activity
  is itself a real, computable result.
- Q: Kind `38384` (rating) was originally scoped as a node-level Rating Signals metric (FR-007). Why
  was it discarded instead of fixed? → A: Two independent problems, either alone survivable,
  together fatal. First, `total_rating` is not a recomputable arithmetic mean. Tracing
  `User::update_rating` in `mostro-core` (`src/user.rs:65-89`), the function whose output is
  published verbatim as the `total_rating` tag, shows it folds in the previous call's `last_rating`
  (one vote stale) through an incremental weighted update, with the first vote pre-halved. A worked
  trace with ratings `[5, 3, 4]` yields `3.5`, not the true mean `4.0`. That alone would still be
  usable, labeled honestly as a self-reported figure. What makes it unusable as a node metric is the
  second, deeper problem: the event's identity and its content describe two different things. In
  `mostro/src/app/rate_user.rs`, `update_user_rating_event(&counterpart_trade_pubkey, ...)` keys the
  event's `d` tag to `counterpart_trade_pubkey`, which despite its name is set to whichever side
  sent the rating (the rater), not the rated party. But the tag content (`total_reviews`,
  `total_rating`, and so on) is built from `user_to_vote`, the recipient being rated. And that
  rater's pubkey is itself a one-time trade key that rotates on every order (the same rotation
  documented for Premium Signal), not a stable identity. So a single kind `38384` event is neither
  "the node's rating" nor cleanly "one trader's snapshot": its key changes every time the same
  person rates again, and that key never matches the person whose stats are inside it. There is no
  reliable way to group these events by subject at all, let alone aggregate them into a node-level
  figure. See FR-007's rejection entry in Requirements for the full rationale.
- Q: Does the disputes ratio (FR-006) count only unresolved disputes, or all of them, and can
  resolved vs. still-active disputes be told apart? → A: The ratio counts all disputes regardless
  of resolution, deduplicated by the dispute's `d` tag (kind `38386` is a NIP-33 replaceable event,
  republished at each status change per `mostro/src/app/dispute.rs`, confirmed by
  `publish_dispute_event` and the dispute-close path both calling `new_dispute_event` with the same
  `dispute.id` and an updated `s` tag). That same deduplication step already yields each dispute's
  latest status at no extra cost, so System also reports how many of the deduplicated disputes are
  resolved (`s` = `settled`, `seller-refunded`, or `released`) versus still active (`s` = `initiated`
  or `in-progress`).
- Q: How is the effective kind `38385` instance-status event selected when a multi-relay fetch
  returns more than one copy for the same node (replaceable-event propagation lag across relays),
  and what happens when none is valid? → A: Kind `38385` is a NIP-33 replaceable event keyed by the
  node's own pubkey as the `d` tag, republished on a timer (`mostro/src/nip33.rs`'s
  `new_info_event`, called from `scheduler.rs::job_info_event_send`), so relays may briefly disagree
  on which copy is current. System MUST select the event with the highest `created_at` across all
  fetched relays; if multiple events tie on `created_at`, System MUST break the tie by the
  lexicographically greatest Nostr event id. If no valid `38385` event is found for the node, or the
  selected event's `bond_enabled` tag is missing or not parseable as `true`/`false`, bond
  enforcement MUST be reported as unknown rather than defaulted to `false`, since a missing signal
  is not evidence of a disabled policy.
- Q: What exact procedure deduplicates repeated kind `38383` events for the same order, and how
  does that connect to the `s=success` filter used throughout this spec? → A: Two ordered steps,
  not one. First, group all fetched events by the order's `d` tag and select, per group, the single
  event with the highest `created_at` (ties broken by the lexicographically greatest event id, the
  same rule used for FR-012) — that selected event is the order's current, final state, consistent
  with FR-014's `created_at` ordering requirement. Second, and only after that selection, check
  whether the selected event's `s` tag equals `success`; if it does not, the order is excluded from
  every successful-order-based metric in this spec (FR-002 through FR-005, FR-008 through FR-011),
  regardless of what any earlier, superseded event for that same `d` tag showed. This timestamp is
  each event's real signing-time `created_at`; it is not subject to NIP-59 gift-wrap timestamp
  randomization, since that applies only to the private DM transport between user and node, not to
  these public NIP-33 events (`mostro/src/nip33.rs`'s `create_event` signs with the real clock time,
  no jitter).
- Q: The "highest `created_at` wins" rule selects a current or final event state throughout this
  spec (FR-002, FR-006, FR-012, FR-014). What if a fetched event's `created_at` is later than
  report-generation time, for example from relay data corruption or clock skew? → A: Any event
  whose `created_at` is later than report-generation time MUST be excluded from consideration
  entirely wherever this rule is applied, not merely deprioritized, since a Nostr event timestamped
  in the future relative to the report cannot be a legitimate signing time the report should trust.
  This prevents a single anomalous event from always winning deduplication regardless of what
  else exists, or from being counted inside a rolling window (FR-004) that has not happened yet.
- Q: What exact Nostr tags carry the fiat currency and payment method values for FR-008/FR-009, are
  they ever absent, and how is each distribution's denominator defined? → A: `f` (fiat currency) and
  `pm` (payment method) on kind `38383` order events, confirmed in `mostro/src/nip33.rs`'s
  `order_to_tags`: both are pushed unconditionally whenever the order reaches a publishable state,
  so a missing tag key is not a reachable case for well-formed orders. The two tags are not
  symmetric. `f` always carries exactly one value (`order.fiat_code`), so the fiat-currency
  distribution is `orders_in_that_currency / orders_with_a_nonempty_f_value`, a strict partition
  of that denominator that sums to 100% (see FR-008; orders with an empty `f` value are excluded
  from both numerator and denominator, not counted toward `qualifying_orders` in this ratio).
  `pm` can carry multiple values (`order.payment_method` is split on commas, e.g.
  "SEPA,Bank transfer" → two values), so it is a usage ranking, not a per-order partition: the
  payment-method distribution is `mentions_of_that_method / total_payment_method_mentions` across
  all qualifying orders, which also sums to 100% but ranks methods by how often they are offered,
  not by what share of orders offer them. A qualifying order whose `f` value is empty, or whose
  `pm` produces zero values after the comma split, is excluded from that specific breakdown,
  consistent with FR-013's general handling of unusable data.

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
disputes, resolved or not, so they can avoid nodes that look healthy on paper but have real trust
problems. This story originally also covered "poor user ratings," discarded as a node-level signal
— see FR-007's rejection rationale: kind `38384` rating data is per-trader, not per-node, so it
cannot support this story's premise.

**Why this priority**: Longevity and volume alone can be gamed or can mask a node that mistreats
counterparties; dispute data is the next most important signal after basic activity.

**Independent Test**: Run the CLI against a node with a known dispute event; verify the
disputes-per-100-trades ratio, with its resolved/active breakdown, is reported independently of the
Phase 1 (P1) metrics.

**Acceptance Scenarios**:

1. **Given** a node with 3 unique disputes (by `d` tag, regardless of how many status-transition
   events each republished) and 150 successful trades, **When** the trader runs the CLI, **Then**
   the report shows a disputes-per-100-successful-trades ratio of 2.0.
2. **Given** a node with zero disputes and 150 successful trades, **When** the trader runs the CLI,
   **Then** the report shows a disputes-per-100-successful-trades ratio of 0.0 rather than failing
   or omitting the section.

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
2. **Given** a node whose historical median `premium` is X% and one order published with a
   `premium` far above X%, **When** the trader runs the CLI, **Then** the report shows that
   order's `premium_deviation` as a large value relative to that node's own order flow, not against
   a global benchmark — reported as a raw signal, not a pass/fail flag (see Assumptions on
   presentation-level "abnormal" thresholds).

---

### Edge Cases

- What happens when a node has dev-fee-payment events but zero successful orders? The report MUST
  show longevity as computable while explicitly stating there are no completed trades, not error out.
- What happens when a node has zero dev-fee-payment events but has successful orders? Per FR-001,
  `days_active` MUST fall back to the elapsed time from the node's first successful order to
  report-generation time (not to its last order, see FR-001's rationale), and `first_seen_at` MUST
  be reported as unavailable, not error out or silently omit longevity.
- What happens when a node has neither dev-fee-payment events nor successful orders? Per FR-001,
  System MUST report both `first_seen_at` and `days_active` as unavailable, since neither the
  primary computation nor its fallback has any timestamp to anchor on.
- What happens when a configured relay is unreachable? Per the project constitution, the tool MUST
  warn and continue with the remaining relays, only failing if none succeed.
- How does the system handle duplicate order events for the same order ID (state updates)? For each
  order's `d` tag, System MUST select the single event with the highest `created_at` (ties broken
  by event id) as that order's current state, then check whether that selected state's `s` tag
  equals `success`; only orders whose selected current state is `success` count toward any
  successful-order-based metric (see Clarifications for the full procedure).
- How does the system handle malformed or incomplete events (missing required tags)? They MUST be
  ignored safely without crashing the report.
- What happens when a node has zero disputes? The disputes-per-100-successful-trades ratio MUST be
  reported as `0.0`, not omitted or treated as an error (see User Story 2), provided the node has
  at least one successful trade. Per FR-006, a zero successful-trade count takes precedence and
  MUST be reported as not applicable instead, whether or not the node has any disputes, since the
  ratio has no denominator to divide by. This edge case previously also covered rating events;
  Rating Signals were discarded, see FR-007.
- What happens when total successful trades is zero? Ratios that divide by successful trade count
  (e.g., disputes per 100 trades) MUST be reported as not applicable rather than as a division error.

## Requirements *(mandatory)*

### Functional Requirements

**Historical reputation (Keep, from spec v1.1)**

- **FR-001**: System MUST compute `first_seen_at` and `days_active` from the oldest kind `8383`
  dev-fee-payment event published by the node that also carries `z=dev-fee-payment` and the `y`
  tag's first value as `mostro` (see FR-015), excluding any kind `8383` event without both. When
  the node has zero such events, System MUST fall back to computing `days_active` as the elapsed
  time in days between the node's first qualifying successful order's `created_at` and
  report-generation time, and MUST report `first_seen_at` as unavailable in that case, since there
  is no dev-fee anchor to derive it from. This intentionally corrects an inconsistency in the
  current mostro-score prototype's fallback (`src/main.rs`), which spans the first order to the
  *last* order instead of to now, making `days_active` stop increasing at the last trade and read
  `0` for a node with exactly one successful order; the primary dev-fee-based path already measures
  elapsed time to now, and this fallback MUST match that same semantic. When the node has neither a
  qualifying dev-fee-payment event nor a qualifying successful order, this fallback has no order to
  anchor on either; System MUST report both `first_seen_at` and `days_active` as unavailable in
  that case, not attempt to compute a value from missing data.
- **FR-002**: System MUST compute `total_successful_trades` from kind `38383` order events that
  also carry `z=order` and the `y` tag's first value as `mostro` (see FR-015), excluding any kind
  `38383` event without both, first deduplicated by the order's `d` tag to each order's current
  state (the event with the highest `created_at`, ties broken by event id), then filtered to only
  those whose selected state carries `s=success` (see Clarifications for the full procedure).
  `total_successful_trades` counts every order in that set regardless of its `amt` value.
  `total_volume_sats` sums only the subset of that same set whose `amt` tag is present and parses
  as a non-negative integer; an order with a missing or unparseable `amt` still counts toward
  `total_successful_trades`, but is excluded from `total_volume_sats` and from FR-003, since amount
  is unusable data on that order specifically, not evidence the trade did not happen (consistent
  with FR-013).
- **FR-003**: System MUST compute `min_trade_sats`, `max_trade_sats`, `mean_trade_sats`, and
  `median_trade_sats` from the same successful-order set, restricted to orders with a present,
  parseable `amt` value as defined in FR-002. All four MUST be reported as not applicable when
  that restricted set is empty, whether because there are zero successful orders at all or
  because none of the successful orders has a usable `amt` value, rather than as zero or an error.
  System MUST treat `median_trade_sats` as the
  primary reference for typical trade size in any user-facing risk assessment, per the project
  constitution's statistical-robustness principle. All four MUST be reported as not applicable
  when the node has zero successful orders, rather than as zero or an error.
- **FR-004**: System MUST compute `last_successful_trade_at` and `days_since_last_trade` (kept from
  spec v1.1) as the `created_at` of the node's most recent successful order and the elapsed time
  since it, reported as not applicable when zero successful orders exist. System MUST also compute
  rolling `successful_trades_last_7d`, `successful_trades_last_30d`, and
  `successful_trades_last_90d` as counts of successful orders whose `created_at` falls within the
  last `N × 24` hours counted back from report-generation time, both ends inclusive.
- **FR-005**: System MUST compute `active_days_last_30d` as the count of distinct UTC calendar days,
  derived from each order's `created_at`, containing at least one successful order within the 30
  UTC calendar days ending on and including today, and `max_consecutive_inactive_days_last_30d` as
  the longest run of consecutive UTC calendar days with zero successful orders within that same
  window. A node with no successful orders in the window MUST report `active_days_last_30d = 0` and
  `max_consecutive_inactive_days_last_30d = 30`.

**Risk signals (New)**

- **FR-006**: System MUST compute a disputes-per-100-successful-trades ratio from kind `38386`
  dispute events, deduplicated by the dispute's `d` tag so each unique dispute counts once using
  its latest reported `s` (status) — the event with the highest `created_at`, ties broken by the
  lexicographically greatest event id, the same rule used everywhere else in this spec — regardless
  of how many status transitions were published for it, reported as not applicable when successful
  trade count is zero. This counts a dispute once it
  exists, independent of its status; resolution outcome does not change the count. System MUST
  additionally report, from that same deduplicated set, counts of resolved (`s` = `settled`,
  `seller-refunded`, or `released`) versus still-active (`s` = `initiated` or `in-progress`)
  disputes. A deduplicated dispute whose latest `s` value is missing or does not match any of
  those five known values still counts once toward the main ratio, since a valid dispute event
  exists for it regardless of its status tag, but MUST be reported in a third, separate count
  (unknown status) rather than folded into either the resolved or the active count, so that
  resolved plus active plus unknown always adds up to the total.
- **FR-007 — Discarded, not part of this spec**: Rating Signals were considered but rejected after
  verification against the real protocol. Kind `38384` is not a node-level rating, and its event
  identity does not even match its own content. In `mostro/src/app/rate_user.rs`,
  `update_user_rating_event(&counterpart_trade_pubkey, ...)` keys the event's `d` tag to whichever
  side sent the rating (despite the variable's name, not the rated party), while the tag content is
  built from the recipient's own stats. That rater's pubkey is also a one-time trade key that
  rotates on every order, the same rotation documented for Premium Signal, so it is not even a
  stable identity to group by. There is no reliable way to find "all of this trader's ratings," let
  alone "this node's rating," from these events. Aggregating across whatever a node has published
  would measure the reputation of whichever traders happened to trade through it, which is noise
  about those traders, not a signal about the node's own conduct, unlike Premium Signal's node-level
  order-flow reframing (see Clarifications), which measures something the node's own order flow
  actually exhibits. This requirement number is intentionally left unused rather than reassigned, so
  FR-008 through FR-012's existing references and numbering stay stable.

**Descriptive context (New)**

- **FR-008**: System MUST compute the relative distribution of fiat currencies from the `f` tag as
  `orders_in_that_currency / orders_with_a_nonempty_f_value` per currency, where the denominator is
  the node's qualifying successful orders that carry a non-empty `f` value, excluding any orders
  with an empty `f` value entirely (from both numerator and denominator). Each order counted in
  that denominator carries exactly one `f` value, so this is a strict partition of it and its
  percentages sum to 100%. Reported as not applicable when every qualifying order has an empty `f`
  value, leaving a zero denominator.
- **FR-009**: System MUST compute the relative usage ranking of payment methods from the `pm` tag
  across the node's qualifying successful orders, as `mentions_of_that_method /
  total_payment_method_mentions`, where the denominator sums every method mention across all
  qualifying orders (an order listing multiple methods contributes one mention per method). This
  ranks methods by how often they are offered and sums to 100%; it is not a per-order partition.
  Orders whose `pm` produces zero values are excluded from this computation. Reported as not
  applicable when every qualifying order's `pm` produces zero values, leaving a zero denominator.
  Method values MUST be compared exactly as published, byte for byte, with no trimming of
  whitespace and no case normalization, since `order_to_tags` in `mostro/src/nip33.rs` performs
  none when splitting `payment_method` on commas. "SEPA,Bank transfer" and "SEPA, Bank transfer"
  therefore produce different values (`"Bank transfer"` vs. `" Bank transfer"`) and MUST be
  reported as distinct methods; grouping near-duplicate labels together is a presentation-layer
  decision, out of scope for this spec. Empty tokens from leading, trailing, or consecutive commas
  (as in `"SEPA,"`, `",SEPA"`, or `"SEPA,,Cash"`) never reach a published `pm` tag: `order_to_tags`
  filters them out (`.filter(|s| !s.is_empty())`) before publishing, so a real event's `pm` values
  are never empty strings. An empty value MUST NOT be counted as a method if one is ever
  encountered anyway, since that would only happen on a malformed or non-conforming event.
- **FR-010**: System MUST compute `std_dev_trade_sats` as the population standard deviation (divide
  by `N`, not `N-1`) of trade size in sats across the same amt-restricted set defined in FR-002,
  since this set is the node's complete historical record, not a sample of a larger population.
  System MUST also compute the coefficient of variation (`std_dev_trade_sats / median_trade_sats`),
  reported as not applicable when fewer than 2 orders with a parseable `amt` exist, or when
  `median_trade_sats` is exactly `0`, since dividing by a zero median is undefined regardless of
  sample size. The coefficient of variation, not the raw
  standard deviation, MUST be the value used when comparing consistency across nodes of different
  trade volume; it has no fixed upper bound, and a higher value indicates less consistent trade
  sizing. This remains a raw statistic rather than a pass/fail judgment, consistent with the
  project's existing "derived indicators are presentation-level, non-normative" convention.
- **FR-011**: System MUST compute `premium_baseline` as the median `premium` (a market-price
  markup/discount percentage) across the node's successful order events that carry a valid
  `premium` tag, and `premium_dispersion` as the population standard deviation (divide by `N`, not
  `N-1`, the same rule and rationale as FR-010: this set is the node's complete record, not a
  sample), in percentage points, of
  that same set; both MUST be reported as not applicable when fewer than 2 such orders exist. A
  valid `premium` tag is one whose value parses as a signed integer, matching `order.premium`'s
  type of `i64` in `mostro-core` and its serialization via `order.premium.to_string()` in
  `mostro/src/nip33.rs`. There is no percent sign, no decimal point, and no duplicate values to
  handle, since the tag always carries exactly one plain integer string. A tag that is empty, that
  fails to parse as an integer, or that is missing entirely, is treated as not carrying a valid
  premium, and that order is excluded from `premium_baseline`, `premium_dispersion`, and its own
  `premium_deviation`, consistent with FR-013's general handling of unusable data. When
  at least 2 such orders exist, System MUST also report, for each order in that set,
  `premium_deviation` as that order's `premium` minus `premium_baseline` computed over the
  remaining orders, excluding the order itself from its own baseline; with fewer than 2 orders,
  `premium_deviation` is likewise not applicable, since a leave-one-out baseline needs at least one
  other order to exist. This measures the price consistency of the order flow published through
  the node, not an individual maker's behavior, since an order's `creator_pubkey` is a one-time
  trade key that cannot be tracked across orders (see Clarifications). This MUST NOT be computed
  from trade size in sats, which is a distinct metric already covered by FR-003 and FR-010.
- **FR-012**: System MUST report whether a node enforces anti-abuse bonds, read from the
  `bond_enabled` tag on the node's kind `38385` instance status event, restricting the candidate
  set to events whose `d` tag equals the node's own pubkey (the canonical NIP-33 replaceable-event
  coordinate is `(kind, pubkey, d)`, and `d = node pubkey` for kind `38385`), then selecting the
  event with the highest `created_at` within that set and breaking ties by the lexicographically
  greatest event id. This value MUST NOT be derived from individual kind `38383` order events,
  which never carry bond data. When no valid `38385` event exists for the node, or `bond_enabled`
  is missing or not parseable as `true`/`false`, System MUST report bond enforcement as unknown
  rather than `false`.

**Data integrity (Keep, from spec v1.1)**

- **FR-013**: System MUST ignore malformed or incomplete events safely without crashing.
- **FR-014**: System MUST process events ordered by `created_at`, with ties broken by the
  lexicographically greatest event id, the same tie-break used throughout this spec (FR-002,
  FR-006, FR-012) wherever a "current" or "final" event state must be selected from multiple
  candidates. Any event whose `created_at` is later than report-generation time MUST be excluded
  from this ordering entirely, not merely deprioritized (see Clarifications).
- **FR-015**: System MUST scope every kind `8383`, `38383`, `38385`, and `38386` event used in a
  node's report to (a) that node's own pubkey as the event's Nostr author (signer), since all four
  event types are published by the node's own daemon under its own key, (b) the event's `z` tag
  matching its expected subtype — `dev-fee-payment` for kind `8383`, `order` for kind `38383`,
  `info` for kind `38385`, `dispute` for kind `38386` — and (c) the event's `y` tag carrying
  `mostro` as its first value. Kind `38384` (rating) is deliberately excluded from this list: it is
  not used by any active requirement in this spec (see FR-007's rejection), and even if it were,
  `mostro-core`'s `Rating::to_tags()` never emits a `y` tag, so a blanket rule across all five kinds
  would have made it unimplementable regardless. None of these checks are a signature-forgery
  concern (Nostr signatures cannot be forged for another pubkey, and tag values are part of the
  signed content); they exist so a relay fetch never mixes another node's events, or another
  application's or subtype's same-kind events, into this node's report.

### Key Entities

- **Node**: A Mostro instance identified by its Nostr pubkey; the subject of the report. Owns all
  metrics in this spec.
- **Dev-Fee-Payment Event**: Kind `8383` event anchoring the node's earliest known trading
  activity (longevity anchor).
- **Order Event**: Kind `38383` event representing a trade; only the final `s=success` state per
  order `d` tag counts toward historical and liveness metrics. Carries a `premium` tag (percentage
  markup/discount over market price), set by the order's maker but aggregated per-node by the
  Premium Signal since makers cannot be tracked across orders (see Clarifications), distinct from
  trade size in sats.
- **Rating Event — discarded, not used**: Kind `38384` NIP-33 replaceable event, keyed by the
  rater's one-time trade pubkey (not the node's), while carrying the rated recipient's
  `total_reviews`, `total_rating`, `last_rating`, `max_rate`, and `min_rate` snapshot. Not an
  entity this spec's metrics consume; see FR-007's rejection rationale for why the key and the
  content describe two different people, and neither is the node.
- **Dispute Event**: Kind `38386` NIP-33 replaceable event, keyed by a `d` tag holding the dispute
  id, republished with an updated `s` (status) tag at each lifecycle transition (`initiated`,
  `in-progress`, then `settled`, `seller-refunded`, or `released`); deduplicated by that `d` tag so
  each dispute counts once, using its latest status, regardless of how many transitions were
  published.
- **Instance Status Event**: Kind `38385` NIP-33 replaceable event, keyed by the node's own pubkey
  as the `d` tag and republished on a timer, carrying node-level policy tags including
  `bond_enabled`; distinct from individual order events and never present on kind `38383`. When
  multiple copies are fetched across relays, the one with the highest `created_at` is authoritative,
  ties broken by event id.
- **Metric Report**: The complete set of computed values for a node, the artifact this feature
  produces; consumed by the CLI's reporting/presentation layer (out of scope for this spec).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: For any node with at least one dev-fee-payment event, the tool produces a complete
  historical reputation section (longevity, cumulative performance, trade statistics) without
  manual interpretation of raw events.
- **SC-002**: For any node with dispute events, the tool surfaces the dispute risk signal, so a
  trader can identify a node with a track record of disputes without cross referencing raw Nostr
  events themselves. This criterion previously also covered rating events; Rating Signals were
  discarded — see FR-007.
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
- This spec assumes access to kind `38385` and `38386` events is available on the same relays
  already used for kind `8383` and `38383`, with no additional relay configuration required. Kind
  `38384` is intentionally not listed, since no active requirement in this spec consumes it (see
  FR-007's rejection).
