mod fetch;
mod models;

use clap::Parser;
use colored::Colorize;
use fetch::client::{EventSource, RelayEventSource};
use models::core::partition_by_z_y_tag;
use models::dev_fee::aggregate_dev_fee_events;
use models::order::aggregate_order_events;
use nostr_sdk::prelude::*;
use std::collections::HashSet;

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

    writeln!(out, "Analyzing Mostro Node: {}", public_key.to_bech32()?)?;
    writeln!(out, "Hex: {}", public_key.to_hex())?;

    writeln!(
        out,
        "Connected to relays. Fetching history... (this might take a moment)"
    )?;

    // 4. Fetch Both Event Types
    let events: Vec<Event> = event_source.fetch(public_key).await?;

    writeln!(out, "Fetched {} events. Analyzing...", events.len())?;

    // Print sample events to understand structure
    writeln!(out, "\n=== SAMPLE EVENTS (first 3) ===")?;
    for (idx, event) in events.iter().take(3).enumerate() {
        writeln!(out, "\nEvent #{}", idx + 1)?;
        writeln!(out, "  ID: {}", event.id)?;
        writeln!(out, "  created_at: {}", event.created_at)?;
        writeln!(out, "  Tags:")?;
        for tag in event.tags.iter() {
            writeln!(out, "    {:?}", tag.as_slice())?;
        }
    }
    writeln!(out, "==============================\n")?;

    // Separate dev fee events and order events
    let (dev_fee_events, order_events) = partition_by_z_y_tag(events);

    writeln!(
        out,
        "Found {} dev fee events and {} order events",
        dev_fee_events.len(),
        order_events.len()
    )?;

    // Process dev fee events to get instance start timestamp
    let mut stats = MostroStats::default();

    let dev_fee_aggregate = aggregate_dev_fee_events(dev_fee_events);
    stats.first_dev_fee_ts = dev_fee_aggregate.first_dev_fee_ts;
    if let Some(first_dev_fee_ts) = dev_fee_aggregate.first_dev_fee_ts {
        writeln!(out, "\n=== MOSTRO TRADING ACTIVITY ===")?;
        writeln!(
            out,
            "First dev fee payment: {}",
            chrono::DateTime::from_timestamp(first_dev_fee_ts, 0).unwrap_or_default()
        )?;
        writeln!(out, "Total dev fee events: {}", dev_fee_aggregate.count)?;
        writeln!(out, "================================\n")?;
    } else {
        writeln!(
            out,
            "\n⚠ Warning: No dev fee events found (z=dev-fee-payment, y=mostro)."
        )?;
        writeln!(
            out,
            "Falling back to order timestamps for days_active calculation.\n"
        )?;
    }

    // 5. Analyze orders
    let order_aggregate = aggregate_order_events(order_events);
    stats.first_order_ts = order_aggregate.first_order_ts;
    stats.last_order_ts = order_aggregate.last_order_ts;
    stats.successful_orders = order_aggregate.successful_orders;
    stats.total_volume_sats = order_aggregate.total_volume_sats;
    stats.trade_amounts = order_aggregate.trade_amounts;
    stats.successful_trade_timestamps = order_aggregate.successful_trade_timestamps;

    // Print debug information
    writeln!(out, "\n=== DEBUG INFORMATION ===")?;
    writeln!(
        out,
        "Total order events fetched: {}",
        order_aggregate.total_order_count
    )?;
    writeln!(
        out,
        "Unique orders after deduplication: {}",
        order_aggregate.unique_order_count
    )?;

    if !order_aggregate.s_tag_distribution.is_empty() {
        writeln!(out, "\nStatus distribution for order events (s tag):")?;
        for (status, count) in order_aggregate.s_tag_distribution.iter() {
            writeln!(out, "  s='{}': {} events", status, count)?;
        }
    } else {
        writeln!(out, "\nNo order events found with s tags")?;
    }
    writeln!(out, "========================\n")?;

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
                writeln!(out, "No events found.")?;
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

    // Header
    writeln!(
        out,
        "\n{}",
        "========================================".cyan()
    )?;
    writeln!(
        out,
        "{}",
        "     MOSTRO NODE REPUTATION REPORT     ".cyan().bold()
    )?;
    writeln!(out, "{}", "========================================".cyan())?;
    writeln!(out, "Node: {}", public_key.to_bech32()?)?;

    // Section: Longevity (4.1.1)
    writeln!(
        out,
        "{}",
        "----------------------------------------".dimmed()
    )?;
    writeln!(out, "{}", "LONGEVITY".bold())?;
    if let Some(start_ts) = instance_started {
        writeln!(
            out,
            "  First Activity:  {}",
            chrono::DateTime::from_timestamp(start_ts, 0).unwrap_or_default()
        )?;
        writeln!(out, "  Days Active:     {:.1} days", days_active)?;
    } else {
        writeln!(
            out,
            "  {} Days Active:     {:.1} days (estimated from orders)",
            "⚠".yellow(),
            days_active
        )?;
    }

    // Section: Liveness (4.2.1) - PROMINENT per spec
    writeln!(
        out,
        "{}",
        "----------------------------------------".dimmed()
    )?;
    writeln!(out, "{}", "LIVENESS".bold())?;
    if stats.last_order_ts > 0 {
        let relative_time = format_relative_time(stats.last_order_ts, now);
        let last_trade_display = format!(
            "  Last Trade:      {} ({})",
            chrono::DateTime::from_timestamp(stats.last_order_ts, 0).unwrap_or_default(),
            relative_time
        );

        // Color based on activity status
        if days_since_last > 30 {
            writeln!(out, "{}", last_trade_display.red())?;
            writeln!(
                out,
                "  Days Since Last: {} {}",
                days_since_last,
                "INACTIVE".red().bold()
            )?;
        } else if days_since_last > 7 {
            writeln!(out, "{}", last_trade_display.yellow())?;
            writeln!(
                out,
                "  Days Since Last: {} {}",
                days_since_last,
                "LOW ACTIVITY".yellow()
            )?;
        } else {
            writeln!(out, "{}", last_trade_display.green())?;
            writeln!(
                out,
                "  Days Since Last: {} {}",
                days_since_last,
                "ACTIVE".green()
            )?;
        }
    } else {
        writeln!(out, "  {} No successful trades recorded", "⚠".yellow())?;
    }

    // Section: Rolling Windows (4.2.2)
    writeln!(
        out,
        "{}",
        "----------------------------------------".dimmed()
    )?;
    writeln!(out, "{}", "RECENT ACTIVITY".bold())?;
    writeln!(out, "  Last 7 days:     {} trades", trades_7d)?;
    writeln!(out, "  Last 30 days:    {} trades", trades_30d)?;
    writeln!(out, "  Last 90 days:    {} trades", trades_90d)?;

    // Section: Activity Consistency (4.2.3)
    writeln!(
        out,
        "{}",
        "----------------------------------------".dimmed()
    )?;
    writeln!(out, "{}", "ACTIVITY CONSISTENCY (30 days)".bold())?;
    writeln!(out, "  Active Days:     {}/30", active_days_30d)?;
    if max_inactive_gap > 7 {
        writeln!(
            out,
            "  Max Inactive Gap: {} days {}",
            max_inactive_gap,
            "⚠".yellow()
        )?;
    } else {
        writeln!(out, "  Max Inactive Gap: {} days", max_inactive_gap)?;
    }

    // Section: Cumulative Performance (4.1.2)
    writeln!(
        out,
        "{}",
        "----------------------------------------".dimmed()
    )?;
    writeln!(out, "{}", "CUMULATIVE PERFORMANCE".bold())?;
    writeln!(out, "  Successful Trades: {}", stats.successful_orders)?;
    writeln!(
        out,
        "  Total Volume:      {} sats ({:.4} BTC)",
        stats.total_volume_sats,
        stats.total_volume_sats as f64 / 100_000_000.0
    )?;

    // Section: Trade Statistics (4.1.3)
    if !stats.trade_amounts.is_empty() {
        writeln!(
            out,
            "{}",
            "----------------------------------------".dimmed()
        )?;
        writeln!(out, "{}", "TRADE STATISTICS".bold())?;
        writeln!(out, "  Min Trade:       {} sats", min_trade)?;
        writeln!(out, "  Max Trade:       {} sats", max_trade)?;
        writeln!(out, "  Mean Trade:      {:.0} sats", mean_trade)?;
        writeln!(out, "  Median Trade:    {} sats", median_trade)?;
    }

    // Trust Score
    writeln!(
        out,
        "{}",
        "----------------------------------------".dimmed()
    )?;
    let score = calculate_score(&stats, days_active);
    let score_display = format!("TRUST SCORE:       {}/100", score);
    if score >= 70 {
        writeln!(out, "{}", score_display.green().bold())?;
    } else if score >= 40 {
        writeln!(out, "{}", score_display.yellow().bold())?;
    } else {
        writeln!(out, "{}", score_display.red().bold())?;
    }
    writeln!(out, "{}", "========================================".cyan())?;

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

