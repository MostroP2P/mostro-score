//! The CLI resolution/validation surface: relay well-formedness, `--format`/`--color`/
//! `--no-color` resolution, and their mutual-exclusion validation. Every function here
//! is pure — no `clap` parsing, no process environment — so `main.rs` stays the CLI
//! orchestration boundary, and every rule stays unit-testable without a subprocess.

use crate::cli::duration::{parse_date_bound, since_bound_seconds, until_bound_seconds};
use crate::config::file::ConfigFile;
use crate::config::paths_defaults::DEFAULT_RELAY;
use crate::error::AppError;
use crate::report::content::SectionFilter;
use crate::report::render::Format;
use crate::stats::grid::Granularity;
use chrono::{DateTime, Datelike, Utc};
use nostr_sdk::prelude::*;

pub fn resolve_format(explicit: Option<Format>, context_default: Format) -> Format {
    explicit.unwrap_or(context_default)
}

/// `--output` skips terminal detection entirely (colored console output has no benefit
/// in a file), resolving straight to `Format::Plain`.
pub fn resolve_context_default(stdout_is_terminal: bool, output_present: bool) -> Format {
    if output_present {
        Format::Plain
    } else {
        crate::report::render::select_format_for_context(stdout_is_terminal)
    }
}

/// The format `resolve_format` falls back to when there's no explicit `--format`: a
/// configuration-sourced value wins first, then (with `--output` present) the `--color`
/// upgrade is skipped, then the ordinary `--color` upgrade.
pub fn resolve_format_before_explicit(
    config_format: Option<Format>,
    output_present: bool,
    context_default: Format,
    color: bool,
) -> Format {
    match config_format {
        Some(config_format) => config_format,
        None if output_present => context_default,
        None => apply_color_format_override(context_default, color),
    }
}

/// `--output` is only valid with `plain`/`json`; a resolved `console` format combined
/// with `--output` is rejected before any relay is queried.
pub fn validate_output_format(output_present: bool, format: Format) -> Result<(), AppError> {
    if output_present && format == Format::Console {
        return Err(AppError::UsageError(
            "--output is only valid with --format plain or --format json, not console".to_string(),
        ));
    }
    Ok(())
}

/// `--color` upgrades an automatic *plain* default to *console* so piping into a
/// color-aware pager works as intended. Only applies at the fully automatic step: no
/// explicit `--format` and no configuration-sourced value.
pub fn apply_color_format_override(context_default: Format, color_flag: bool) -> Format {
    if color_flag && context_default == Format::Plain {
        Format::Console
    } else {
        context_default
    }
}

/// `--no-color` forces off, `--color` forces on, neither defers to the automatic
/// policy. Only `Format::Console` ever consults this.
pub fn resolve_color_override(color: bool, no_color: bool) -> Option<bool> {
    if no_color {
        Some(false)
    } else if color {
        Some(true)
    } else {
        None
    }
}

pub fn validate_color_flags(color: bool, no_color: bool) -> Result<(), AppError> {
    if color && no_color {
        return Err(AppError::UsageError(
            "--color and --no-color are mutually exclusive".to_string(),
        ));
    }
    Ok(())
}

pub fn resolve_sections(raw: Option<&str>) -> Result<SectionFilter, AppError> {
    match raw {
        None => Ok(SectionFilter::all()),
        Some(raw) => SectionFilter::parse(raw).map_err(|invalid_tokens| {
            AppError::UsageError(format!(
                "Unrecognized --sections value(s): {}. Valid section names are: fetch, \
                 activity, stats, recommendations",
                invalid_tokens.join(", ")
            ))
        }),
    }
}

/// Reuses `nostr_sdk::RelayUrl::parse`, the same check `fetch::client` performs
/// internally, so the two layers never disagree on what's well-formed.
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

pub fn resolve_relays(explicit_raw: Option<&str>, config_relays: Option<&[String]>) -> Vec<String> {
    if let Some(explicit_raw) = explicit_raw {
        return explicit_raw.split(',').map(|s| s.to_string()).collect();
    }
    if let Some(config_relays) = config_relays {
        return config_relays.to_vec();
    }
    vec![DEFAULT_RELAY.to_string()]
}

