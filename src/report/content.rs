//! The recommendations block's content assembly.
//!
//! Scope note: only 3 deterministic, existence-based triggers are implemented here —
//! zero successful trades, disputes present, bond policy not enabled. Two
//! threshold-based triggers (premium dispersion, trade-size coefficient of variation)
//! are deferred until real-node evidence picks their boundaries; `nothing_notable` is
//! therefore scoped to these 3 triggers only.
//!
//! Every trigger here reads data already computed elsewhere (`NodeMetrics::cumulative`,
//! `NodeMetrics::disputes`, `NodeMetrics::bond_policy`) — no new statistics are computed
//! in this module, only plain-language messages assembled from them, so a trader can
//! understand each signal without leaving the tool.

use crate::stats::NodeMetrics;
use serde::Serialize;

/// One recommendation the block surfaces.
///
/// `metric` is the dotted path (matching `stats`'s own JSON structure) of the field the
/// guidance refers to, or `None` for guidance that synthesizes several — every trigger
/// implemented here points at exactly one field, so `metric` is always `Some`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecommendationItem {
    pub id: String,
    pub metric: Option<String>,
    pub message: String,
}

/// The recommendations block. `nothing_notable` is `true` only when none of the
/// currently implemented triggers fire; `items` is empty in that case. This block must
/// explicitly state there is nothing notable to flag rather than omitting itself — the
/// console/plain-text renderers turn this boolean into that sentence for human readers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ReportRecommendations {
    pub nothing_notable: bool,
    pub items: Vec<RecommendationItem>,
}

/// Zero completed trades: states plainly there is no completed-trade track record yet,
/// no comparison implied since Cumulative Performance has no cross-node baseline.
fn zero_trades_recommendation(metrics: &NodeMetrics) -> Option<RecommendationItem> {
    if metrics.cumulative.total_successful_trades != 0 {
        return None;
    }

    Some(RecommendationItem {
        id: "no_completed_trades".to_string(),
        metric: Some("stats.cumulative.total_successful_trades".to_string()),
        message: "This node has no completed trade history yet — there is no \
                  successful-trade track record to evaluate."
            .to_string(),
    })
}

/// Disputes present: states the raw dispute and trade counts, deliberately without
/// "high"/"many" or any other comparative language, since Dispute Signals has no
/// cross-node baseline.
fn disputes_recommendation(metrics: &NodeMetrics) -> Option<RecommendationItem> {
    if metrics.disputes.total_disputes == 0 {
        return None;
    }

    Some(RecommendationItem {
        id: "disputes_present".to_string(),
        metric: Some("stats.disputes.disputes_per_100_trades".to_string()),
        message: format!(
            "This node has had {} dispute(s) recorded, alongside {} completed \
             trade(s). Review its dispute history before trading with it.",
            metrics.disputes.total_disputes, metrics.cumulative.total_successful_trades
        ),
    })
}

/// Bond policy not enabled: states the exact status (`disabled` or `unknown`)
/// neutrally. Bond Policy must never be reported as if unknown were equivalent to
/// disabled, or as implying which status is safer. The wording below states only the
/// raw fact — whether a bond deposit is required, or whether that could be determined
/// at all — with no claim about how protective or risky either status is.
fn bond_policy_recommendation(metrics: &NodeMetrics) -> Option<RecommendationItem> {
    let message = match metrics.bond_policy.status {
        "enabled" => return None,
        "disabled" => "This node's bond policy is disabled: it does not require a bond \
                       deposit from traders."
            .to_string(),
        _ => "This node's bond policy could not be determined from its published data \
              — it is unclear whether a bond deposit is required."
            .to_string(),
    };

    Some(RecommendationItem {
        id: "bond_policy_not_enabled".to_string(),
        metric: Some("stats.bond_policy.status".to_string()),
        message,
    })
}

/// Which of the 4 filterable console/plain-text sections render. The node identity
/// header is not represented here since it always renders regardless of `--sections`.
/// `--format json` never consults this: `report::render::json` always emits the
/// complete structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectionFilter {
    pub fetch: bool,
    pub activity: bool,
    pub stats: bool,
    pub recommendations: bool,
}

impl SectionFilter {
    /// The omitted-`--sections`-flag default: every section renders.
    pub fn all() -> Self {
        SectionFilter {
            fetch: true,
            activity: true,
            stats: true,
            recommendations: true,
        }
    }

    /// Parses a comma-separated `--sections` value into a `SectionFilter`, matching
    /// each token case-sensitively against the 4 valid names. No trimming: a token
    /// with surrounding whitespace does not match any of the 4 exact names. Collects
    /// every invalid token encountered, not just the first, so a caller can report the
    /// complete list in one validation error.
    pub fn parse(raw: &str) -> Result<SectionFilter, Vec<String>> {
        let tokens: Vec<String> = raw.split(',').map(|token| token.to_string()).collect();
        Self::from_tokens(&tokens)
    }

