use clap::Parser;
use colored::Colorize;
use nostr_sdk::prelude::*;
use nostr_sdk::{Alphabet, SingleLetterTag};
use std::collections::{HashMap, HashSet};
use std::time::Duration;

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
    first_dev_fee_ts: Option<i64>,   // From oldest kind 8383 z=dev-fee-payment event
    first_order_ts: i64,              // First order timestamp
    last_order_ts: i64,               // Last order timestamp
    // Trade amount statistics (Section 4.1.3)
    trade_amounts: Vec<u64>,
    // Rolling window data (Section 4.2.2)
    successful_trade_timestamps: Vec<i64>,
}

#[tokio::main]
async fn main() -> Result<() > {
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

    println!("Analyzing Mostro Node: {}", public_key.to_bech32()?);
    println!("Hex: {}", public_key.to_hex());

    // 2. Setup Client
    let client = Client::new(Keys::generate());
    let relays: Vec<&str> = args.relays.split(',').collect();
    
    for relay in relays {
        client.add_relay(relay).await?;
    }
    
    client.connect().await;
    println!("Connected to relays. Fetching history... (this might take a moment)");

    // 3. Create Filters
    // Filter 1: Development Fee Events (z=dev-fee-payment, y=mostro)
    let dev_fee_filter = Filter::new()
        .kind(Kind::Custom(8383))
        .author(public_key)
        .custom_tag(SingleLetterTag::lowercase(Alphabet::Z), vec!["dev-fee-payment"])
        .custom_tag(SingleLetterTag::lowercase(Alphabet::Y), vec!["mostro"]);

    // Filter 2: Order Events (z=order)
    let order_filter = Filter::new()
        .kind(Kind::Custom(38383))
        .author(public_key)
        .custom_tag(SingleLetterTag::lowercase(Alphabet::Z), vec!["order"]);

    // 4. Fetch Both Event Types
    let events = client.fetch_events(vec![dev_fee_filter, order_filter], Duration::from_secs(10)).await?;
    
    println!("Fetched {} events. Analyzing...", events.len());

    // Print sample events to understand structure
    println!("\n=== SAMPLE EVENTS (first 3) ===");
    for (idx, event) in events.iter().take(3).enumerate() {
        println!("\nEvent #{}", idx + 1);
        println!("  ID: {}", event.id);
        println!("  created_at: {}", event.created_at);
        println!("  Tags:");
        for tag in event.tags.iter() {
            println!("    {:?}", tag.as_slice());
        }
    }
    println!("==============================\n");

    // Separate dev fee events and order events
    let mut dev_fee_events: Vec<Event> = Vec::new();
    let mut order_events: Vec<Event> = Vec::new();

    for event in events {
        let z_tag = event.tags.iter()
            .find(|t| t.as_slice().first().map(|s| s.as_str()) == Some("z"))
            .and_then(|t| t.as_slice().get(1))
            .map(|s| s.as_str());

        let y_tag = event.tags.iter()
            .find(|t| t.as_slice().first().map(|s| s.as_str()) == Some("y"))
            .and_then(|t| t.as_slice().get(1))
            .map(|s| s.as_str());

        match (z_tag, y_tag) {
            (Some("dev-fee-payment"), Some("mostro")) => dev_fee_events.push(event),
            (Some("order"), _) => order_events.push(event),
            _ => {}
        }
    }

    println!("Found {} dev fee events and {} order events", dev_fee_events.len(), order_events.len());

    // Process dev fee events to get instance start timestamp
    let mut stats = MostroStats::default();

    if !dev_fee_events.is_empty() {
        // Sort by created_at to get the earliest one
        dev_fee_events.sort_by_key(|e| e.created_at);
        let oldest_dev_fee = &dev_fee_events[0];

        stats.first_dev_fee_ts = Some(oldest_dev_fee.created_at.as_u64() as i64);

        println!("\n=== MOSTRO TRADING ACTIVITY ===");
        println!("First dev fee payment: {}", chrono::DateTime::from_timestamp(oldest_dev_fee.created_at.as_u64() as i64, 0).unwrap_or_default());
        println!("Total dev fee events: {}", dev_fee_events.len());
        println!("================================\n");
    } else {
        println!("\n⚠ Warning: No dev fee events found (z=dev-fee-payment, y=mostro).");
        println!("Falling back to order timestamps for days_active calculation.\n");
    }

    // 5. Analyze orders
    stats.first_order_ts = i64::MAX;

    // Debug tracking
    let total_order_count = order_events.len();
    let mut s_tag_distribution: HashMap<String, usize> = HashMap::new();

    // We need to deduplicate orders because Mostro updates the same order (same 'd' tag) multiple times.
    // We care about the *latest* state of each order.
    let mut orders_map: HashMap<String, Event> = HashMap::new();

    for event in order_events {
        // Track order time range
        if (event.created_at.as_u64() as i64) < stats.first_order_ts {
            stats.first_order_ts = event.created_at.as_u64() as i64;
        }
        if (event.created_at.as_u64() as i64) > stats.last_order_ts {
            stats.last_order_ts = event.created_at.as_u64() as i64;
        }

        // Track status distribution for all fetched events (all are orders now)
        let s_tag = event.tags.iter().find(|t| t.as_slice().first().map(|s| s.as_str()) == Some("s"));
        let s_value = s_tag.and_then(|t| t.as_slice().get(1)).map(|s| s.to_string());
        match &s_value {
            Some(val) => {
                *s_tag_distribution.entry(val.clone()).or_insert(0) += 1;
            }
            None => {
                *s_tag_distribution.entry("(missing)".to_string()).or_insert(0) += 1;
            }
        }

        // If it's an order, map it by 'd' tag (Order ID) to get the final state
        if let Some(d_tag) = event.tags.iter().find(|t| t.as_slice().first().map(|s| s.as_str()) == Some("d")) {
            if let Some(order_id) = d_tag.as_slice().get(1) {
                // Logic: Keep the event with the latest created_at for this Order ID
                match orders_map.get(order_id.as_str()) {
                    Some(existing) => {
                        if event.created_at > existing.created_at {
                            orders_map.insert(order_id.to_string(), event.clone());
                        }
                    }
                    None => {
                        orders_map.insert(order_id.to_string(), event.clone());
                    }
                }
            }
        }
    }

    // Print debug information
    println!("\n=== DEBUG INFORMATION ===");
    println!("Total order events fetched: {}", total_order_count);
    println!("Unique orders after deduplication: {}", orders_map.len());

    if !s_tag_distribution.is_empty() {
        println!("\nStatus distribution for order events (s tag):");
        for (status, count) in s_tag_distribution.iter() {
            println!("  s='{}': {} events", status, count);
        }
    } else {
        println!("\nNo order events found with s tags");
    }
    println!("========================\n");

    // Process the final state of unique orders
    for (_order_id, event) in orders_map {
        // Check Status 's'
        let s_tag = event.tags.iter().find(|t| t.as_slice().first().map(|s| s.as_str()) == Some("s"));
        let status = s_tag.and_then(|t| t.as_slice().get(1)).map(|s| s.as_str()).unwrap_or("unknown");

        if status == "success" {
            stats.successful_orders += 1;
            let event_ts = event.created_at.as_u64() as i64;
            stats.successful_trade_timestamps.push(event_ts);

            // Get Amount 'amt' (sats)
            if let Some(amt_tag) = event.tags.iter().find(|t| t.as_slice().first().map(|s| s.as_str()) == Some("amt")) {
                if let Some(amt_str) = amt_tag.as_slice().get(1) {
                    if let Ok(amount) = amt_str.parse::<u64>() {
                        stats.total_volume_sats += amount;
                        stats.trade_amounts.push(amount);
                    }
                }
            }
        }
    }

    // 6. Output Report
    let now = chrono::Utc::now().timestamp();

    // Calculate days_active from dev fee events or fallback to orders
    let (days_active, instance_started) = match stats.first_dev_fee_ts {
        Some(start_ts) => {
            let days = (now - start_ts) as f64 / 86400.0;
            (days, Some(start_ts))
        }
        None => {
            // Fallback: use order timestamps
            if stats.last_order_ts == 0 {
                println!("No events found.");
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
    println!("\n{}", "========================================".cyan());
    println!("{}", "     MOSTRO NODE REPUTATION REPORT     ".cyan().bold());
    println!("{}", "========================================".cyan());
    println!("Node: {}", public_key.to_bech32()?);

    // Section: Longevity (4.1.1)
    println!("{}", "----------------------------------------".dimmed());
    println!("{}", "LONGEVITY".bold());
    if let Some(start_ts) = instance_started {
        println!(
            "  First Activity:  {}",
            chrono::DateTime::from_timestamp(start_ts, 0).unwrap_or_default()
        );
        println!("  Days Active:     {:.1} days", days_active);
    } else {
        println!(
            "  {} Days Active:     {:.1} days (estimated from orders)",
            "⚠".yellow(),
            days_active
        );
    }

    // Section: Liveness (4.2.1) - PROMINENT per spec
    println!("{}", "----------------------------------------".dimmed());
    println!("{}", "LIVENESS".bold());
    if stats.last_order_ts > 0 {
        let relative_time = format_relative_time(stats.last_order_ts, now);
        let last_trade_display = format!(
            "  Last Trade:      {} ({})",
            chrono::DateTime::from_timestamp(stats.last_order_ts, 0).unwrap_or_default(),
            relative_time
        );

        // Color based on activity status
        if days_since_last > 30 {
            println!("{}", last_trade_display.red());
            println!("  Days Since Last: {} {}", days_since_last, "INACTIVE".red().bold());
        } else if days_since_last > 7 {
            println!("{}", last_trade_display.yellow());
            println!(
                "  Days Since Last: {} {}",
                days_since_last,
                "LOW ACTIVITY".yellow()
            );
        } else {
            println!("{}", last_trade_display.green());
            println!("  Days Since Last: {} {}", days_since_last, "ACTIVE".green());
        }
    } else {
        println!("  {} No successful trades recorded", "⚠".yellow());
    }

    // Section: Rolling Windows (4.2.2)
    println!("{}", "----------------------------------------".dimmed());
    println!("{}", "RECENT ACTIVITY".bold());
    println!("  Last 7 days:     {} trades", trades_7d);
    println!("  Last 30 days:    {} trades", trades_30d);
    println!("  Last 90 days:    {} trades", trades_90d);

    // Section: Activity Consistency (4.2.3)
    println!("{}", "----------------------------------------".dimmed());
    println!("{}", "ACTIVITY CONSISTENCY (30 days)".bold());
    println!("  Active Days:     {}/30", active_days_30d);
    if max_inactive_gap > 7 {
        println!(
            "  Max Inactive Gap: {} days {}",
            max_inactive_gap,
            "⚠".yellow()
        );
    } else {
        println!("  Max Inactive Gap: {} days", max_inactive_gap);
    }

    // Section: Cumulative Performance (4.1.2)
    println!("{}", "----------------------------------------".dimmed());
    println!("{}", "CUMULATIVE PERFORMANCE".bold());
    println!("  Successful Trades: {}", stats.successful_orders);
    println!(
        "  Total Volume:      {} sats ({:.4} BTC)",
        stats.total_volume_sats,
        stats.total_volume_sats as f64 / 100_000_000.0
    );

    // Section: Trade Statistics (4.1.3)
    if !stats.trade_amounts.is_empty() {
        println!("{}", "----------------------------------------".dimmed());
        println!("{}", "TRADE STATISTICS".bold());
        println!("  Min Trade:       {} sats", min_trade);
        println!("  Max Trade:       {} sats", max_trade);
        println!("  Mean Trade:      {} sats", mean_trade);
        println!("  Median Trade:    {} sats", median_trade);
    }

    // Trust Score
    println!("{}", "----------------------------------------".dimmed());
    let score = calculate_score(&stats, days_active);
    let score_display = format!("TRUST SCORE:       {}/100", score);
    if score >= 70 {
        println!("{}", score_display.green().bold());
    } else if score >= 40 {
        println!("{}", score_display.yellow().bold());
    } else {
        println!("{}", score_display.red().bold());
    }
    println!("{}", "========================================".cyan());

    Ok(())
}

/// Compute trade amount statistics (Section 4.1.3)
fn compute_trade_stats(amounts: &[u64]) -> (u64, u64, u64, u64) {
    if amounts.is_empty() {
        return (0, 0, 0, 0);
    }

    let mut sorted = amounts.to_vec();
    sorted.sort_unstable();

    let min = sorted[0];
    let max = sorted[sorted.len() - 1];
    let mean = amounts.iter().sum::<u64>() / amounts.len() as u64;

    // Median calculation
    let median = if sorted.len() % 2 == 0 {
        (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2
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