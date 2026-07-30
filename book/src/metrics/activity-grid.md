# Activity grid

A time-bucketed table of successful trades, volume, and median trade size, built from
the node's qualifying successful orders.

## Range

`--since`/`--until` set an explicit range. It always wins, even over what orders exist:
an explicit range with zero orders inside it still renders a real grid with empty
buckets spanning that range, never the null/empty result reserved for a node with no
successful orders at all. With neither flag, the range is inferred from the orders'
own earliest/latest timestamp.

## Granularity and its threshold

`--view` forces `daily`, `monthly`, or `yearly`. Without it, granularity is chosen
automatically from the range's span:

| Range | Granularity |
|---|---|
| ≤ 90 days | daily |
| ≤ 730 days (~2 years) | monthly |
| beyond that | yearly |

These two boundaries were picked by reasoning about a terminal table's usable row
count — a table with hundreds of rows is unusable — not measured empirically. When a
`--view` override forces daily granularity over a range wider than 90 days anyway, the
tool prints a stderr warning naming the resulting row count, using this same boundary
so the warning and the automatic-selection rule never disagree about what counts as
"too wide."

A defaulted range (no explicit `--since`/`--until`) still snaps to the enclosing
bucket's start/end once a granularity is known — e.g. a forced monthly view snaps to
the first/last day of the calendar month, not a raw mid-month timestamp.

## Progress indicator threshold

A separate, unrelated threshold: `mostro-score` prints a "still fetching" message to
stderr if a relay fetch runs past **3 seconds**. This was set from real measurement, not
reasoning: 3 connect-and-fetch round trips against the default relay
(`wss://relay.mostro.network`) with a real pubkey took 2.06s/1.96s/1.69s. Normal
single-relay operation sits around 2 seconds, so 3 seconds is comfortably above that
variance while still catching a genuinely slow fetch. Suppressed by `--quiet`.
