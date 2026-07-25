mod fetch;
mod models;
mod report;
mod stats;

use clap::Parser;
use fetch::client::{EventSource, RelayEventSource};
use models::core::partition_by_z_y_tag;
use models::dev_fee::aggregate_dev_fee_events;
use models::order::aggregate_order_events;
use nostr_sdk::prelude::*;
use report::render::console;
use stats::lifecycle::{compute_activity_consistency, compute_rolling_windows};
use stats::trade_size::compute_trade_stats;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Mostro Pubkey (npub or hex) to analyze
    #[arg(short, long)]
    pubkey: String,

    /// Relays to connect to (comma separated)
    #[arg(short, long, default_value = "wss://relay.mostro.network")]
    relays: String,
}

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

/// PR 1 Step 0: today's `main()` body, extracted verbatim into an ordinary (not
/// `#[cfg(test)]`-gated) private function, callable by production `main()` in a normal
/// `cargo build`. Five parameters replace the one caller-supplied input and the three
/// hidden dependencies the original body had (relay query, clock read, and the
/// `println!`/`eprintln!` calls — the writers count as two, one per stream). This is a
/// purely mechanical wrap: no logic changes, no output changes.
#[allow(clippy::too_many_arguments)]
async fn run<E: EventSource>(
    public_key: PublicKey,
    event_source: E,
    now: &dyn Fn() -> chrono::DateTime<chrono::Utc>,
    out: &mut impl std::io::Write,
    err: &mut impl std::io::Write,
) -> Result<()> {
    let _ = &err; // no diagnostic routing to `err` yet — that is PR 2's job.

    console::render_identity_header(out, public_key)?;
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
    let score = stats::calculate_score(&stats, days_active);
    console::render_trust_score_section(out, score)?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();

    // 1. Parse Pubkey
    let public_key = match PublicKey::parse(&args.pubkey) {
        Ok(pk) => pk,
        Err(_) => {
            eprintln!("Error: Invalid public key format.");
            return Ok(());
        }
    };

    let relays: Vec<String> = args.relays.split(',').map(|s| s.to_string()).collect();
    let event_source = RelayEventSource { relays };
    let now = chrono::Utc::now;

    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();

    run(public_key, event_source, &now, &mut stdout, &mut stderr).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Fixture `EventSource`: replays a canned event set (captured once from a real
    /// relay round trip, Step -1) instead of querying relays. No network access.
    struct FixtureEventSource {
        events: Vec<Event>,
    }

    impl EventSource for FixtureEventSource {
        async fn fetch(&self, _public_key: PublicKey) -> Result<Vec<Event>> {
            Ok(self.events.clone())
        }
    }

    fn load_fixture_events(path: &str) -> Vec<Event> {
        let raw = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
        raw.lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| Event::from_json(l).unwrap_or_else(|e| panic!("parse event line: {e}")))
            .collect()
    }

    /// Normalizes the "Status distribution for order events (s tag):" block: its lines
    /// come from `HashMap` iteration, so their order is nondeterministic across process
    /// runs, independent of any code change. Both sides of a comparison must have that
    /// block's lines sorted before the rest of the text is compared byte-for-byte.
    fn normalize_s_tag_distribution(text: &str) -> String {
        let lines: Vec<&str> = text.lines().collect();
        let mut normalized: Vec<String> = Vec::with_capacity(lines.len());
        let mut i = 0;
        while i < lines.len() {
            let line = lines[i];
            normalized.push(line.to_string());
            if line == "Status distribution for order events (s tag):" {
                i += 1;
                let mut block: Vec<&str> = Vec::new();
                while i < lines.len() && lines[i].trim_start().starts_with("s=") {
                    block.push(lines[i]);
                    i += 1;
                }
                block.sort_unstable();
                normalized.extend(block.into_iter().map(|s| s.to_string()));
                continue;
            }
            i += 1;
        }
        normalized.join("\n")
    }

    #[tokio::test]
    async fn wrapped_run_matches_step_minus_1_golden_scenario_1() {
        let fixture_events = load_fixture_events("tests/fixtures/scenario1_events.ndjson");
        let expected_stdout = fs::read_to_string("tests/fixtures/scenario1_stdout.txt")
            .expect("read golden stdout capture");
        let expected_stderr = fs::read_to_string("tests/fixtures/scenario1_stderr.txt")
            .expect("read golden stderr capture");
        let now_pre: i64 = fs::read_to_string("tests/fixtures/scenario1_now_pre.txt")
            .expect("read golden now")
            .trim()
            .parse()
            .expect("valid now timestamp");

        let public_key =
            PublicKey::parse("82fa8cb978b43c79b2156585bac2c011176a21d2aead6d9f7c575c005be88390")
                .unwrap();
        let event_source = FixtureEventSource {
            events: fixture_events,
        };
        let frozen_now =
            move || chrono::DateTime::<chrono::Utc>::from_timestamp(now_pre, 0).unwrap();

        let mut out: Vec<u8> = Vec::new();
        let mut err: Vec<u8> = Vec::new();

        run(public_key, event_source, &frozen_now, &mut out, &mut err)
            .await
            .expect("run() succeeds against the fixture event source");

        let actual_stdout = String::from_utf8(out).expect("stdout is valid utf-8");
        let actual_stderr = String::from_utf8(err).expect("stderr is valid utf-8");

        assert_eq!(
            normalize_s_tag_distribution(&actual_stdout),
            normalize_s_tag_distribution(&expected_stdout),
            "wrapped run() stdout must match Step -1's golden scenario 1 capture, \
             modulo HashMap-ordered s_tag_distribution lines"
        );
        assert_eq!(
            actual_stderr, expected_stderr,
            "wrapped run() stderr must match Step -1's golden scenario 1 capture"
        );
    }

    // Step B's dedup/dev-fee-selection/z-y-partitioning characterization tests moved to
    // `src/models/dedup.rs`, `src/models/dev_fee.rs`, and `src/models/core.rs`
    // respectively, alongside the functions they cover (T023-T025).

    // Step A's compute_trade_stats, compute_rolling_windows/compute_activity_consistency,
    // format_relative_time, and calculate_score characterization tests moved to
    // `src/stats/trade_size.rs`, `src/stats/lifecycle.rs`, `src/report/format.rs`, and
    // `src/stats/mod.rs` respectively, alongside the functions they cover (T033-T036).
}