/// The top-level entry point `main.rs` calls once flags are parsed. Relay
/// well-formedness is a separate check `main.rs` runs on its own.
#[allow(clippy::too_many_arguments)]
pub fn resolve_run_options(
    explicit_format: Option<Format>,
    color: bool,
    no_color: bool,
    quiet: bool,
    stdout_is_terminal: bool,
    time_range_inputs: TimeRangeInputs,
    now: DateTime<Utc>,
    sections_raw: Option<String>,
    config: Option<&ConfigFile>,
    output_present: bool,
) -> Result<crate::report::render::RunOptions, AppError> {
    validate_color_flags(color, no_color)?;

    let config_sections = config.and_then(|config| config.sections);
    let sections = match sections_raw {
        Some(raw) => resolve_sections(Some(raw.as_str()))?,
        None => config_sections.unwrap_or_else(SectionFilter::all),
    };

    let config_format = config.and_then(|config| config.format);
    let config_view = config.and_then(|config| config.view);
    let config_color_override = config.and_then(|config| config.color_override);

    let context_default = resolve_context_default(stdout_is_terminal, output_present);
    let format_before_explicit =
        resolve_format_before_explicit(config_format, output_present, context_default, color);
    let format = resolve_format(explicit_format, format_before_explicit);
    validate_output_format(output_present, format)?;

    let mut time_range = resolve_time_range(time_range_inputs, now)?;
    if time_range.view.is_none() {
        time_range.view = config_view;
    }

    let color_override = resolve_color_override(color, no_color).or(config_color_override);

    Ok(crate::report::render::RunOptions {
        format,
        quiet,
        color_override,
        since: time_range.since,
        until: time_range.until,
        view: time_range.view,
        sections,
    })
}

/// Raw, unparsed `--since`/`--until`/`--view` flag values. Kept separate from
/// `cli::args::Args` so this module never depends on `clap`.
#[derive(Debug, Clone)]
pub struct TimeRangeInputs {
    pub since_raw: Option<String>,
    pub until_raw: Option<String>,
    pub view: Option<Granularity>,
}

/// `since`/`until` are `None` when deferred to a data-dependent default only `run()`
/// can resolve: the node's earliest order, or `run()`'s own `report_generated_at`
/// captured after fetching (not an earlier `now` read here, which could diverge from it
/// by however long the fetch takes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedTimeRange {
    pub since: Option<i64>,
    pub until: Option<i64>,
    pub view: Option<Granularity>,
}

/// Resolves `--since`/`--until`/`--view` into their final values, performing every
/// validation that must happen before querying any relay.
pub fn resolve_time_range(
    inputs: TimeRangeInputs,
    now: DateTime<Utc>,
) -> Result<ResolvedTimeRange, AppError> {
    let today = now.date_naive();

    let since_explicit = inputs
        .since_raw
        .as_deref()
        .map(|raw| parse_date_bound(raw, today).map(since_bound_seconds))
        .transpose()?;
    let until_explicit = inputs
        .until_raw
        .as_deref()
        .map(|raw| parse_date_bound(raw, today).map(until_bound_seconds))
        .transpose()?;

    // `--until` stays deferred in the since-only case, like `--since` does in the
    // until-only case: `run()` resolves it against its own `report_generated_at`,
    // captured after fetching, not this earlier `now` — otherwise an event created in
    // the gap could count in general statistics but be silently excluded from the grid.
    let (since, until) = match (since_explicit, until_explicit) {
        (None, None) => (None, None),
        (Some(since), None) => (Some(since), None),
        (None, Some(until)) => (None, Some(until)),
        (Some(since), Some(until)) => (Some(since), Some(until)),
    };

    // Applies whenever `--since` was explicitly given, using `now` as a stand-in for the
    // eventual until-default. A *defaulted* `--since` is exempt.
    if let Some(since) = since {
        let effective_until = until.unwrap_or_else(|| now.timestamp());
        if since > effective_until {
            return Err(AppError::UsageError(format!(
                "--since resolves to a point later than --until ({since} > {effective_until}); \
                 --since must not be later than --until"
            )));
        }
    }

    if let Some(view) = inputs.view {
        validate_explicit_view_alignment(view, since_explicit, until_explicit)?;
    }

    Ok(ResolvedTimeRange {
        since,
        until,
        view: inputs.view,
    })
}

