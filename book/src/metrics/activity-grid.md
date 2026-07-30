# Activity grid

### What it is

A time-bucketed table of successful trades, volume, and median trade size per bucket —
where [cumulative performance](trade-size-consistency.md#cumulative-performance) shows
totals, this shows *when* that activity happened.

### Source

The same qualifying successful orders used throughout the report (see
[Trade size and consistency](trade-size-consistency.md#what-counts-as-a-qualifying-order)),
bucketed by timestamp.

## Range

`--since`/`--until` set an explicit window; without them, the range is inferred from
the node's own earliest/latest qualifying order.

An explicit range always wins, even with zero orders inside it: the grid still renders
every bucket across that range showing zero trades, rather than the empty/null result
reserved for a node with no successful orders at all.

## Granularity and its threshold

`--view` forces `daily`, `monthly`, or `yearly`. Without it:

| Range | Granularity |
|---|---|
| ≤ 90 days | daily |
| ≤ 730 days (~2 years) | monthly |
| beyond that | yearly |

**Source of the boundaries:** practical, not statistical — reasoned from usable
terminal-table row count (a daily grid over 2 years would produce 700+ rows), not
measured or derived from a formula.

If `--view` forces daily granularity over a range wider than 90 days anyway,
`mostro-score` prints a stderr warning naming the resulting row count, using the same
90-day boundary so the warning and the automatic rule never disagree.

A defaulted range still snaps to the chosen granularity's boundaries — e.g. a forced
monthly view snaps to the first/last day of the calendar month, not a raw timestamp.

## Progress indicator threshold

### What it is

A "still fetching" message printed to stderr when a relay fetch runs past **3
seconds**, so a slow fetch doesn't look like a stall. Suppressed by `--quiet`.

### Source

Direct measurement, not reasoning: 3 real connect-and-fetch round trips against
`wss://relay.mostro.network` took 2.06s/1.96s/1.69s. Normal operation sits around 2s,
so 3s sits comfortably above that variance while still catching a genuinely slow
fetch.
