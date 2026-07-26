use clap::Parser;
use mostro_score::cli::args::Args;
use mostro_score::cli::options::{
    apply_color_format_override, resolve_format, resolve_run_options, validate_relay_urls,
    TimeRangeInputs,
};
use mostro_score::error::exit_code::exit_code_for;
use mostro_score::error::AppError;
use mostro_score::fetch::client::RelayEventSource;
use mostro_score::report::progress::TerminalProgressReporter;
use mostro_score::report::render::{render_fatal_error, select_format_for_context, Format};
use nostr_sdk::prelude::*;
use std::io::IsTerminal;

#[tokio::main]
async fn main() {
    env_logger::init();
    let args = Args::parse();

    // PR 9 (003 FR-010/FR-011): format is resolved once, up front, so every fatal error
    // below — including one raised while resolving the options themselves — already
    // knows which format to render through. `--color`'s only format-selection effect
    // (upgrading an automatic plain default to console) never depends on whether
    // `--color`/`--no-color` are contradictory, so it is always safe to compute here,
    // ahead of that validation.
    let stdout_is_terminal = std::io::stdout().is_terminal();
    let context_default = select_format_for_context(stdout_is_terminal);
    let explicit_format = args.format.map(Format::from);
    let error_render_format = resolve_format(
        explicit_format,
        apply_color_format_override(context_default, args.color),
    );

    // `resolve_run_options` calls `validate_color_flags` as its own first step, so this
    // is the single place that contradiction (`--color` and `--no-color` together) is
    // checked, rather than duplicating the check here as well.
    let time_range_now = chrono::Utc::now();
    let time_range_inputs = TimeRangeInputs {
        since_raw: args.since.clone(),
        until_raw: args.until.clone(),
        view: args.view.map(Into::into),
    };
    let options = match resolve_run_options(
        explicit_format,
        args.color,
        args.no_color,
        args.quiet,
        stdout_is_terminal,
        time_range_inputs,
        time_range_now,
    ) {
        Ok(options) => options,
        Err(usage_error) => exit_with_error(usage_error, error_render_format),
    };

    // 1. Parse Pubkey (T064/T065: exits `5` via AppError::InvalidPubkey, matching
    // 002 FR-019 and the JSON fatal-error envelope's `invalid_pubkey` code, instead of
    // PR 1's preserved-verbatim deviation of printing and returning success).
    let public_key = match PublicKey::parse(&args.pubkey) {
        Ok(pk) => pk,
        Err(_) => exit_with_error(AppError::InvalidPubkey, options.format),
    };

    let relays: Vec<String> = args.relays.split(',').map(|s| s.to_string()).collect();
    // T170/T171 (003 FR-002/FR-003, FR-013a): a malformed relay is rejected here, with an
    // actionable message naming it, before a `RelayEventSource` is ever constructed —
    // replacing the previous behavior of letting a malformed URL reach `Client::add_relay`
    // and fold into a generic `AppError::RelaysUnreachable`.
    if let Err(usage_error) = validate_relay_urls(&relays) {
        exit_with_error(usage_error, options.format);
    }

    // T138 (002 FR-014): binds the concrete terminal `ProgressReporter` at construction,
    // here in the wiring root, so `fetch::client` itself never depends on `report`.
    // PR 9 (003 FR-012): `options.quiet` is already resolved by this point, so the
    // progress indicator is suppressed from construction rather than filtered later.
    let event_source = RelayEventSource::with_progress_reporter(
        relays,
        TerminalProgressReporter::new(options.quiet),
    );
    let now = chrono::Utc::now;

    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();

    if let Err(err) = mostro_score::run(
        public_key,
        event_source,
        &now,
        &mut stdout,
        &mut stderr,
        &options,
    )
    .await
    {
        exit_with_error(err, options.format);
    }
}

/// Maps `run()`'s (or pubkey parsing's, or CLI validation's) error to its exit code
/// (T062/T063) and renders it through the single format-aware fatal-error rendering
/// point (002 FR-011, PR 9). Swallows a write failure from `render_fatal_error` rather
/// than propagating it — the mapped exit code must still apply even if the message can't
/// be printed (e.g. a closed stderr/stdout).
fn exit_with_error(err: AppError, format: Format) -> ! {
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    let _ = render_fatal_error(&mut stdout, &mut stderr, &err, format);
    std::process::exit(exit_code_for(&err));
}

// PR 1 Step D (T042): the golden-baseline test that lived here as a same-crate unit test
// moved to `tests/metrics_end_to_end.rs` as a real integration test against
// `mostro_score::run()`, now that `run()` is part of the library's public surface. The
// invalid-`--pubkey` branch above stays pinned at binary level by Step -1's scenario 2
// capture (T002), asserted by `tests/error_handling.rs`.
