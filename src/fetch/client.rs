use nostr_sdk::prelude::*;
use nostr_sdk::{Alphabet, SingleLetterTag};
use std::time::Duration;

/// Dev-fee payment event kind (not in mostro-core)
const DEV_FEE_EVENT_KIND: u16 = 8383;

/// Seam introduced by PR 1 Step 0: the event-fetching surface `run()` depends on, so
/// production code and tests can supply different implementations (real relays vs. a
/// fixture replaying a captured event set). A generic bound, not `&dyn EventSource`,
/// since only two implementations exist and stable async-fn-in-traits needs no boxing
/// for static dispatch.
pub trait EventSource {
    async fn fetch(&self, public_key: PublicKey) -> Result<Vec<Event>>;
}

/// Production `EventSource`: connects to the configured relays and issues the same two
/// filters `main()` used to build inline (dev-fee events, then order events), chaining
/// both result sets into one `Vec<Event>` exactly as before.
pub struct RelayEventSource {
    pub relays: Vec<String>,
}

impl EventSource for RelayEventSource {
    async fn fetch(&self, public_key: PublicKey) -> Result<Vec<Event>> {
        let client = Client::new(Keys::generate());

        for relay in &self.relays {
            client.add_relay(relay.as_str()).await?;
        }

        client.connect().await;

        // Filter 1: Development Fee Events (z=dev-fee-payment, y=mostro)
        let dev_fee_filter = Filter::new()
            .kind(Kind::Custom(DEV_FEE_EVENT_KIND))
            .author(public_key)
            .custom_tag(SingleLetterTag::lowercase(Alphabet::Z), "dev-fee-payment")
            .custom_tag(SingleLetterTag::lowercase(Alphabet::Y), "mostro");

        // Filter 2: Order Events (z=order)
        let order_filter = Filter::new()
            .kind(Kind::PeerToPeerOrder)
            .author(public_key)
            .custom_tag(SingleLetterTag::lowercase(Alphabet::Z), "order");

        let dev_fee_events_result = client
            .fetch_events(dev_fee_filter, Duration::from_secs(10))
            .await?;
        let order_events_result = client
            .fetch_events(order_filter, Duration::from_secs(10))
            .await?;

        Ok(dev_fee_events_result
            .into_iter()
            .chain(order_events_result.into_iter())
            .collect())
    }
}
