/// Compute trade amount statistics (Section 4.1.3)
pub fn compute_trade_stats(amounts: &[u64]) -> (u64, u64, f64, u64) {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
