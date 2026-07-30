# Trade size and consistency

## Cumulative performance

Total successful trades and total volume in sats, over the node's full history.

## Trade size

Min, max, mean, median, and population standard deviation of trade amounts (sats),
plus the coefficient of variation (std dev / median): lower is more consistent.

All fields are `N/A` when there are zero qualifying successful orders with a parseable
`amt`. The coefficient of variation has its own, stricter rule: `N/A` when fewer than 2
orders exist, or when the median is exactly `0` (dividing by a zero median is
undefined regardless of sample size).

The median over an even-sized set is the average of its two middle values, which can be
a genuine fraction (e.g. `[0, 1]` medians to `0.5`) — it is never truncated to an
integer.
