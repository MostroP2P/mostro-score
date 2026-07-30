//! Parsing and calendar arithmetic for `--since`/`--until` values. No I/O and no clock
//! reads of its own — the caller always supplies `now` explicitly.

use crate::error::AppError;
use chrono::{Datelike, NaiveDate};

/// Parses either an ISO 8601 date (`YYYY-MM-DD`) or a relative shorthand (`Nd`/`Nmo`/`Ny`,
/// `N` a positive integer), resolved relative to `now`.
pub fn parse_date_bound(input: &str, now: NaiveDate) -> Result<NaiveDate, AppError> {
    if let Ok(date) = NaiveDate::parse_from_str(input, "%Y-%m-%d") {
        return Ok(date);
    }
    parse_shorthand(input, now)
}

pub fn since_bound_seconds(date: NaiveDate) -> i64 {
    date.and_hms_opt(0, 0, 0)
        .unwrap_or_default()
        .and_utc()
        .timestamp()
}

pub fn until_bound_seconds(date: NaiveDate) -> i64 {
    date.and_hms_opt(23, 59, 59)
        .unwrap_or_default()
        .and_utc()
        .timestamp()
}

fn parse_shorthand(input: &str, now: NaiveDate) -> Result<NaiveDate, AppError> {
    let (number_part, unit) = if let Some(stripped) = input.strip_suffix("mo") {
        (stripped, "mo")
    } else if let Some(stripped) = input.strip_suffix('d') {
        (stripped, "d")
    } else if let Some(stripped) = input.strip_suffix('y') {
        (stripped, "y")
    } else {
        return Err(invalid_date_bound(input));
    };

    let count: i64 = number_part.parse().map_err(|_| invalid_date_bound(input))?;
    if count <= 0 {
        return Err(invalid_date_bound(input));
    }

    // Checked throughout: an extreme count must produce a usage error, never a panic.
    let resolved = match unit {
        "d" => {
            chrono::TimeDelta::try_days(count - 1).and_then(|delta| now.checked_sub_signed(delta))
        }
        "mo" => subtract_months_clamped(now, count),
        "y" => subtract_years_clamped(now, count),
        _ => unreachable!("unit is always one of d/mo/y"),
    };
    resolved.ok_or_else(|| invalid_date_bound(input))
}

fn invalid_date_bound(input: &str) -> AppError {
    AppError::UsageError(format!(
        "Invalid date or duration '{input}': expected an ISO 8601 date (YYYY-MM-DD) or a \
         positive shorthand duration using d/mo/y (e.g. 30d, 6mo, 1y)"
    ))
}

/// Clamps to the last valid day of the resulting month when `date`'s day-of-month
/// doesn't exist there (e.g. March 31 minus 1 month has no "February 31").
fn subtract_months_clamped(date: NaiveDate, months: i64) -> Option<NaiveDate> {
    let total_months = (date.year() as i64)
        .checked_mul(12)?
        .checked_add(date.month0() as i64)?
        .checked_sub(months)?;
    let target_year = i32::try_from(total_months.div_euclid(12)).ok()?;
    let target_month = (total_months.rem_euclid(12) as u32) + 1;

    NaiveDate::from_ymd_opt(target_year, target_month, date.day())
        .or_else(|| last_day_of_month(target_year, target_month))
}

/// Clamps February 29 to February 28 when the resulting year isn't a leap year.
fn subtract_years_clamped(date: NaiveDate, years: i64) -> Option<NaiveDate> {
    let target_year = i32::try_from((date.year() as i64).checked_sub(years)?).ok()?;

    NaiveDate::from_ymd_opt(target_year, date.month(), date.day())
        .or_else(|| last_day_of_month(target_year, date.month()))
}