    /// The same per-token matching logic as `parse`, over an already-split slice of
    /// tokens rather than a single comma-separated string. Configuration files
    /// naturally represent this as a TOML array (unlike the CLI flag's comma-separated
    /// string), so this is the entry point the configuration-file loader consults
    /// (`config::file::validate`); `parse` builds on top of this to avoid duplicating
    /// the match arms.
    pub fn from_tokens(tokens: &[String]) -> Result<SectionFilter, Vec<String>> {
        let mut filter = SectionFilter {
            fetch: false,
            activity: false,
            stats: false,
            recommendations: false,
        };
        let mut invalid_tokens = Vec::new();

        for token in tokens {
            match token.as_str() {
                "fetch" => filter.fetch = true,
                "activity" => filter.activity = true,
                "stats" => filter.stats = true,
                "recommendations" => filter.recommendations = true,
                other => invalid_tokens.push(other.to_string()),
            }
        }

        if invalid_tokens.is_empty() {
            Ok(filter)
        } else {
            Err(invalid_tokens)
        }
    }
}

/// Assembles the recommendations block from `NodeMetrics`, scoped to the 3
/// deterministic triggers described in this module's doc comment. Pure assembly over
/// already-computed metrics — no new statistics are computed here.
pub fn assemble_recommendations_section(metrics: &NodeMetrics) -> ReportRecommendations {
    let items: Vec<RecommendationItem> = [
        zero_trades_recommendation(metrics),
        disputes_recommendation(metrics),
        bond_policy_recommendation(metrics),
    ]
    .into_iter()
    .flatten()
    .collect();

    ReportRecommendations {
        nothing_notable: items.is_empty(),
        items,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::too_many_arguments)]
    fn metrics_with(
        successful_orders: usize,
        total_disputes: usize,
        bond_policy_status: &'static str,
    ) -> NodeMetrics {
        NodeMetrics::compute(
            None,
            successful_orders,
            0,
            &[],
            &[],
            total_disputes,
            0,
            total_disputes,
            0,
            &[],
            &[],
            &[],
            bond_policy_status,
            100 * 86400,
        )
    }

    #[test]
    fn zero_successful_trades_triggers_a_recommendation() {
        let metrics = metrics_with(0, 0, "enabled");

        let item = zero_trades_recommendation(&metrics).expect("recommendation fires");

        assert_eq!(item.id, "no_completed_trades");
        assert_eq!(
            item.metric,
            Some("stats.cumulative.total_successful_trades".to_string())
        );
        assert!(item.message.contains("no completed trade history"));
    }

    #[test]
    fn nonzero_successful_trades_does_not_trigger_the_zero_trades_recommendation() {
        let metrics = metrics_with(3, 0, "enabled");

        assert_eq!(zero_trades_recommendation(&metrics), None);
    }

    #[test]
    fn disputes_present_triggers_a_recommendation_with_the_raw_counts() {
        let metrics = metrics_with(10, 2, "enabled");

        let item = disputes_recommendation(&metrics).expect("recommendation fires");

        assert_eq!(item.id, "disputes_present");
        assert_eq!(
            item.metric,
            Some("stats.disputes.disputes_per_100_trades".to_string())
        );
        assert!(item.message.contains('2'));
        assert!(item.message.contains("10"));
    }

    /// Comparative language ("high"/"many"/"elevated") is forbidden for Dispute
    /// Signals, since it has no cross-node baseline.
    #[test]
    fn disputes_recommendation_never_uses_comparative_language() {
        let metrics = metrics_with(1, 50, "enabled");

        let item = disputes_recommendation(&metrics).expect("recommendation fires");

        let lowercase_message = item.message.to_lowercase();
        assert!(!lowercase_message.contains("high"));
        assert!(!lowercase_message.contains("many"));
        assert!(!lowercase_message.contains("elevated"));
    }

    #[test]
    fn zero_disputes_does_not_trigger_the_disputes_recommendation() {
        let metrics = metrics_with(10, 0, "enabled");

        assert_eq!(disputes_recommendation(&metrics), None);
    }

    #[test]
    fn bond_policy_disabled_triggers_a_recommendation() {
        let metrics = metrics_with(1, 0, "disabled");

        let item = bond_policy_recommendation(&metrics).expect("recommendation fires");

        assert_eq!(item.id, "bond_policy_not_enabled");
        assert_eq!(item.metric, Some("stats.bond_policy.status".to_string()));
        assert!(item.message.contains("does not require a bond deposit"));
    }

    #[test]
    fn bond_policy_unknown_triggers_a_recommendation_distinct_from_disabled() {
        let metrics = metrics_with(1, 0, "unknown");

        let item = bond_policy_recommendation(&metrics).expect("recommendation fires");

        assert_eq!(item.id, "bond_policy_not_enabled");
        assert!(item.message.contains("could not be determined"));
        assert!(!item.message.contains("does not require a bond deposit"));
    }

    #[test]
    fn bond_policy_enabled_does_not_trigger_a_recommendation() {
        let metrics = metrics_with(1, 0, "enabled");

        assert_eq!(bond_policy_recommendation(&metrics), None);
    }

    /// Bond policy must be reported neutrally, never implying which status is safer.
    #[test]
    fn bond_policy_recommendation_never_implies_which_status_is_safer() {
        for status in ["disabled", "unknown"] {
            let metrics = metrics_with(1, 0, status);
            let item = bond_policy_recommendation(&metrics).expect("recommendation fires");
            let lowercase_message = item.message.to_lowercase();

            assert!(!lowercase_message.contains("protection"));
            assert!(!lowercase_message.contains("risk"));
            assert!(!lowercase_message.contains("less safe"));
            assert!(!lowercase_message.contains("more safe"));
            assert!(!lowercase_message.contains("safer"));
        }
    }

    #[test]
    fn nothing_notable_is_true_when_none_of_the_three_triggers_fire() {
        let metrics = metrics_with(5, 0, "enabled");

        let recommendations = assemble_recommendations_section(&metrics);

        assert!(recommendations.nothing_notable);
        assert!(recommendations.items.is_empty());
    }

    #[test]
    fn nothing_notable_is_false_when_any_single_trigger_fires() {
        let metrics = metrics_with(0, 0, "enabled");

        let recommendations = assemble_recommendations_section(&metrics);

        assert!(!recommendations.nothing_notable);
        assert_eq!(recommendations.items.len(), 1);
        assert_eq!(recommendations.items[0].id, "no_completed_trades");
    }

    #[test]
    fn all_three_triggers_can_fire_together_and_each_produces_its_own_item() {
        let metrics = metrics_with(0, 3, "unknown");

        let recommendations = assemble_recommendations_section(&metrics);

        assert!(!recommendations.nothing_notable);
        assert_eq!(recommendations.items.len(), 3);
        let ids: Vec<&str> = recommendations
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect();
        assert_eq!(
            ids,
            vec![
                "no_completed_trades",
                "disputes_present",
                "bond_policy_not_enabled",
            ]
        );
    }

    // ---- SectionFilter::parse ----

    #[test]
    fn section_filter_all_enables_every_field() {
        let filter = SectionFilter::all();

        assert!(filter.fetch);
        assert!(filter.activity);
        assert!(filter.stats);
        assert!(filter.recommendations);
    }

    #[test]
    fn section_filter_parse_accepts_a_single_valid_token() {
        let filter = SectionFilter::parse("stats").expect("valid token");

        assert!(!filter.fetch);
        assert!(!filter.activity);
        assert!(filter.stats);
        assert!(!filter.recommendations);
    }

    #[test]
    fn section_filter_parse_accepts_multiple_valid_tokens_in_any_order() {
        let filter = SectionFilter::parse("stats,fetch").expect("valid tokens");

        assert!(filter.fetch);
        assert!(!filter.activity);
        assert!(filter.stats);
        assert!(!filter.recommendations);
    }

    #[test]
    fn section_filter_parse_accepts_all_four_valid_tokens() {
        let filter =
            SectionFilter::parse("fetch,activity,stats,recommendations").expect("valid tokens");

        assert!(filter.fetch);
        assert!(filter.activity);
        assert!(filter.stats);
        assert!(filter.recommendations);
    }

    #[test]
    fn section_filter_parse_rejects_an_unrecognized_token() {
        let error = SectionFilter::parse("stats,bogus").expect_err("unrecognized token");

        assert_eq!(error, vec!["bogus".to_string()]);
    }

    #[test]
    fn section_filter_parse_collects_every_invalid_token_encountered() {
        let error = SectionFilter::parse("bogus,also-bogus").expect_err("both tokens invalid");

        assert_eq!(error, vec!["bogus".to_string(), "also-bogus".to_string()]);
    }

    /// A capitalized token must be rejected, not silently accepted as its lowercase
    /// equivalent.
    #[test]
    fn section_filter_parse_is_case_sensitive() {
        let error = SectionFilter::parse("Fetch").expect_err("capitalized token is invalid");

        assert_eq!(error, vec!["Fetch".to_string()]);
    }

    #[test]
    fn section_filter_parse_rejects_an_empty_string() {
        let error = SectionFilter::parse("").expect_err("empty string has no valid token");

        assert_eq!(error, vec!["".to_string()]);
    }

    /// No trimming: a token with surrounding whitespace does not match any of the 4
    /// exact names.
    #[test]
    fn section_filter_parse_does_not_trim_whitespace_around_tokens() {
        let error = SectionFilter::parse(" stats ,fetch").expect_err("whitespace is not trimmed");

        assert_eq!(error, vec![" stats ".to_string()]);
    }

    // ---- SectionFilter::from_tokens ----

    #[test]
    fn from_tokens_accepts_valid_tokens() {
        let filter = SectionFilter::from_tokens(&["activity".to_string(), "stats".to_string()])
            .expect("valid tokens");

        assert!(filter.activity);
        assert!(filter.stats);
        assert!(!filter.fetch);
        assert!(!filter.recommendations);
    }

    #[test]
    fn from_tokens_rejects_an_unrecognized_token() {
        let error =
            SectionFilter::from_tokens(&["bogus".to_string()]).expect_err("unrecognized token");

        assert_eq!(error, vec!["bogus".to_string()]);
    }
}
