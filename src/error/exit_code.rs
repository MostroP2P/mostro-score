//! PR 2 (T062-T063): `AppError` -> process exit-code mapping per 002 FR-019. Exit code `0`
//! has no entry here since it is simply the absence of an `AppError` (the default success
//! exit). PR 3 (T096) adds exit code `4` (`no_usable_events`), now that the four-kind
//! event scoping it depends on exists.

use crate::error::AppError;

pub fn exit_code_for(error: &AppError) -> i32 {
    match error {
        AppError::UsageError(_) => 2,
        AppError::RelaysUnreachable => 3,
        AppError::InvalidPubkey => 5,
        AppError::NoUsableEvents => 4,
        AppError::Other(_) => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_pubkey_maps_to_exit_code_5() {
        assert_eq!(exit_code_for(&AppError::InvalidPubkey), 5);
    }

    #[test]
    fn relays_unreachable_maps_to_exit_code_3() {
        assert_eq!(exit_code_for(&AppError::RelaysUnreachable), 3);
    }

    #[test]
    fn usage_error_maps_to_exit_code_2() {
        assert_eq!(
            exit_code_for(&AppError::UsageError("bad flag combination".to_string())),
            2
        );
    }

    #[test]
    fn no_usable_events_maps_to_exit_code_4() {
        assert_eq!(exit_code_for(&AppError::NoUsableEvents), 4);
    }

    #[test]
    fn other_maps_to_exit_code_1() {
        let source: Box<dyn std::error::Error> = "boom".into();
        assert_eq!(exit_code_for(&AppError::from(source)), 1);
    }
}