fn last_day_of_month(year: i32, month: u32) -> Option<NaiveDate> {
    let (next_year, next_month) = if month == 12 {
        (year.checked_add(1)?, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(next_year, next_month, 1)?.pred_opt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    #[test]
    fn parse_date_bound_accepts_a_plain_iso_8601_date() {
        let resolved = parse_date_bound("2026-03-15", date(2026, 7, 25)).unwrap();
        assert_eq!(resolved, date(2026, 3, 15));
    }

    #[test]
    fn parse_date_bound_resolves_nd_shorthand_including_today() {
        let resolved = parse_date_bound("7d", date(2026, 1, 10)).unwrap();
        assert_eq!(resolved, date(2026, 1, 4));
    }

    #[test]
    fn parse_date_bound_resolves_nmo_shorthand_without_minus_one_adjustment() {
        let resolved = parse_date_bound("1mo", date(2026, 5, 15)).unwrap();
        assert_eq!(resolved, date(2026, 4, 15));
    }

    #[test]
    fn parse_date_bound_clamps_nmo_shorthand_to_the_last_day_of_a_shorter_month() {
        let resolved = parse_date_bound("1mo", date(2026, 3, 31)).unwrap();
        assert_eq!(resolved, date(2026, 2, 28));
    }

    #[test]
    fn parse_date_bound_resolves_ny_shorthand_without_minus_one_adjustment() {
        let resolved = parse_date_bound("1y", date(2026, 6, 1)).unwrap();
        assert_eq!(resolved, date(2025, 6, 1));
    }

    #[test]
    fn parse_date_bound_clamps_ny_shorthand_landing_on_a_non_leap_year() {
        let resolved = parse_date_bound("1y", date(2024, 2, 29)).unwrap();
        assert_eq!(resolved, date(2023, 2, 28));
    }

    #[test]
    fn parse_date_bound_rejects_zero() {
        let error = parse_date_bound("0d", date(2026, 1, 1)).expect_err("0 is rejected");
        assert!(matches!(error, AppError::UsageError(_)));
    }

    #[test]
    fn parse_date_bound_rejects_a_negative_count() {
        let error = parse_date_bound("-5d", date(2026, 1, 1)).expect_err("negative is rejected");
        assert!(matches!(error, AppError::UsageError(_)));
    }

    #[test]
    fn parse_date_bound_rejects_an_unrepresentable_day_count_instead_of_panicking() {
        let error = parse_date_bound(&format!("{}d", i64::MAX), date(2026, 1, 1))
            .expect_err("an unrepresentable day count is rejected, not panicking");
        assert!(matches!(error, AppError::UsageError(_)));
    }

    #[test]
    fn parse_date_bound_rejects_an_unrepresentable_month_count_instead_of_panicking() {
        let error = parse_date_bound(&format!("{}mo", i64::MAX), date(2026, 1, 1))
            .expect_err("an unrepresentable month count is rejected, not panicking");
        assert!(matches!(error, AppError::UsageError(_)));
    }

    #[test]
    fn parse_date_bound_rejects_an_unrepresentable_year_count_instead_of_wrapping() {
        let error = parse_date_bound(&format!("{}y", i64::MAX), date(2026, 1, 1))
            .expect_err("an unrepresentable year count is rejected, not silently wrapped");
        assert!(matches!(error, AppError::UsageError(_)));
    }

    #[test]
    fn parse_date_bound_rejects_an_unrecognized_unit() {
        let error = parse_date_bound("30x", date(2026, 1, 1)).expect_err("30x is rejected");
        assert!(matches!(error, AppError::UsageError(_)));
        assert!(error.to_string().contains("30x"));
    }

    #[test]
    fn parse_date_bound_rejects_an_unparseable_string() {
        let error = parse_date_bound("not-a-date", date(2026, 1, 1))
            .expect_err("garbage input is rejected");
        assert!(matches!(error, AppError::UsageError(_)));
        assert!(error.to_string().contains("not-a-date"));
    }

    #[test]
    fn since_bound_seconds_resolves_to_start_of_day_utc() {
        let seconds = since_bound_seconds(date(1970, 1, 2));
        assert_eq!(seconds, 86400);
    }

    #[test]
    fn until_bound_seconds_resolves_to_the_last_whole_second_of_day_utc() {
        let seconds = until_bound_seconds(date(1970, 1, 1));
        assert_eq!(seconds, 86399);
    }
}
