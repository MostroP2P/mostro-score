# Longevity and liveness

## Longevity

### What it is

How long the node has been operating: `first_seen_at` (a date) and `days_active` (a
day count).

### Source

The oldest dev-fee-payment event (kind `8383`, `z=dev-fee-payment`, `y=mostro`) — a
small fee every Mostro instance pays each time it completes a trade. When the node has
none, longevity falls back to its oldest qualifying successful order instead (see
[Trade size and consistency](trade-size-consistency.md#what-counts-as-a-qualifying-order)),
measured to *now*, not to the node's last order — otherwise a node with exactly one
trade would always read zero days active.

If neither exists, both fields are not applicable: the node has no publicly visible
history yet, not an error.

### How to read it

A longer `days_active` means more chances for the node's behavior to have been tested,
but says nothing about how those trades went. Pair it with
[dispute signals](disputes-bond-policy.md) and
[cumulative performance](trade-size-consistency.md#cumulative-performance).

## Liveness

### What it is

Whether the node is *currently* active: last successful trade, days since it, and
rolling 7/30/90-day successful-trade counts.

### Source

The same order events (kind `38383`) used throughout the report, filtered to `success`
status. Zero successful orders reports every field as not applicable.

### How to read it

Strong longevity with no recent trades may mean the node has gone quiet, been
abandoned, or is in a slow period — liveness doesn't say which, only that current
activity is low.

## Activity consistency

### What it is

How evenly trading is spread out: active days in the last 30, and the longest gap of
consecutive inactive days in that same window.

### Source

Order timestamps, counted over a fixed window of exactly 30 UTC calendar days ending
today — aligned to day boundaries, not a rolling 30×86400-second cutoff, which would
include an extra day whenever "now" isn't exactly at midnight UTC.

### How to read it

Many active days with a short max gap means steady trading. Few active days with a long
gap means bursty or mostly-quiet trading.
