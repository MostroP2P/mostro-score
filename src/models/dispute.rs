//! Stub scaffolding for kind `38386` dispute events. Base `src/main.rs` had no dispute
//! aggregation to move verbatim; the real dedup-by-`d`-tag and resolved/active/unknown
//! classification logic is implemented in PR 3 (T086-T087).

use nostr_sdk::prelude::*;

/// Placeholder dispute type; fields land with PR 3's classification logic.
#[allow(dead_code)]
pub struct DisputeEvent {
    pub event: Event,
}
