# Fiat, payment method, and premium

These three signals read the `f`, `pm`, and `premium` tags on qualifying successful
orders. Comparisons are byte-for-byte: no trimming, no case normalization — `"USD"` and
`"usd"`, or `"Cash"` and `" Cash"`, are distinct values.

## Fiat currency breakdown

Ranked distribution of the `f` tag's value, descending by share, ties broken by
currency name ascending. `N/A` when no qualifying order carries a non-empty `f` value.

## Payment method breakdown

Ranked distribution of `pm` mentions (a multi-value Nostr tag: one order can mention
several methods, and each mention counts, not each order). `N/A` when there are no
mentions at all.

## Premium signal

Median (`premium_baseline_percent`) and population standard deviation
(`premium_dispersion_percent`) of the `premium` tag, parsed as a signed integer
percentage. Both `N/A` when fewer than 2 orders carry a valid `premium` value.
