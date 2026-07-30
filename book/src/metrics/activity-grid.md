# Activity grid

The activity grid is the one part of the report that shows change over time instead of
a single lifetime summary: a table with one row per time bucket, and for each bucket,
how many successful trades happened, how much volume they moved, and the median trade
size within that bucket. Where [cumulative performance](trade-size-consistency.md) tells
you the totals, the activity grid tells you *when* that activity happened — steadily
across the node's history, concentrated in one burst, or trailing off recently.

It's built from the same qualifying successful orders used everywhere else in the
report (see [Trade size and consistency](trade-size-consistency.md#what-counts-as-a-qualifying-order)),
bucketed by each order's timestamp.

## Range

By default, the grid spans the node's own observed lifetime: from its earliest
qualifying order to its latest. `--since` and `--until` narrow that to an explicit
window instead, and once given, that window is authoritative — even if it turns out to
contain zero orders. In that case the grid still renders every bucket across the
requested range, each showing zero trades, rather than collapsing to the empty/null
result that's reserved specifically for a node with no successful orders at all. That
distinction matters: an empty grid over a requested range tells you "nothing happened
here," which is different information from "this node has no order history to
report."

## Granularity and its threshold

Each row in the grid represents a day, a month, or a year, depending on the
granularity. `--view` lets you force one explicitly; without it, `mostro-score` picks
automatically based on how wide the requested (or inferred) range is:

| Range | Granularity |
|---|---|
| ≤ 90 days | daily |
| ≤ 730 days (~2 years) | monthly |
| beyond that | yearly |

The reasoning behind these two boundaries is practical, not statistical: a daily grid
over a two-year range would produce over 700 rows, which is unreadable in a terminal
table, so the tool switches to coarser buckets before that happens. The 90-day and
730-day cutoffs were chosen by thinking through what a usable table size looks like,
not derived from a formula.

Because a `--view` override can still force daily granularity over a much wider range
than the automatic rule would ever choose on its own, `mostro-score` prints a stderr
warning whenever that happens, naming the exact number of rows the result will have.
The warning reuses the same 90-day boundary the automatic selection uses, so the two
can never disagree about what counts as "too wide."

One more detail worth knowing if you inspect `range_start`/`range_end` closely: even a
range you didn't set explicitly still gets aligned to the chosen granularity's
boundaries. A grid forced to monthly view snaps its displayed range to the first and
last day of the calendar month, not to the raw timestamp of whichever order happened to
be first or last.

## Progress indicator threshold

Unrelated to the grid itself, but worth documenting here since it's the report's other
numeric threshold: while fetching data from relays, `mostro-score` prints a "still
fetching" message to stderr if the fetch takes longer than **3 seconds**, so you're not
left wondering whether the tool has stalled. Unlike the granularity boundaries above,
this number came from direct measurement rather than reasoning: three real
connect-and-fetch round trips against the default relay
(`wss://relay.mostro.network`) took 2.06s, 1.96s, and 1.69s. Normal single-relay
operation sits around two seconds, so three seconds sits comfortably above that normal
variance while still catching a fetch that's genuinely running slow. Pass `--quiet` to
suppress it along with the tool's other transient status messages.
