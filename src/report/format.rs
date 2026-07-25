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

#[cfg(test)]
mod tests {
    use super::*;

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
