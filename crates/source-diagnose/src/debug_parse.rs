//! Parse debug_source / check text into repair layers (Python `repair_debug_parse`).

use regex::Regex;
use serde::{Deserialize, Serialize};
use source_types::Layer;
use std::sync::OnceLock;

fn search_path_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)(?:/s\.php\b|/search\.php\b|/so\.php\b|/modules/article/search\.php\b|/search(?:\.html)?(?:\?|$)|[?&](?:keyword|searchkey|q|wd)=)",
        )
        .expect("search path regex")
    })
}

/// True when URL path/query looks like a search endpoint (wmp8 trap).
pub fn looks_like_search_url(url: Option<&str>) -> bool {
    let Some(raw) = url.filter(|u| !u.is_empty()) else {
        return false;
    };
    let u = raw.split_whitespace().next().unwrap_or(raw);
    let (path, query) = match u.find('?') {
        Some(i) => (&u[..i], &u[i + 1..]),
        None => {
            // strip scheme://host
            if let Some(rest) = u.split("://").nth(1) {
                let path = rest.find('/').map(|i| &rest[i..]).unwrap_or("/");
                (path, "")
            } else {
                (u, "")
            }
        }
    };
    let blob = format!("{path}?{query}");
    search_path_re().is_match(&blob)
}

/// Structured parse of a `debug_source` log (Python `parse_debug_text`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DebugParse {
    pub search_list: Option<i64>,
    pub search_books: Option<i64>,
    pub toc_list: Option<i64>,
    pub toc_chapters: Option<i64>,
    pub content_empty: bool,
    pub toc_empty: bool,
    pub download_empty: bool,
    pub channel_busy: bool,
    pub list_empty_fallback_detail: bool,
    pub detail_url: Option<String>,
    pub toc_url: Option<String>,
    pub fake_detail: bool,
    /// Raw layer string before contract mapping (`busy`/`unknown` → skip).
    pub layer_raw: String,
}

impl DebugParse {
    pub fn layer_contract(&self) -> Layer {
        match self.layer_raw.as_str() {
            "search" => Layer::Search,
            "toc" => Layer::Toc,
            "content" => Layer::Content,
            "file_download" => Layer::FileDownload,
            "ok" => Layer::Ok,
            "explore" => Layer::Explore,
            // busy / unknown → skip for DiagnoseResult schema
            _ => Layer::Skip,
        }
    }
}

pub fn parse_debug_text(text: &str) -> DebugParse {
    let text = text;
    let mut out = DebugParse {
        content_empty: text.contains("内容为空") || text.contains("ContentEmptyException"),
        toc_empty: text.contains("目录列表为空") || text.contains("TocEmptyException"),
        download_empty: text.contains("下载链接为空"),
        channel_busy: text.contains("调试通道占用") || text.contains("校验通道占用"),
        list_empty_fallback_detail: text.contains("列表为空,按详情页解析")
            || text.contains("列表为空，按详情页解析"),
        layer_raw: "unknown".into(),
        ..DebugParse::default()
    };

    let size_re = Regex::new(r"列表大小:(\d+)").expect("size re");
    let sizes: Vec<i64> = size_re
        .captures_iter(text)
        .filter_map(|c| c.get(1)?.as_str().parse().ok())
        .collect();
    if let Some(c) = Regex::new(r"书籍总数:(\d+)")
        .ok()
        .and_then(|r| r.captures(text))
    {
        out.search_books = c.get(1).and_then(|m| m.as_str().parse().ok());
    }
    if let Some(c) = Regex::new(r"目录总数:(\d+)")
        .ok()
        .and_then(|r| r.captures(text))
    {
        out.toc_chapters = c.get(1).and_then(|m| m.as_str().parse().ok());
    }
    if !sizes.is_empty() {
        out.search_list = Some(sizes[0]);
        if sizes.len() > 1 {
            out.toc_list = Some(sizes[1]);
        }
    }

    let get_re = Regex::new(r"≡获取成功:(.+)").expect("get re");
    for m in get_re.captures_iter(text) {
        let u = m
            .get(1)
            .map(|x| x.as_str().trim().split_whitespace().next().unwrap_or(""))
            .unwrap_or("");
        if looks_like_search_url(Some(u)) {
            continue;
        }
        if out.detail_url.is_none() && u.starts_with("http") {
            out.detail_url = Some(u.to_string());
        }
        let low = u.to_ascii_lowercase();
        if low.contains("/list/")
            || low.contains("mulu")
            || low.contains("catalog")
            || low.contains("/index/")
        {
            out.toc_url = Some(u.to_string());
        }
    }

    let mut fake = false;
    let books = out.search_books.unwrap_or(0);
    if out.list_empty_fallback_detail
        && books <= 1
        && matches!(out.search_list, Some(0) | None)
    {
        fake = true;
    }
    if looks_like_search_url(out.detail_url.as_deref()) {
        fake = true;
    }
    if (out.search_list.unwrap_or(0) >= 2 || books >= 2) && !out.list_empty_fallback_detail {
        fake = false;
    }
    out.fake_detail = fake;

    out.layer_raw = if out.channel_busy {
        "busy".into()
    } else if out.download_empty {
        "file_download".into()
    } else if fake {
        "search".into()
    } else if out.search_books == Some(0)
        || (out.search_list == Some(0) && out.search_books.is_none() && text.contains("未获取到书籍"))
    {
        "search".into()
    } else if out.toc_empty || out.toc_list == Some(0) || out.toc_chapters == Some(0) {
        "toc".into()
    } else if out.content_empty {
        "content".into()
    } else if books > 0 && out.toc_chapters.unwrap_or(0) > 0 {
        "ok".into()
    } else if books > 0 {
        "toc".into()
    } else {
        "unknown".into()
    };
    out
}

pub fn layer_from_check_message(msg: &str) -> Layer {
    let mut msg = msg.to_string();
    for tok in ["发现正文失效", "发现目录失效", "发现规则为空", "发现失效"] {
        msg = msg.replace(tok, "");
    }
    if msg.contains("搜索目录") || (msg.contains("目录") && msg.contains("搜索")) {
        return Layer::Toc;
    }
    if msg.contains("搜索正文") || (msg.contains("正文") && msg.contains("搜索")) {
        return Layer::Content;
    }
    if msg.contains("搜索失效") {
        return Layer::Search;
    }
    if msg.contains("目录") {
        return Layer::Toc;
    }
    if msg.contains("正文") {
        return Layer::Content;
    }
    if msg.contains("下载链接") {
        return Layer::FileDownload;
    }
    Layer::Skip // unknown → skip for contract
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_detail_forces_search() {
        let text = "列表为空,按详情页解析\n书籍总数:1\n列表大小:0\n≡获取成功:https://m.wmp8.com/s.php?q=x";
        let p = parse_debug_text(text);
        assert!(p.fake_detail);
        assert_eq!(p.layer_raw, "search");
        assert_eq!(p.layer_contract(), Layer::Search);
    }

    #[test]
    fn toc_empty_layer() {
        let text = "书籍总数:3\n目录列表为空\n列表大小:3\n列表大小:0";
        let p = parse_debug_text(text);
        assert_eq!(p.layer_raw, "toc");
    }

    #[test]
    fn check_msg_search() {
        assert_eq!(layer_from_check_message("校验失败:搜索失效"), Layer::Search);
    }
}
