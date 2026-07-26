/// Format relative time for human readability (Section 6.1)
pub fn format_relative_time(timestamp: i64, now: i64) -> String {
    let diff_secs = now - timestamp;

    if diff_secs < 0 {
        return "in the future".to_string();
    }

    let days = diff_secs / 86400;
    let hours = (diff_secs % 86400) / 3600;

    match days {
        0 => {
            if hours == 0 {
                "less than an hour ago".to_string()
            } else if hours == 1 {
                "1 hour ago".to_string()
            } else {
                format!("{} hours ago", hours)
            }
        }
        1 => "1 day ago".to_string(),
        2..=6 => format!("{} days ago", days),
        7..=13 => "1 week ago".to_string(),
        14..=29 => format!("{} weeks ago", days / 7),
        30..=59 => "1 month ago".to_string(),
        60..=364 => format!("{} months ago", days / 30),
        _ => format!("{} years ago", days / 365),
    }
}

/// A single shared not-applicable marker for console/plain-text output (FR-018's
/// numeric contract is JSON-only; this module never touches JSON). Replaces the several
/// inline `"N/A"` literals the old ad hoc renderer repeated verbatim.
pub const NOT_APPLICABLE: &str = "N/A";

/// Renders an already-computed value, or `NOT_APPLICABLE` when it is `None` — the one
/// shared helper every not-applicable console/plain-text field can call, instead of each
/// call site repeating its own `unwrap_or_else(|| "N/A".to_string())`.
pub fn display_or_not_applicable<T: std::fmt::Display>(value: Option<T>) -> String {
    match value {
        Some(value) => value.to_string(),
        None => NOT_APPLICABLE.to_string(),
    }
}

/// Groups a string of ASCII digits into thousands with `,` separators (e.g. `"12345"` ->
/// `"12,345"`). Private: callers go through `format_sats_thousands`/
/// `format_decimal_thousands` below, which handle the sign and fractional part this
/// helper does not need to know about.
fn group_thousands(digits: &str) -> String {
    let chars: Vec<char> = digits.chars().collect();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, character) in chars.iter().enumerate() {
        if index > 0 && (chars.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(*character);
    }
    grouped
}

/// 002 FR-018: an integer sats/fiat value with thousands separators, for console and
/// plain-text display only — this module is presentation-only, so that restriction is
/// automatically respected; the not-yet-implemented JSON renderer must never call this.
pub fn format_sats_thousands(value: u64) -> String {
    group_thousands(&value.to_string())
}

/// Same contract as `format_sats_thousands`, for a fractional value (mean/median sats,
/// premium percentages), rounded to one decimal place — matching the old renderer's
/// `{:.1}` convention — with thousands separators applied to the integer part only.
pub fn format_decimal_thousands(value: f64) -> String {
    let rounded = format!("{value:.1}");
    let (integer_part, fractional_part) =
        rounded.split_once('.').unwrap_or((rounded.as_str(), "0"));
    let is_negative = integer_part.starts_with('-');
    let digits = integer_part.trim_start_matches('-');
    let grouped = group_thousands(digits);
    format!(
        "{}{grouped}.{fractional_part}",
        if is_negative { "-" } else { "" }
    )
}

/// 002 FR-015's color policy, resolved from already-gathered inputs so the decision
/// itself is unit-testable without depending on the real process environment or
/// terminal state. `term_is_dumb` is checked before any override is applied: it is the
/// one exception FR-015 states no explicit override may bypass, since it reflects a
/// technical incapability, not a preference — unlike `NO_COLOR` or a non-terminal
/// destination, both of which an explicit "force on" override may still cross.
pub fn resolve_color_enabled(
    automatic_choice: bool,
    term_is_dumb: bool,
    force_color: Option<bool>,
) -> bool {
    if term_is_dumb {
        return false;
    }
    force_color.unwrap_or(automatic_choice)
}

