//! PR 8's explicit-format-override skeleton (002 FR-010) extended in PR 9 (003
//! FR-001..FR-013a) into the full CLI resolution/validation surface: relay well-
//! formedness, `--format`/`--color`/`--no-color` resolution, and their mutual-exclusion
//! validation. Every function here is pure — no `clap` parsing, no process environment —
//! so `main.rs` stays the only place that touches `Args`, `std::env`, or `std::io`
//! directly, and every resolution/validation rule stays unit-testable without a
//! subprocess.

use crate::error::AppError;
use crate::report::render::Format;
use nostr_sdk::prelude::*;

/// 002 FR-010: an explicit format choice always overrides the context-based default,
/// exactly like `resolve_color_enabled` does for the color override.
pub fn resolve_format(explicit: Option<Format>, context_default: Format) -> Format {
    explicit.unwrap_or(context_default)
}

/// 003 FR-011: `--color` implies console format only when format resolution reaches the
/// fully automatic step — no explicit `--format` flag and no configuration-file value
/// (there is no config file yet; PR 12's job, so today "no config value" is always
/// true) — overriding an automatic *plain* default to *console* so piping into a
/// color-aware pager works as intended. Has no effect when the automatic default is
/// already `console`, and callers must not apply this when `--format` was explicit:
/// `resolve_format` already gives an explicit choice full precedence regardless of what
/// this function returns.
pub fn apply_color_format_override(context_default: Format, color_flag: bool) -> Format {
    if color_flag && context_default == Format::Plain {
        Format::Console
    } else {
        context_default
    }
}

/// 003 FR-011: `--no-color` forces color off, `--color` forces it on, and neither
/// defers to the automatic tty/`NO_COLOR`/`TERM=dumb` policy (`report::format::
/// color_enabled_for_stdout`). Only `Format::Console` ever consults this; it is
/// harmless, but not meaningful, for `Format::Plain`/`Format::Json`.
pub fn resolve_color_override(color: bool, no_color: bool) -> Option<bool> {
    if no_color {
        Some(false)
    } else if color {
        Some(true)
    } else {
        None
    }
}

/// 003 FR-011 Edge Case: `--color` and `--no-color` together is a contradictory
/// combination, rejected as a usage error (exit code `2`, 003 FR-013a) rather than
/// silently letting one win.
pub fn validate_color_flags(color: bool, no_color: bool) -> Result<(), AppError> {
    if color && no_color {
        return Err(AppError::UsageError(
            "--color and --no-color are mutually exclusive".to_string(),
        ));
    }
    Ok(())
}

/// 003 FR-002/FR-003 Edge Case, FR-013a: rejects a malformed `--relays`/
/// `MOSTRO_SCORE_RELAYS` entry with an actionable message naming the exact malformed
/// string, before any connection attempt. Reuses `nostr_sdk::RelayUrl::parse` — the same
/// well-formedness check `fetch::client::RelayEventSource::connect` already performs
/// internally via `Client::add_relay` — so the two layers can never disagree on what
/// counts as well-formed; this function only runs earlier, before a `RelayEventSource`
/// is ever constructed.
pub fn validate_relay_urls(relays: &[String]) -> Result<(), AppError> {
    for relay in relays {
        if let Err(error) = RelayUrl::parse(relay) {
            return Err(AppError::UsageError(format!(
                "Invalid relay URL '{relay}': {error}"
            )));
        }
    }
    Ok(())
}

