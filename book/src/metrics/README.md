# Metrics

Everything in this section is computed directly from Nostr events the node itself has
published — nothing here is self-reported by the node operator, and nothing is a
subjective score. Each event is first scoped to the node being queried: its pubkey must
match as the event's author, the event's `z` tag must match what that kind expects
(order, dispute, dev-fee-payment, or instance info), and its `y` tag must read
`mostro`. Any event that fails that scoping, or belongs to any other kind entirely, is
silently excluded before a single metric is computed — so a report never mixes in
another node's activity, or another application's use of the same Nostr kinds.

Every metric follows the same rule when there isn't enough underlying data to compute
it: it reports its absence explicitly (`N/A` in console/plain text, `null` in JSON)
rather than a fabricated zero. A `0` you see in the report is always a real, computed
answer — for example, a node can genuinely have zero disputes on a healthy trade
history — never a stand-in for missing data. Each page below explains, for its own
metrics, exactly what triggers the not-applicable case.

The pages are grouped by what they help you evaluate:

- [Longevity and liveness](longevity-liveness.md) — how long the node has run, and
  whether it's still active.
- [Trade size and consistency](trade-size-consistency.md) — how much it trades, and
  how uniform those trades are.
- [Disputes and bond policy](disputes-bond-policy.md) — how its trades have gone wrong,
  and what protection it offers traders.
- [Fiat, payment method, and premium](context-signals.md) — what kind of trading it
  does, and at what price.
- [Activity grid](activity-grid.md) — when that activity happened, bucketed over time.
