//! Family scoring and Identify thresholds (§3.4).

use source_pattern::structural_hash_from_source;
use source_types::{
    BookSource, Fingerprint, FingerprintRule, IdentifyResult, IdentifyRunnerUp, RepairConfig,
    SiteFamily, Url,
};

use crate::match_rule::rule_matches;

/// Rules belonging to one family (borrowed slice).
#[derive(Debug, Clone, Copy)]
pub struct FamilyRules<'a> {
    pub family: &'a SiteFamily,
    pub rules: &'a [FingerprintRule],
}

/// Sum matched rule weights; return (score, signal ids).
pub fn score_family(
    rules: &[FingerprintRule],
    source: &BookSource,
    html: &str,
) -> (f64, Vec<String>) {
    let mut score = 0.0;
    let mut signals = Vec::new();
    for r in rules {
        if r.weight <= 0.0 {
            continue;
        }
        if rule_matches(r, source, html) {
            score += r.weight;
            signals.push(r.id.clone());
        }
    }
    signals.sort();
    signals.dedup();
    (score, signals)
}

/// `family = argmax`; Unknown when below min_score or margin to runner-up.
pub fn identify(
    url: Url,
    source: &BookSource,
    html: &str,
    families: &[FamilyRules<'_>],
    config: &RepairConfig,
) -> IdentifyResult {
    let mut scored: Vec<(SiteFamily, f64, Vec<String>)> = families
        .iter()
        .map(|f| {
            let (score, signals) = score_family(f.rules, source, html);
            (f.family.clone(), score, signals)
        })
        .collect();
    scored.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.as_str().cmp(b.0.as_str()))
    });

    let top = scored.first().cloned();
    let runner = scored.get(1).map(|(fam, score, _)| IdentifyRunnerUp {
        family: fam.clone(),
        score: *score,
    });

    let (mut family, score, signals) = match top {
        Some((fam, sc, sig)) => (fam, sc, sig),
        None => (SiteFamily::unknown(), 0.0, Vec::new()),
    };

    let below_min = score < config.identify_min_score;
    let thin_margin = runner
        .as_ref()
        .map(|r| score - r.score < config.identify_margin)
        .unwrap_or(false);
    if below_min || thin_margin {
        family = SiteFamily::unknown();
    }

    let confidence = if score <= 0.0 {
        0.0
    } else {
        (score / (score + config.identify_min_score)).clamp(0.0, 1.0)
    };

    let fingerprint = Fingerprint {
        signals,
        structural_hash: structural_hash_from_source(source),
        confidence,
    };

    let mut result = IdentifyResult::new(url, family, fingerprint, score);
    result.runner_up = runner;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use source_types::{FingerprintMatchKind, FingerprintRule};

    fn fr(id: &str, weight: f64, kind: FingerprintMatchKind, pattern: &str) -> FingerprintRule {
        FingerprintRule {
            id: id.into(),
            weight,
            match_kind: kind,
            pattern: pattern.into(),
        }
    }

    #[test]
    fn identifies_when_margin_ok() {
        let xun = SiteFamily::new(SiteFamily::XUNSEARCH_PID);
        let jieqi = SiteFamily::new(SiteFamily::JIEQI_MOBILE);
        let xun_rules = vec![
            fr(
                "search:xunsearch_q",
                2.0,
                FingerprintMatchKind::SearchUrlRegex,
                r"search\.php\?q=",
            ),
            fr(
                "html:xunsearch",
                1.5,
                FingerprintMatchKind::HtmlRegex,
                r"(?i)xunsearch",
            ),
        ];
        let jieqi_rules = vec![fr(
            "list:sitebox",
            1.0,
            FingerprintMatchKind::SelectorPresent,
            "#sitebox",
        )];
        let src = BookSource::new(json!({
            "searchUrl": "/search.php?q={{key}}",
            "bookSourceType": 0
        }));
        let html = "powered by xunsearch";
        let families = [
            FamilyRules {
                family: &xun,
                rules: &xun_rules,
            },
            FamilyRules {
                family: &jieqi,
                rules: &jieqi_rules,
            },
        ];
        let r = identify(
            Url::new("https://ex.com").unwrap(),
            &src,
            html,
            &families,
            &RepairConfig::default(),
        );
        assert_eq!(r.family.as_str(), SiteFamily::XUNSEARCH_PID);
        assert!(r.score >= 3.0);
        assert!(r.runner_up.is_some());
    }

    #[test]
    fn unknown_below_min_score() {
        let fam = SiteFamily::new(SiteFamily::GENERIC_FORM);
        let rules = vec![fr(
            "weak",
            0.5,
            FingerprintMatchKind::HtmlRegex,
            r"form",
        )];
        let src = BookSource::new(json!({}));
        let families = [FamilyRules {
            family: &fam,
            rules: &rules,
        }];
        let r = identify(
            Url::new("https://ex.com").unwrap(),
            &src,
            "<form>",
            &families,
            &RepairConfig::default(),
        );
        assert!(r.family.is_unknown());
        assert!(r.score < 2.0);
    }

    #[test]
    fn unknown_when_margin_thin() {
        let a = SiteFamily::new("A");
        let b = SiteFamily::new("B");
        let rules_a = vec![fr(
            "a",
            2.2,
            FingerprintMatchKind::HtmlRegex,
            r"alpha",
        )];
        let rules_b = vec![fr(
            "b",
            2.0,
            FingerprintMatchKind::HtmlRegex,
            r"beta",
        )];
        let src = BookSource::new(json!({}));
        let html = "alpha beta";
        let families = [
            FamilyRules {
                family: &a,
                rules: &rules_a,
            },
            FamilyRules {
                family: &b,
                rules: &rules_b,
            },
        ];
        let r = identify(
            Url::new("https://ex.com").unwrap(),
            &src,
            html,
            &families,
            &RepairConfig::default(),
        );
        // margin 0.2 < 0.5 → Unknown
        assert!(r.family.is_unknown());
        assert!(r.runner_up.is_some());
    }
}
