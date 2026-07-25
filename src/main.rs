use clap::Parser;
use mostro_score::fetch::client::RelayEventSource;
use nostr_sdk::prelude::*;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Mostro Pubkey (npub or hex) to analyze
    #[arg(short, long)]
    pubkey: String,

    /// Relays to connect to (comma separated)
    #[arg(short, long, default_value = "wss://relay.mostro.network")]
    relays: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();

    // 1. Parse Pubkey
    let public_key = match PublicKey::parse(&args.pubkey) {
        Ok(pk) => pk,
        Err(_) => {
            eprintln!("Error: Invalid public key format.");
            return Ok(());
        }
    };

    let relays: Vec<String> = args.relays.split(',').map(|s| s.to_string()).collect();
    let event_source = RelayEventSource::new(relays);
    let now = chrono::Utc::now;

    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();

    mostro_score::run(public_key, event_source, &now, &mut stdout, &mut stderr).await
}

// PR 1 Step D (T042): the golden-baseline test that lived here as a same-crate unit test
// moved to `tests/metrics_end_to_end.rs` as a real integration test against
// `mostro_score::run()`, now that `run()` is part of the library's public surface. The
// invalid-`--pubkey` branch above stays pinned at binary level by Step -1's scenario 2
// capture (T002), asserted by `tests/cli_behavior.rs` (Step D2, T044-T045).
