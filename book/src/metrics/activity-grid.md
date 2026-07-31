# Activity grid

**Definition.** A time-bucketed view of trading activity: one row per interval, each
reporting the successful-trade count, sats volume, and median trade size within that
interval. Where cumulative performance reports lifetime totals, the grid reports how
those totals are distributed over time.

**Computation.** Built from the
[qualifying successful orders](trade-size-consistency.md#qualifying-orders), assigned to
buckets by `created_at`. Buckets are contiguous and gap-free across the full range — an
interval with no trades is emitted with zero counts and a null median, not omitted, so
row position corresponds to elapsed time. Range and bucket width are resolved by the
rules below.

**Not applicable.** The whole grid — granularity, both range bounds, and the bucket
list — when the node has no qualifying orders and no explicit range was requested.

**Usability.** Distinguishes sustained activity from a single burst at equal lifetime
totals, and locates when a node's trading started, peaked, or stopped.

## Range resolution

With neither `--since` nor `--until`, the range is inferred from the earliest and latest
qualifying order. An explicit bound overrides inference and is authoritative even when
it contains no orders: the grid then emits every bucket across the requested range with
zero counts.

That case is distinct from the null result described above. An empty populated grid
asserts that no activity occurred in a specific interval; a null grid asserts that no
interval could be determined. The two are not interchangeable.

## Granularity

`--view` sets the bucket width explicitly. Otherwise it is selected from the range span:

| Range span | Granularity |
|---|---|
| ≤ 90 days | `daily` |
| ≤ 730 days | `monthly` |
| > 730 days | `yearly` |

Once granularity is known, both range bounds are snapped outward to the enclosing bucket
boundary — a monthly grid reports the first and last instant of the enclosing calendar
months, not the raw order timestamps. When snapping widens the range, the widened
interval is also what gets counted, so the grid never claims to cover a period it
excludes orders from.

**Threshold derivation.** The two boundaries are reasoned from output legibility, not
measured: a daily grid over a two-year span yields more than 700 rows, which exceeds
what a terminal table can usefully present. They are selected to keep row count bounded
at each tier, and are not derived from a statistical property of the data.

Because `--view` can force `daily` over a span the automatic rule would never select, a
warning naming the resulting row count is written to stderr whenever that occurs. The
warning reuses the same 90-day boundary, so it cannot disagree with the selection rule.

## Progress indicator threshold

Not a metric — process feedback, documented here because it is the report's other
numeric threshold. A status line is written to stderr when a relay fetch exceeds 3
seconds, distinguishing a slow fetch from a stalled process. Suppressed by `--quiet`.

**Threshold derivation.** Measured, not reasoned: three connect-and-fetch round trips
against `wss://relay.mostro.network` completed in 2.06 s, 1.96 s, and 1.69 s. Nominal
single-relay operation centers near 2 seconds, so 3 seconds sits above normal variance
while still surfacing a genuinely degraded fetch.
