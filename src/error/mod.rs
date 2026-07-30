//! The typed error taxonomy every fallible path in the library and the binary converges
//! on. `AppError` is `thiserror`-backed so each variant carries its own user-facing
//! `Display` message (no stack traces, no raw errors on any user-facing path) and its own
//! exit code, mapped in `exit_code.rs`.
//!
//! Every unclassified error -- anything reaching `?` from `nostr_sdk`, `std::io`, or any
//! other dependency already returning `Box<dyn std::error::Error>` -- converts into
//! `AppError::Other` automatically via `#[from]`, so existing `?` call sites throughout
//! `lib.rs` and `report/render` needed no per-call-site change.

pub mod exit_code;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Invalid public key format.")]
    InvalidPubkey,

    /// A single failed relay among several that succeeded is a warning, not this
    /// variant, which only fires when every configured relay fails to connect.
    #[error("None of the configured relays could be reached.")]
    RelaysUnreachable(Vec<crate::fetch::client::RelayConnectFailure>),

    #[error("{0}")]
    UsageError(String),

    /// Distinct from a fetch that returns events with, say, zero successful orders:
    /// that is a valid, reportable state, not this variant.
    #[error(
        "No usable dev-fee, order, dispute, or instance-status events were found for this node."
    )]
    NoUsableEvents,

    /// The catch-all for any failure not covered by another variant. `#[from]` matches
    /// the `Box<dyn std::error::Error>` that every existing `nostr_sdk`/`std::io` call
    /// site already produces via `?`, so no call site needed rewriting to adopt this
    /// taxonomy.
    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error>),
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        AppError::Other(Box::new(error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_pubkey_message_matches_the_user_facing_text() {
        assert_eq!(
            AppError::InvalidPubkey.to_string(),
            "Invalid public key format."
        );
    }

    #[test]
    fn relays_unreachable_message_matches_the_user_facing_text() {
        assert_eq!(
            AppError::RelaysUnreachable(vec![]).to_string(),
            "None of the configured relays could be reached."
        );
    }

    #[test]
    fn usage_error_message_carries_its_own_text_through() {
        assert_eq!(
            AppError::UsageError("--force requires --init-config".to_string()).to_string(),
            "--force requires --init-config"
        );
    }

    #[test]
    fn no_usable_events_message_matches_the_user_facing_text() {
        assert_eq!(
            AppError::NoUsableEvents.to_string(),
            "No usable dev-fee, order, dispute, or instance-status events were found for this node."
        );
    }

    #[test]
    fn other_wraps_and_forwards_display_from_a_boxed_error() {
        let source: Box<dyn std::error::Error> = "boom".into();
        let err = AppError::from(source);

        assert_eq!(err.to_string(), "boom");
    }
}
