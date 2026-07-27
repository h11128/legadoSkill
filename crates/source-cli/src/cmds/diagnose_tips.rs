//! Build diagnose tips from layer + live probe (Python `repair_diagnose.suggest`).

use source_probe::LiveProbeResult;
use source_types::{DiagnoseResult, Layer};

/// Layer-only tips (no network). Always safe to call.
pub fn layer_tips(diag: &DiagnoseResult) -> Vec<String> {
    let mut tips = Vec::new();
    if diag.fake_detail == Some(true) {
        tips.push(
            "TRAP fake_detail: detail_url is search page / list-empty fallback — fix SEARCH first"
                .into(),
        );
    }
    match diag.layer {
        Layer::Search => {
            tips.push(
                "Fix searchUrl + ruleSearch (bookList/name/bookUrl). Probe forms + common paths + score."
                    .into(),
            );
        }
        Layer::Toc => {
            tips.push("Search OK — do NOT rewrite search. Fix tocUrl + ruleToc.".into());
        }
        Layer::Content => {
            tips.push("TOC OK — fix ruleContent.content against chapter HTML".into());
        }
        Layer::FileDownload => {
            tips.push("type=3: downloadUrls; bookUrl must be detail not search page".into());
        }
        _ => {}
    }
    tips
}

/// Append live-probe tips and fill `evidence.search_url` when best is known.
pub fn enrich_with_live_probe(diag: &mut DiagnoseResult, live: &LiveProbeResult) {
    if diag.tips.is_empty() {
        diag.tips = layer_tips(diag);
    }
    if live.search_endpoint_dead {
        diag.tips.push(
            "TRAP 搜索口挂了: form endpoint HTTP 5xx — SKIP (not a selector bug)".into(),
        );
    }
    if let Some(ref best) = live.best {
        if best.score >= 2 {
            diag.evidence.search_url = Some(best.search_url.clone());
            diag.tips.push(format!(
                "probe.best score={} url={}",
                best.score, best.search_url
            ));
        } else if !live.search_endpoint_dead {
            diag.tips.push(format!(
                "probe.best weak score={} — try common paths / JS forms",
                best.score
            ));
        }
        if live.gbk {
            diag.tips
                .push("GBK meta detected — append ,{\"charset\":\"GBK\"} on searchUrl".into());
        }
    } else if diag.layer == Layer::Search && !live.search_endpoint_dead {
        if let Some(f) = live.offline.forms.first() {
            diag.tips
                .push(format!("form action (no live best): {}", f.action));
        }
    }
    if live
        .ranked
        .first()
        .map(|r| r.score <= 0)
        .unwrap_or(false)
        && !live.search_endpoint_dead
    {
        diag.tips.push(
            "TRAP: form candidates scored ≤0 (homepage shell?) — try /search.php?q= etc.".into(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use source_types::Url;

    #[test]
    fn fake_detail_tip() {
        let mut d = DiagnoseResult::new(Url::new("http://ex.com/").unwrap(), Layer::Search);
        d.fake_detail = Some(true);
        let tips = layer_tips(&d);
        assert!(tips.iter().any(|t| t.contains("fake_detail")));
    }
}
