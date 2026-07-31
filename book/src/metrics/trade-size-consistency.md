# Trade size and consistency

## Qualifying orders

Every metric on this page is computed over the same input set, defined once here.

Order events (kind `38383`) are republished on each status transition, so a single
order appears multiple times on a relay. Orders are deduplicated by their `d` tag,
retaining only the most recent published state per order; ties on `created_at` are
broken by the greatest event id, making the selection deterministic regardless of the
order in which relays return events. An order qualifies only if that retained state
carries `s=success`. Canceled, pending, and expired orders contribute to no metric on
this page.

## Cumulative performance

**Definition.** Lifetime totals: the count of qualifying successful orders, and the sum
of their sats amounts.

**Computation.** A direct sum over the qualifying set, with no time window. Amounts are
accumulated with saturating addition, so a crafted extreme `amt` on an untrusted relay
event cannot overflow or wrap the total.

**Not applicable.** Never. A node with no qualifying orders reports zero for both,
which is a computed result.

**Usability.** Establishes scale. It is a lower bound on realized activity and carries
no recency information — pair it with liveness to determine whether the volume is
current or historical.

## Trade size

**Definition.** The distribution of individual trade amounts: minimum, maximum, mean,
median, population standard deviation, and coefficient of variation.

**Computation.** Read from the `amt` tag, parsed as an unsigned integer of sats. A
qualifying order whose `amt` is absent or unparseable is excluded from this
distribution only; it still counts toward cumulative performance and liveness, since
the trade demonstrably occurred.

The standard deviation is population, not sample — divided by N rather than N−1 —
because the qualifying set is the node's complete published record, not a sample drawn
from a larger one. The coefficient of variation is the standard deviation divided by
the median, yielding a dimensionless ratio comparable across nodes of different trade
sizes.

Over an even-sized set the median is the mean of the two central values and is reported
at full precision, including fractional sats. Truncating it to an integer would both
misstate the median and distort the coefficient of variation computed from it.

**Not applicable.** All six figures, when no qualifying order carries a parseable
`amt`. The coefficient of variation additionally, when fewer than two such orders exist
or the median is exactly `0`, since a zero denominator leaves the ratio undefined
regardless of sample size.

**Usability.** The coefficient of variation is the operative figure: it answers whether
a node's trades cluster around a typical size or span a wide range. A low value
indicates predictable sizing. A high value is not itself adverse — it may reflect a node
serving both retail and large trades — but it reduces the predictive value of the
median, which is relevant when sizing a trade well outside the node's typical range.
