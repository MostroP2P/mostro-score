mod error;
mod models;
mod stats;

use clap::Parser;
use colored::Colorize;
use mostro_core::prelude::Status as OrderStatus;
use nostr_sdk::prelude::*;
use nostr_sdk::{Alphabet, SingleLetterTag};
use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;

use crate::error::{AppError, Result};
use crate::models::MostroStats;
use crate::stats::{
    calculate_score, compute_activity_consistency, compute_rolling_windows, compute_trade_stats,
    format_relative_time,
};

const DEV_FEE_EVENT_KIND: u16 = 8383;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    pubkey: String,

    #[arg(short, long, default_value = "wss://relay.mostro.network")]
    relays: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();

    let public_key =
        PublicKey::parse(&args.pubkey).map_err(|e| AppError::InvalidPubkey(e.to_string()))?;

    let bech32 = public_key.to_bech32().expect("PublicKey::to_bech32 is infallible");
    println!("Analyzing Mostro Node: {}", bech32);
    println!("Hex: {}", public_key.to_hex());

    // 2. Setup Client
    let client = Client::new(Keys::generate());
    let relays: Vec<&str> = args.relays.split(',').collect();

    let mut connected = 0;
    for relay in &relays {
        match client.add_relay(*relay).await {
            Ok(_) => connected += 1,
            Err(e) => eprintln!("Warning: could not add relay {} ({})", relay, e),
        }
    }

    if connected == 0 {
        return Err(AppError::NoRelaysAvailable);
    }

    client.connect().await;
    println!(
        "Connected to {}/{} relays. Fetching history...",
        connected,
        relays.len()
    );

    // 3. Create Filters
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

    // 4. Fetch Both Event Types
    let dev_fee_events_result = client.fetch_events(dev_fee_filter, Duration::from_secs(10)).await?;
    let order_events_result = client.fetch_events(order_filter, Duration::from_secs(10)).await?;
    let events: Vec<Event> = dev_fee_events_result.into_iter().chain(order_events_result.into_iter()).collect();
    
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
        let status_str = s_tag.and_then(|t| t.as_slice().get(1)).map(|s| s.as_str()).unwrap_or("unknown");

        if OrderStatus::from_str(status_str) == Ok(OrderStatus::Success) {
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
                return Err(AppError::NoEvents);
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
    println!(
        "Node: {}",
        public_key.to_bech32().expect("PublicKey::to_bech32 is infallible")
    );

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
        println!("  Mean Trade:      {:.0} sats", mean_trade);
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