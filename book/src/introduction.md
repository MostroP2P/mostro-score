# Introduction

`mostro-score` is a CLI tool that computes reputation statistics for a
[Mostro](https://mostro.network) node by reading the Nostr events it has published.

Mostro nodes publish public, verifiable Nostr events for every order, dispute,
instance-status update, and dev-fee payment they process. `mostro-score` fetches those
events from one or more relays, scopes them to a single node's pubkey, and turns them
into a report: how long the node has been active, how much volume it has moved, how
consistently it trades, how its disputes resolve, and whether it enforces a bond
policy.

The report has no single "trust score." Each metric is reported on its own, with enough
context to interpret it, so a trader forms their own judgment instead of trusting a
single number.

## Report sections

1. **Node identity** — the pubkey being queried, in both hex and npub form.
2. **Relay fetch summary** — which relays connected, and the deduplicated event counts
   backing every other section.
3. **Activity grid** — a time-bucketed table of successful trades and volume.
4. **General statistics** — longevity, cumulative performance, trade size, liveness,
   activity consistency, dispute signals, fiat/payment-method breakdowns, premium
   signal, and bond policy.
5. **Recommendations** — plain-language flags for zero trade history, disputes present,
   or a disabled/unknown bond policy.
