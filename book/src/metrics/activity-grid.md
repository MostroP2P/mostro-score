# Activity grid

**Definition.** A time-bucketed view of trading activity: one row per interval, each
reporting the successful-trade count, sats volume, and median trade size within that
interval.

**Computation.** Built from the
[qualifying successful orders](trade-size-consistency.md#qualifying-orders), assigned to
buckets by `created_at`. Buckets are contiguous and gap-free across the full range — an
interval with no trades is emitted with zero counts and a null median, not omitted, so
row position corresponds to elapsed time.

Where cumulative performance reports totals, the grid reports their distribution over
time.

## Range resolution

**Definition.** The interval the grid spans, reported as `activity.range_start` and
`activity.range_end`.

**Computation.** With neither `--since` nor `--until`, the range is inferred from the
earliest and latest qualifying order. An explicit bound overrides inference and is
authoritative even when it contains no orders: the grid then emits every bucket across
the requested range with zero counts.

That case is distinct from the null result reserved for a node with no qualifying
orders and no explicit range. An empty populated grid asserts that no activity occurred
in a specific interval; a null grid asserts that no interval could be determined. The
two are not interchangeable.

## Granularity

**Definition.** The bucket width: `daily`, `monthly`, or `yearly`.

**Computation.** `--view` sets it explicitly. Otherwise it is selected from the range
span:

| Range span | Granularity |
|---|---|
| ≤ 90 days | `daily` |
| ≤ 730 days | `monthly` |
| > 730 days | `yearly` |

Once granularity is known, both range bounds are snapped outward to the enclosing
bucket boundary — a monthly grid reports the first and last instant of the enclosing
calendar months, not the raw order timestamps. When snapping widens the range, the
widened interval is also what gets counted, so the grid never claims to cover a period
it excludes orders from.

**Threshold derivation.** The two boundaries are reasoned from output legibility, not
measured: a daily grid over a two-year span yields more than 700 rows, which exceeds
what a terminal table can usefully present. They are selected to keep row count bounded
at each tier, and are not derived from a statistical property of the data.

Because `--view` can force `daily` over a span the automatic rule would never select,
a warning naming the resulting row count is written to stderr whenever that occurs. The
warning reuses the same 90-day boundary, so it cannot disagree with the selection rule.

**Usability.** Distinguishes sustained activity from a single burst at equal totals,
and locates when a node's activity started, peaked, or stopped.

## Progress indicator threshold

**Definition.** A status line written to stderr when a relay fetch exceeds 3 seconds,
distinguishing a slow fetch from a stalled process. Suppressed by `--quiet`.

**Threshold derivation.** Measured, not reasoned: three connect-and-fetch round trips
against `wss://relay.mostro.network` completed in 2.06 s, 1.96 s, and 1.69 s. Nominal
single-relay operation centers near 2 seconds, so 3 seconds sits above normal variance
while still surfacing a genuinely degraded fetch.

This threshold governs process feedback only and does not affect any reported metric.
