#!/usr/bin/env python3
"""Decision tree + resolved-URL classification for repair."""

from __future__ import annotations

import re
from typing import Any
from urllib.parse import urlparse

from repair_helpers import layer_for_fail

# Priority for repair queues (lower = sooner)
LAYER_PRIORITY = {
    "toc": 10,
    "content": 20,
    "search": 30,
    "js": 40,
    "timeout": 80,
    "unknown": 90,
    "skip": 100,
}

DISABLE_LAYERS = frozenset({"skip"})
# fail-msg / group hints that mean disable without repair
DISABLE_HINTS = (
    "域名失效",
    "网站失效",
    "下载链接为空",
    "非书源",
    "Timed out",
    "校验超时",
)


def decide(fail_msg: str, smells: list[dict[str, str]] | None = None) -> dict[str, Any]:
    layer = layer_for_fail(fail_msg)
    action = "fix"
    reason = layer
    if layer in DISABLE_LAYERS or any(h in (fail_msg or "") for h in DISABLE_HINTS):
        # timeout alone: still try once unless explicitly 域名/网站失效
        if "超时" in (fail_msg or "") or "Timed out" in (fail_msg or ""):
            if any(h in (fail_msg or "") for h in ("域名失效", "网站失效")):
                action, reason = "disable", "dead_host"
            else:
                action, reason = "skip", "timeout_defer"
        elif "下载链接" in (fail_msg or ""):
            action, reason = "skip", "file_source"
        else:
            action, reason = "disable", "dead_or_invalid"
    if layer == "toc" and smells:
        issues = {s.get("issue") for s in smells}
        if issues & {"broad_a_href_regex", "maybe_content_not_catalog"}:
            action, reason = "auto_patch", "toc_smell"
    return {
        "layer": layer,
        "action": action,
        "reason": reason,
        "priority": LAYER_PRIORITY.get(layer, 90),
    }


def classify_resolved_url(url: str, html: str | None = None) -> dict[str, Any]:
    """Classify a resolved toc/detail URL: homepage | content | catalog | other."""
    p = urlparse(url or "")
    path = (p.path or "/").rstrip("/") or "/"
    kind = "other"
    if path == "/" or path.lower() in {"/index", "/index.html", "/index.htm"}:
        kind = "homepage"
    elif re.search(r"/\d+\.html?$", path) and not re.search(
        r"/(book|info|txt|xs|novel)/", path, re.I
    ):
        # …/20810/1.html style chapter content
        if re.search(r"/\d+/\d+\.html?$", path):
            kind = "content"
    elif re.search(r"/(read|chapter|mulu|catalog|toc)(/|$)", path, re.I):
        kind = "catalog"
    if html:
        low = html.lower()
        catalog_hits = sum(
            1
            for k in ("chapter-list", "catalog", "mulu", "章节列表", "目录")
            if k in low or k in html
        )
        content_hits = sum(
            1 for k in ("yd_text", "content_txt", "chaptercontent", "正文") if k in low
        )
        if catalog_hits >= 2 and kind != "homepage":
            kind = "catalog"
        elif content_hits >= 2 and catalog_hits == 0:
            kind = "content"
    return {"url": url, "kind": kind, "path": path}


def queue_sort_key(item: dict[str, Any]) -> tuple[int, str]:
    fail = str(item.get("message") or item.get("fail_msg") or "")
    d = decide(fail)
    return (int(d["priority"]), str(item.get("url") or ""))
