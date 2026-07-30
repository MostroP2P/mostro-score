# Longevity and liveness

## Longevity

`first_seen_at` and `days_active` measure how long the node has been operating.

The primary anchor is the oldest dev-fee-payment event (kind `8383`). When none exists,
it falls back to the elapsed time between the node's first qualifying successful order
and now — the same "elapsed to now" semantic as the primary path, not first-order to
last-order, since that would stop increasing after the node's last trade and read `0`
for a node with exactly one successful order. When neither anchor exists, both fields
are `N/A`.

## Liveness

Last successful trade, days since it, and rolling 7/30/90-day successful-trade counts.
`N/A` when the node has zero successful orders.

## Activity consistency

Active days and the longest gap of consecutive inactive days, both over a fixed window
of exactly 30 UTC calendar days ending on and including today — not a rolling
30×86400-second cutoff, which would include an extra day whenever the current instant
isn't exactly at a UTC day boundary.
