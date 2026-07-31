# Longevity and liveness

## Longevity

**Definition.** The node's operating age: `first_seen_at`, the timestamp of its
earliest observed activity, and `days_active`, the elapsed days from that point to
report-generation time.

**Computation.** The primary anchor is the oldest dev-fee-payment event (kind `8383`).
Mostro instances emit one per completed trade as part of the protocol's fee split, so
the anchor is not something an operator can suppress without also forgoing trades. When
no dev-fee event exists, `days_active` falls back to the oldest qualifying successful
order, measured to report-generation time — not to the node's most recent order, which
would freeze the value at the interval between two trades and report `0` for a node
with exactly one. The fallback path leaves `first_seen_at` unset, since no dev-fee
event exists to date it.

**Not applicable.** Both fields, when the node has neither a dev-fee event nor a
qualifying successful order. `first_seen_at` alone, whenever the fallback path applies.

**Usability.** Establishes how much operating history exists to evaluate. It is a
precondition for interpreting the other metrics, not a quality signal on its own: a
high `days_active` with zero successful trades describes a node that has existed
without trading.

## Liveness

**Definition.** Current activity level: the timestamp of the most recent successful
trade, elapsed days since it, and successful-trade counts over rolling 7, 30, and
90-day windows.

**Computation.** Derived from the `created_at` timestamps of qualifying successful
orders. The three rolling counts are independent windows, not cumulative buckets — a
trade three days old is counted in all three.

**Not applicable.** The last-trade timestamp and elapsed days, when no successful order
exists. The three rolling counts are always computed; zero is a result, not an absence.

**Usability.** Distinguishes an actively used node from a historically active one.
Read alongside longevity: strong `days_active` with zero trades in the 90-day window
indicates a node that has stopped trading, whatever the reason.

## Activity consistency

**Definition.** Distribution of trading across time: the count of distinct calendar
days with at least one successful trade in the last 30, and the longest run of
consecutive inactive days within that window.

**Computation.** The window spans exactly 30 UTC calendar days, ending on and
including the current day. Both bounds are aligned to day indices before comparison
rather than derived by subtracting 30 × 86400 seconds, which would span 31 distinct day
indices whenever report-generation time is not exactly midnight UTC. The inactive-run
calculation includes the gap preceding the first active day and the gap following the
last, not only the gaps between active days.

**Not applicable.** Never. A node with no trades in the window reports zero active days
and a 30-day inactive run, both computed results.

**Usability.** Separates steady trading from bursty trading at equal trade counts. Two
nodes with the same 30-day volume differ materially if one traded on 20 days and the
other on 2.
