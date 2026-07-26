//! Format selection (002 FR-010): `Format` stands in for "which renderer to use", and
//! `select_format_for_context` is a pure function deciding the context-based default —
//! console when stdout is an interactive terminal, plain-text when it is redirected or
//! piped. JSON is never auto-selected: 002 FR-009 lists it as a machine-readable format a
//! caller opts into explicitly, not a sensible default for either execution context. This
//! PR only builds and unit-tests the selection function in isolation, the same way PR 7d
//! built `report::format::resolve_color_enabled` as a pure function before any `--color`
//! flag existed; wiring an actual `--format` CLI flag is PR 9's job (`cli::options` holds
//! PR 8's own explicit-override skeleton over this function's result).

pub mod console;
pub mod json;
pub mod plain;

use std::io::IsTerminal;

/// Which renderer a caller should use for a report. `Console`/`Plain` are the two
/// context-selectable defaults (002 FR-010); `Json` is always an explicit choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Console,
    Plain,
    Json,
}

/// 002 FR-010's context-based default, resolved from an already-gathered tty flag so the
/// decision itself is unit-testable without depending on the real process environment —
/// the same pattern `report::format::resolve_color_enabled` already uses for FR-015's
/// color policy.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 002 FR-010: an interactive terminal defaults to the console format.
    #[test]
    fn select_format_for_context_uses_console_when_stdout_is_a_terminal() {
        assert_eq!(select_format_for_context(true), Format::Console);
    }

    /// 002 FR-010: redirected/piped output defaults to plain-text, never JSON — JSON is
    /// always an explicit opt-in (002 FR-009).
    #[test]
    fn select_format_for_context_uses_plain_text_when_stdout_is_piped() {
        assert_eq!(select_format_for_context(false), Format::Plain);
    }
}
