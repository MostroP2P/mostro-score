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
use models::order::aggregate_order_events;
use nostr_sdk::prelude::*;
use report::render::console;
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
    console::render_identity_header(out, public_key)?;

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
    console::render_connecting_message(err)?;

    // 4. Fetch Both Event Types
    let events: Vec<Event> = event_source.fetch(public_key).await?;

    console::render_fetched_count(err, events.len())?;
    console::render_sample_events(err, &events)?;

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
    let fetch_outcome = compute_relay_fetch_outcome(
        dev_fee_event_count,
        order_event_count,
        &order_aggregate,
        partitioned.instance_status_events,
        partitioned.dispute_events,
        &public_key,
    );
    if fetch_outcome.has_no_usable_events() {
        return Err(AppError::NoUsableEvents);
    }

    console::render_partition_summary(out, dev_fee_event_count, order_event_count)?;

    let dev_fee_aggregate = aggregate_dev_fee_events(dev_fee_events);
    console::render_dev_fee_section(
        out,
        err,
        &dev_fee_aggregate,
        order_aggregate.successful_orders > 0,
    )?;

    // 5. Analyze orders (already aggregated above, for the exit-4 gate)
    console::render_order_debug_section(err, &order_aggregate)?;

    // 6. Output Report
    let now = report_generated_at.timestamp();

    // Every core reputation metric (001 FR-001 through FR-005, FR-010), including every
    // not-applicable edge case (e.g. neither a dev-fee anchor nor a qualifying successful
    // order), is assembled here rather than short-circuited: a node whose only usable
    // data is a dispute or instance-status event (PR 3) must still receive a full report.
    let metrics = NodeMetrics::compute(
        dev_fee_aggregate.first_dev_fee_ts,
        order_aggregate.successful_orders,
        order_aggregate.total_volume_sats,
        &order_aggregate.trade_amounts,
        &order_aggregate.successful_trade_timestamps,
        now,
    );

    console::render_report_header(out, public_key)?;
    console::render_longevity_section(
        out,
        metrics.longevity.days_active,
        metrics.longevity.first_seen_at,
    )?;
    console::render_liveness_section(
        out,
        metrics.liveness.last_successful_trade_at,
        now,
        metrics.liveness.days_since_last_trade,
    )?;
    console::render_recent_activity_section(
        out,
        metrics.liveness.trades_last_7d,
        metrics.liveness.trades_last_30d,
        metrics.liveness.trades_last_90d,
    )?;
    console::render_activity_consistency_section(
        out,
        metrics.consistency.active_days_last_30d,
        metrics.consistency.max_consecutive_inactive_days_last_30d,
    )?;
    console::render_cumulative_performance_section(
        out,
        metrics.cumulative.total_successful_trades,
        metrics.cumulative.total_volume_sats,
    )?;
    console::render_trade_statistics_section(
        out,
        metrics.trade_size.min_trade_sats,
        metrics.trade_size.max_trade_sats,
        metrics.trade_size.mean_trade_sats,
        metrics.trade_size.median_trade_sats,
    )?;
    let score = stats::calculate_score(
        metrics.cumulative.total_successful_trades,
        metrics.cumulative.total_volume_sats,
        metrics.longevity.days_active.unwrap_or(0.0),
    );
    console::render_trust_score_section(out, score)?;

    Ok(())
}