/// Only applies to values the caller actually typed — a defaulted `--since`/`--until`
/// is snapped instead, inside `stats::grid`, never rejected here.
fn validate_explicit_view_alignment(
    view: Granularity,
    since_explicit: Option<i64>,
    until_explicit: Option<i64>,
) -> Result<(), AppError> {
    if let Some(since) = since_explicit {
        if !is_first_day_of_period(view, since) {
            return Err(AppError::UsageError(format!(
                "--since ({since}) does not resolve to the first day of a calendar {}, \
                 required when --view {} is explicit",
                period_name(view),
                view_name(view)
            )));
        }
    }
    if let Some(until) = until_explicit {
        if !is_last_day_of_period(view, until) {
            return Err(AppError::UsageError(format!(
                "--until ({until}) does not resolve to the last day of a calendar {}, \
                 required when --view {} is explicit",
                period_name(view),
                view_name(view)
            )));
        }
    }
    Ok(())
}

fn period_name(view: Granularity) -> &'static str {
    match view {
        Granularity::Daily => "day",
        Granularity::Monthly => "month",
        Granularity::Yearly => "year",
    }
}

fn view_name(view: Granularity) -> &'static str {
    match view {
        Granularity::Daily => "daily",
        Granularity::Monthly => "monthly",
        Granularity::Yearly => "yearly",
    }
}

fn date_from_seconds(timestamp: i64) -> chrono::NaiveDate {
    DateTime::<Utc>::from_timestamp(timestamp, 0)
        .unwrap_or_default()
        .date_naive()
}

fn is_first_day_of_period(view: Granularity, timestamp: i64) -> bool {
    match view {
        Granularity::Daily => true,
        Granularity::Monthly => date_from_seconds(timestamp).day() == 1,
        Granularity::Yearly => {
            let date = date_from_seconds(timestamp);
            date.month() == 1 && date.day() == 1
        }
    }
}

