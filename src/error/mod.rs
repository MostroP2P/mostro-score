//! PR 2 (T060-T061): the typed error taxonomy every fallible path in the library and the
//! binary converges on. `AppError` is `thiserror`-backed so each variant carries its own
//! user-facing `Display` message (Principle VI: no stack traces, no raw errors on any
//! user-facing path) and its own exit code, mapped in `exit_code.rs`.
//!
//! Every unclassified error — anything reaching `?` from `nostr_sdk`, `std::io`, or any
//! other dependency already returning `Box<dyn std::error::Error>` (the crate-wide alias
//! `nostr_sdk::prelude::Result` uses) — converts into `AppError::Other` automatically via
//! `#[from]`, so existing `?` call sites throughout `lib.rs` and `report/render` needed no
//! per-call-site change. Exit code `4` (`AppError::NoUsableEvents`) is out of scope here:
//! it needs the four-kind event scoping PR 3 introduces to know what "usable" means.

pub mod exit_code;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    /// 002 FR-019 exit code `5`: a syntactically supplied `--pubkey` that fails format
    /// validation. Message text matches the JSON fatal-error envelope's `invalid_pubkey`
    /// entry (plan.md's JSON output contract section), so the console and JSON paths
    /// eventually say the same thing.
    #[error("Invalid public key format.")]
    InvalidPubkey,

    /// 002 FR-019 exit code `3`: every configured relay failed to connect. A single failed
    /// relay among several that succeeded is a warning (Technical Context), not this
    /// variant.
    #[error("None of the configured relays could be reached.")]
    RelaysUnreachable,

    /// 002 FR-019 exit code `2`: a usage error detected by application code after argument
    /// parsing succeeds (e.g. `--force` without `--init-config`, `--since` after
    /// `--until`). No PR-2 call site constructs this yet — the flags it validates land in
    /// PR 9/PR 10/PR 12 — but the exit-code mapping is part of this PR's scope (T062).
    #[error("{0}")]
    UsageError(String),

    /// 002 FR-019 exit code `4`: a relay fetch that returns zero usable events across
    /// all four scoped kinds (dev-fee, order, dispute, instance-status) — nothing about
    /// this node can be reported. Distinct from a fetch that returns events with, say,
    /// zero successful orders: that is a valid, reportable state (FR-002/FR-003), not
    /// this variant.
    #[error(
        "No usable dev-fee, order, dispute, or instance-status events were found for this node."
    )]
    NoUsableEvents,

    /// 002 FR-019 exit code `1`: the catch-all for any failure not covered by another
    /// variant. `#[from]` matches the `Box<dyn std::error::Error>` that every existing
    /// `nostr_sdk`/`std::io` call site already produces via `?`, so no call site needed
    /// rewriting to adopt this taxonomy.
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
            AppError::RelaysUnreachable.to_string(),
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
