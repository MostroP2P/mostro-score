# Flags reference

| Flag | Description |
|---|---|
| `-p`, `--pubkey <PUBKEY>` | The node's public key to look up (npub or hex). Falls back to `MOSTRO_SCORE_PUBKEY`, then a saved configuration file. Not required with `--init-config`. |
| `-r`, `--relays <RELAYS>` | Nostr relays to query, comma separated. Falls back to `MOSTRO_SCORE_RELAYS`, then a saved configuration file, then the compiled-in default relay. |
| `--format <FORMAT>` | `console`, `plain`, or `json`. Defaults to a saved configuration value, otherwise a colored console view in a terminal or plain text when not. `json` is always an explicit choice. |
| `--color` | Force colored console output on, even when not printing to a terminal. No effect on plain-text or json. |
| `--no-color` | Force colored output off. |
| `-q`, `--quiet` | Hide progress messages while connecting and fetching data. |
| `--since <DATE>` | Only include activity on or after this date. Accepts `YYYY-MM-DD` or a shorthand like `30d`, `6mo`, `1y`. |
| `--until <DATE>` | Only include activity on or before this date. Same format as `--since`. |
| `--view <GRANULARITY>` | `daily`, `monthly`, or `yearly`. Forces the activity grid's bucket size instead of automatic selection. |
| `--sections <SECTIONS>` | Comma-separated, no spaces: `fetch`, `activity`, `stats`, `recommendations`. Has no effect with `--format json`, which always includes everything. |
| `-d`, `--config-dir <DIR>` | Read (or, with `--init-config`, write) the configuration file in this directory instead of the default location. |
| `--init-config` | Create a starter configuration file, then exit without generating a report. |
| `--force` | Allow `--init-config` to overwrite an existing configuration file. Requires `--init-config`. |
| `-o`, `--output <FILE>` | Write the report to this file instead of printing it. Not compatible with the console format; defaults to plain text when `--format` is omitted. |

## Date shorthand

`--since`/`--until` accept an ISO 8601 date (`2026-01-15`) or a relative shorthand
resolved against the current date: `Nd` (days), `Nmo` (months), `Ny` (years) — e.g.
`30d`, `6mo`, `1y`. Month/year subtraction clamps to the last valid day of the target
month when the original day doesn't exist there (e.g. one month before March 31 lands
on February 28/29).