/// Gathers the real environment/terminal state for stdout and applies
/// `resolve_color_enabled`. `anstream::AutoStream::choice` supplies the automatic
/// default — honoring `NO_COLOR`/`CLICOLOR`/tty detection via `anstyle-query`, the same
/// detection stack `clap`'s own color support already pulls in — while `TERM=dumb` is
/// checked directly here, since FR-015 needs to treat it as never-overridable, a
/// distinction `AutoStream::choice`'s single returned `ColorChoice` cannot express by
/// itself (it folds "dumb terminal" and "piped output" into the same `Never` result).
pub fn color_enabled_for_stdout(force_color: Option<bool>) -> bool {
    let stdout = std::io::stdout();
    let automatic_choice = anstream::AutoStream::choice(&stdout) == anstream::ColorChoice::Always;
    let term_is_dumb = std::env::var_os("TERM").is_some_and(|term| term == "dumb");
    resolve_color_enabled(automatic_choice, term_is_dumb, force_color)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_or_not_applicable_shows_the_value_when_present() {
        assert_eq!(display_or_not_applicable(Some(42)), "42");
    }

    #[test]
    fn display_or_not_applicable_shows_the_marker_when_absent() {
        assert_eq!(display_or_not_applicable::<u64>(None), "N/A");
    }

    #[test]
    fn format_sats_thousands_groups_large_values() {
        assert_eq!(format_sats_thousands(1_234_567), "1,234,567");
    }

    #[test]
    fn format_sats_thousands_leaves_small_values_alone() {
        assert_eq!(format_sats_thousands(42), "42");
    }

    #[test]
    fn format_decimal_thousands_groups_the_integer_part_and_rounds_to_one_decimal() {
        assert_eq!(format_decimal_thousands(1_234_567.89), "1,234,567.9");
    }

    #[test]
    fn format_decimal_thousands_preserves_a_negative_sign() {
        assert_eq!(format_decimal_thousands(-1_234.5), "-1,234.5");
    }

    /// FR-015: color is disabled automatically when the automatic choice says so.
    #[test]
    fn resolve_color_enabled_follows_the_automatic_choice_with_no_override() {
        assert!(!resolve_color_enabled(false, false, None));
        assert!(resolve_color_enabled(true, false, None));
    }

    /// FR-015: an explicit override always wins over the automatic choice, in both
    /// directions, when the terminal is not `TERM=dumb`.
    #[test]
    fn resolve_color_enabled_honors_an_explicit_override() {
        assert!(resolve_color_enabled(false, false, Some(true)));
        assert!(!resolve_color_enabled(true, false, Some(false)));
    }

    /// FR-015: `TERM=dumb` disables color regardless of the automatic choice or any
    /// override — the one exception no explicit override may bypass.
    #[test]
    fn resolve_color_enabled_term_dumb_overrides_everything() {
        assert!(!resolve_color_enabled(true, true, None));
        assert!(!resolve_color_enabled(false, true, Some(true)));
    }

    #[test]
    fn format_relative_time_future_timestamp() {
        assert_eq!(format_relative_time(200, 100), "in the future");
    }

    #[test]
    fn format_relative_time_less_than_an_hour() {
        assert_eq!(format_relative_time(0, 1800), "less than an hour ago");
    }

    #[test]
    fn format_relative_time_exactly_one_hour() {
        assert_eq!(format_relative_time(0, 3600), "1 hour ago");
    }

    #[test]
    fn format_relative_time_multiple_hours() {
        assert_eq!(format_relative_time(0, 3 * 3600), "3 hours ago");
    }

    #[test]
    fn format_relative_time_exactly_one_day() {
        assert_eq!(format_relative_time(0, 86400), "1 day ago");
    }

    #[test]
    fn format_relative_time_several_days() {
        assert_eq!(format_relative_time(0, 4 * 86400), "4 days ago");
    }

    #[test]
    fn format_relative_time_one_week_boundary() {
        assert_eq!(format_relative_time(0, 7 * 86400), "1 week ago");
    }

    #[test]
    fn format_relative_time_several_weeks() {
        assert_eq!(format_relative_time(0, 20 * 86400), "2 weeks ago");
    }

    #[test]
    fn format_relative_time_one_month_boundary() {
        assert_eq!(format_relative_time(0, 30 * 86400), "1 month ago");
    }

    #[test]
    fn format_relative_time_several_months() {
        assert_eq!(format_relative_time(0, 90 * 86400), "3 months ago");
    }

    #[test]
    fn format_relative_time_one_year_or_more() {
        assert_eq!(format_relative_time(0, 400 * 86400), "1 years ago");
    }
}
