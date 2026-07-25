use clap::Parser;
use mostro_score::error::exit_code::exit_code_for;
use mostro_score::error::AppError;
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
async fn main() {
    env_logger::init();
    let args = Args::parse();

    // 1. Parse Pubkey (T064/T065: exits `5` via AppError::InvalidPubkey, matching
    // 002 FR-019 and the JSON fatal-error envelope's `invalid_pubkey` code, instead of
    // PR 1's preserved-verbatim deviation of printing and returning success).
    let public_key = match PublicKey::parse(&args.pubkey) {
        Ok(pk) => pk,
        Err(_) => exit_with_error(AppError::InvalidPubkey.into()),
    };

    let relays: Vec<String> = args.relays.split(',').map(|s| s.to_string()).collect();
    let event_source = RelayEventSource::new(relays);
    let now = chrono::Utc::now;

    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();

    if let Err(err) =
        mostro_score::run(public_key, event_source, &now, &mut stdout, &mut stderr).await
    {
        exit_with_error(err);
    }
}

/// Maps any error `run()` (or pubkey parsing) produces to its exit code (T062/T063) and
/// prints a user-facing message, never a raw `Debug` dump (Principle VI) — Rust's default
/// `Result`-returning-`main` behavior does neither, so `main` handles this explicitly.
fn exit_with_error(err: Box<dyn std::error::Error>) -> ! {
    let code = match err.downcast::<AppError>() {
        Ok(app_err) => {
            eprintln!("Error: {app_err}");
            exit_code_for(&app_err)
        }
        Err(err) => {
            // Debug, not Display: matches the pristine binary's exact fixture text for
            // this unclassified path (Rust's default `Result`-returning-`main` prints
            // `Error: {:?}`). Not one of PR 2's named deviations to fix — preserved
            // verbatim, per the "move code, don't improve it" mandate.
            eprintln!("Error: {err:?}");
            1
        }
    };
    std::process::exit(code);
}

// PR 1 Step D (T042): the golden-baseline test that lived here as a same-crate unit test
// moved to `tests/metrics_end_to_end.rs` as a real integration test against
// `mostro_score::run()`, now that `run()` is part of the library's public surface. The
// invalid-`--pubkey` branch above stays pinned at binary level by Step -1's scenario 2
// capture (T002), asserted by `tests/cli_behavior.rs` (Step D2, T044-T045).