/// Compute trade amount statistics (Section 4.1.3)
fn compute_trade_stats(amounts: &[u64]) -> (u64, u64, f64, u64) {
    if amounts.is_empty() {
        return (0, 0, 0.0, 0);
    }

    let mut sorted = amounts.to_vec();
    sorted.sort_unstable();

    let min = sorted[0];
    let max = sorted[sorted.len() - 1];
    let sum: u128 = amounts.iter().map(|&v| v as u128).sum();
    let mean = sum as f64 / amounts.len() as f64;

    // Median calculation
    let median = if sorted.len().is_multiple_of(2) {
        ((sorted[sorted.len() / 2 - 1] as u128 + sorted[sorted.len() / 2] as u128) / 2) as u64
    } else {
        sorted[sorted.len() / 2]
    };

    (min, max, mean, median)
}

/// Compute rolling window metrics (Section 4.2.2)
fn compute_rolling_windows(timestamps: &[i64], now: i64) -> (usize, usize, usize) {
    let day_7 = now - (7 * 86400);
    let day_30 = now - (30 * 86400);
    let day_90 = now - (90 * 86400);

    let last_7d = timestamps.iter().filter(|&&ts| ts >= day_7).count();
    let last_30d = timestamps.iter().filter(|&&ts| ts >= day_30).count();
    let last_90d = timestamps.iter().filter(|&&ts| ts >= day_90).count();

    (last_7d, last_30d, last_90d)
}

