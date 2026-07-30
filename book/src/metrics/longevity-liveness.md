# Longevity and liveness

## Longevity

Longevity answers a simple question: how long has this node actually been running?
It's reported as two numbers, `first_seen_at` (a date) and `days_active` (a count),
and both exist to give a trader a sense of track record before they trust a node with
a trade.

The primary source is the dev-fee-payment event, kind `8383` with `z=dev-fee-payment`
and `y=mostro`. Every time a Mostro instance completes a trade, it pays a small
development fee, and that payment is published as a Nostr event carrying a timestamp.
`mostro-score` looks at every dev-fee event the node has ever published and takes the
oldest one — that timestamp becomes `first_seen_at`, and `days_active` is simply the
number of days between it and now. This is the most reliable anchor available, because
dev-fee payments are tied to the protocol's own fee-splitting logic, not to anything the
node operator controls or could omit.

Not every node has a dev-fee history to draw on — a very new node, or one running a
build that predates the dev-fee mechanism, might have none. When that happens,
`mostro-score` falls back to the node's own order history: it looks at the oldest
qualifying successful order (see [Trade size and consistency](trade-size-consistency.md)
for what "qualifying" means) and measures from there to now instead. This fallback
deliberately measures from the order to *now*, not from the first order to the *last*
order, because the latter would freeze at whatever the gap between two trades happened
to be — a node with exactly one successful trade would always show zero days active,
which misrepresents a node that traded once and then kept running.

If neither a dev-fee event nor a qualifying successful order exists at all, there's
nothing to measure from, and both fields print as not applicable (`N/A` in
console/plain text, `null` in JSON). That's not an error; it just means the node has no
publicly visible trading history yet.

**How to read it:** a longer `days_active` generally means more chances for the node's
behavior to have been tested by real trades, but it says nothing on its own about
whether those trades went well — pair it with the [dispute signals](disputes-bond-policy.md)
and [cumulative performance](trade-size-consistency.md) before drawing a conclusion.

## Liveness

Where longevity looks at the whole lifetime, liveness looks at whether the node is
*currently* active. It reports the timestamp of the node's last successful trade, how
many days have passed since then, and how many successful trades happened in the last
7, 30, and 90 days.

The source is the same order events (kind `38383`) used everywhere else in the report,
filtered down to the ones whose final, deduplicated status is `success`. A node with
zero successful orders reports every liveness field as not applicable — there's no last
trade to measure from, and the three rolling counts are all zero by definition rather
than missing.

**How to read it:** liveness is the most direct signal of whether a node is still being
actively used right now. A node with strong historical longevity but no successful
trades in the last 90 days may have gone quiet, been abandoned, or simply be in a slow
period — the report doesn't guess which, it just gives you the raw numbers to judge for
yourself.

## Activity consistency

Activity consistency measures how evenly a node's trading is spread out, rather than
clustered in a burst and then silent. It's reported as two numbers: how many distinct
calendar days had at least one successful trade in the last 30 days, and the longest
stretch of consecutive inactive days within that same window.

The window is exactly 30 UTC calendar days, ending on and including today — not a
rolling 30×86400-second cutoff. That distinction matters at the edges: measuring by raw
seconds instead of calendar days can silently include an extra day whenever the current
moment isn't exactly at midnight UTC, so `mostro-score` aligns both ends of the window
to day boundaries first.

**How to read it:** a node active on most of the last 30 days, with a short maximum
gap, is trading steadily. A node with only one or two active days and a gap of 28 days
either trades in occasional bursts or has mostly gone quiet — again, the numbers don't
label which one it is, but they give you enough to notice the pattern.
