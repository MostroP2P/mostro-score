# Fiat, payment method, and premium

These three describe what kind of trading a node does, not whether it's trustworthy.
All three read tags on the same qualifying successful orders described in
[Trade size and consistency](trade-size-consistency.md#what-counts-as-a-qualifying-order),
and all compare values byte-for-byte: no trimming, no case folding. `"USD"`/`"usd"` and
`"Cash"`/`" Cash"` are distinct values, since the protocol itself doesn't normalize them.

## Fiat currency breakdown

### What it is

Which fiat currencies the node's trades settle in, ranked by share of orders,
descending (ties broken alphabetically).

### Source

The `f` tag on each qualifying successful order. Not applicable when no order carries
a non-empty value.

### How to read it

Tells you what to expect before trading — a EUR-heavy node isn't a bad fit for USD
trading, but it's useful context up front.

## Payment method breakdown

### What it is

Ranked distribution of payment methods used, by mention count, not order count.

### Source

The `pm` tag, a multi-value Nostr tag: one order can mention several methods, and each
mention counts individually. An order listing 3 methods contributes 3 mentions, not 1.
Not applicable when there are no mentions at all.

### How to read it

Use it to check whether the node typically supports your preferred payment method.

## Premium signal

### What it is

`premium_baseline_percent` (median) and `premium_dispersion_percent` (population
standard deviation) of the node's pricing premium/discount versus market rate.

### Source

The `premium` tag, a signed integer percentage (negative = discount, positive =
markup). Both fields need at least 2 qualifying orders with a valid `premium` value;
otherwise not applicable.

### How to read it

The baseline is the premium to expect on a typical trade. The dispersion tells you how
reliable that expectation is — high dispersion means premiums swing widely between
orders.
