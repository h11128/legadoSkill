#!/usr/bin/env python3
"""Lightweight knowledge lookup for repair (docs + css notes + past retros)."""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
SEARCH_ROOTS = [
    ROOT / "docs",
    ROOT / "assets",
]
GLOBS = ("*.md", "*.txt")
MAX_HITS = 8
MAX_SNIPPET = 220


def _iter_files() -> list[Path]:
    files: list[Path] = []
    for root in SEARCH_ROOTS:
        if not root.is_dir():
            continue
        for pattern in GLOBS:
            files.extend(root.rglob(pattern))
    # Prefer high-value docs first
    rank = {
        "ESSENTIAL_KNOWLEDGE_SUMMARY.md": 0,
        "TOC_PAGINATION_RULES.md": 1,
        "HTML_AUTHENTICITY_CHECKLIST.md": 2,
        "source-repair-retrospective.md": 3,
        "css选择器规则.txt": 4,
    }
    files.sort(key=lambda p: (rank.get(p.name, 50), str(p)))
    return files


def search_knowledge(query: str, layer: str = "") -> list[dict[str, Any]]:
    tokens = [t for t in re.split(r"[\s,|/]+", query) if len(t) >= 2]
    if layer:
        tokens.append(layer)
    extra = {
        "toc": ["tocUrl", "目录", "catalog"],
        "content": ["正文", "content"],
        "search": ["searchUrl", "搜索"],
    }.get(layer, [])
    tokens.extend(extra)
    tokens = list(dict.fromkeys(tokens))[:12]
    hits: list[dict[str, Any]] = []
    for path in _iter_files():
        try:
            text = path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        score = 0
        matched = []
        for tok in tokens:
            if tok.lower() in text.lower() or tok in text:
                score += text.lower().count(tok.lower()[:40])
                matched.append(tok)
        if score <= 0:
            continue
        # snippet around first match
        snippet = text[:MAX_SNIPPET].replace("\n", " ")
        for tok in matched:
            idx = text.lower().find(tok.lower())
            if idx >= 0:
                start = max(0, idx - 60)
                snippet = text[start : start + MAX_SNIPPET].replace("\n", " ")
                break
        hits.append({
            "path": str(path.relative_to(ROOT)),
            "score": score,
            "matched": matched[:6],
            "snippet": snippet,
        })
        if len(hits) >= MAX_HITS * 3:
            break
    hits.sort(key=lambda h: -int(h["score"]))
    return hits[:MAX_HITS]
