/// Trade-size statistics (Section 4.1.3, 001 FR-003, FR-010). `min_trade_sats`,
/// `max_trade_sats`, `mean_trade_sats`, and `median_trade_sats` are `None` only when the
/// amt-restricted set (001 FR-002) is empty. `std_dev_trade_sats` follows that same
/// empty-set rule. `coefficient_of_variation` has its own, stricter not-applicable rule
/// (FR-010): `None` when fewer than 2 orders exist, or when `median_trade_sats` is
/// exactly `0`, since dividing by a zero median is undefined regardless of sample size.
/// `median_trade_sats` is `f64`, not `u64`: an even-sized set's median is the average of
/// its two middle values, which is legitimately fractional (e.g. `[0, 1]` medians to
/// `0.5`), and truncating it to an integer would both misreport the value and corrupt
/// `coefficient_of_variation`'s division. `amt` comes from an untrusted relay event
/// (`models::order`'s `saturating_add` on the same field carries the identical caveat):
/// converting it to `f64` loses precision above 2^53, but that threshold is already over
/// 4x the entire 21M BTC supply in sats, so no realistic `amt` value is affected — only a
/// deliberately crafted, physically impossible one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TradeSizeStats {
    pub min_trade_sats: Option<u64>,
    pub max_trade_sats: Option<u64>,
    pub mean_trade_sats: Option<f64>,
    pub median_trade_sats: Option<f64>,
    pub std_dev_trade_sats: Option<f64>,
    pub coefficient_of_variation: Option<f64>,
}

const NOT_APPLICABLE: TradeSizeStats = TradeSizeStats {
    min_trade_sats: None,
    max_trade_sats: None,
    mean_trade_sats: None,
    median_trade_sats: None,
    std_dev_trade_sats: None,
    coefficient_of_variation: None,
};

/// Compute trade amount statistics (Section 4.1.3)
pub fn compute_trade_stats(amounts: &[u64]) -> TradeSizeStats {
    if amounts.is_empty() {
        return NOT_APPLICABLE;
    }

    let mut sorted = amounts.to_vec();
    sorted.sort_unstable();

    let min = sorted[0];
    let max = sorted[sorted.len() - 1];
    let sum: u128 = amounts.iter().map(|&v| v as u128).sum();
    let mean = sum as f64 / amounts.len() as f64;

    let median: f64 = if sorted.len().is_multiple_of(2) {
        (sorted[sorted.len() / 2 - 1] as f64 + sorted[sorted.len() / 2] as f64) / 2.0
    } else {
        sorted[sorted.len() / 2] as f64
    };

    // Population standard deviation (divide by N, not N-1, per FR-010): this
    // amt-restricted set is the node's complete historical record, not a sample.
    let variance = amounts
        .iter()
        .map(|&v| {
            let diff = v as f64 - mean;
            diff * diff
        })
        .sum::<f64>()
        / amounts.len() as f64;
    let std_dev = variance.sqrt();

    let coefficient_of_variation = if amounts.len() < 2 || median == 0.0 {
        None
    } else {
        Some(std_dev / median)
    };

    TradeSizeStats {
        min_trade_sats: Some(min),
        max_trade_sats: Some(max),
        mean_trade_sats: Some(mean),
        median_trade_sats: Some(median),
        std_dev_trade_sats: Some(std_dev),
        coefficient_of_variation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn compute_trade_stats_empty_reports_not_applicable() {
        let stats = compute_trade_stats(&[]);
        assert_eq!(stats, TradeSizeStats::default_not_applicable());
    }

    #[test]
    fn compute_trade_stats_single_value_has_zero_std_dev_but_no_coefficient_of_variation() {
        let stats = compute_trade_stats(&[100]);
        assert_eq!(stats.min_trade_sats, Some(100));
        assert_eq!(stats.max_trade_sats, Some(100));
        assert_eq!(stats.mean_trade_sats, Some(100.0));
        assert_eq!(stats.median_trade_sats, Some(100.0));
        assert_eq!(stats.std_dev_trade_sats, Some(0.0));
        // FR-010: fewer than 2 orders -> not applicable, even though std_dev is defined.
        assert_eq!(stats.coefficient_of_variation, None);
    }

    #[test]
    fn compute_trade_stats_odd_count_median_is_middle_value() {
        let stats = compute_trade_stats(&[10, 30, 20]);
        assert_eq!(stats.min_trade_sats, Some(10));
        assert_eq!(stats.max_trade_sats, Some(30));
        assert_eq!(stats.mean_trade_sats, Some(20.0));
        assert_eq!(stats.median_trade_sats, Some(20.0));
    }

    #[test]
    fn compute_trade_stats_even_count_median_is_average_of_middle_two() {
        let stats = compute_trade_stats(&[10, 20, 30, 40]);
        assert_eq!(stats.min_trade_sats, Some(10));
        assert_eq!(stats.max_trade_sats, Some(40));
        assert_eq!(stats.mean_trade_sats, Some(25.0));
        assert_eq!(stats.median_trade_sats, Some(25.0));
    }

    /// Regression: an even-sized set whose middle two values average to a fraction
    /// (e.g. 0 and 1) must report that fraction exactly, not truncate it to an integer
    /// — plan.md's JSON output contract explicitly states `median_trade_sats` over an
    /// even-sized set is legitimately fractional, and truncating it would also corrupt
    /// `coefficient_of_variation`'s division (a truncated-to-zero median would wrongly
    /// report CV as not applicable instead of a real value).
    #[test]
    fn compute_trade_stats_even_count_fractional_median_is_not_truncated() {
        let stats = compute_trade_stats(&[0, 1]);
        assert_eq!(stats.median_trade_sats, Some(0.5));
        assert_eq!(stats.coefficient_of_variation, Some(1.0));
    }

    /// Documents the accepted `f64` precision boundary for a deliberately crafted,
    /// physically impossible `amt` value: above 2^53, distinct `u64` values can collapse
    /// to the same `f64`, so the median loses its fractional part here. This never
    /// happens for any real sat amount (2^53 is already over 4x the entire 21M BTC supply)
    /// and never panics; it is accepted imprecision, not undefined behavior.
    #[test]
    fn compute_trade_stats_amounts_above_f64_precision_limit_do_not_panic() {
        let two_pow_53 = 1u64 << 53;
        let stats = compute_trade_stats(&[two_pow_53, two_pow_53 + 1]);

        // The true median is `two_pow_53 as f64 + 0.5`, but both inputs collapse to the
        // same f64 representation above this threshold.
        assert_eq!(stats.median_trade_sats, Some(two_pow_53 as f64));
    }

    #[test]
    fn compute_trade_stats_computes_population_std_dev_and_coefficient_of_variation() {
        let stats = compute_trade_stats(&[10, 20, 30, 40]);
        // mean = 25; deviations -15,-5,5,15; squared 225,25,25,225; sum 500; /4 = 125.
        assert_close(stats.std_dev_trade_sats.unwrap(), 125.0_f64.sqrt());
        assert_close(
            stats.coefficient_of_variation.unwrap(),
            125.0_f64.sqrt() / 25.0,
        );
    }

    #[test]
    fn compute_trade_stats_zero_median_reports_coefficient_of_variation_as_not_applicable() {
        let stats = compute_trade_stats(&[0, 0]);
        assert_eq!(stats.median_trade_sats, Some(0.0));
        assert_eq!(stats.std_dev_trade_sats, Some(0.0));
        assert_eq!(stats.coefficient_of_variation, None);
    }

    impl TradeSizeStats {
        fn default_not_applicable() -> Self {
            NOT_APPLICABLE
        }
    }
}
