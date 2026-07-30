pub mod core;
pub mod dedup;
pub mod dev_fee;
pub mod dispute;
pub mod instance_status;
pub mod order;

/// Shared test-only event-construction helper, used by every `models` submodule's
/// characterization tests to build hand-crafted, signed `Event`s without a real relay
/// round trip.
#[cfg(test)]
pub(crate) mod test_support {
    use nostr_sdk::prelude::*;

    pub(crate) fn make_event(kind: u16, created_at: u64, tags: Vec<(&str, &str)>) -> Event {
        let keys = Keys::generate();
        make_event_with_keys(&keys, kind, created_at, tags)
    }

    /// Same as `make_event`, signed with a caller-supplied key pair, so tests exercising
    /// author scoping can build multiple events sharing (or deliberately not sharing)
    /// the same signer.
    pub(crate) fn make_event_with_keys(
        keys: &Keys,
        kind: u16,
        created_at: u64,
        tags: Vec<(&str, &str)>,
    ) -> Event {
        let parsed_tags: Vec<Tag> = tags
            .into_iter()
            .map(|(name, value)| Tag::parse([name, value]).expect("valid tag"))
            .collect();
        EventBuilder::new(Kind::Custom(kind), "")
            .tags(parsed_tags)
            .custom_created_at(Timestamp::from(created_at))
            .sign_with_keys(keys)
            .expect("event signs")
    }
}
