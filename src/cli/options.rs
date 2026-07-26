//! PR 8's explicit-format-override skeleton (002 FR-010): the same override-precedence
//! pattern PR 7d already established for color (`report::format::resolve_color_enabled`)
//! — a pure function taking an already-resolved optional override plus the context-based
//! default, with no `clap` argument parsing here. `main.rs`'s `Args` struct still owns
//! every actual CLI flag; wiring a real `--format` flag onto this function is PR 9's job.

use crate::report::render::Format;

/// 002 FR-010: an explicit format choice always overrides the context-based default,
/// exactly like `resolve_color_enabled` does for the color override.
pub fn resolve_format(explicit: Option<Format>, context_default: Format) -> Format {
    explicit.unwrap_or(context_default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_format_uses_the_context_default_with_no_explicit_override() {
        assert_eq!(resolve_format(None, Format::Console), Format::Console);
        assert_eq!(resolve_format(None, Format::Plain), Format::Plain);
    }

    /// 002 FR-010: an explicit format choice always overrides the context-based default,
    /// in every direction.
    #[test]
    fn resolve_format_honors_an_explicit_override_over_the_context_default() {
        assert_eq!(
            resolve_format(Some(Format::Json), Format::Console),
            Format::Json
        );
        assert_eq!(
            resolve_format(Some(Format::Plain), Format::Console),
            Format::Plain
        );
        assert_eq!(
            resolve_format(Some(Format::Console), Format::Plain),
            Format::Console
        );
    }
}
