//! Stub scaffolding for kind `38385` instance-status events. Base `src/main.rs` had no
//! instance-status aggregation to move verbatim; the real selection logic (`d` = node
//! pubkey, highest `created_at`) is implemented in PR 3 (T088-T089).

use nostr_sdk::prelude::*;

/// Placeholder instance-status type; fields land with PR 3's selection logic.
#[allow(dead_code)]
pub struct InstanceStatusEvent {
    pub event: Event,
}
