# Mostro Score

A CLI tool that computes reputation statistics for a [Mostro](https://mostro.network)
node by reading the public Nostr events it has published.

## Overview

Mostro nodes publish public, verifiable Nostr events for every order, dispute,
instance-status update, and dev-fee payment they process. `mostro-score` fetches those
events from one or more relays, scopes them to a single node's pubkey, and turns them
into a report: how long the node has been active, how much volume it has moved, how
consistently it trades, how its disputes resolve, and whether it enforces a bond
policy.

There is no single trust score. Each metric is reported on its own, with enough
context to interpret it, so a trader forms their own judgment instead of trusting one
number.

## Features

- **Reputation report**: node identity, relay fetch summary, a time-bucketed activity
  grid, general statistics, and plain-language recommendations.
- **Multiple output formats**: colored console tables, plain text for scripting, or a
  stable JSON schema for machine consumption.
- **Time-scoped reports**: `--since`/`--until`/`--view` to narrow the activity grid to
  a date range and granularity.
- **Saved configuration**: `--init-config` scaffolds a config file so preferred flags
  don't need to be repeated on every run.
- **Event deduplication**: replaceable events (orders, disputes, instance status) are
  deduplicated to their latest published state before anything is computed.

## How it works

`mostro-score` fetches four kinds of Nostr events, each scoped to the queried node
(pubkey as author, expected `z` tag, `y=mostro`):

- **Dev-fee payments** (kind `8383`) — anchor the node's longevity.
- **Orders** (kind `38383`) — back trade size, liveness, activity consistency, and the
  fiat/payment-method/premium breakdowns. Only orders whose final, deduplicated status
  is `success` count toward these metrics.
- **Disputes** (kind `38386`) — back the dispute signals.
- **Instance status** (kind `38385`) — backs the bond-policy signal.

See [the book](book/src/metrics/README.md) for what each metric measures, how to
interpret it, and its exact source event/tag.

## Installation

### Prerequisites

- Rust 1.94.0 or later (pinned via `rust-toolchain.toml`)
- Network access to a Nostr relay

### Build from source

```bash
git clone https://github.com/MostroP2P/mostro-score.git
cd mostro-score
cargo build --release
```

The binary will be available at `target/release/mostro-score`.

### Install globally

```bash
cargo install --path .
```

## Usage

### Basic usage

```bash
mostro-score --pubkey npub1...
```

`--pubkey` can also come from the `MOSTRO_SCORE_PUBKEY` environment variable or a saved
configuration file.

### Custom relays

```bash
mostro-score --pubkey npub1... --relays wss://relay.mostro.network,wss://relay.damus.io
```

### Save preferred defaults

```bash
mostro-score --init-config
```

Scaffolds a starter `config.toml` (relays active, everything else as commented-out
examples) so `--pubkey`, `--format`, `--view`, `--color`, and `--sections` don't need
repeating on every run.

### Flags

Run `mostro-score --help` for the full, current list. The most commonly used:

| Flag | Purpose |
|---|---|
| `-p, --pubkey <PUBKEY>` | Node to analyze (npub or hex) |
| `-r, --relays <RELAYS>` | Relays to query, comma separated |
| `--format <console\|plain\|json>` | Output format |
| `--since <DATE>` / `--until <DATE>` | Scope the activity grid (`YYYY-MM-DD` or `30d`/`6mo`/`1y`) |
| `--view <daily\|monthly\|yearly>` | Force the activity grid's granularity |
| `--sections <LIST>` | Only render these report sections |
| `-o, --output <FILE>` | Write the report to a file |
| `--init-config` | Scaffold a configuration file |

Full reference, including every flag's precedence and every metric's methodology: see
[book/](book/src/SUMMARY.md).

## Documentation

- **[The book](book/src/SUMMARY.md)** — installation, the full flags reference, the
  configuration file, output formats, and every report metric.
- [Mostro protocol documentation](https://mostro.network/protocol/other_events.html) —
  the Nostr event kinds this tool reads.

## Contributing

This project follows a spec-driven development workflow using
[spec-kit](https://github.com/github/spec-kit). Project principles and constraints are
ratified in [`.specify/memory/constitution.md`](.specify/memory/constitution.md), which
requires every feature to go through this gated sequence, with a review gate before
moving to the next step:

`constitution → specify → clarify → plan → checklist → tasks → analyze → implement → converge`

Contributions are welcome! Please:

1. Fork the repository.
2. Read the project constitution before proposing a change.
3. Create a feature branch and run the sequence above via spec-kit's `/speckit-*`
   commands, in order, rather than editing code directly.
4. Submit a pull request once `/speckit-converge` reports a clean result with no new
   tasks appended (if it appended tasks, repeat `/speckit-implement` -> review ->
   `/speckit-converge` until it does); skipping a step requires a documented
   justification in the feature's spec directory, per the constitution.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for
details.

## Related projects

- [Mostro](https://github.com/MostroP2P/mostro) — the Mostro daemon implementation.
- [Nostr SDK](https://github.com/rust-nostr/nostr) — the Rust Nostr SDK.

## Support

For issues, questions, or contributions, please open an issue on GitHub.

## Disclaimer

This tool provides statistical analysis based on public Nostr events. Its metrics are
indicators, not a guarantee, and should not be the sole factor in deciding whether to
trade with a Mostro node. Always practice safe trading habits and start with small
amounts when using a node for the first time.
