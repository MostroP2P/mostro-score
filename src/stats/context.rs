//! Descriptive context signals: fiat-currency distribution, payment-method usage
//! ranking, and premium consistency, computed from the already extracted `f`/`pm`/
//! `premium` values on the node's qualifying successful orders
//! (`models::order::OrderAggregate`). Takes plain slices, not the aggregate itself,
//! matching `stats::lifecycle`/`stats::trade_size`/`stats::disputes`'s existing pattern
//! of depending on primitives rather than `models::*` types.

use std::cmp::Ordering;
use std::collections::HashMap;

/// One currency's share of the fiat-currency distribution.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct FiatCurrencyShare {
    pub currency: String,
    pub orders: usize,
    pub share_percent: f64,
}

/// The fiat-currency distribution. `orders_considered` is the denominator: the count of
/// qualifying successful orders carrying a non-empty `f` value. `distribution` is `None`
/// only when that denominator is zero (every qualifying order had an empty `f` value, or
/// there were no qualifying orders at all).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct FiatBreakdown {
    pub orders_considered: usize,
    pub distribution: Option<Vec<FiatCurrencyShare>>,
}

/// One payment method's share of the usage ranking.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct PaymentMethodShare {
    pub method: String,
    pub mentions: usize,
    pub share_percent: f64,
}

/// The payment-method usage ranking. `total_mentions` is the denominator: the count of
/// `pm` mentions across every qualifying successful order (not a per-order count).
/// `distribution` is `None` only when that denominator is zero.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct PaymentMethodBreakdown {
    pub total_mentions: usize,
    pub distribution: Option<Vec<PaymentMethodShare>>,
}

/// Premium consistency signal. Both fields are `None` only when fewer than 2 orders
/// carry a valid `premium` tag.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct PremiumSignal {
    pub premium_baseline_percent: Option<f64>,
    pub premium_dispersion_percent: Option<f64>,
}

fn ranked_shares<T>(
    counts: HashMap<String, usize>,
    total: usize,
    build: impl Fn(String, usize, f64) -> T,
    label_of: impl Fn(&T) -> &str,
    share_of: impl Fn(&T) -> f64,
) -> Vec<T> {
    let mut shares: Vec<T> = counts
        .into_iter()
        .map(|(label, count)| {
            let share_percent = (count as f64 / total as f64) * 100.0;
            build(label, count, share_percent)
        })
        .collect();

    // Descending `share_percent`, ties broken by label ascending — deterministic even
    // though the underlying tally is a `HashMap` with no guaranteed iteration order.
    shares.sort_by(|a, b| {
        share_of(b)
            .partial_cmp(&share_of(a))
            .unwrap_or(Ordering::Equal)
            .then_with(|| label_of(a).cmp(label_of(b)))
    });

    shares
}

/// Computes the fiat-currency distribution from the `f` tag's value on each qualifying
/// successful order, already filtered to exclude empty values by
/// `models::order::aggregate_order_events`. Byte-for-byte comparison, no trimming, no
/// case normalization.
pub fn compute_fiat_breakdown(fiat_values: &[String]) -> FiatBreakdown {
    let orders_considered = fiat_values.len();

    if fiat_values.is_empty() {
        return FiatBreakdown {
            orders_considered: 0,
            distribution: None,
        };
    }

    let mut counts: HashMap<String, usize> = HashMap::new();
    for currency in fiat_values {
        *counts.entry(currency.clone()).or_insert(0) += 1;
    }

    let distribution = ranked_shares(
        counts,
        orders_considered,
        |currency, orders, share_percent| FiatCurrencyShare {
            currency,
            orders,
            share_percent,
        },
        |share| share.currency.as_str(),
        |share| share.share_percent,
    );

    FiatBreakdown {
        orders_considered,
        distribution: Some(distribution),
    }
}

