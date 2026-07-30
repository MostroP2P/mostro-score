use clap::Parser;
use mostro_score::cli::args::Args;
use mostro_score::cli::options::{
    apply_color_format_override, resolve_context_default, resolve_format,
    resolve_format_before_explicit, resolve_relays, resolve_run_options, validate_relay_urls,
    TimeRangeInputs,
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
    // `--output`'s presence is irrelevant to `--init-config`'s own scaffold-only error
    // format below (the two flags have no interaction), so that path keeps using the
    // plain terminal-based default; the report-generating path's
    // `error_render_format`/`resolve_run_options` below consult `output_present`
    // through `resolve_context_default` instead.
    let context_default = select_format_for_context(stdout_is_terminal);
    let output_present = args.output.is_some();
    let explicit_format = args.format.map(Format::from);

    // Resolves the config file's path once, up front — needed both by `--init-config`
    // and by the ordinary report-generating path below.
    let config_path = config::paths_defaults::default_config_path(args.config_dir.as_deref());

    // `--init-config` short-circuits entirely, before the existing configuration file
    // is ever loaded -- loading it here would parse/validate a file `--init-config` is
    // not supposed to consult at all, surfacing a stray warning for a malformed file it
    // is about to overwrite anyway. Its own error format uses only the explicit
    // `--format` flag and automatic resolution, never a config-sourced value.
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

    // Loads the configuration file, warning to stderr and falling back fully to the
    // flag/environment-variable/compiled-default chain on any problem (absence is
    // silent). Loaded before `error_render_format` below so a config-sourced `format`
    // value governs every fatal error from this point on, not just a successfully
    // resolved report (a config-sourced `format` gets the same precedence an explicit
    // `--format` flag would have). Only reached once the `--init-config` short-circuit
    // above has already returned.
    let mut config_load_err = std::io::stderr();
    let config_file = config::file::load_config_file(&config_path, &mut config_load_err);

    // Format is resolved once, up front, so every fatal error below — including one
    // raised while resolving the options themselves — already knows which format to
    // render through. `--color`'s only format-selection effect (upgrading an automatic
    // plain default to console) never depends on whether `--color`/`--no-color` are
    // contradictory, so it is always safe to compute here, ahead of that validation.
    // `--output`'s presence skips the terminal-detection automatic default, resolving
    // straight to plain instead -- the same rule `resolve_run_options` applies below,
    // kept consistent via the shared `resolve_context_default` helper rather than each
    // duplicating its own branch.
    let context_default_for_report = resolve_context_default(stdout_is_terminal, output_present);
    let config_format = config_file.as_ref().and_then(|config| config.format);
    let format_before_explicit = resolve_format_before_explicit(
        config_format,
        output_present,
        context_default_for_report,
        args.color,
    );
    let error_render_format = resolve_format(explicit_format, format_before_explicit);

    // `--force` without `--init-config` has no meaning on its own and is rejected as a
    // usage error, validated after format resolution so it renders through the
    // resolved `--format` (e.g. `--format json --force` still produces a JSON fatal
    // envelope).
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
        output_present,
    ) {
        Ok(options) => options,
        Err(usage_error) => exit_with_error(usage_error, error_render_format),
    };

    // `--pubkey` is required for every report-generating invocation (the
    // `--init-config` short-circuit above already returned before this point), but not
    // for `--init-config` itself — so this check, not clap's native required-argument
    // handling, enforces it, still as a usage error (exit code `2`). Precedence,
    // amended 2026-07-26: explicit flag/environment variable (already collapsed by
    // clap), then the config file's `pubkey` value (already validated as well-formed
    // at config-load time), then this usage error.
    let pubkey_raw = match args
        .pubkey
        .as_deref()
        .or_else(|| config_file.as_ref().and_then(|c| c.pubkey.as_deref()))
    {
        Some(pubkey) => pubkey,
        None => exit_with_error(
            AppError::UsageError("--pubkey (or MOSTRO_SCORE_PUBKEY) is required".to_string()),
            error_render_format,
        ),
    };

    // An unparseable pubkey exits `5` via AppError::InvalidPubkey, matching the JSON
    // fatal-error envelope's `invalid_pubkey` code.
    let public_key = match PublicKey::parse(pubkey_raw) {
        Ok(pk) => pk,
        Err(_) => exit_with_error(AppError::InvalidPubkey, options.format),
    };

    // Resolves the full `--relays` precedence chain — explicit flag/environment
    // variable (already collapsed by clap), then the config file's `relays` value,
    // then the compiled-in default.
    let relays = resolve_relays(
        args.relays.as_deref(),
        config_file.as_ref().and_then(|c| c.relays.as_deref()),
    );
    // A malformed relay is rejected here, with an actionable message naming it, before
    // a `RelayEventSource` is ever constructed — replacing the previous behavior of
    // letting a malformed URL reach `Client::add_relay` and fold into a generic
    // `AppError::RelaysUnreachable`.
    if let Err(usage_error) = validate_relay_urls(&relays) {
        exit_with_error(usage_error, options.format);
    }

    // Binds the concrete terminal `ProgressReporter` at construction, here in the
    // wiring root, so `fetch::client` itself never depends on `report`.
    // `options.quiet` is already resolved by this point, so the progress indicator is
    // suppressed from construction rather than filtered later.
    let event_source = RelayEventSource::with_progress_reporter(
        relays,
        TerminalProgressReporter::new(options.quiet),
    );
    let now = chrono::Utc::now;

    let mut stderr = std::io::stderr();

    // `--output` writes the report to the given file instead of standard output --
    // opened/created/truncated before any relay connection, so a bad path or
    // permissions error is caught here, before any relay is queried, and surfaces as an
    // `AppError::Other` (exit `1`) via `AppError`'s existing `#[from] std::io::Error`
    // conversion, rendered through the same pre-relay `exit_with_error` pattern every
    // other pre-relay failure already uses.
    let mut stdout;
    let mut output_file;
    let mut out: &mut dyn std::io::Write = match args.output.as_ref() {
        Some(path) => match std::fs::File::create(path) {
            Ok(file) => {
                output_file = file;
                &mut output_file
            }
            Err(io_error) => exit_with_error(AppError::from(io_error), options.format),
        },
        None => {
            stdout = std::io::stdout();
            &mut stdout
        }
    };

    if let Err(err) = mostro_score::run(
        public_key,
        event_source,
        &now,
        &mut out,
        &mut stderr,
        &options,
    )
    .await
    {
        // `File::create` above already truncated `--output`'s destination; if the run
        // fails afterward, that leaves a stray empty file with no confirmation
        // message, which could be mistaken for real (empty) output. Removing it here
        // keeps failure and file-creation atomic from the user's point of view --
        // best-effort, since the failure being reported already takes priority over a
        // cleanup error.
        if let Some(path) = args.output.as_ref() {
            let _ = std::fs::remove_file(path);
        }
        exit_with_error(err, options.format);
    }

    // A diagnostic fact naming the exact written path, never suppressed by `--quiet`
    // -- matching the existing precedent set by the relay-failure warnings and the
    // no-dev-fee-anchor warning in `lib.rs::run`, neither of which `--quiet`
    // suppresses either.
    if let Some(path) = args.output.as_ref() {
        eprintln!("Report written to {}", path.display());
    }
}

/// Maps `run()`'s (or pubkey parsing's, or CLI validation's) error to its exit code and
/// renders it through the single format-aware fatal-error rendering point. Swallows a
/// write failure from `render_fatal_error` rather than propagating it — the mapped
/// exit code must still apply even if the message can't be printed (e.g. a closed
/// stderr/stdout).
fn exit_with_error(err: AppError, format: Format) -> ! {
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    let _ = render_fatal_error(&mut stdout, &mut stderr, &err, format);
    std::process::exit(exit_code_for(&err));
}
