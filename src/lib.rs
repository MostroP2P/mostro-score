//! Library target. Holds every module (`cli`, `config`, `error`, `fetch`, `models`,
//! `report`, `stats`) so `tests/` integration tests can exercise them without a
//! binary-only crate. `main.rs` is the thin binary target: argument parsing, wiring,
//! and dispatch only.

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
use report::render::{console, json, plain, Format, RunOptions};
use stats::grid::{compute_activity_grid, wide_range_warning_message, GridOrder, GridRange};
use stats::NodeMetrics;

/// The crate's public entry point. Returns `Result<(), AppError>` directly so every
/// propagated error is already part of the typed taxonomy — `AppError::Other`'s
/// `#[from] Box<dyn Error>` covers anything not already a more specific variant, so
/// `main`'s error handling never needs to downcast or fall back to a raw `Debug` dump.
#[allow(clippy::too_many_arguments)]
pub async fn run<E: EventSource>(
    public_key: PublicKey,
    event_source: E,
    now: &dyn Fn() -> chrono::DateTime<chrono::Utc>,
    out: &mut impl std::io::Write,
    err: &mut impl std::io::Write,
    options: &RunOptions,
) -> Result<(), AppError> {
    // A malformed relay fails here, before "Connected to relays" ever prints. A total
    // outage (every configured relay failed) is fatal; partial failure among relays
    // that did connect is only a warning.
    let connection = event_source.connect().await?;
    if connection.connected_count == 0 {
        return Err(AppError::RelaysUnreachable(connection.failed));
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
    // Transient status, not report content. `--quiet` suppresses this line and the
    // "Fetched N events" line below only — the relay-warning loop above and the
    // no-dev-fee-anchor warning below are diagnostic facts, never suppressed.
    if !options.quiet {
        writeln!(
            err,
            "Connected to relays. Fetching history... (this might take a moment)"
        )?;
    }

    let events: Vec<Event> = event_source.fetch(public_key).await?;
    if !options.quiet {
        writeln!(err, "Fetched {} events. Analyzing...", events.len())?;
    }

    // Captured once and reused for both the future-event exclusion below and the
    // report's own "generated_at": a single consistent instant for the whole run, not
    // two separate clock reads that could disagree if execution takes measurable time.
    let report_generated_at = now();
    let events = exclude_future_events(
        events,
        Timestamp::from(report_generated_at.timestamp() as u64),
    );

    // Routes the four scoped event kinds into their own buckets, then gates on whether
    // any of them yielded usable data at all. A node with zero successful orders but a
    // usable dispute or instance-status event is still a valid, reportable state.
    let partitioned = partition_scoped_events(events, &public_key);
    // Dev-fee events have no `d`-tag replaceable-event semantics of their own (unlike
    // orders/disputes/instance-status), so event-id dedup is their only dedup axis;
    // deduplicated once, here, and reused below so it is never rescanned or recounted.
    let dev_fee_events = dedup_events_by_id(partitioned.dev_fee_events);
    let dev_fee_event_count = dev_fee_events.len();
    let order_event_count = dedup_by_event_id_count(&partitioned.order_events);
    let order_aggregate = aggregate_order_events(partitioned.order_events);
    // Computed once and reused for both the usable-data gate below and this node's
    // dispute stats, rather than aggregated a second time from the same event set.
    let dispute_aggregate = aggregate_dispute_events(partitioned.dispute_events);
    // Computed once and reused for both the usable-data gate below and this node's
    // bond-policy signal, rather than aggregated a second time from the same event set.
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
    // The no-dev-fee-anchor fallback is a diagnostic fact not otherwise visible in the
    // report's `days_active` figure — it explains why that figure is an estimate rather
    // than the primary dev-fee-anchored value — so it stays a stderr warning.
    if dev_fee_aggregate.first_dev_fee_ts.is_none() && order_aggregate.successful_orders > 0 {
        writeln!(
            err,
            "Warning: no dev-fee anchor found; falling back to order timestamps for days_active."
        )?;
    }

    // Computes every core reputation metric, including every not-applicable edge case
    // (e.g. neither a dev-fee anchor nor a qualifying successful order) — a node whose
    // only usable data is a dispute or instance-status event must still receive a full
    // report.
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

    // The activity grid's own input shape, built from the same qualifying successful
    // orders `NodeMetrics::compute` above already consumed — no second scan over the
    // fetched events, since `OrderAggregate::qualifying_orders` was populated in the
    // same aggregation loop as every other `order_aggregate` field.
    let grid_orders: Vec<GridOrder> = order_aggregate
        .qualifying_orders
        .iter()
        .map(|&(created_at, amount_sats)| GridOrder {
            created_at,
            amount_sats,
        })
        .collect();
    // The activity grid's own range, resolved from `options.since`/`options.until` —
    // `cli::options` already resolved everything explicitly given. Two deferred
    // defaults are resolved here, now that `grid_orders` and `report_generated_at` are
    // available: `--until` alone defers `since` to the node's earliest order; `--since`
    // alone defers `until` to this same `report_generated_at` instant, deliberately,
    // rather than an earlier `now` — using an earlier instant here would let it diverge
    // from the one used for future-event exclusion and the report's own `generated_at`.
    // General statistics above cover the node's full history, unaffected by this range.
    let grid_range = match (options.since, options.until) {
        (None, None) => GridRange::Unbounded,
        (None, Some(until)) => match grid_orders.iter().map(|order| order.created_at).min() {
            Some(earliest) => GridRange::Bounded {
                since: earliest,
                until,
            },
            // Nothing to anchor "earliest" on: fall back to inferring the range as
            // normal, rather than inventing a value.
            None => GridRange::Unbounded,
        },
        (Some(since), until) => GridRange::Bounded {
            since,
            until: until.unwrap_or_else(|| report_generated_at.timestamp()),
        },
    };
    let activity_grid = compute_activity_grid(&grid_orders, grid_range, options.view);
    // Fires when `--view`/`--since`/`--until` combine to force a daily grid over a wide
    // range. A diagnostic fact about the requested range, not transient status
    // narration, so it is never suppressed by `--quiet`.
    if let (Some(granularity), Some(range_start), Some(range_end)) = (
        activity_grid.granularity,
        activity_grid.range_start,
        activity_grid.range_end,
    ) {
        if let Some(warning) = wide_range_warning_message(granularity, range_start, range_end) {
            writeln!(err, "{warning}")?;
        }
    }

    // Assembles the complete report. Rendering it below may narrow the filterable
    // sections per `options.sections`; the assembled `Report` itself is always complete.
    let report = assemble_report(
        public_key,
        &connection,
        &fetch_outcome,
        &metrics,
        &activity_grid,
        report_generated_at,
    )?;
    // Dispatches on the resolved `Format`. `options.color_override` only ever affects
    // `Format::Console`; the plain and JSON renderers never look at color at all.
    match options.format {
        Format::Console => {
            console::render(out, &report, options.color_override, &options.sections)?
        }
        Format::Plain => plain::render(out, &report, &options.sections)?,
        Format::Json => json::render(out, &report)?,
    }

    Ok(())
}
