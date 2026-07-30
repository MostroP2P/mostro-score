//! Format selection: `Format` stands in for "which renderer to use", and
//! `select_format_for_context` is a pure function deciding the context-based default —
//! console when stdout is an interactive terminal, plain-text when it is redirected or
//! piped. JSON is never auto-selected: it is a machine-readable format a caller opts
//! into explicitly, not a sensible default for either execution context.

pub mod console;
pub mod json;
pub mod plain;

use crate::report::content::SectionFilter;
use crate::stats::grid::Granularity;
use std::io::IsTerminal;

/// Which renderer a caller should use for a report. `Console`/`Plain` are the two
/// context-selectable defaults; `Json` is always an explicit choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Console,
    Plain,
    Json,
}

/// The context-based default, resolved from an already-gathered tty flag so the
/// decision itself is unit-testable without depending on the real process environment —
/// the same pattern `report::format::resolve_color_enabled` already uses for the color
/// policy.
pub fn select_format_for_context(stdout_is_terminal: bool) -> Format {
    if stdout_is_terminal {
        Format::Console
    } else {
        Format::Plain
    }
}

/// Gathers the real stdout tty state and applies `select_format_for_context`. Uses
/// `std::io::IsTerminal`, the same tty-detection primitive `report::format::
/// color_enabled_for_stdout` already relies on through `anstream`'s own terminal
/// detection — not reinvented here.
pub fn default_format_for_stdout() -> Format {
    select_format_for_context(std::io::stdout().is_terminal())
}

/// The resolved, per-invocation options `run()` needs to render its output — bundled
/// into one struct so `run()`'s already-`#[allow(clippy::too_many_arguments)]`
/// signature does not grow further with three separate parameters. `cli::options` builds
/// this from the parsed CLI flags, environment, and automatic-detection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunOptions {
    pub format: Format,
    /// Suppresses progress indicators and transient status narration.
    pub quiet: bool,
    /// `--color`/`--no-color`'s resolved override: `None` defers to the automatic
    /// tty/`NO_COLOR`/`TERM=dumb` policy. Only `Format::Console` ever consults this;
    /// `Format::Plain`/`Format::Json` never render color, so this field is harmless,
    /// not meaningful, for those two formats.
    pub color_override: Option<bool>,
    /// The activity grid's resolved `--since` bound, in epoch seconds. `None` means
    /// either both `--since`/`--until` were omitted (full history), or `--until` alone
    /// was given and `run()` still needs to resolve this to the node's earliest
    /// available history once it has fetched data — `cli::options` cannot know that
    /// value itself.
    pub since: Option<i64>,
    /// The activity grid's resolved `--until` bound, in epoch seconds. `None` means
    /// either both `--since`/`--until` were omitted (full history), or `--since` alone
    /// was given and `run()` still needs to resolve this to its own
    /// `report_generated_at` (captured after connecting/fetching) rather than an
    /// earlier `now` read before any relay is even queried.
    pub until: Option<i64>,
    /// An explicit `--view` flag's resolved granularity, forcing the activity grid's
    /// bucket size instead of automatic selection.
    pub view: Option<Granularity>,
    /// Which console/plain-text sections render. `Format::Json` never consults this —
    /// it always emits the complete structure.
    pub sections: SectionFilter,
}

/// The single format-aware fatal-error rendering point, used both by `main.rs` for a
/// pre-`run()` fatal error (`AppError::InvalidPubkey`, a malformed `--relays` entry)
/// and for whatever `run()` itself propagates. `Format::Json` writes
/// `json::error_envelope`'s single-line document to `out` — the same stream a successful
/// JSON report already uses (`json::render`), since a fatal error replaces the report on
/// that stream rather than living on a separate one. `Format::Console`/`Format::Plain`
/// write the existing plain `"Error: {error}"` line to `err` (an injected writer, not a
/// hardcoded `std::io::stderr()` handle, matching `run()`'s own `out`/`err` injection
/// convention elsewhere in this codebase — this is what makes the message itself, not
/// just its absence from `out`, directly assertable in a test).
pub fn render_fatal_error(
    out: &mut impl std::io::Write,
    err: &mut impl std::io::Write,
    error: &crate::error::AppError,
    format: Format,
) -> std::io::Result<()> {
    match format {
        Format::Json => {
            let envelope = json::error_envelope(error);
            writeln!(out, "{envelope}")
        }
        Format::Console | Format::Plain => writeln!(err, "Error: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AppError;

    #[test]
    fn select_format_for_context_uses_console_when_stdout_is_a_terminal() {
        assert_eq!(select_format_for_context(true), Format::Console);
    }

    /// Redirected/piped output defaults to plain-text, never JSON — JSON is always an
    /// explicit opt-in.
    #[test]
    fn select_format_for_context_uses_plain_text_when_stdout_is_piped() {
        assert_eq!(select_format_for_context(false), Format::Plain);
    }

    /// A JSON-format fatal error writes the envelope to `out`, the same stream a
    /// successful JSON report would use, and nothing to `err`.
    #[test]
    fn render_fatal_error_writes_the_json_envelope_to_out_for_json_format() {
        let error = AppError::InvalidPubkey;
        let mut out: Vec<u8> = Vec::new();
        let mut err: Vec<u8> = Vec::new();

        render_fatal_error(&mut out, &mut err, &error, Format::Json).expect("renders");

        let rendered = String::from_utf8(out).expect("valid utf8");
        assert!(rendered.contains("\"code\":\"invalid_pubkey\""));
        assert!(rendered.contains("\"schema_version\""));
        assert!(err.is_empty());
    }

    /// Console/plain fatal-error rendering is unchanged from before this function
    /// existed: a plain `"Error: {error}"` line to `err`, never to `out` — now directly
    /// assertable since `err` is an injected writer, not the real process stderr.
    #[test]
    fn render_fatal_error_writes_the_plain_message_to_err_for_console_or_plain_format() {
        let error = AppError::InvalidPubkey;
        for format in [Format::Console, Format::Plain] {
            let mut out: Vec<u8> = Vec::new();
            let mut err: Vec<u8> = Vec::new();

            render_fatal_error(&mut out, &mut err, &error, format).expect("renders");

            assert!(
                out.is_empty(),
                "console/plain fatal errors must go to `err`, never to `out`"
            );
            let rendered = String::from_utf8(err).expect("valid utf8");
            assert_eq!(rendered, "Error: Invalid public key format.\n");
        }
    }
}
