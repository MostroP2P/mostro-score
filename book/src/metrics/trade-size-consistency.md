# Trade size and consistency

## What counts as a qualifying order

Order events (kind `38383`, `z=order`, `y=mostro`) republish on every status change.
`mostro-score` deduplicates by the order's `d` tag, keeps only the most recent
published state, and counts an order as qualifying only if that final state's `s` tag
reads `success`. A canceled order never contributes to these metrics.

## Cumulative performance

### What it is

Total successful trades and total sats volume, over the node's full history.

### Source

Every qualifying successful order, summed.

### How to read it

A floor, not the full picture: a node could have moved volume years ago and gone
dormant since. Pair it with [liveness](longevity-liveness.md#liveness).

## Trade size

### What it is

The shape of individual trades: min, max, mean, median, standard deviation (sats), and
the coefficient of variation (std dev ÷ median) — a single, scale-independent number
for how spread out trade sizes are.

### Source

The `amt` tag on each qualifying successful order. An order with no parseable `amt`
still counts toward cumulative performance and liveness, just not toward this
calculation.

### Not-applicable rules

- Every field: not applicable with zero qualifying orders carrying a parseable `amt`.
- Coefficient of variation, additionally: not applicable with fewer than 2 orders, or
  when the median is exactly `0` (dividing by a zero median is undefined).
- The median over an even-sized set is a genuine fraction (e.g. `0.5`), never truncated
  to an integer.

### How to read it

A low coefficient of variation means consistent trade sizes. A high one isn't
necessarily bad — it may just mean the node serves both small and large trades — but
it's worth knowing before trading far outside the node's typical size.
