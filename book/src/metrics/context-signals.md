# Fiat, payment method, and premium

These three signals are less about whether a node is trustworthy and more about what
kind of trading it actually does — which currencies it settles in, how traders pay,
and how its pricing compares to the market rate. All three read tags on the same
qualifying successful orders described in
[Trade size and consistency](trade-size-consistency.md#what-counts-as-a-qualifying-order),
and all three compare their values byte-for-byte: no trimming whitespace, no case
folding. `"USD"` and `"usd"` are different values here, and so are `"Cash"` and
`" Cash"` — because the Mostro protocol itself doesn't normalize these values before
publishing them, and silently merging them could hide a real formatting bug in a
node's own software.

## Fiat currency breakdown

This shows which fiat currencies the node's trades settle in, and how much of its
volume each one represents — a currency's order count and its percentage share,
ranked from most to least common (ties are broken alphabetically, so the ranking is
always deterministic). It's built from the `f` tag on each qualifying successful
order.

If no qualifying order carries a non-empty `f` value, there's no distribution to build,
and the field reports as not applicable rather than an empty list.

**How to read it:** this tells you what to expect if you trade with the node — a node
that mostly settles in EUR isn't necessarily a bad fit if you want USD, but it's useful
context before you start.

## Payment method breakdown

Similar in shape to the fiat breakdown, but built from the `pm` tag, which records the
payment methods (bank transfer, cash, a specific app) buyers and sellers have used.
Unlike most tags in this report, `pm` can carry more than one value per order — a
single order might list several accepted methods — so this breakdown counts every
individual *mention* across all qualifying orders, not one count per order. A node
where every order lists three payment methods will show three times as many mentions
as orders, and that's expected, not a bug.

It reports as not applicable when there are no `pm` mentions at all across the node's
qualifying orders.

**How to read it:** use this to gauge whether the node typically supports the payment
method you plan to use, before you commit to a trade.

## Premium signal

Mostro orders are usually priced at some premium or discount relative to the market
rate, expressed as a signed percentage in the `premium` tag (a negative value means a
discount, a positive one a markup). This signal reports two numbers computed from that
tag across the node's qualifying successful orders: `premium_baseline_percent`, the
median premium the node has actually charged, and `premium_dispersion_percent`, the
population standard deviation around that median — how much the premium tends to swing
from order to order.

Both numbers need at least two data points to mean anything, so they report as not
applicable whenever fewer than two qualifying orders carry a valid, parseable `premium`
value.

**How to read it:** the baseline tells you roughly what premium to expect from this
node on a typical trade. The dispersion tells you how much that can vary — a low
dispersion means the node prices consistently near its baseline, while a high one means
premiums swing widely between orders, so the baseline alone is a less reliable
predictor of what you'll actually be offered.