/// Compute activity consistency (Section 4.2.3)
fn compute_activity_consistency(timestamps: &[i64], now: i64) -> (usize, usize) {
    let day_30_ago = now - (30 * 86400);

    // Get unique days with trades in last 30 days
    let active_days: HashSet<i64> = timestamps
        .iter()
        .filter(|&&ts| ts >= day_30_ago)
        .map(|&ts| ts / 86400) // Convert to day number
        .collect();

    let active_days_count = active_days.len();

    // Calculate max consecutive inactive days
    if active_days.is_empty() {
        return (0, 30);
    }

    let mut days: Vec<i64> = active_days.into_iter().collect();
    days.sort_unstable();

    let today = now / 86400;
    let day_30_start = day_30_ago / 86400;

    let mut max_gap = 0usize;
    let mut prev_day = day_30_start;

    for &day in &days {
        let gap = (day - prev_day - 1).max(0) as usize;
        max_gap = max_gap.max(gap);
        prev_day = day;
    }

    // Check gap from last active day to today
    let final_gap = (today - prev_day).max(0) as usize;
    max_gap = max_gap.max(final_gap);

    (active_days_count, max_gap)
}

/// Format relative time for human readability (Section 6.1)
fn format_relative_time(timestamp: i64, now: i64) -> String {
    let diff_secs = now - timestamp;

    if diff_secs < 0 {
        return "in the future".to_string();
    }

    let days = diff_secs / 86400;
    let hours = (diff_secs % 86400) / 3600;

    match days {
        0 => {
            if hours == 0 {
                "less than an hour ago".to_string()
            } else if hours == 1 {
                "1 hour ago".to_string()
            } else {
                format!("{} hours ago", hours)
            }
        }
        1 => "1 day ago".to_string(),
        2..=6 => format!("{} days ago", days),
        7..=13 => "1 week ago".to_string(),
        14..=29 => format!("{} weeks ago", days / 7),
        30..=59 => "1 month ago".to_string(),
        60..=364 => format!("{} months ago", days / 30),
        _ => format!("{} years ago", days / 365),
    }
}

