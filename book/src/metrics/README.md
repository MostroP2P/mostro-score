# Metrics

Every metric is computed from Nostr events scoped to the queried node: its own pubkey
as author, the kind's expected `z` tag value, and `y=mostro`. Events failing that scope,
or any other kind, are silently excluded.

A metric is `null`/`N/A` when there isn't enough data to compute it, never a fabricated
zero. Each page below documents what triggers that case for its own metrics.
