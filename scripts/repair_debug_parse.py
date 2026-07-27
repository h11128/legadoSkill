#!/usr/bin/env python3
"""Parse debug_source / check text into repair layers."""

from __future__ import annotations

import re
from typing import Any
from urllib.parse import urlparse

# wmp8: empty list → parse search page as "1 book" → false toc layer
_SEARCH_PATH_RE = re.compile(
    r"(?:/s\.php\b|/search\.php\b|/so\.php\b|/modules/article/search\.php\b|"
    r"/search(?:\.html)?(?:\?|$)|[?&](?:keyword|searchkey|q|wd)=)",
    re.I,
)


def looks_like_search_url(url: str | None) -> bool:
    if not url:
        return False
    u = url.split()[0]
    p = urlparse(u)
    return bool(_SEARCH_PATH_RE.search((p.path or "") + "?" + (p.query or "")))


def parse_debug_text(text: str) -> dict[str, Any]:
    """Extract search/toc/content signals from a debug_source log."""
    text = text or ""
    out: dict[str, Any] = {
        "search_list": None,
        "search_books": None,
        "toc_list": None,
        "toc_chapters": None,
        "content_empty": "内容为空" in text or "ContentEmptyException" in text,
        "toc_empty": "目录列表为空" in text or "TocEmptyException" in text,
        "download_empty": "下载链接为空" in text,
        "channel_busy": "调试通道占用" in text or "校验通道占用" in text,
        "list_empty_fallback_detail": "列表为空,按详情页解析" in text or "列表为空，按详情页解析" in text,
        "detail_url": None,
        "toc_url": None,
        "fake_detail": False,
        "layer": "unknown",
    }
    sizes = [int(x) for x in re.findall(r"列表大小:(\d+)", text)]
    books = re.search(r"书籍总数:(\d+)", text)
    chapters = re.search(r"目录总数:(\d+)", text)
    if books:
        out["search_books"] = int(books.group(1))
    if chapters:
        out["toc_chapters"] = int(chapters.group(1))
    if sizes:
        out["search_list"] = sizes[0]
        if len(sizes) > 1:
            out["toc_list"] = sizes[1]
    for m in re.finditer(r"≡获取成功:(.+)", text):
        u = m.group(1).strip().split()[0]
        if looks_like_search_url(u):
            continue  # never treat search endpoint as detail
        if out["detail_url"] is None and u.startswith("http"):
            out["detail_url"] = u
        if "/list/" in u or "mulu" in u or "catalog" in u or "/index/" in u:
            out["toc_url"] = u
    # Fake detail: empty list → parse search page as 1 book (wmp8)
    fake = False
    if out["list_empty_fallback_detail"] and (out["search_books"] or 0) <= 1 and (
        out["search_list"] in (0, None)
    ):
        fake = True
    if looks_like_search_url(out.get("detail_url")):
        fake = True
    # Strong search list → not fake even if early log lines mention search URL
    if (out["search_list"] or 0) >= 2 or (out["search_books"] or 0) >= 2:
        if not out["list_empty_fallback_detail"]:
            fake = False
    out["fake_detail"] = fake

    if out["channel_busy"]:
        out["layer"] = "busy"
    elif out["download_empty"]:
        out["layer"] = "file_download"
    elif fake:
        out["layer"] = "search"  # wmp8: do NOT treat as toc
    elif out["search_books"] == 0 or (
        out["search_list"] == 0 and out["search_books"] is None and "未获取到书籍" in text
    ):
        out["layer"] = "search"
    elif out["toc_empty"] or out["toc_list"] == 0 or out["toc_chapters"] == 0:
        out["layer"] = "toc"
    elif out["content_empty"]:
        out["layer"] = "content"
    elif (out["search_books"] or 0) > 0 and (out["toc_chapters"] or 0) > 0:
        out["layer"] = "ok"
    elif (out["search_books"] or 0) > 0:
        out["layer"] = "toc"
    return out


def layer_from_check_message(msg: str) -> str:
    msg = msg or ""
    # ignore 发现 for default repair routing
    for tok in ("发现正文失效", "发现目录失效", "发现规则为空", "发现失效"):
        msg = msg.replace(tok, "")
    if "搜索目录" in msg or ("目录" in msg and "搜索" in msg):
        return "toc"
    if "搜索正文" in msg or ("正文" in msg and "搜索" in msg):
        return "content"
    if "搜索失效" in msg:
        return "search"
    if "目录" in msg:
        return "toc"
    if "正文" in msg:
        return "content"
    if "下载链接" in msg:
        return "file_download"
    return "unknown"


def meaningful_changes(changes: list[str]) -> list[str]:
    """Filter out rate-only noise (wave20 lesson)."""
    return [c for c in changes if "concurrentRate" not in c]