fn calculate_score(stats: &MostroStats, days_active: f64) -> u64 {
    let mut score = 0.0;

    // 1. Age (Max 30 pts for > 1 year)
    score += (days_active / 365.0).min(1.0) * 30.0;

    // 2. Volume (Max 40 pts for > 1 BTC volume)
    let btc_vol = stats.total_volume_sats as f64 / 100_000_000.0;
    score += (btc_vol / 1.0).min(1.0) * 40.0;

    // 3. Success Count (Max 30 pts for > 100 orders)
    score += (stats.successful_orders as f64 / 100.0).min(1.0) * 30.0;

    score as u64
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

    // Step A — characterization tests for functions that are already standalone,
    // unaffected by Step 0's wrap. Written directly against current `main.rs` code,
    // before any module file is touched.

    #[test]
    fn compute_trade_stats_empty_returns_zeros() {
        assert_eq!(compute_trade_stats(&[]), (0, 0, 0.0, 0));
    }

    #[test]
    fn compute_trade_stats_single_value() {
        assert_eq!(compute_trade_stats(&[100]), (100, 100, 100.0, 100));
    }

    #[test]
    fn compute_trade_stats_odd_count_median_is_middle_value() {
        let (min, max, mean, median) = compute_trade_stats(&[10, 30, 20]);
        assert_eq!(min, 10);
        assert_eq!(max, 30);
        assert_eq!(mean, 20.0);
        assert_eq!(median, 20);
    }

    #[test]
    fn compute_trade_stats_even_count_median_is_average_of_middle_two() {
        let (min, max, mean, median) = compute_trade_stats(&[10, 20, 30, 40]);
        assert_eq!(min, 10);
        assert_eq!(max, 40);
        assert_eq!(mean, 25.0);
        assert_eq!(median, 25);
    }

    #[test]
    fn compute_rolling_windows_counts_each_window_independently() {
        let now = 1_000_000_i64;
        let timestamps = vec![
            now - 86400,      // within 7, 30, 90
            now - 10 * 86400, // within 30, 90
            now - 60 * 86400, // within 90 only
            now - 91 * 86400, // outside all windows
        ];
        assert_eq!(compute_rolling_windows(&timestamps, now), (1, 2, 3));
    }

    #[test]
    fn compute_rolling_windows_empty_is_all_zero() {
        assert_eq!(compute_rolling_windows(&[], 1_000_000), (0, 0, 0));
    }

    #[test]
    fn compute_activity_consistency_no_trades_is_zero_active_thirty_gap() {
        assert_eq!(compute_activity_consistency(&[], 1_000_000), (0, 30));
    }

    #[test]
    fn compute_activity_consistency_counts_unique_active_days_and_max_gap() {
        let now = 30 * 86400_i64;
        // Active on day 0 and day 10 (relative to the 30-day window start), leaving a
        // 9-day gap between them and a 20-day gap from day 10 to "today" (day 30).
        let timestamps = vec![100, 10 * 86400 + 100];
        let (active_days, max_gap) = compute_activity_consistency(&timestamps, now);
        assert_eq!(active_days, 2);
        assert_eq!(max_gap, 20);
    }

    #[test]
    fn format_relative_time_future_timestamp() {
        assert_eq!(format_relative_time(200, 100), "in the future");
    }

    #[test]
    fn format_relative_time_less_than_an_hour() {
        assert_eq!(format_relative_time(0, 1800), "less than an hour ago");
    }

    #[test]
    fn format_relative_time_exactly_one_hour() {
        assert_eq!(format_relative_time(0, 3600), "1 hour ago");
    }

    #[test]
    fn format_relative_time_multiple_hours() {
        assert_eq!(format_relative_time(0, 3 * 3600), "3 hours ago");
    }

    #[test]
    fn format_relative_time_exactly_one_day() {
        assert_eq!(format_relative_time(0, 86400), "1 day ago");
    }

    #[test]
    fn format_relative_time_several_days() {
        assert_eq!(format_relative_time(0, 4 * 86400), "4 days ago");
    }

    #[test]
    fn format_relative_time_one_week_boundary() {
        assert_eq!(format_relative_time(0, 7 * 86400), "1 week ago");
    }

    #[test]
    fn format_relative_time_several_weeks() {
        assert_eq!(format_relative_time(0, 20 * 86400), "2 weeks ago");
    }

    #[test]
    fn format_relative_time_one_month_boundary() {
        assert_eq!(format_relative_time(0, 30 * 86400), "1 month ago");
    }

    #[test]
    fn format_relative_time_several_months() {
        assert_eq!(format_relative_time(0, 90 * 86400), "3 months ago");
    }

    #[test]
    fn format_relative_time_one_year_or_more() {
        assert_eq!(format_relative_time(0, 400 * 86400), "1 years ago");
    }

    #[test]
    fn calculate_score_zero_activity_is_zero() {
        let stats = MostroStats::default();
        assert_eq!(calculate_score(&stats, 0.0), 0);
    }

    #[test]
    fn calculate_score_caps_each_component_at_its_maximum() {
        let stats = MostroStats {
            total_volume_sats: 200_000_000, // > 1 BTC, caps volume component
            successful_orders: 500,         // > 100, caps success-count component
            ..Default::default()
        };
        // Age caps at 365+ days (30 pts) + volume caps at 40 pts + success caps at 30 pts.
        assert_eq!(calculate_score(&stats, 400.0), 100);
    }

    #[test]
    fn calculate_score_partial_credit_is_proportional() {
        let stats = MostroStats {
            total_volume_sats: 50_000_000, // 0.5 BTC -> 20 pts
            successful_orders: 50,         // 50/100 -> 15 pts
            ..Default::default()
        };
        // days_active/365 = 0.5 -> 15 pts age + 20 pts volume + 15 pts success = 50.
        assert_eq!(calculate_score(&stats, 182.5), 50);
    }
}
