#!/usr/bin/env python3
"""Pure helpers for book-source repair (no MCP I/O except via callers)."""

from __future__ import annotations

import json
import re
import urllib.error
import urllib.request
from typing import Any
from urllib.parse import urljoin

RATE_HINTS = ("搜索时间间隔", "请稍后再搜索", "访问过于频繁", "人机验证", "captcha")
TOC_HINTS = ("目录", "章节", "在线阅读", "开始阅读", "阅读", "catalog", "chapter", "read")
SKIP_TAGS = ("域名失效", "网站失效")


def layer_for_fail(msg: str) -> str:
    m = msg or ""
    if any(t in m for t in SKIP_TAGS):
        return "skip"
    if "目录" in m:
        return "toc"
    if "正文" in m:
        return "content"
    if "js失效" in m or "EcmaError" in m:
        return "js"
    if "下载链接" in m:
        return "skip"
    if "搜索" in m or "发现" in m:
        return "search"
    if "超时" in m or "Timed out" in m:
        return "timeout"
    return "unknown"


def smell_rules(source: dict[str, Any]) -> list[dict[str, str]]:
    smells: list[dict[str, str]] = []
    info = source.get("ruleBookInfo") or {}
    if not isinstance(info, dict):
        info = {}
    toc = str(info.get("tocUrl") or "")
    name = str(info.get("name") or "")
    if re.search(r"a@href##", toc) and "text." not in toc:
        smells.append({
            "field": "ruleBookInfo.tocUrl",
            "issue": "broad_a_href_regex",
            "hint": "May resolve first unmatched link to homepage; narrow selector",
        })
    if "||" in name and "##" in name:
        smells.append({
            "field": "ruleBookInfo.name",
            "issue": "fallback_mixed_with_regex",
            "hint": "Do not mix || fallback with ## replace on same field",
        })
    if re.search(r"(在线阅读|开始阅读|全文)", toc) and "read" in toc.lower():
        smells.append({
            "field": "ruleBookInfo.tocUrl",
            "issue": "maybe_content_not_catalog",
            "hint": "tocUrl may point at content page; clear or retarget catalog",
        })
    return smells


def header_map(source: dict[str, Any]) -> dict[str, str]:
    raw = source.get("header") or source.get("headerMap") or ""
    headers = {
        "User-Agent": (
            "Mozilla/5.0 (Linux; Android 13) AppleWebKit/537.36 "
            "(KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36"
        ),
        "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
    }
    if isinstance(raw, dict):
        headers.update({str(k): str(v) for k, v in raw.items()})
        return headers
    text = str(raw).strip()
    if not text:
        return headers
    try:
        parsed = json.loads(text)
        if isinstance(parsed, dict):
            headers.update({str(k): str(v) for k, v in parsed.items()})
            return headers
    except json.JSONDecodeError:
        pass
    for line in text.replace("\\n", "\n").splitlines():
        if ":" in line:
            k, v = line.split(":", 1)
            headers[k.strip()] = v.strip()
    return headers


def fetch_page(url: str, headers: dict[str, str], timeout: float = 25.0) -> dict[str, Any]:
    req = urllib.request.Request(url, headers=headers, method="GET")
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            body = resp.read()
            final = resp.geturl()
            ctype = resp.headers.get("Content-Type", "")
            code = resp.status
    except urllib.error.HTTPError as exc:
        body = exc.read() if exc.fp else b""
        final, ctype, code = url, "", int(exc.code)
    except urllib.error.URLError as exc:
        return {"ok": False, "error": str(exc), "url": url}
    text = body.decode("utf-8", errors="replace")
    rate = any(h in text for h in RATE_HINTS)
    links: list[dict[str, str]] = []
    for m in re.finditer(
        r'<a[^>]+href=["\']([^"\']+)["\'][^>]*>(.*?)</a>',
        text,
        flags=re.I | re.S,
    ):
        href, label = m.group(1), re.sub(r"<[^>]+>", "", m.group(2)).strip()
        label_l = label.lower()
        if any(h in label or h in label_l or h in href.lower() for h in TOC_HINTS):
            links.append({"text": label[:80], "href": urljoin(final, href)})
            if len(links) >= 30:
                break
    return {
        "ok": True,
        "status": code,
        "final_url": final,
        "content_type": ctype,
        "bytes": len(body),
        "body": body,
        "rate_limited": rate,
        "toc_candidate_links": links[:20],
        "snippet": text[:500],
    }
