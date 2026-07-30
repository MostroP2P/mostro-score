# Trade size and consistency

## What counts as a qualifying order

Both metrics on this page draw from the same underlying set: order events, kind
`38383`, scoped to the queried node as author with `z=order` and `y=mostro`. Mostro
republishes an order event every time its status changes, so the same order can appear
on a relay several times over its lifetime. `mostro-score` deduplicates by the order's
`d` tag, keeping only each order's most recent published state, and then counts an
order as "qualifying" only if that final state's `s` tag reads `success`. An order that
was created, matched, and then canceled never contributes to these numbers — only
trades that actually completed do.

## Cumulative performance

This is the simplest metric in the report: how many trades has the node completed,
ever, and how much sats volume did they move in total. It has no time window and no
"N/A" case — a node with zero successful trades reports `0` for both, which is a real,
meaningful answer (this node has no completed trade history), not a missing value.

**How to read it:** cumulative performance is a floor, not a full picture. A node could
have moved a large volume years ago and gone dormant since — pair this with
[liveness](longevity-liveness.md) to see whether that volume reflects an active node or
a historical one.

## Trade size

Trade size describes the shape of the node's individual trades: the smallest and
largest amounts (in sats), the mean and median, the standard deviation, and the
coefficient of variation — the standard deviation divided by the median, which
collapses "how spread out are the trade sizes" into a single, scale-independent number.
A coefficient of variation near zero means the node's trades tend to be similar in
size; a high one means trade sizes swing widely, from tiny to very large, on the same
node.

The amount comes from the `amt` tag on each qualifying successful order, parsed as an
integer number of sats. Not every order publishes a parseable `amt` — when one doesn't,
that order still counts toward [cumulative performance](#cumulative-performance) and
[liveness](longevity-liveness.md), but it's simply excluded from this specific
calculation, since there's nothing valid to average in.

When there isn't enough data, the fields report their absence rather than a misleading
number. With zero qualifying orders carrying a parseable `amt`, every field here is not
applicable. The coefficient of variation has an even stricter rule on top of that: it's
not applicable whenever fewer than two orders exist (variation needs at least two
points to mean anything) or whenever the median trade size is exactly zero, since
dividing by a zero median is mathematically undefined no matter how many samples you
have.

One deliberate precision detail: when the number of qualifying orders is even, the
median is the average of the two middle values, and that average is reported exactly
as computed, including a fractional sats value like `0.5` — it is never rounded down to
a whole number, since doing so would also quietly corrupt the coefficient of variation
that's computed from it.

**How to read it:** a low coefficient of variation suggests a node that handles
similarly-sized trades consistently. A very high one isn't necessarily bad — it might
just mean the node serves both small retail trades and large ones — but it's worth
knowing before you send a trade far outside what the node normally handles.
