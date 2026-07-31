# Disputes and bond policy

## Dispute signals

**Definition.** Counts of disputes by resolution state — resolved, active, and
unknown — their total, and the rate of disputes per 100 successful trades.

**Computation.** Dispute events (kind `38386`) are replaceable and republished on each
status change, so they are deduplicated by `d` tag to each dispute's most recent state
using the same tie-break rule as orders. The retained state's `s` tag determines
classification:

| `s` value | Classified as |
|---|---|
| `settled`, `seller-refunded`, `released` | Resolved |
| `initiated`, `in-progress` | Active |
| Any other value, or absent | Unknown |

A dispute with an unrecognized status is classified `unknown` rather than discarded:
the event establishes that a dispute occurred, independently of whether its outcome can
be determined. The three classes therefore always sum to the total.

The rate normalizes the total against `stats.cumulative.total_successful_trades`. It is
not applicable only when that denominator is zero. A node with successful trades and no
disputes reports `0.0`, which is a computed result, not an absence.

**Not applicable.** The rate alone, when the node has zero successful trades. The four
counts are always computed.

**Usability.** The rate is the comparable figure across nodes; the raw total is not,
since it scales with volume. A non-zero `unknown_status_disputes` warrants separate
attention: it indicates dispute activity the node's own published data does not fully
resolve.

## Bond policy

**Definition.** Whether the node requires a trader-posted bond before entering a trade,
as a three-valued status: `enabled`, `disabled`, or `unknown`.

**Computation.** Read from the `bond_enabled` tag on the node's current instance-status
event (kind `38385`), selected as the highest `created_at` among events whose `d` tag
equals the node's own pubkey. The tag maps `true` to `enabled` and `false` to
`disabled`; a missing instance-status event, a missing tag, or any unparseable value
maps to `unknown`.

`unknown` is never collapsed into `disabled`. The two describe different epistemic
states — one confirms no bond is required, the other confirms nothing — and merging
them would present an absence of data as a positive finding.

**Not applicable.** Never. Absence of data is reported as `unknown`, which is itself
one of the three values.

**Usability.** Descriptive only. The report does not rank the three statuses, because a
bond requirement raises the cost of trading and the cost of bad-faith behavior
simultaneously, and which tradeoff is preferable depends on context outside this tool's
inputs. Treat it as an input to a decision, not a verdict.
