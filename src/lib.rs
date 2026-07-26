//! Library target. Holds every constitution module (`cli`, `config`, `error`, `fetch`,
//! `models`, `report`, `stats`) so `tests/` integration tests can exercise them without a
//! binary-only crate, per the plan's Structure Decision. `main.rs` is the thin binary
//! target: argument parsing, wiring, and dispatch only.

pub mod cli;
pub mod config;
pub mod error;
pub mod fetch;
pub mod models;
pub mod report;
pub mod stats;

use error::AppError;
use fetch::client::EventSource;
use fetch::filters_summary::{
    compute_relay_fetch_outcome, dedup_by_event_id_count, partition_scoped_events,
};
use models::core::exclude_future_events;
use models::dedup::dedup_events_by_id;
use models::dev_fee::aggregate_dev_fee_events;
use models::dispute::aggregate_dispute_events;
use models::instance_status::aggregate_instance_status;
use models::order::aggregate_order_events;
use nostr_sdk::prelude::*;
use report::model::assemble_report;
use report::render::console;
use stats::grid::{compute_activity_grid, GridOrder};
use stats::NodeMetrics;

/// PR 1 Step D: the module move — the wiring-only wrapped function relocated here as
/// this crate's public entry point, keeping the clock call at the same logical point.
/// PR 2 (T061/T069): returns `Result<(), AppError>` directly so every propagated error is
/// already part of the typed taxonomy — `AppError::Other`'s `#[from] Box<dyn Error>` covers
/// anything not already a more specific variant, so `main`'s error handling never needs to
/// downcast or fall back to a raw `Debug` dump.
#[allow(clippy::too_many_arguments)]
pub async fn run<E: EventSource>(
    public_key: PublicKey,
    event_source: E,
    now: &dyn Fn() -> chrono::DateTime<chrono::Utc>,
    out: &mut impl std::io::Write,
    err: &mut impl std::io::Write,
) -> Result<(), AppError> {
    // 2/3. Setup Client, add relays, connect — matches the original code's ordering:
    // a malformed relay fails here, before "Connected to relays" ever prints. T067/T069:
    // a total outage (every configured relay failed) is fatal; one or more failures among
    // relays that did connect is a warning to `err`, not a failure (Technical Context).
    let connection = event_source.connect().await?;
    if connection.connected_count == 0 {
        return Err(AppError::RelaysUnreachable);
    }
    if !connection.failed.is_empty() {
        for failure in &connection.failed {
            writeln!(
                err,
                "Warning: relay {} unreachable: {}",
                failure.url, failure.error
            )?;
        }
    }
    // PR 7d: transient status, not report content (002 FR-017) — written directly as a
    // plain `writeln!` to `err`, matching the relay-warning loop above, now that the
    // ad hoc `console::render_connecting_message` wrapper it used to go through is
    // removed along with the rest of PR 1's pre-`Report` renderer. `--quiet` suppresses
    // this line later (PR 9's T174/T175).
    writeln!(
        err,
        "Connected to relays. Fetching history... (this might take a moment)"
    )?;

    // 4. Fetch every scoped event kind
    let events: Vec<Event> = event_source.fetch(public_key).await?;
    writeln!(err, "Fetched {} events. Analyzing...", events.len())?;

    // Captured once, here, and reused for both FR-014's future-event exclusion below
    // and the report's own "now" (Section 6): a single consistent instant for the
    // whole run, not two separate clock reads that could disagree if execution takes
    // any measurable time.
    let report_generated_at = now();
    let events = exclude_future_events(
        events,
        Timestamp::from(report_generated_at.timestamp() as u64),
    );

    // PR 3 (T095/T096): route the four scoped kinds (001 FR-015) into their own
    // buckets, then gate on whether any of them yielded usable data at all (002
    // FR-019 exit code 4). A node with, say, zero successful orders but a usable
    // dispute or instance-status event is still a valid, reportable state.
    let partitioned = partition_scoped_events(events, &public_key);
    // Dev-fee events have no `d`-tag replaceable-event semantics of their own (unlike
    // orders/disputes/instance-status), so event-id dedup is their only dedup axis;
    // deduplicated once, here, and reused below so it is never rescanned or recounted.
    let dev_fee_events = dedup_events_by_id(partitioned.dev_fee_events);
    let dev_fee_event_count = dev_fee_events.len();
    let order_event_count = dedup_by_event_id_count(&partitioned.order_events);
    let order_aggregate = aggregate_order_events(partitioned.order_events);
    // PR 5: computed once here and reused both for the exit-4 gate below and for this
    // node's dispute stats (FR-006), rather than aggregated a second time from the
    // same event set.
    let dispute_aggregate = aggregate_dispute_events(partitioned.dispute_events);
    // PR 6: computed once here and reused both for the exit-4 gate below and for this
    // node's bond-policy signal (FR-012), rather than aggregated a second time from the
    // same event set.
    let instance_status_aggregate =
        aggregate_instance_status(partitioned.instance_status_events, &public_key);
    let fetch_outcome = compute_relay_fetch_outcome(
        dev_fee_event_count,
        order_event_count,
        &order_aggregate,
        &instance_status_aggregate,
        &dispute_aggregate,
    );
    if fetch_outcome.has_no_usable_events() {
        return Err(AppError::NoUsableEvents);
    }

    let dev_fee_aggregate = aggregate_dev_fee_events(dev_fee_events);
    // PR 7d: the no-dev-fee-anchor fallback is a diagnostic fact not otherwise visible
    // in the report's plain `days_active` figure (it explains *why* that figure is an
    // estimate rather than the primary dev-fee-anchored value), so it stays a stderr
    // warning even though the ad hoc "Found N dev fee events"/"MOSTRO TRADING ACTIVITY"
    // console output it used to accompany is removed entirely — that information is now
    // covered by the report's own fetch and longevity sections.
    if dev_fee_aggregate.first_dev_fee_ts.is_none() && order_aggregate.successful_orders > 0 {
        writeln!(
            err,
            "Warning: no dev-fee anchor found; falling back to order timestamps for days_active."
        )?;
    }

    // 6. Compute every core reputation metric (001 FR-001 through FR-006, FR-010),
    // including every not-applicable edge case (e.g. neither a dev-fee anchor nor a
    // qualifying successful order) — a node whose only usable data is a dispute or
    // instance-status event (PR 3) must still receive a full report.
    let now = report_generated_at.timestamp();
    let metrics = NodeMetrics::compute(
        dev_fee_aggregate.first_dev_fee_ts,
        order_aggregate.successful_orders,
        order_aggregate.total_volume_sats,
        &order_aggregate.trade_amounts,
        &order_aggregate.successful_trade_timestamps,
        dispute_aggregate.total_disputes,
        dispute_aggregate.resolved,
        dispute_aggregate.active,
        dispute_aggregate.unknown,
        &order_aggregate.fiat_values,
        &order_aggregate.payment_method_mentions,
        &order_aggregate.premium_values,
        instance_status_aggregate
            .bond_enabled
            .as_bond_policy_status(),
        now,
    );

    // PR 7d (002 FR-004): the activity grid's own input shape, built from the same
    // qualifying successful orders `NodeMetrics::compute` above already consumed — no
    // second scan over the fetched events, since `OrderAggregate::qualifying_orders`
    // was populated in the same aggregation loop as every other `order_aggregate` field.
    let grid_orders: Vec<GridOrder> = order_aggregate
        .qualifying_orders
        .iter()
        .map(|&(created_at, amount_sats)| GridOrder {
            created_at,
            amount_sats,
        })
        .collect();
    let activity_grid = compute_activity_grid(&grid_orders);

    // 7. Assemble and render the complete 5-section report (002 FR-001), replacing PR
    // 1's ad hoc per-section `console::render_*` pipeline entirely.
    let report = assemble_report(
        public_key,
        &connection,
        &fetch_outcome,
        &metrics,
        &activity_grid,
        report_generated_at,
    )?;
    console::render(out, &report, None)?;

    Ok(())
}
