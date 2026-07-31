# Metrics

Every metric is derived from Nostr events published by the node itself. No value is
self-reported by the operator, and no value is a composite or weighted score.

## Index

| Metric | Measures |
|---|---|
| [Longevity](longevity-liveness.md#longevity) | How long the node has been operating |
| [Liveness](longevity-liveness.md#liveness) | Whether it is still trading now |
| [Activity consistency](longevity-liveness.md#activity-consistency) | How evenly its trading is spread across the last 30 days |
| [Cumulative performance](trade-size-consistency.md#cumulative-performance) | Lifetime trade count and total volume |
| [Trade size](trade-size-consistency.md#trade-size) | The distribution of individual trade amounts |
| [Dispute signals](disputes-bond-policy.md#dispute-signals) | Dispute counts by outcome, and the rate per 100 trades |
| [Bond policy](disputes-bond-policy.md#bond-policy) | Whether the node requires a trader bond |
| [Fiat currency breakdown](context-signals.md#fiat-currency-breakdown) | Which currencies its trades settle in |
| [Payment method breakdown](context-signals.md#payment-method-breakdown) | Which payment methods its orders accept |
| [Premium signal](context-signals.md#premium-signal) | Its pricing relative to market rate, and how much that varies |
| [Activity grid](activity-grid.md) | When that trading actually happened, over time |

## Event scoping

Before any computation, a fetched event must satisfy three conditions or it is
discarded: its author must be the queried pubkey, its `z` tag must match the value
expected for its kind, and its `y` tag must be `mostro`. Events of any other kind are
excluded outright. A report therefore never mixes in another node's activity, or
another application's use of the same kinds.

| Kind | `z` value | Backs |
|---|---|---|
| `8383` | `dev-fee-payment` | Longevity |
| `38383` | `order` | Trade size, liveness, consistency, activity grid, context signals |
| `38385` | `info` | Bond policy |
| `38386` | `dispute` | Dispute signals |

## Not-applicable values

A metric that cannot be computed reports its absence explicitly: `N/A` in
console and plain-text output, `null` in JSON. A `0` is always a computed result, never
a placeholder — a node can genuinely have zero disputes across a long trade history.
Each metric states its own condition.
