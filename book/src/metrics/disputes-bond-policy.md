# Disputes and bond policy

## Dispute signals

### What it is

How many disputes the node has had, how they resolved, and disputes per 100 successful
trades — a normalized rate for comparing nodes of different sizes.

### Source

Dispute events (kind `38386`), deduplicated by `d` tag to each dispute's latest state.
The final `s` tag classifies it: `resolved` (`settled`/`seller-refunded`/`released`),
`active` (`initiated`/`in-progress`), or `unknown` (anything else, or missing) — still
counted once toward the total, since a real dispute event exists regardless of status.

### Not-applicable rule

`disputes_per_100_trades` is not applicable only when successful trades are zero —
there's no denominator. Zero disputes with trades present is a real `0.0`, not
not-applicable.

### How to read it

Compare the rate, not the raw count, across nodes. `unknown`-status disputes are worth
a second look — something happened that the node's own data doesn't fully explain.

## Bond policy

### What it is

Whether the node requires traders to lock a bond before a trade: `enabled`,
`disabled`, or `unknown`.

### Source

The node's most recent instance-status event (kind `38385`), reading its
`bond_enabled` tag. `true`/`false` map directly; a missing event or an unparseable
value maps to `unknown` — never collapsed into `disabled`.

### How to read it

Descriptive, not a verdict: the report never implies which status is safer. Weigh it
alongside dispute history and trade record, not as a pass/fail check.
