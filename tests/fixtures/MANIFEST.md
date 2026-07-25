# Step -1 golden baseline — capture manifest

Captured against the current unmodified binary (pre-PR1) per
`specs/004-phase-4-implementation/plan.md`'s Step -1. Real relay: `wss://relay.mostro.network`
unless noted. Each scenario has `scenarioN_stdout.txt`, `scenarioN_stderr.txt`,
`scenarioN_exit.txt`, and (where the run depends on real historical events)
`scenarioN_now_pre.txt` / `scenarioN_now_post.txt` (wall-clock second immediately before/after
the process ran) and `scenarioN_events.ndjson` (the exact event set fetched, one JSON event per
line, in `dev_fee_events` then `order_events` order — the same shape `EventSource::fetch` returns).

| # | Scenario | Pubkey (hex) | Notes |
|---|----------|--------------|-------|
| 1 | Happy path | `82fa8cb978b43c79b2156585bac2c011176a21d2aead6d9f7c575c005be88390` | Real node, ~161 days of history, last activity 109+ days before capture (outside the 90-day rolling window and 30-day consistency window, so no printed value sits near a boundary). 188 dev-fee events, 300 raw order events, 20 raw `s=success` order events. |
| 2 | Malformed `--pubkey` | n/a | Literal string `not-a-valid-pubkey`. No relay access — pubkey parsing fails in `main()` before any fetch. |
| 3 | Malformed `--relays` | n/a | Literal string `not a valid relay url` passed with a valid pubkey — `client.add_relay(...)` fails at parse time. |
| 4 | Well-formed but unreachable `--relays` | n/a | `ws://127.0.0.1:1` (closed local port, no OS-level timeout risk). Reused scenario 1's pubkey; irrelevant since the relay never answers. |
| 5 | Fresh never-published keypair | `a9b1ee37f17b4fa8b514804b03d92774ff8250923091d6e08ac3172334588449` | Freshly generated via `Keys::generate()`, never published; zero fetched events. |
| 6 | Qualifying orders, no qualifying dev-fee event | `c6e5e031989223dd63e6ed49f0905a19a92ed86e0754721d6071133a9340bf7e` | Real node, 0 dev-fee events, 300 raw order events (247 `s=success` after dedup), last activity 100 days before capture. Exercises the no-dev-fee-events warning and the `days_active` fallback-to-orders path. |
| 7 | Qualifying dev-fee event, zero qualifying orders | `00000235a3e904cfe1213a8a54d6f1ec1bef7cc6bfaabd6193e82931ccf1366a` | Real node ("cuba" in `.atl/mostros.md`), 8 dev-fee events, 0 order events returned by this specific capture run (order-event delivery from this relay for this author was observed to vary between separate live queries during exploration — the committed fixture is the exact byte-for-byte result of the one official capture run, replayed from here on, no further live queries). |
| 8 | Two relays, one reachable, one not | same as scenario 7 | `--relays "wss://relay.mostro.network,ws://127.0.0.1:1"`. Current (pre-PR1) binary has no partial-relay-failure handling, so output is identical in shape to scenario 7 — real, unmodified behavior; PR 2/3 add the graceful-degradation warning this scenario is meant to eventually distinguish. **`scenario8_events.ndjson` is not used by `tests/cli_behavior.rs`'s scenario 8 test** — discovered during PR 1 Step D2: that file has 8 dev-fee events (identical ids to `scenario7_events.ndjson`) plus 15 extra kind-`38383` order events that do not appear in `scenario8_stdout.txt` (which is byte-identical to `scenario7_stdout.txt`, both showing 0 order events). The extra events came from the throwaway capture harness's separate, later query of the same relay/author, hitting the same flaky-delivery behavior noted below — not from the official binary run that produced `scenario8_stdout.txt`. `tests/cli_behavior.rs` replays `scenario7_events.ndjson` for scenario 8 instead, which is verified consistent with the captured output. |

## Candidate node pool

Source: `.atl/mostros.md` (pubkeys the maintainer identified as real, settled Mostro nodes) plus
an author-scoped probe of `wss://relay.mostro.network`'s recent kind-`38383` order events to find a
node with orders but zero dev-fee events (scenario 6's requirement, not satisfied by any of the
six `.atl/mostros.md` entries).

| Label | Pubkey (hex) | dev_fee | orders (raw) | days since last order |
|---|---|---|---|---|
| cuba | `00000235a3e904cfe1213a8a54d6f1ec1bef7cc6bfaabd6193e82931ccf1366a` | 8 | 0 (this capture) | n/a |
| espana | `0000cc02101ec29eea9ce623258752b9d7da66c27845ed26846dd0b0fc736b40` | 174 | 28 | 9 (too recent for a settled scenario) |
| colombia | `00000978acc594c506976c655b6decbf2d4af25ffdaa6680f2a9568b0a88441b` | 61-126 | 0-12 (varied between queries) | 9 |
| bolivia | `fcc2a0bd8f5803f6dd8b201a1ddb67a4b6e268371fe7353d41d2b6684af7a61e` | 0 | 244 | n/a |
| venezuela | `000009ee1e4b1dc7add19ab30e4ef854d7b562e208b62686fd9002b50b24dabb` | 10-136 | 0-2 (varied between queries) | 9 |
| default/oldest | `82fa8cb978b43c79b2156585bac2c011176a21d2aead6d9f7c575c005be88390` | 188 | 300 | 109 — used for scenario 1 |
| (discovered) | `c6e5e031989223dd63e6ed49f0905a19a92ed86e0754721d6071133a9340bf7e` | 0 | 300 (247 `s=success`) | 100 — used for scenario 6 |

`wss://relay.mostro.network` does not always return an author's complete order-event history in a
single `fetch_events` call within a 10-15s timeout — repeated queries for the same author against
the same filter returned different counts during exploration (observed for cuba, colombia,
venezuela). This does not affect the committed fixtures: each is the untouched byte-for-byte result
of the one official capture run recorded above, never re-queried live again.
