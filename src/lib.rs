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
use report::render::{console, json, plain, Format, RunOptions};
use stats::grid::{compute_activity_grid, wide_range_warning_message, GridOrder, GridRange};
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
    options: &RunOptions,
) -> Result<(), AppError> {
    // 2/3. Setup Client, add relays, connect — matches the original code's ordering:
    // a malformed relay fails here, before "Connected to relays" ever prints. T067/T069:
    // a total outage (every configured relay failed) is fatal; one or more failures among
    // relays that did connect is a warning to `err`, not a failure (Technical Context).
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
    // PR 7d: transient status, not report content (002 FR-017) — written directly as a
    // plain `writeln!` to `err`, matching the relay-warning loop above, now that the
    // ad hoc `console::render_connecting_message` wrapper it used to go through is
    // removed along with the rest of PR 1's pre-`Report` renderer. PR 9 (003 FR-012):
    // `--quiet` suppresses this line and the "Fetched N events" line below — both are
    // transient status narration, never report content, so they are the only two lines
    // `options.quiet` affects; the relay-warning loop above and the no-dev-fee-anchor
    // warning below are diagnostic facts, not transient narration, and are never
    // suppressed.
    if !options.quiet {
        writeln!(
            err,
            "Connected to relays. Fetching history... (this might take a moment)"
        )?;
    }

    // 4. Fetch every scoped event kind
    let events: Vec<Event> = event_source.fetch(public_key).await?;
    if !options.quiet {
        writeln!(err, "Fetched {} events. Analyzing...", events.len())?;
    }

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
    // PR 10 (003 FR-004): the activity grid's own range, resolved from `options.since`/
    // `options.until` — `cli::options` has already resolved everything explicitly given.
    // Two deferred defaults are resolved here, now that both `grid_orders` and
    // `report_generated_at` are available: `--until` alone defers `since` to the node's
    // earliest order (a data-dependent value `cli::options` cannot know); `--since` alone
    // defers `until` to this same `report_generated_at` instant, deliberately, rather
    // than an earlier `now` `cli::options` would have had to read before any relay was
    // even queried — using that earlier instant here would let it silently diverge from
    // the one used for FR-014's future-event exclusion and the report's own
    // `generated_at`. General statistics (above) are computed over the node's full
    // history, entirely unaffected by this range.
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
    // 003 FR-005a/FR-007: fires when `--view`/`--since`/`--until` combine to force a
    // daily grid over a wide range. A diagnostic fact about the requested range, not
    // transient status narration, so it is never suppressed by `--quiet`, matching the
    // relay-warning loop above.
    if let (Some(granularity), Some(range_start), Some(range_end)) = (
        activity_grid.granularity,
        activity_grid.range_start,
        activity_grid.range_end,
    ) {
        if let Some(warning) = wide_range_warning_message(granularity, range_start, range_end) {
            writeln!(err, "{warning}")?;
        }
    }

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
    // PR 9 (003 FR-010): dispatches on the resolved `Format` instead of always rendering
    // console — `options.color_override` only ever affects `Format::Console`; the plain
    // and JSON renderers never look at color at all.
    match options.format {
        Format::Console => console::render(out, &report, options.color_override)?,
        Format::Plain => plain::render(out, &report)?,
        Format::Json => json::render(out, &report)?,
    }

    Ok(())
}
