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

use fetch::client::EventSource;
use models::core::partition_by_z_y_tag;
use models::dev_fee::aggregate_dev_fee_events;
use models::order::aggregate_order_events;
use nostr_sdk::prelude::*;
use report::render::console;
use stats::lifecycle::{compute_activity_consistency, compute_rolling_windows};
use stats::trade_size::compute_trade_stats;

#[derive(Debug, Default)]
struct MostroStats {
    successful_orders: usize,
    total_volume_sats: u64,
    first_dev_fee_ts: Option<i64>, // From oldest kind 8383 z=dev-fee-payment event
    first_order_ts: i64,           // First order timestamp
    last_order_ts: i64,            // Last order timestamp
    // Trade amount statistics (Section 4.1.3)
    trade_amounts: Vec<u64>,
    // Rolling window data (Section 4.2.2)
    successful_trade_timestamps: Vec<i64>,
}

/// PR 1 Step D: the module move — the wiring-only wrapped function relocated here as
/// this crate's public entry point, keeping the clock call at the same logical point.
/// Same `Result` alias as Step 0's wrap (`Result<T, Box<dyn std::error::Error>>` via
/// `nostr_sdk::prelude::*`); PR 2's T061/T069 later swap it for `AppError`.
#[allow(clippy::too_many_arguments)]
pub async fn run<E: EventSource>(
    public_key: PublicKey,
    event_source: E,
    now: &dyn Fn() -> chrono::DateTime<chrono::Utc>,
    out: &mut impl std::io::Write,
    err: &mut impl std::io::Write,
) -> Result<()> {
    let _ = &err; // no diagnostic routing to `err` yet — that is PR 2's job.

    console::render_identity_header(out, public_key)?;

    // 2/3. Setup Client, add relays, connect — matches the original code's ordering:
    // a malformed relay fails here, before "Connected to relays" ever prints.
    event_source.connect().await?;
    console::render_connecting_message(out)?;

    // 4. Fetch Both Event Types
    let events: Vec<Event> = event_source.fetch(public_key).await?;

    console::render_fetched_count(out, events.len())?;
    console::render_sample_events(out, &events)?;

    // Separate dev fee events and order events
    let (dev_fee_events, order_events) = partition_by_z_y_tag(events);
    console::render_partition_summary(out, dev_fee_events.len(), order_events.len())?;

    // Process dev fee events to get instance start timestamp
    let mut stats = MostroStats::default();

    let dev_fee_aggregate = aggregate_dev_fee_events(dev_fee_events);
    stats.first_dev_fee_ts = dev_fee_aggregate.first_dev_fee_ts;
    console::render_dev_fee_section(out, &dev_fee_aggregate)?;

    // 5. Analyze orders
    let order_aggregate = aggregate_order_events(order_events);
    console::render_order_debug_section(out, &order_aggregate)?;
    stats.first_order_ts = order_aggregate.first_order_ts;
    stats.last_order_ts = order_aggregate.last_order_ts;
    stats.successful_orders = order_aggregate.successful_orders;
    stats.total_volume_sats = order_aggregate.total_volume_sats;
    stats.trade_amounts = order_aggregate.trade_amounts;
    stats.successful_trade_timestamps = order_aggregate.successful_trade_timestamps;

    // 6. Output Report
    let now = now().timestamp();

    // Calculate days_active from dev fee events or fallback to orders
    let (days_active, instance_started) = match stats.first_dev_fee_ts {
        Some(start_ts) => {
            let days = (now - start_ts) as f64 / 86400.0;
            (days, Some(start_ts))
        }
        None => {
            // Fallback: use order timestamps
            if stats.last_order_ts == 0 {
                console::render_no_events_found(out)?;
                return Ok(());
            }
            let days = (stats.last_order_ts - stats.first_order_ts) as f64 / 86400.0;
            (days, None)
        }
    };

    // Compute all derived metrics
    let (min_trade, max_trade, mean_trade, median_trade) =
        compute_trade_stats(&stats.trade_amounts);
    let (trades_7d, trades_30d, trades_90d) =
        compute_rolling_windows(&stats.successful_trade_timestamps, now);
    let (active_days_30d, max_inactive_gap) =
        compute_activity_consistency(&stats.successful_trade_timestamps, now);
    let days_since_last = if stats.last_order_ts > 0 {
        ((now - stats.last_order_ts) as f64 / 86400.0).floor() as u64
    } else {
        0
    };

    console::render_report_header(out, public_key)?;
    console::render_longevity_section(out, days_active, instance_started)?;
    console::render_liveness_section(out, stats.last_order_ts, now, days_since_last)?;
    console::render_recent_activity_section(out, trades_7d, trades_30d, trades_90d)?;
    console::render_activity_consistency_section(out, active_days_30d, max_inactive_gap)?;
    console::render_cumulative_performance_section(
        out,
        stats.successful_orders,
        stats.total_volume_sats,
    )?;
    console::render_trade_statistics_section(
        out,
        stats.trade_amounts.is_empty(),
        min_trade,
        max_trade,
        mean_trade,
        median_trade,
    )?;
    let score = stats::calculate_score(
        stats.successful_orders,
        stats.total_volume_sats,
        days_active,
    );
    console::render_trust_score_section(out, score)?;

    Ok(())
}
