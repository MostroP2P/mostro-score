# Fiat, payment method, and premium

These three signals characterize the kind of trading a node does rather than its
reliability. All are computed over the
[qualifying successful orders](trade-size-consistency.md#qualifying-orders) defined in
the previous page.

## String comparison rule

All three compare tag values byte-for-byte: no whitespace trimming, no case folding, no
Unicode normalization. `USD` and `usd` are distinct currencies here, as are `Cash` and
` Cash`.

This is deliberate. The Mostro protocol does not normalize these values before
publishing them, so merging variants would conceal a formatting inconsistency in a
node's own software rather than report it. The breakdowns describe what the node
actually published.

## Fiat currency breakdown

**Definition.** The distribution of settlement currencies across qualifying orders, as
an order count and percentage share per currency.

**Computation.** Tallied from the `f` tag, one value per order. Orders with an empty
`f` value are excluded from both numerator and denominator rather than grouped under an
empty-string bucket. Results are ranked by descending share, with ties broken by
currency name ascending, so the ordering is deterministic despite the underlying tally
being unordered.

**Usability.** Establishes which currencies the node actually settles in, and in what
proportion, before committing to a trade denominated in one of them.

## Payment method breakdown

**Definition.** The distribution of payment methods across qualifying orders, as a
mention count and percentage share per method.

**Computation.** Tallied from the `pm` tag. Unlike every other tag in this report, `pm`
is multi-valued: a single order may list several accepted methods, and each is counted
as an independent mention. The denominator is therefore total mentions, not total
orders, and a node whose orders each list three methods reports three times as many
mentions as orders. Empty values are filtered defensively, as relay data is untrusted.
Ranking follows the same rule as the fiat breakdown.

**Usability.** Establishes whether the node supports an intended payment method, and
how central that method is to its trading.

## Premium signal

**Definition.** The node's pricing relative to market rate:
`premium_baseline_percent`, the median premium applied, and
`premium_dispersion_percent`, the population standard deviation around it.

**Computation.** Read from the `premium` tag, parsed as a signed integer percentage
where negative denotes a discount and positive a markup. Orders with a missing or
unparseable value are excluded rather than treated as zero, which would bias the
baseline toward the mean. Both figures require at least two valid values; dispersion is
undefined for a single point, and a single-point median would misrepresent a baseline.

The standard deviation is population, consistent with
[trade size](trade-size-consistency.md#trade-size).

**Usability.** The baseline is the premium to expect on a typical order. The dispersion
qualifies that expectation: high dispersion means quoted premiums vary substantially
between orders, so the baseline is a weak predictor of any individual quote.
