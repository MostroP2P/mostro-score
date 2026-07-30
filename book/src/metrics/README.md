# Metrics

Every metric here is computed from Nostr events the node itself has published — nothing
is self-reported or a subjective score. Each event is first scoped to the queried node
(pubkey as author, expected `z` tag, `y=mostro`); anything failing that scope, or
belonging to another kind, is silently excluded before any metric is computed.

A missing value always reports as not applicable (`N/A` in console/plain text, `null`
in JSON), never a fabricated zero — a `0` you see is always a real computed answer.
Each page states the exact rule for its own metrics.

Each metric page follows the same structure: **What it is**, **Source** (the Nostr
event kind/tag it's computed from), and **How to read it**.

- [Longevity and liveness](longevity-liveness.md) — how long the node has run, and
  whether it's still active.
- [Trade size and consistency](trade-size-consistency.md) — how much it trades, and how
  uniform those trades are.
- [Disputes and bond policy](disputes-bond-policy.md) — how its trades have gone wrong,
  and what protection it offers traders.
- [Fiat, payment method, and premium](context-signals.md) — what kind of trading it
  does, and at what price.
- [Activity grid](activity-grid.md) — when that activity happened, bucketed over time.