/// 003 FR-010..FR-013a: the top-level resolution/validation entry point `main.rs` calls
/// once flags are parsed — composes every pure function above into the final
/// `RunOptions` a report-generating invocation needs. Fails only on the one validation
/// this composition can raise (`--color`/`--no-color` together); relay well-formedness
/// (`validate_relay_urls`) is a separate, independent check `main.rs` runs on the parsed
/// `--relays` list.
pub fn resolve_run_options(
    explicit_format: Option<Format>,
    color: bool,
    no_color: bool,
    quiet: bool,
    stdout_is_terminal: bool,
) -> Result<crate::report::render::RunOptions, AppError> {
    validate_color_flags(color, no_color)?;

    let context_default = crate::report::render::select_format_for_context(stdout_is_terminal);
    let format = resolve_format(
        explicit_format,
        apply_color_format_override(context_default, color),
    );

    Ok(crate::report::render::RunOptions {
        format,
        quiet,
        color_override: resolve_color_override(color, no_color),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::render::RunOptions;

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

    /// 003 FR-011: `--color` upgrades an automatic *plain* default to *console*.
    #[test]
    fn apply_color_format_override_upgrades_an_automatic_plain_default_to_console() {
        assert_eq!(
            apply_color_format_override(Format::Plain, true),
            Format::Console
        );
    }

    /// 003 FR-011: `--color` has no effect when the automatic default is already
    /// console, or when `--color` was not passed at all.
    #[test]
    fn apply_color_format_override_is_a_no_op_outside_the_plain_plus_color_case() {
        assert_eq!(
            apply_color_format_override(Format::Console, true),
            Format::Console
        );
        assert_eq!(
            apply_color_format_override(Format::Plain, false),
            Format::Plain
        );
    }

    #[test]
    fn resolve_color_override_defers_to_automatic_policy_with_neither_flag() {
        assert_eq!(resolve_color_override(false, false), None);
    }

    #[test]
    fn resolve_color_override_forces_off_with_no_color() {
        assert_eq!(resolve_color_override(false, true), Some(false));
    }

    #[test]
    fn resolve_color_override_forces_on_with_color() {
        assert_eq!(resolve_color_override(true, false), Some(true));
    }

    #[test]
    fn validate_color_flags_rejects_both_flags_together() {
        let error = validate_color_flags(true, true).expect_err("both flags is contradictory");
        assert!(matches!(error, AppError::UsageError(_)));
    }

    #[test]
    fn validate_color_flags_accepts_every_other_combination() {
        assert!(validate_color_flags(false, false).is_ok());
        assert!(validate_color_flags(true, false).is_ok());
        assert!(validate_color_flags(false, true).is_ok());
    }

    #[test]
    fn validate_relay_urls_accepts_every_well_formed_relay() {
        let relays = vec![
            "wss://relay.mostro.network".to_string(),
            "ws://localhost:7000".to_string(),
        ];
        assert!(validate_relay_urls(&relays).is_ok());
    }

    /// 003's `--relays` Edge Case: a malformed entry is rejected naming the exact
    /// malformed string, not a generic message.
    #[test]
    fn validate_relay_urls_rejects_a_malformed_relay_naming_it_in_the_message() {
        let relays = vec!["wss://good.example".to_string(), "not-a-url".to_string()];

        let error = validate_relay_urls(&relays).expect_err("a malformed relay is rejected");

        assert!(matches!(error, AppError::UsageError(_)));
        assert!(error.to_string().contains("not-a-url"));
    }

    /// Consistency with `fetch::client::RelayEventSource::connect()`'s own validation:
    /// a URL with an unsupported scheme (neither `ws://` nor `wss://`) is malformed too.
    #[test]
    fn validate_relay_urls_rejects_an_unsupported_scheme() {
        let relays = vec!["https://relay.example".to_string()];

        let error = validate_relay_urls(&relays).expect_err("an unsupported scheme is rejected");

        assert!(error.to_string().contains("https://relay.example"));
    }

    #[test]
    fn resolve_run_options_rejects_contradictory_color_flags() {
        let result = resolve_run_options(None, true, true, false, true);
        assert!(matches!(result, Err(AppError::UsageError(_))));
    }

    /// 003 FR-010/FR-011: with no explicit `--format`, `--color` upgrades the automatic
    /// plain (non-terminal) default to console, and forces the color override on.
    #[test]
    fn resolve_run_options_applies_the_color_format_upgrade_and_override() {
        let options = resolve_run_options(None, true, false, false, false)
            .expect("color alone is not contradictory");

        assert_eq!(
            options,
            RunOptions {
                format: Format::Console,
                quiet: false,
                color_override: Some(true),
            }
        );
    }

    /// 003 FR-011: `--color` has no effect on an explicit `--format plain`/`--format
    /// json` choice — the format resolution ignores `--color` entirely once an explicit
    /// format is present.
    #[test]
    fn resolve_run_options_explicit_format_ignores_the_color_flag() {
        let options = resolve_run_options(Some(Format::Json), true, false, false, false)
            .expect("color alone is not contradictory");

        assert_eq!(options.format, Format::Json);
    }

    #[test]
    fn resolve_run_options_threads_quiet_through_unchanged() {
        let options =
            resolve_run_options(None, false, false, true, true).expect("no validation failure");
        assert!(options.quiet);
    }
}
