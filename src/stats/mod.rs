pub mod lifecycle;
pub mod trade_size;

/// PR 1 Step C: the legacy trust-score calculation, relocated here verbatim while it is
/// still called by the wrapped function. `pub(crate)` rather than fully private: it must
/// stay reachable from `main.rs`'s `run()`, but has no spec backing (Complexity
/// Tracking) and is removed entirely in PR 7's T124 once the report model no longer
/// needs it.
pub(crate) fn calculate_score(stats: &crate::MostroStats, days_active: f64) -> u64 {
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

    #[test]
    fn calculate_score_zero_activity_is_zero() {
        let stats = crate::MostroStats::default();
        assert_eq!(calculate_score(&stats, 0.0), 0);
    }

    #[test]
    fn calculate_score_caps_each_component_at_its_maximum() {
        let stats = crate::MostroStats {
            total_volume_sats: 200_000_000, // > 1 BTC, caps volume component
            successful_orders: 500,         // > 100, caps success-count component
            ..Default::default()
        };
        // Age caps at 365+ days (30 pts) + volume caps at 40 pts + success caps at 30 pts.
        assert_eq!(calculate_score(&stats, 400.0), 100);
    }

    #[test]
    fn calculate_score_partial_credit_is_proportional() {
        let stats = crate::MostroStats {
            total_volume_sats: 50_000_000, // 0.5 BTC -> 20 pts
            successful_orders: 50,         // 50/100 -> 15 pts
            ..Default::default()
        };
        // days_active/365 = 0.5 -> 15 pts age + 20 pts volume + 15 pts success = 50.
        assert_eq!(calculate_score(&stats, 182.5), 50);
    }
}
