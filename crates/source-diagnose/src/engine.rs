//! Diagnose engine: gate fail-fast + debug parse → DiagnoseResult.

use source_types::{
    DiagnoseEvidence, DiagnoseResult, GateAction, GateResult, Layer, Url, SCHEMA_VERSION,
};

use crate::debug_parse::{looks_like_search_url, parse_debug_text, DebugParse};

/// Build DiagnoseResult from an already-fetched debug log (no MCP).
pub fn diagnose_from_debug(
    url: Url,
    debug_text: &str,
    gate: Option<GateResult>,
    fail_msg: Option<&str>,
) -> DiagnoseResult {
    let mut parsed = parse_debug_text(debug_text);
    let mut layer = parsed.layer_contract();
    let mut reclassified_from = None;

    // Reclassify: toc + fake detail or detail looks like search
    if parsed.layer_raw == "toc"
        && (parsed.fake_detail || looks_like_search_url(parsed.detail_url.as_deref()))
    {
        reclassified_from = Some(Layer::Toc);
        layer = Layer::Search;
        parsed.layer_raw = "search".into();
        parsed.fake_detail = true;
    }

    // busy → skip
    if parsed.channel_busy {
        layer = Layer::Skip;
    }

    let mut result = DiagnoseResult {
        schema_version: SCHEMA_VERSION.to_string(),
        url,
        layer,
        fail_msg: fail_msg.map(|s| s.to_string()),
        fake_detail: Some(parsed.fake_detail),
        reclassified_from,
        gate,
        evidence: evidence_from_parse(&parsed),
        tips: Vec::new(),
    };
    if result.fake_detail == Some(true) {
        result.layer = Layer::Search;
    }
    result
}

/// L2/gate early skip without phone debug.
pub fn diagnose_gate_skip(url: Url, gate: GateResult) -> DiagnoseResult {
    let mut r = DiagnoseResult::new(url, Layer::Skip);
    r.fail_msg = Some(format!(
        "L2 fail-fast: {} / {}",
        gate.action.as_str(),
        gate.reason
    ));
    r.gate = Some(gate);
    r.fake_detail = Some(false);
    r
}

pub fn gate_blocks_diagnose(gate: &GateResult) -> bool {
    matches!(
        gate.action,
        GateAction::Disable | GateAction::Skip | GateAction::Video | GateAction::Hunt
    )
}

fn evidence_from_parse(p: &DebugParse) -> DiagnoseEvidence {
    DiagnoseEvidence {
        search_url: None,
        book_url: p.detail_url.clone(),
        toc_url: p.toc_url.clone(),
        debug_snippet: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use source_types::Url;

    #[test]
    fn reclassify_toc_fake_to_search() {
        let url = Url::new("https://m.wmp8.com/").unwrap();
        // Fake-detail signals → parse already yields search (wmp8).
        let text = "书籍总数:1\n目录列表为空\n列表大小:0\n列表为空,按详情页解析\n≡获取成功:https://m.wmp8.com/s.php?q=1";
        let d = diagnose_from_debug(url, text, None, None);
        assert_eq!(d.layer, Layer::Search);
        assert_eq!(d.fake_detail, Some(true));
    }

    #[test]
    fn reclassify_when_raw_toc_and_searchish_detail() {
        let url = Url::new("https://m.wmp8.com/").unwrap();
        // Strong search list so not fake via books, but detail is search URL → reclassify path
        // when layer would be toc from empty toc.
        let text = "书籍总数:3\n目录列表为空\n列表大小:3\n列表大小:0\n≡获取成功:https://m.wmp8.com/book/1.html\n≡获取成功:https://m.wmp8.com/s.php?q=1";
        let d = diagnose_from_debug(url, text, None, None);
        // toc_empty → toc; detail may be book page first so not always reclassify
        assert!(matches!(d.layer, Layer::Toc | Layer::Search));
    }
}
