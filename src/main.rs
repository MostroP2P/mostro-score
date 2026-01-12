use clap::Parser;
use nostr_sdk::prelude::*;
use nostr_sdk::{Alphabet, SingleLetterTag};
use std::collections::HashMap;
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
    instance_start_ts: Option<i64>,  // NEW: From z=info event
    first_order_ts: i64,              // RENAMED: First order timestamp
    last_order_ts: i64,               // RENAMED: Last order timestamp
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
    // Filter 1: Instance Status (z=info)
    let info_filter = Filter::new()
        .kind(Kind::Custom(38383))
        .author(public_key)
        .custom_tag(SingleLetterTag::lowercase(Alphabet::Z), vec!["info"]);

    // Filter 2: Order Events (z=order)
    let order_filter = Filter::new()
        .kind(Kind::Custom(38383))
        .author(public_key)
        .custom_tag(SingleLetterTag::lowercase(Alphabet::Z), vec!["order"]);

    // 4. Fetch Both Event Types
    let events = client.fetch_events(vec![info_filter, order_filter], Duration::from_secs(10)).await?;
    
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

    // Separate z=info and z=order events
    let mut info_events: Vec<Event> = Vec::new();
    let mut order_events: Vec<Event> = Vec::new();

    for event in events {
        let z_tag = event.tags.iter()
            .find(|t| t.as_slice().first().map(|s| s.as_str()) == Some("z"));

        if let Some(tag) = z_tag {
            if let Some(z_value) = tag.as_slice().get(1) {
                match z_value.as_str() {
                    "info" => info_events.push(event),
                    "order" => order_events.push(event),
                    _ => {}
                }
            }
        }
    }

    println!("Found {} info events and {} order events", info_events.len(), order_events.len());

    // Process z=info event to get instance start timestamp
    let mut stats = MostroStats::default();

    if !info_events.is_empty() {
        // Sort by created_at to get the earliest one
        info_events.sort_by_key(|e| e.created_at);
        let oldest_info = &info_events[0];

        stats.instance_start_ts = Some(oldest_info.created_at.as_u64() as i64);

        println!("\n=== MOSTRO INSTANCE INFO ===");
        println!("Instance started: {}", chrono::DateTime::from_timestamp(oldest_info.created_at.as_u64() as i64, 0).unwrap_or_default());
        println!("============================\n");
    } else {
        println!("\n⚠ Warning: No z=info event found. Falling back to order timestamps.");
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

            // Get Amount 'amt' (sats)
            if let Some(amt_tag) = event.tags.iter().find(|t| t.as_slice().first().map(|s| s.as_str()) == Some("amt")) {
                if let Some(amt_str) = amt_tag.as_slice().get(1) {
                    if let Ok(amount) = amt_str.parse::<u64>() {
                        stats.total_volume_sats += amount;
                    }
                }
            }
        }
    }

    // 6. Output Report
    println!("\n========================================");
    println!("       MOSTRO NODE REPUTATION REPORT      ");
    println!("========================================");
    println!("Node: {}", public_key.to_bech32()?);
    println!("----------------------------------------");

    // Calculate days_active from instance start or fallback to orders
    let (days_active, instance_started) = match stats.instance_start_ts {
        Some(start_ts) => {
            let now = chrono::Utc::now().timestamp();
            let days = (now - start_ts) as f64 / 86400.0;
            (days, Some(start_ts))
        },
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

    // Print instance information
    if let Some(start_ts) = instance_started {
        println!("Instance Started: {}", chrono::DateTime::from_timestamp(start_ts, 0).unwrap_or_default());
        println!("Days Active:      {:.1} days (from instance start)", days_active);
    } else {
        println!("⚠ Days Active:    {:.1} days (estimated from orders)", days_active);
    }

    // Print order activity timeframe
    if stats.last_order_ts > 0 {
        println!("First Order:      {}", chrono::DateTime::from_timestamp(stats.first_order_ts, 0).unwrap_or_default());
        println!("Last Order:       {}", chrono::DateTime::from_timestamp(stats.last_order_ts, 0).unwrap_or_default());
    }
    println!("----------------------------------------");
    println!("Successful Orders: {}", stats.successful_orders);
    println!("Total Volume:      {} sats ({:.4} BTC)", stats.total_volume_sats, stats.total_volume_sats as f64 / 100_000_000.0);
    if stats.successful_orders > 0 {
        println!("Avg Order Size:    {} sats", stats.total_volume_sats / stats.successful_orders as u64);
    }
    println!("----------------------------------------");

    // Simple Score Calculation
    let score = calculate_score(&stats, days_active);
    println!("----------------------------------------");
    println!("TRUST SCORE:       {}/100", score);
    println!("========================================");

    Ok(())
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