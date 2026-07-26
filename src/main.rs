use clap::Parser;
use mostro_score::cli::args::Args;
use mostro_score::cli::options::{
    apply_color_format_override, resolve_format, resolve_relays, resolve_run_options,
    validate_relay_urls, TimeRangeInputs,
};
use mostro_score::config;
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

    let stdout_is_terminal = std::io::stdout().is_terminal();
    let context_default = select_format_for_context(stdout_is_terminal);
    let explicit_format = args.format.map(Format::from);

    // PR 12 (003 FR-014): resolves the config file's path once, up front — needed both
    // by `--init-config` (FR-017) and by the ordinary report-generating path below
    // (FR-016).
    let config_path = config::paths_defaults::default_config_path(args.config_dir.as_deref());

    // PR 12 (003 FR-019): `--init-config` short-circuits entirely, before the existing
    // configuration file is ever loaded -- loading it here would parse/validate a file
    // `--init-config` is not supposed to consult at all, surfacing a stray warning for a
    // malformed file it is about to overwrite anyway. Its own error format uses only the
    // explicit `--format` flag and automatic resolution, never a config-sourced value.
    if args.init_config {
        let init_error_format = resolve_format(
            explicit_format,
            apply_color_format_override(context_default, args.color),
        );
        match config::init::scaffold_config_file(&config_path, args.force) {
            Ok(()) => {
                println!("Wrote configuration file to {}", config_path.display());
                std::process::exit(0);
            }
            Err(usage_error) => exit_with_error(usage_error, init_error_format),
        }
    }

    // PR 12 (003 FR-015/FR-015a): loads the configuration file, warning to stderr and
    // falling back fully to the flag/environment-variable/compiled-default chain on any
    // problem (absence is silent, per FR-015). Loaded before `error_render_format` below
    // so a config-sourced `format` value governs every fatal error from this point on,
    // not just a successfully resolved report (FR-011 gives a config-sourced `format`
    // the same precedence an explicit `--format` flag would have). Only reached once the
    // `--init-config` short-circuit above has already returned.
    let mut config_load_err = std::io::stderr();
    let config_file = config::file::load_config_file(&config_path, &mut config_load_err);

    // PR 9 (003 FR-010/FR-011): format is resolved once, up front, so every fatal error
    // below — including one raised while resolving the options themselves — already
    // knows which format to render through. `--color`'s only format-selection effect
    // (upgrading an automatic plain default to console) never depends on whether
    // `--color`/`--no-color` are contradictory, so it is always safe to compute here,
    // ahead of that validation.
    let format_before_explicit = match config_file.as_ref().and_then(|config| config.format) {
        Some(config_format) => config_format,
        None => apply_color_format_override(context_default, args.color),
    };
    let error_render_format = resolve_format(explicit_format, format_before_explicit);

    // PR 12 (003 FR-018): `--force` without `--init-config` has no meaning on its own
    // and is rejected as a usage error, validated after format resolution so it renders
    // through the resolved `--format` (e.g. `--format json --force` still produces a
    // JSON fatal envelope).
    if args.force {
        exit_with_error(
            AppError::UsageError("--force requires --init-config".to_string()),
            error_render_format,
        );
    }

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
        args.sections.clone(),
        config_file.as_ref(),
    ) {
        Ok(options) => options,
        Err(usage_error) => exit_with_error(usage_error, error_render_format),
    };

    // 003 FR-001/FR-017: `--pubkey` is required for every report-generating invocation
    // (the `--init-config` short-circuit above already returned before this point), but
    // not for `--init-config` itself — so this check, not clap's native
    // required-argument handling, enforces it, still as a usage error (exit code `2`,
    // 003 FR-013a).
    let pubkey_raw = match args.pubkey.as_deref() {
        Some(pubkey) => pubkey,
        None => exit_with_error(
            AppError::UsageError("--pubkey (or MOSTRO_SCORE_PUBKEY) is required".to_string()),
            error_render_format,
        ),
    };

    // 1. Parse Pubkey (T064/T065: exits `5` via AppError::InvalidPubkey, matching
    // 002 FR-019 and the JSON fatal-error envelope's `invalid_pubkey` code, instead of
    // PR 1's preserved-verbatim deviation of printing and returning success).
    let public_key = match PublicKey::parse(pubkey_raw) {
        Ok(pk) => pk,
        Err(_) => exit_with_error(AppError::InvalidPubkey, options.format),
    };

    // PR 12 (003 FR-002/FR-016): resolves the full `--relays` precedence chain —
    // explicit flag/environment variable (already collapsed by clap), then the config
    // file's `relays` value, then the compiled-in default.
    let relays = resolve_relays(
        args.relays.as_deref(),
        config_file.as_ref().and_then(|c| c.relays.as_deref()),
    );
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
