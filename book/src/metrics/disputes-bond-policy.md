# Disputes and bond policy

## Dispute signals

Total, resolved, active, and unknown-status disputes (kind `38386`, deduplicated to
each dispute's latest status), plus disputes per 100 successful trades.
`disputes_per_100_trades` is `N/A` only when the node has zero successful trades —
there is no denominator — regardless of how many disputes exist. Zero disputes with a
nonzero trade count is a real ratio of `0.0`, not `N/A`.

A dispute's status is `resolved` for `settled`/`seller-refunded`/`released`, `active`
for `initiated`/`in-progress`, and `unknown` for anything else or a missing status —
still counted once toward the total, since a valid dispute event exists for it
regardless of status.

## Bond policy

Whether the node requires a bond deposit from traders: `enabled`, `disabled`, or
`unknown` (kind `38385` instance-status event, reading its `bond_enabled` tag).
`unknown` covers both a missing instance-status event and one whose `bond_enabled`
value fails to parse — never defaulting to `disabled`, and never implying which status
is safer.
