#!/usr/bin/env python3
"""Common source-rule smells learned from deep fixes (wmp8 / gongzicp)."""

from __future__ import annotations

import re
from typing import Any


def fix_webview_quotes(text: str | None) -> tuple[str | None, bool]:
    """Legado URL options must be JSON with double quotes: {"webView":true}.

    gongzicp lesson: {'webView': true} is ignored → SPA shell → 内容为空.
    """
    if not text or "webView" not in text:
        return text, False
    new = text
    # {'webView': true} / {'webView':true} / {"webView": true} spacing
    new2 = re.sub(
        r"\{\s*'webView'\s*:\s*(true|false)\s*\}",
        r'{"webView":\1}',
        new,
        flags=re.I,
    )
    new2 = re.sub(
        r'\{\s*"webView"\s*:\s*(true|false)\s*\}',
        lambda m: '{"webView":' + m.group(1).lower() + "}",
        new2,
        flags=re.I,
    )
    return new2, new2 != text


def suggest_api_toc(rule_book_info: dict[str, Any] | None, rule_toc: dict[str, Any] | None) -> list[str]:
    """Empty tocUrl + JSON chapterList → need dedicated chapter API (gongzicp)."""
    tips: list[str] = []
    info = rule_book_info or {}
    toc = rule_toc or {}
    toc_url = str(info.get("tocUrl") or "").strip()
    chapter_list = str(toc.get("chapterList") or "")
    if not toc_url and ("$." in chapter_list or "data.list" in chapter_list):
        tips.append(
            "Empty tocUrl but JSON ruleToc — set tocUrl to chapter API "
            "(e.g. gongzicp: .../novel/chapterGetList?nid={{$.novel_id}}). "
            "Reusing novelInfo yields 目录列表为空."
        )
    return tips


def fix_bookurl_class_space(book_url: str | None) -> tuple[str | None, bool]:
    """po18f: `class.X a@href` does not match; use `class.X@tag.a@href`.

    Also drop `||@js:baseUrl` which masks empty bookUrl → detail=search.php.
    """
    if not book_url:
        return book_url, False
    parts = [p.strip() for p in book_url.split("||") if p.strip()]
    cleaned: list[str] = []
    changed = False
    for p in parts:
        if re.fullmatch(r"@?js:baseUrl", p, flags=re.I):
            changed = True
            continue
        # class.foo a@href / class.foo a.0@href → class.foo@tag.a(@.0)?@href
        m = re.match(r"^(class\.\w+)\s+a(?:\.(\d+))?(@href)$", p, flags=re.I)
        if m:
            idx = f".{m.group(2)}" if m.group(2) is not None else ""
            cleaned.append(f"{m.group(1)}@tag.a{idx}{m.group(3)}")
            changed = True
            continue
        cleaned.append(p)
    if not cleaned:
        return book_url, False
    new = "||".join(cleaned)
    return new, new != book_url


def suggest_bookurl_selector(rule_search: dict[str, Any] | None, parsed: dict[str, Any] | None = None) -> list[str]:
    tips: list[str] = []
    rs = rule_search or {}
    bu = str(rs.get("bookUrl") or "")
    if re.search(r"class\.\w+\s+a(?:\.\d+)?@href", bu):
        tips.append(
            "TRAP bookUrl class-space: `class.X a@href` fails in Legado — "
            "use `class.X@tag.a@href` (or `.X a@href`)"
        )
    if "@js:baseUrl" in bu.replace(" ", ""):
        tips.append(
            "TRAP bookUrl||@js:baseUrl: empty bookUrl falls back to search page → fake_detail"
        )
    if parsed and parsed.get("fake_detail") and bu:
        tips.append("fake_detail + bookUrl present — rewrite bookUrl selector before touching toc")
    return tips


def apply_safe_rule_fixes(source: dict[str, Any]) -> list[str]:
    """Apply non-destructive learned fixes. Returns change labels."""
    changes: list[str] = []
    toc = source.get("ruleToc")
    if isinstance(toc, dict):
        cu = toc.get("chapterUrl")
        fixed, changed = fix_webview_quotes(str(cu) if cu is not None else None)
        if changed and fixed is not None:
            toc = dict(toc)
            toc["chapterUrl"] = fixed
            source["ruleToc"] = toc
            changes.append("webview_quotes")
    # also scan content nextContentUrl rarely
    content = source.get("ruleContent")
    if isinstance(content, dict):
        for key in ("nextContentUrl", "content"):
            val = content.get(key)
            if isinstance(val, str) and "webView" in val:
                fixed, changed = fix_webview_quotes(val)
                if changed and fixed is not None:
                    content = dict(content)
                    content[key] = fixed
                    source["ruleContent"] = content
                    changes.append(f"webview_quotes:{key}")
    rs = source.get("ruleSearch")
    if isinstance(rs, dict):
        bu = rs.get("bookUrl")
        fixed_bu, bu_changed = fix_bookurl_class_space(str(bu) if bu is not None else None)
        if bu_changed and fixed_bu is not None:
            rs = dict(rs)
            rs["bookUrl"] = fixed_bu
            source["ruleSearch"] = rs
            changes.append("bookUrl_class_space")
    return changes
