# Disputes and bond policy

## Dispute signals

A dispute means a trade broke down badly enough that a third party had to step in and
resolve it. This section reports how many disputes a node has had, how they were
resolved, and how that compares to the node's trade volume — because a handful of
disputes on a node with thousands of trades reads very differently from the same
handful on a node with only a dozen.

The source is the dispute event, kind `38386`, scoped to the node. Like orders, a
dispute is republished every time its status changes, so `mostro-score` deduplicates by
the dispute's `d` tag and keeps only its most recent state. That final state's `s` tag
is then classified into one of three buckets: `resolved` (`settled`,
`seller-refunded`, or `released` — the dispute concluded), `active` (`initiated` or
`in-progress` — it's still open), or `unknown` (any other value, or a missing one). A
dispute lands in `unknown` rather than being dropped, because the event itself proves a
real dispute happened, even if its final outcome can't be classified from the tag.

The report also computes disputes per 100 successful trades, a normalized rate that
lets you compare nodes of very different sizes on equal footing. This rate is not
applicable only when the node has zero successful trades at all — there's no
denominator to divide by. A node with disputes but zero trades in the denominator
(possible if every trade failed or was canceled) reports the rate as not applicable
too, for the same reason. A node with trades but *zero* disputes reports the rate as a
real `0.0`, which is a meaningful, favorable number, not a placeholder for missing data.

**How to read it:** look at the rate, not the raw count, when comparing nodes. A high
rate is a real warning sign; a low one, especially alongside a long trade history, is a
positive signal. `unknown`-status disputes are worth a second look on their own — they
mean something happened that the node's own data doesn't fully explain.

## Bond policy

Some Mostro nodes require traders to lock a small bond before entering a trade, as a
deterrent against bad-faith behavior. Bond policy reports whether this node does:
`enabled`, `disabled`, or `unknown`.

The source is the instance-status event, kind `38385`, which a Mostro node republishes
periodically with its own operational settings, including a `bond_enabled` tag.
`mostro-score` selects the node's single most recent instance-status event and reads
that tag directly: `true` maps to `enabled`, `false` maps to `disabled`. Anything
else — a missing instance-status event entirely, or a `bond_enabled` value that isn't
recognizably `true`/`false` — maps to `unknown`. `unknown` is deliberately never
collapsed into `disabled`: not knowing whether a bond is required is a different, more
uncertain situation than confirming one isn't, and the report is written to keep that
distinction visible rather than picking a side.

**How to read it:** this metric is descriptive, not a verdict. The report will never
tell you `enabled` is safer than `disabled` or vice versa, because that depends on
context this tool doesn't have — a bond requirement raises the cost of trading but also
raises the cost of bad-faith behavior on both sides. Treat it as one more fact to weigh
alongside the node's dispute history and trade record, not a pass/fail check.
