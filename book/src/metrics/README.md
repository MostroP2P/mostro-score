# Metrics

Every metric is derived from Nostr events published by the node itself. No value is
self-reported by the operator, and no value is a composite or weighted score.

## Event scoping

Before any computation, each fetched event must satisfy three conditions or it is
discarded: its author must be the queried pubkey, its `z` tag must match the value
expected for its kind, and its `y` tag must be `mostro`. Events of any other kind are
excluded outright. This guarantees a report never mixes in another node's activity or
another application's use of the same kinds.

| Kind | `z` value | Backs |
|---|---|---|
| `8383` | `dev-fee-payment` | Longevity |
| `38383` | `order` | Trade size, liveness, consistency, activity grid, context signals |
| `38385` | `info` | Bond policy |
| `38386` | `dispute` | Dispute signals |

## Not-applicable semantics

A metric that cannot be computed reports its absence explicitly: `N/A` in
console/plain-text output, `null` in JSON. A `0` is always a computed result, never a
placeholder — a node can genuinely have zero disputes across a long trade history. The
table below states the exact condition per field.

## Field reference

| Field | Type | Source | Not applicable when |
|---|---|---|---|
| `stats.longevity.first_seen_at` | string, RFC 3339 UTC | `8383` `created_at` | No dev-fee event exists |
| `stats.longevity.days_active` | number, days | `8383`, falling back to `38383` | No dev-fee event and no qualifying order |
| `stats.cumulative.total_successful_trades` | integer | `38383` `s=success` | Never |
| `stats.cumulative.total_volume_sats` | integer, sats | `38383` tag `amt` | Never |
| `stats.trade_size.min_trade_sats` | integer, sats | `38383` tag `amt` | No qualifying order with a parseable `amt` |
| `stats.trade_size.max_trade_sats` | integer, sats | `38383` tag `amt` | No qualifying order with a parseable `amt` |
| `stats.trade_size.mean_trade_sats` | number, sats | `38383` tag `amt` | No qualifying order with a parseable `amt` |
| `stats.trade_size.median_trade_sats` | number, sats | `38383` tag `amt` | No qualifying order with a parseable `amt` |
| `stats.trade_size.std_dev_trade_sats` | number, sats | `38383` tag `amt` | No qualifying order with a parseable `amt` |
| `stats.trade_size.coefficient_of_variation` | number, ratio | Derived from std dev and median | Fewer than 2 orders, or median is exactly `0` |
| `stats.liveness.last_successful_trade_at` | string, RFC 3339 UTC | `38383` `created_at` | No successful order |
| `stats.liveness.days_since_last_trade` | integer, days | Derived from `last_successful_trade_at` | No successful order |
| `stats.liveness.successful_trades_last_7d` | integer | `38383` `created_at` | Never |
| `stats.liveness.successful_trades_last_30d` | integer | `38383` `created_at` | Never |
| `stats.liveness.successful_trades_last_90d` | integer | `38383` `created_at` | Never |
| `stats.consistency.active_days_last_30d` | integer, days | `38383` `created_at` | Never |
| `stats.consistency.max_consecutive_inactive_days_last_30d` | integer, days | `38383` `created_at` | Never |
| `stats.disputes.total_disputes` | integer | `38386`, deduplicated by `d` | Never |
| `stats.disputes.resolved_disputes` | integer | `38386` tag `s` | Never |
| `stats.disputes.active_disputes` | integer | `38386` tag `s` | Never |
| `stats.disputes.unknown_status_disputes` | integer | `38386` tag `s` | Never |
| `stats.disputes.disputes_per_100_trades` | number, rate | `38386` and `38383` | Zero successful trades |
| `stats.fiat_breakdown.orders_considered` | integer | `38383` tag `f` | Never |
| `stats.fiat_breakdown.distribution` | array of `{currency, orders, share_percent}` | `38383` tag `f` | No order carries a non-empty `f` |
| `stats.payment_method_breakdown.total_mentions` | integer | `38383` tag `pm` | Never |
| `stats.payment_method_breakdown.distribution` | array of `{method, mentions, share_percent}` | `38383` tag `pm` | No `pm` mentions exist |
| `stats.premium.premium_baseline_percent` | number, percent | `38383` tag `premium` | Fewer than 2 orders with a valid `premium` |
| `stats.premium.premium_dispersion_percent` | number, percent | `38383` tag `premium` | Fewer than 2 orders with a valid `premium` |
| `stats.bond_policy.status` | string enum | `38385` tag `bond_enabled` | Never (`unknown` is itself a value) |
| `activity.granularity` | string enum | Derived from the range span | Zero successful orders and no explicit range |
| `activity.range_start` | string, RFC 3339 UTC | Range bounds or order timestamps | Zero successful orders and no explicit range |
| `activity.range_end` | string, RFC 3339 UTC | Range bounds or order timestamps | Zero successful orders and no explicit range |
| `activity.buckets` | array of `{bucket_start, successful_trades, volume_sats, median_trade_sats}` | `38383` | Zero successful orders and no explicit range |

## Pages

Each page below defines its metrics, states how they are computed, and describes what
decision they support.

- [Longevity and liveness](longevity-liveness.md)
- [Trade size and consistency](trade-size-consistency.md)
- [Disputes and bond policy](disputes-bond-policy.md)
- [Fiat, payment method, and premium](context-signals.md)
- [Activity grid](activity-grid.md)
