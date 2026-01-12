# Mostro Score

A CLI tool to analyze and calculate reputation statistics for Mostro P2P nodes by analyzing public Nostr events.

## Overview

Mostro Stats provides transparency and trust metrics for Mostro nodes operating on the Nostr network. By analyzing historical trading data published as Nostr events, this tool calculates objective reputation scores based on trading volume, operational longevity, and successful order completion rates.

## Features

- **Trust Score Calculation**: Generates a 0-100 trust score based on multiple factors
- **Volume Analysis**: Tracks total trading volume in sats and BTC
- **Longevity Metrics**: Calculates how long a Mostro node has been operational
- **Order Statistics**: Counts successful trades and calculates average order sizes
- **Event Deduplication**: Properly handles order state updates to count unique orders
- **Flexible Relay Support**: Connect to any Nostr relay or multiple relays
- **Debug Mode**: Detailed event analysis and status distribution tracking

## How It Works

The tool fetches Nostr events (kind 38383) published by Mostro nodes and analyzes:

1. **Mostro Instance Status Events** (z=info): Used to determine when the node started operating
2. **Order Events** (z=order): Tracks successful trades, volumes, and timestamps

### Trust Score Components

The trust score (0-100) is calculated from three weighted factors:

- **Age** (30 points max): Days active / 365 days
- **Volume** (40 points max): Total BTC volume / 1 BTC
- **Success Count** (30 points max): Successful orders / 100 orders

This scoring mechanism incentivizes long-term honest operation over short-term scams.

## Installation

### Prerequisites

- Rust 1.70+ and Cargo
- Internet connection to access Nostr relays

### Build from Source

```bash
git clone https://github.com/MostroP2P/mostro-score.git
cd mostro-score
cargo build --release
```

The binary will be available at `target/release/mostro-score`

### Install Globally

```bash
cargo install --path .
```

## Usage

### Basic Usage

Analyze a Mostro node by providing its public key (npub or hex format):

```bash
mostro-score --pubkey <MOSTRO_PUBKEY>
```

### Custom Relays

Connect to specific relays (comma-separated):

```bash
mostro-score --pubkey <MOSTRO_PUBKEY> --relays wss://relay.mostro.network,wss://relay.damus.io
```

### Command Line Options

```
Options:
  -p, --pubkey <PUBKEY>    Mostro Pubkey (npub or hex) to analyze [required]
  -r, --relays <RELAYS>    Relays to connect to (comma separated)
                           [default: wss://relay.mostro.network]
  -h, --help              Print help information
  -V, --version           Print version information
```

### Example

```bash
# Using npub format
mostro-score --pubkey npub1abc...xyz

# Using hex format
mostro-score --pubkey a1b2c3d4e5f6...

# With multiple relays
mostro-score -p npub1abc...xyz -r wss://relay.mostro.network,wss://relay.damus.io
```

## Example Output

```
Analyzing Mostro Node: npub1...
Hex: a1b2c3...
Connected to relays. Fetching history... (this might take a moment)
Fetched 1247 events. Analyzing...

========================================
       MOSTRO NODE REPUTATION REPORT
========================================
Node: npub1abc...xyz
----------------------------------------
First Seen:       2024-01-15 10:30:00 UTC
Last Seen:        2026-01-12 14:45:00 UTC
Days Active:      728.2 days
----------------------------------------
Successful Orders: 156
Total Volume:      15,420,000 sats (0.1542 BTC)
Avg Order Size:    98,846 sats
----------------------------------------
TRUST SCORE:       42/100
========================================
```

## Configuration

### Environment Variables

Set `RUST_LOG` for detailed logging:

```bash
export RUST_LOG=debug
mostro-score --pubkey <PUBKEY>
```

### .env File Support

Create a `.env` file in the project root:

```env
RUST_LOG=info
DEFAULT_RELAY=wss://relay.mostro.network
```

## Documentation

- [Reputation System Specification](specs/reputation_system_v1.md) - Detailed explanation of the reputation system design and formulas
- [Protocol Documentation](https://mostro.network/protocol/other_events.html#mostro-instance-status) - Mostro event structures and protocol details

## Roadmap

### Current (v0.1.0)
- Basic reputation scoring from order events
- Volume and success rate tracking
- Simple trust score calculation

### Planned Features
- Dispute tracking and penalties
- User rating system integration
- JSON output format for API integration
- Real-time monitoring mode
- Historical trend analysis
- Scam detection heuristics

## How This Prevents Scams

The reputation system makes exit scams economically unfeasible:

1. **Time Investment**: Building a high trust score requires months of legitimate operation
2. **Volume Requirements**: To attract large orders, nodes must first complete many smaller trades
3. **Sunk Cost**: The reputation becomes a valuable asset that takes effort to build
4. **Economic Incentive**: Long-term fee earnings from legitimate operation exceed one-time scam profits

For detailed economic analysis, see [specs/reputation_system_v1.md](specs/reputation_system_v1.md).

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Submit a pull request

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.

## Related Projects

- [Mostro](https://github.com/MostroP2P/mostro) - The Mostro daemon implementation
- [Nostr SDK](https://github.com/rust-nostr/nostr) - Rust Nostr SDK

## Support

For issues, questions, or contributions, please open an issue on GitHub.

## Disclaimer

This tool provides statistical analysis based on public Nostr events. Trust scores are indicators and should not be the sole factor in deciding whether to trade with a Mostro node. Always practice safe trading habits and start with small amounts when using new services.
