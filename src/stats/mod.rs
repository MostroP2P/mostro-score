pub mod lifecycle;
pub mod trade_size;

/// PR 1 Step C: the legacy trust-score calculation, relocated here verbatim while it is
/// still called by the wrapped function. `pub`, not `pub(crate)`: T039 moves this module
/// into the library crate, so `main.rs`'s binary crate now reaches it across the crate
/// boundary. Takes the two `MostroStats` fields it needs directly rather than the whole
/// struct, since `MostroStats` itself stays in `main.rs` and a library module cannot
/// reference a binary crate's types. No spec backing (Complexity Tracking); removed
/// entirely in PR 7's T124 once the report model no longer needs it.
pub fn calculate_score(successful_orders: usize, total_volume_sats: u64, days_active: f64) -> u64 {
    let mut score = 0.0;

    // 1. Age (Max 30 pts for > 1 year)
    score += (days_active / 365.0).min(1.0) * 30.0;

    // 2. Volume (Max 40 pts for > 1 BTC volume)
    let btc_vol = total_volume_sats as f64 / 100_000_000.0;
    score += (btc_vol / 1.0).min(1.0) * 40.0;

    // 3. Success Count (Max 30 pts for > 100 orders)
    score += (successful_orders as f64 / 100.0).min(1.0) * 30.0;

    score as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculate_score_zero_activity_is_zero() {
        assert_eq!(calculate_score(0, 0, 0.0), 0);
    }

    #[test]
    fn calculate_score_caps_each_component_at_its_maximum() {
        // total_volume_sats > 1 BTC caps the volume component; successful_orders > 100
        // caps the success-count component. Age caps at 365+ days (30 pts) + volume caps
        // at 40 pts + success caps at 30 pts.
        assert_eq!(calculate_score(500, 200_000_000, 400.0), 100);
    }

    #[test]
    fn calculate_score_partial_credit_is_proportional() {
        // 0.5 BTC -> 20 pts volume; 50/100 orders -> 15 pts success;
        // days_active/365 = 0.5 -> 15 pts age. Total: 50.
        assert_eq!(calculate_score(50, 50_000_000, 182.5), 50);
    }
}