fn is_last_day_of_period(view: Granularity, timestamp: i64) -> bool {
    match view {
        Granularity::Daily => true,
        Granularity::Monthly => {
            let date = date_from_seconds(timestamp);
            let next_day = date + chrono::Duration::days(1);
            next_day.month() != date.month()
        }
        Granularity::Yearly => {
            let date = date_from_seconds(timestamp);
            date.month() == 12 && date.day() == 31
        }
    }
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

    // ---- `resolve_context_default`/`validate_output_format` ----

    #[test]
    fn resolve_context_default_uses_the_terminal_based_default_when_output_is_absent() {
        assert_eq!(resolve_context_default(true, false), Format::Console);
        assert_eq!(resolve_context_default(false, false), Format::Plain);
    }

    #[test]
    fn resolve_context_default_resolves_to_plain_when_output_is_present() {
        assert_eq!(resolve_context_default(true, true), Format::Plain);
        assert_eq!(resolve_context_default(false, true), Format::Plain);
    }

    #[test]
    fn validate_output_format_accepts_plain_and_json_when_output_is_present() {
        assert!(validate_output_format(true, Format::Plain).is_ok());
        assert!(validate_output_format(true, Format::Json).is_ok());
    }

    #[test]
    fn validate_output_format_rejects_console_when_output_is_present() {
        let error = validate_output_format(true, Format::Console).expect_err("console is rejected");
        assert!(matches!(error, AppError::UsageError(_)));
    }

    #[test]
    fn validate_output_format_accepts_any_format_when_output_is_absent() {
        assert!(validate_output_format(false, Format::Console).is_ok());
        assert!(validate_output_format(false, Format::Plain).is_ok());
        assert!(validate_output_format(false, Format::Json).is_ok());
    }

    #[test]
    fn resolve_run_options_output_present_resolves_to_plain_over_terminal_detection() {
        let options = resolve_run_options(
            None,
            false,
            false,
            false,
            true,
            no_time_range_inputs(),
            fixed_now(),
            None,
            None,
            true,
        )
        .expect("plain is valid with --output");

        assert_eq!(options.format, Format::Plain);
    }

    /// Regression test: `--color`'s automatic plain-to-console upgrade used to run
    /// unconditionally, undoing `--output`'s forced-plain default.
    #[test]
    fn resolve_run_options_output_present_with_color_flag_still_resolves_to_plain() {
        let options = resolve_run_options(
            None,
            true,
            false,
            false,
            true,
            no_time_range_inputs(),
            fixed_now(),
            None,
            None,
            true,
        )
        .expect("--output --color is valid, not console");

        assert_eq!(options.format, Format::Plain);
    }

    #[test]
    fn resolve_run_options_rejects_explicit_console_format_with_output_present() {
        let result = resolve_run_options(
            Some(Format::Console),
            false,
            false,
            false,
            true,
            no_time_range_inputs(),
            fixed_now(),
            None,
            None,
            true,
        );

        assert!(matches!(result, Err(AppError::UsageError(_))));
    }

    #[test]
    fn resolve_run_options_accepts_explicit_json_format_with_output_present() {
        let options = resolve_run_options(
            Some(Format::Json),
            false,
            false,
            false,
            true,
            no_time_range_inputs(),
            fixed_now(),
            None,
            None,
            true,
        )
        .expect("json is valid with --output");

        assert_eq!(options.format, Format::Json);
    }

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

    #[test]
    fn apply_color_format_override_upgrades_an_automatic_plain_default_to_console() {
        assert_eq!(
            apply_color_format_override(Format::Plain, true),
            Format::Console
        );
    }

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

    #[test]
    fn validate_relay_urls_rejects_a_malformed_relay_naming_it_in_the_message() {
        let relays = vec!["wss://good.example".to_string(), "not-a-url".to_string()];

        let error = validate_relay_urls(&relays).expect_err("a malformed relay is rejected");

        assert!(matches!(error, AppError::UsageError(_)));
        assert!(error.to_string().contains("not-a-url"));
    }

    #[test]
    fn validate_relay_urls_rejects_an_unsupported_scheme() {
        let relays = vec!["https://relay.example".to_string()];

        let error = validate_relay_urls(&relays).expect_err("an unsupported scheme is rejected");

        assert!(error.to_string().contains("https://relay.example"));
    }

    fn no_time_range_inputs() -> TimeRangeInputs {
        TimeRangeInputs {
            since_raw: None,
            until_raw: None,
            view: None,
        }
    }

    fn fixed_now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-07-25T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn resolve_run_options_rejects_contradictory_color_flags() {
        let result = resolve_run_options(
            None,
            true,
            true,
            false,
            true,
            no_time_range_inputs(),
            fixed_now(),
            None,
            None,
            false,
        );
        assert!(matches!(result, Err(AppError::UsageError(_))));
    }

    #[test]
    fn resolve_run_options_applies_the_color_format_upgrade_and_override() {
        let options = resolve_run_options(
            None,
            true,
            false,
            false,
            false,
            no_time_range_inputs(),
            fixed_now(),
            None,
            None,
            false,
        )
        .expect("color alone is not contradictory");

        assert_eq!(
            options,
            RunOptions {
                format: Format::Console,
                quiet: false,
                color_override: Some(true),
                since: None,
                until: None,
                view: None,
                sections: SectionFilter::all(),
            }
        );
    }

    #[test]
    fn resolve_run_options_explicit_format_ignores_the_color_flag() {
        let options = resolve_run_options(
            Some(Format::Json),
            true,
            false,
            false,
            false,
            no_time_range_inputs(),
            fixed_now(),
            None,
            None,
            false,
        )
        .expect("color alone is not contradictory");

        assert_eq!(options.format, Format::Json);
    }

    #[test]
    fn resolve_run_options_threads_quiet_through_unchanged() {
        let options = resolve_run_options(
            None,
            false,
            false,
            true,
            true,
            no_time_range_inputs(),
            fixed_now(),
            None,
            None,
            false,
        )
        .expect("no validation failure");
        assert!(options.quiet);
    }

    #[test]
    fn resolve_run_options_propagates_a_time_range_validation_failure() {
        let inputs = TimeRangeInputs {
            since_raw: Some("2026-06-01".to_string()),
            until_raw: Some("2026-01-01".to_string()),
            view: None,
        };
        let result = resolve_run_options(
            None,
            false,
            false,
            false,
            true,
            inputs,
            fixed_now(),
            None,
            None,
            false,
        );
        assert!(matches!(result, Err(AppError::UsageError(_))));
    }

    // ---- `resolve_sections` ----

    #[test]
    fn resolve_sections_defaults_to_all_sections_when_omitted() {
        let sections = resolve_sections(None).expect("no validation failure");
        assert_eq!(sections, SectionFilter::all());
    }

    #[test]
    fn resolve_sections_parses_a_valid_subset() {
        let sections = resolve_sections(Some("activity,stats")).expect("valid subset");
        assert!(sections.activity);
        assert!(sections.stats);
        assert!(!sections.fetch);
        assert!(!sections.recommendations);
    }

    #[test]
    fn resolve_sections_rejects_an_unrecognized_token() {
        let error = resolve_sections(Some("stats,bogus")).expect_err("unrecognized token");

        assert!(matches!(error, AppError::UsageError(_)));
        let message = error.to_string();
        assert!(message.contains("bogus"));
        assert!(message.contains("fetch"));
        assert!(message.contains("activity"));
        assert!(message.contains("stats"));
        assert!(message.contains("recommendations"));
    }

    #[test]
    fn resolve_run_options_rejects_an_unrecognized_sections_token_before_any_relay_is_queried() {
        let result = resolve_run_options(
            None,
            false,
            false,
            false,
            true,
            no_time_range_inputs(),
            fixed_now(),
            Some("bogus".to_string()),
            None,
            false,
        );
        assert!(matches!(result, Err(AppError::UsageError(_))));
    }

    #[test]
    fn resolve_run_options_threads_a_valid_sections_value_through() {
        let options = resolve_run_options(
            None,
            false,
            false,
            false,
            true,
            no_time_range_inputs(),
            fixed_now(),
            Some("stats".to_string()),
            None,
            false,
        )
        .expect("valid sections value");

        assert!(options.sections.stats);
        assert!(!options.sections.fetch);
    }

    // ---- `resolve_time_range` ----

    #[test]
    fn resolve_time_range_with_both_flags_omitted_covers_full_history() {
        let resolved = resolve_time_range(no_time_range_inputs(), fixed_now()).unwrap();

        assert_eq!(resolved.since, None);
        assert_eq!(resolved.until, None);
    }

    /// `--until` stays deferred to `run()`'s later `report_generated_at`, not this
    /// earlier `now` (a real bug found during review: the two could otherwise diverge).
    #[test]
    fn resolve_time_range_with_only_since_leaves_until_deferred_to_run() {
        let inputs = TimeRangeInputs {
            since_raw: Some("2026-06-01".to_string()),
            until_raw: None,
            view: None,
        };
        let now = fixed_now();

        let resolved = resolve_time_range(inputs, now).unwrap();

        assert_eq!(resolved.until, None);
        assert!(resolved.since.is_some());
    }

    #[test]
    fn resolve_time_range_with_only_until_leaves_since_unresolved() {
        let inputs = TimeRangeInputs {
            since_raw: None,
            until_raw: Some("2026-06-01".to_string()),
            view: None,
        };

        let resolved = resolve_time_range(inputs, fixed_now()).unwrap();

        assert_eq!(resolved.since, None);
        assert!(resolved.until.is_some());
    }

    #[test]
    fn resolve_time_range_rejects_an_explicit_since_later_than_until() {
        let inputs = TimeRangeInputs {
            since_raw: Some("2026-06-01".to_string()),
            until_raw: Some("2026-01-01".to_string()),
            view: None,
        };

        let error = resolve_time_range(inputs, fixed_now()).expect_err("since > until");
        assert!(matches!(error, AppError::UsageError(_)));
    }

    /// The check is not limited to the case where both flags were explicitly typed.
    #[test]
    fn resolve_time_range_rejects_an_explicit_since_in_the_future_with_until_omitted() {
        let inputs = TimeRangeInputs {
            since_raw: Some("2099-01-01".to_string()),
            until_raw: None,
            view: None,
        };

        let error =
            resolve_time_range(inputs, fixed_now()).expect_err("since later than now is rejected");
        assert!(matches!(error, AppError::UsageError(_)));
    }

    /// Not an error here — the emptiness is only knowable once `run()` resolves the
    /// actual earliest order timestamp.
    #[test]
    fn resolve_time_range_does_not_reject_a_defaulted_since_against_until() {
        let inputs = TimeRangeInputs {
            since_raw: None,
            until_raw: Some("2000-01-01".to_string()),
            view: None,
        };

        let resolved = resolve_time_range(inputs, fixed_now()).expect("not an error here");
        assert_eq!(resolved.since, None);
    }

    #[test]
    fn resolve_time_range_threads_an_explicit_view_through_unchanged() {
        let inputs = TimeRangeInputs {
            since_raw: None,
            until_raw: None,
            view: Some(Granularity::Monthly),
        };

        let resolved = resolve_time_range(inputs, fixed_now()).unwrap();
        assert_eq!(resolved.view, Some(Granularity::Monthly));
    }

    #[test]
    fn resolve_time_range_rejects_a_since_misaligned_with_an_explicit_monthly_view() {
        let inputs = TimeRangeInputs {
            since_raw: Some("2026-06-15".to_string()),
            until_raw: Some("2026-06-30".to_string()),
            view: Some(Granularity::Monthly),
        };

        let error = resolve_time_range(inputs, fixed_now())
            .expect_err("since must be the first day of the month");
        assert!(matches!(error, AppError::UsageError(_)));
    }

    #[test]
    fn resolve_time_range_rejects_an_until_misaligned_with_an_explicit_monthly_view() {
        let inputs = TimeRangeInputs {
            since_raw: Some("2026-06-01".to_string()),
            until_raw: Some("2026-06-15".to_string()),
            view: Some(Granularity::Monthly),
        };

        let error = resolve_time_range(inputs, fixed_now())
            .expect_err("until must be the last day of the month");
        assert!(matches!(error, AppError::UsageError(_)));
    }

    #[test]
    fn resolve_time_range_accepts_an_explicit_monthly_view_with_aligned_bounds() {
        let inputs = TimeRangeInputs {
            since_raw: Some("2026-06-01".to_string()),
            until_raw: Some("2026-06-30".to_string()),
            view: Some(Granularity::Monthly),
        };

        let resolved = resolve_time_range(inputs, fixed_now()).expect("already aligned");
        assert_eq!(resolved.view, Some(Granularity::Monthly));
    }

    #[test]
    fn resolve_time_range_rejects_bounds_misaligned_with_an_explicit_yearly_view() {
        let inputs = TimeRangeInputs {
            since_raw: Some("2026-03-01".to_string()),
            until_raw: Some("2026-12-31".to_string()),
            view: Some(Granularity::Yearly),
        };

        let error = resolve_time_range(inputs, fixed_now())
            .expect_err("since must be January 1st for an explicit yearly view");
        assert!(matches!(error, AppError::UsageError(_)));
    }

    /// Only `--until`'s own explicit alignment is validated; `stats::grid` later snaps
    /// whatever the defaulted `--since` turns out to be.
    #[test]
    fn resolve_time_range_never_checks_alignment_of_a_since_left_to_its_default() {
        let inputs = TimeRangeInputs {
            since_raw: None,
            until_raw: Some("2026-06-30".to_string()),
            view: Some(Granularity::Monthly),
        };

        let resolved = resolve_time_range(inputs, fixed_now())
            .expect("an omitted --since is never checked for alignment");
        assert_eq!(resolved.since, None);
        assert_eq!(resolved.view, Some(Granularity::Monthly));
    }

    // ---- `resolve_relays` ----

    #[test]
    fn resolve_relays_uses_the_explicit_value_when_present() {
        let relays = resolve_relays(Some("wss://explicit.example"), None);
        assert_eq!(relays, vec!["wss://explicit.example".to_string()]);
    }

    #[test]
    fn resolve_relays_splits_a_comma_separated_explicit_value() {
        let relays = resolve_relays(Some("wss://a.example,wss://b.example"), None);
        assert_eq!(
            relays,
            vec!["wss://a.example".to_string(), "wss://b.example".to_string()]
        );
    }

    #[test]
    fn resolve_relays_falls_back_to_config_when_explicit_is_absent() {
        let config_relays = vec!["wss://config.example".to_string()];
        let relays = resolve_relays(None, Some(&config_relays));
        assert_eq!(relays, config_relays);
    }

    #[test]
    fn resolve_relays_falls_back_to_the_compiled_default_when_neither_is_present() {
        let relays = resolve_relays(None, None);
        assert_eq!(relays, vec![DEFAULT_RELAY.to_string()]);
    }

    #[test]
    fn resolve_relays_prefers_explicit_over_config() {
        let config_relays = vec!["wss://config.example".to_string()];
        let relays = resolve_relays(Some("wss://explicit.example"), Some(&config_relays));
        assert_eq!(relays, vec!["wss://explicit.example".to_string()]);
    }

    // ---- config-sourced values slot between explicit and automatic ----

    fn config_with(
        format: Option<Format>,
        view: Option<Granularity>,
        color_override: Option<bool>,
    ) -> ConfigFile {
        ConfigFile {
            pubkey: None,
            relays: None,
            format,
            view,
            color_override,
            sections: None,
        }
    }

    #[test]
    fn resolve_run_options_honors_a_config_sourced_format_over_the_automatic_default() {
        let config = config_with(Some(Format::Json), None, None);
        let options = resolve_run_options(
            None,
            false,
            false,
            false,
            true,
            no_time_range_inputs(),
            fixed_now(),
            None,
            Some(&config),
            false,
        )
        .expect("no validation failure");

        assert_eq!(options.format, Format::Json);
    }

    #[test]
    fn resolve_run_options_explicit_format_overrides_a_config_sourced_value() {
        let config = config_with(Some(Format::Json), None, None);
        let options = resolve_run_options(
            Some(Format::Plain),
            false,
            false,
            false,
            true,
            no_time_range_inputs(),
            fixed_now(),
            None,
            Some(&config),
            false,
        )
        .expect("no validation failure");

        assert_eq!(options.format, Format::Plain);
    }

    #[test]
    fn resolve_run_options_honors_a_config_sourced_view_when_view_flag_is_absent() {
        let config = config_with(None, Some(Granularity::Yearly), None);
        let options = resolve_run_options(
            None,
            false,
            false,
            false,
            true,
            no_time_range_inputs(),
            fixed_now(),
            None,
            Some(&config),
            false,
        )
        .expect("no validation failure");

        assert_eq!(options.view, Some(Granularity::Yearly));
    }

    #[test]
    fn resolve_run_options_explicit_view_overrides_a_config_sourced_value() {
        let config = config_with(None, Some(Granularity::Yearly), None);
        let inputs = TimeRangeInputs {
            since_raw: None,
            until_raw: None,
            view: Some(Granularity::Daily),
        };
        let options = resolve_run_options(
            None,
            false,
            false,
            false,
            true,
            inputs,
            fixed_now(),
            None,
            Some(&config),
            false,
        )
        .expect("no validation failure");

        assert_eq!(options.view, Some(Granularity::Daily));
    }

    #[test]
    fn resolve_run_options_honors_a_config_sourced_color_override_when_flags_are_absent() {
        let config = config_with(None, None, Some(true));
        let options = resolve_run_options(
            None,
            false,
            false,
            false,
            true,
            no_time_range_inputs(),
            fixed_now(),
            None,
            Some(&config),
            false,
        )
        .expect("no validation failure");

        assert_eq!(options.color_override, Some(true));
    }

    #[test]
    fn resolve_run_options_honors_a_config_sourced_sections_value_when_flag_is_absent() {
        let mut config = config_with(None, None, None);
        config.sections = Some(SectionFilter::parse("stats").unwrap());
        let options = resolve_run_options(
            None,
            false,
            false,
            false,
            true,
            no_time_range_inputs(),
            fixed_now(),
            None,
            Some(&config),
            false,
        )
        .expect("no validation failure");

        assert!(options.sections.stats);
        assert!(!options.sections.fetch);
    }

    #[test]
    fn resolve_run_options_explicit_sections_overrides_a_config_sourced_value() {
        let mut config = config_with(None, None, None);
        config.sections = Some(SectionFilter::parse("stats").unwrap());
        let options = resolve_run_options(
            None,
            false,
            false,
            false,
            true,
            no_time_range_inputs(),
            fixed_now(),
            Some("fetch".to_string()),
            Some(&config),
            false,
        )
        .expect("no validation failure");

        assert!(options.sections.fetch);
        assert!(!options.sections.stats);
    }

    #[test]
    fn resolve_run_options_explicit_no_color_overrides_a_config_sourced_color_value() {
        let config = config_with(None, None, Some(true));
        let options = resolve_run_options(
            None,
            false,
            true,
            false,
            true,
            no_time_range_inputs(),
            fixed_now(),
            None,
            Some(&config),
            false,
        )
        .expect("no validation failure");

        assert_eq!(options.color_override, Some(false));
    }
}