/// Computes the payment-method usage ranking from every `pm` mention across the node's
/// qualifying successful orders, already flattened by
/// `models::order::aggregate_order_events`. Byte-for-byte comparison is critical here:
/// `"Bank transfer"` and `" Bank transfer"` are distinct methods, never trimmed or
/// normalized.
pub fn compute_payment_method_breakdown(mentions: &[String]) -> PaymentMethodBreakdown {
    let total_mentions = mentions.len();

    if mentions.is_empty() {
        return PaymentMethodBreakdown {
            total_mentions: 0,
            distribution: None,
        };
    }

    let mut counts: HashMap<String, usize> = HashMap::new();
    for method in mentions {
        *counts.entry(method.clone()).or_insert(0) += 1;
    }

    let distribution = ranked_shares(
        counts,
        total_mentions,
        |method, mentions, share_percent| PaymentMethodShare {
            method,
            mentions,
            share_percent,
        },
        |share| share.method.as_str(),
        |share| share.share_percent,
    );

    PaymentMethodBreakdown {
        total_mentions,
        distribution: Some(distribution),
    }
}

/// Computes the premium signal from the `premium` tag parsed as `i64` on each
/// qualifying successful order, already excluded when missing/unparseable by
/// `models::order::aggregate_order_events`. `premium_baseline_percent` is the median;
/// `premium_dispersion_percent` is the population standard deviation (divide by `N`, not
/// `N-1`, the same rule as `stats::trade_size`'s computation) around the mean — a
/// distinct computation from trade-size's own median/std-dev, since this is percentage
/// points over a signed `i64` set, not sats over an unsigned one. `premium` comes from
/// an untrusted relay event (`models::order`'s `saturating_add` on `amt` carries the
/// same caveat): converting to `f64` loses precision above 2^53 in either direction, but
/// no legitimate market-price markup/discount approaches that magnitude — only a
/// deliberately crafted, absurd value would.
pub fn compute_premium_signal(premium_values: &[i64]) -> PremiumSignal {
    if premium_values.len() < 2 {
        return PremiumSignal {
            premium_baseline_percent: None,
            premium_dispersion_percent: None,
        };
    }

    let mut sorted = premium_values.to_vec();
    sorted.sort_unstable();

    let median = if sorted.len().is_multiple_of(2) {
        (sorted[sorted.len() / 2 - 1] as f64 + sorted[sorted.len() / 2] as f64) / 2.0
    } else {
        sorted[sorted.len() / 2] as f64
    };

    let mean = premium_values.iter().map(|&v| v as f64).sum::<f64>() / premium_values.len() as f64;
    let variance = premium_values
        .iter()
        .map(|&v| {
            let diff = v as f64 - mean;
            diff * diff
        })
        .sum::<f64>()
        / premium_values.len() as f64;
    let dispersion = variance.sqrt();

    PremiumSignal {
        premium_baseline_percent: Some(median),
        premium_dispersion_percent: Some(dispersion),
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
    fn compute_fiat_breakdown_empty_is_not_applicable() {
        let breakdown = compute_fiat_breakdown(&[]);
        assert_eq!(breakdown.orders_considered, 0);
        assert_eq!(breakdown.distribution, None);
    }

    #[test]
    fn compute_fiat_breakdown_ranks_by_descending_share_percent() {
        let values = vec![
            "USD".to_string(),
            "USD".to_string(),
            "USD".to_string(),
            "EUR".to_string(),
        ];

        let breakdown = compute_fiat_breakdown(&values);

        assert_eq!(breakdown.orders_considered, 4);
        let distribution = breakdown.distribution.expect("distribution present");
        assert_eq!(distribution.len(), 2);
        assert_eq!(distribution[0].currency, "USD");
        assert_eq!(distribution[0].orders, 3);
        assert_close(distribution[0].share_percent, 75.0);
        assert_eq!(distribution[1].currency, "EUR");
        assert_eq!(distribution[1].orders, 1);
        assert_close(distribution[1].share_percent, 25.0);
    }

    #[test]
    fn compute_fiat_breakdown_breaks_ties_by_currency_ascending() {
        let values = vec!["EUR".to_string(), "USD".to_string()];

        let breakdown = compute_fiat_breakdown(&values);

        let distribution = breakdown.distribution.expect("distribution present");
        assert_eq!(distribution[0].currency, "EUR");
        assert_eq!(distribution[1].currency, "USD");
    }

    #[test]
    fn compute_fiat_breakdown_percentages_sum_to_100() {
        let values = vec![
            "USD".to_string(),
            "EUR".to_string(),
            "EUR".to_string(),
            "GBP".to_string(),
        ];

        let breakdown = compute_fiat_breakdown(&values);

        let total: f64 = breakdown
            .distribution
            .expect("distribution present")
            .iter()
            .map(|share| share.share_percent)
            .sum();
        assert_close(total, 100.0);
    }

    #[test]
    fn compute_payment_method_breakdown_empty_is_not_applicable() {
        let breakdown = compute_payment_method_breakdown(&[]);
        assert_eq!(breakdown.total_mentions, 0);
        assert_eq!(breakdown.distribution, None);
    }

    #[test]
    fn compute_payment_method_breakdown_ranks_mentions_not_orders() {
        // A skewed example so the ranking is unambiguous.
        let mentions = vec!["SEPA".to_string(), "SEPA".to_string(), "Cash".to_string()];

        let breakdown = compute_payment_method_breakdown(&mentions);

        assert_eq!(breakdown.total_mentions, 3);
        let distribution = breakdown.distribution.expect("distribution present");
        assert_eq!(distribution[0].method, "SEPA");
        assert_eq!(distribution[0].mentions, 2);
        assert_close(distribution[0].share_percent, 66.666_666_666_666_66);
        assert_eq!(distribution[1].method, "Cash");
        assert_eq!(distribution[1].mentions, 1);
    }

    /// Byte-for-byte comparison — a leading space makes a distinct method, never merged
    /// or trimmed.
    #[test]
    fn compute_payment_method_breakdown_treats_leading_whitespace_as_a_distinct_method() {
        let mentions = vec!["Bank transfer".to_string(), " Bank transfer".to_string()];

        let breakdown = compute_payment_method_breakdown(&mentions);

        let distribution = breakdown.distribution.expect("distribution present");
        assert_eq!(distribution.len(), 2);
        assert!(distribution.iter().any(|s| s.method == "Bank transfer"));
        assert!(distribution.iter().any(|s| s.method == " Bank transfer"));
    }

    #[test]
    fn compute_premium_signal_fewer_than_two_values_is_not_applicable() {
        assert_eq!(
            compute_premium_signal(&[]),
            PremiumSignal {
                premium_baseline_percent: None,
                premium_dispersion_percent: None,
            }
        );
        assert_eq!(
            compute_premium_signal(&[5]),
            PremiumSignal {
                premium_baseline_percent: None,
                premium_dispersion_percent: None,
            }
        );
    }

    #[test]
    fn compute_premium_signal_computes_median_baseline_and_population_std_dev_dispersion() {
        let signal = compute_premium_signal(&[10, 20, 30, 40]);

        // Median of [10, 20, 30, 40] = 25.
        assert_close(signal.premium_baseline_percent.unwrap(), 25.0);
        // mean = 25; deviations -15,-5,5,15; squared 225,25,25,225; sum 500; /4 = 125.
        assert_close(signal.premium_dispersion_percent.unwrap(), 125.0_f64.sqrt());
    }

    /// `premium` supports negative values (a discount rather than a markup).
    #[test]
    fn compute_premium_signal_handles_negative_premiums() {
        let signal = compute_premium_signal(&[-10, -5, 0, 5, 10]);

        assert_close(signal.premium_baseline_percent.unwrap(), 0.0);
    }

    /// Documents the accepted `f64` precision boundary for a deliberately crafted,
    /// absurd `premium` value (see `stats::trade_size`'s identical documented boundary
    /// for `amt`): above 2^53, distinct `i64` values can collapse to the same `f64`.
    /// This never happens for any legitimate market-price markup/discount and never
    /// panics; it is accepted imprecision, not undefined behavior.
    #[test]
    fn compute_premium_signal_amounts_above_f64_precision_limit_do_not_panic() {
        let two_pow_53 = 1i64 << 53;
        let signal = compute_premium_signal(&[two_pow_53, two_pow_53 + 1]);

        assert_eq!(signal.premium_baseline_percent, Some(two_pow_53 as f64));
    }
}
