#!/usr/bin/env python3
"""Diagnose why wave20 candidates failed (HTTP + debug_source)."""

from __future__ import annotations

import json
import re
import ssl
import sys
import urllib.request
from collections import Counter
from pathlib import Path
from urllib.parse import urlparse

_ROOT = Path(__file__).resolve().parents[1]
_SCRIPTS = Path(__file__).resolve().parent
sys.path.insert(0, str(_SCRIPTS))

from mcp_client import ensure_session, extract_text, get_source, tools_call  # noqa: E402

UA = {
    "User-Agent": (
        "Mozilla/5.0 (Linux; Android 13) AppleWebKit/537.36 "
        "(KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36"
    )
}


def bucket(r: dict) -> str:
    if r.get("http_err"):
        err = r["http_err"]
        if "404" in err:
            return "dead_404"
        if "403" in err:
            return "blocked_403"
        if "401" in err:
            return "auth_401"
        if "451" in err:
            return "legal_451"
        return "http_dead"
    n = r.get("debug_books")
    if n == 0:
        return "alive_search_zero"
    if isinstance(n, int) and n > 0:
        return "search_ok_need_deeper"
    return "unknown"


def main() -> int:
    cfg = json.loads((_ROOT / "config" / "mcp_defaults.json").read_text(encoding="utf-8"))
    mcp, token = cfg["mcp_url"], cfg.get("token") or "1234"
    ensure_session(mcp, token, "why_fail")
    urls = [
        ln.strip()
        for ln in (_ROOT / "temp/full_fix/bench20_urls.txt").read_text(encoding="utf-8").splitlines()
        if ln.strip()
    ]
    ctx = ssl._create_unverified_context()
    rows = []
    for url in urls:
        base = url.split("#", 1)[0]
        if "://" not in base:
            base = "http://" + base
        row: dict = {"url": url, "host": (urlparse(base).hostname or "").lower()}
        try:
            req = urllib.request.Request(base, headers=UA)
            with urllib.request.urlopen(req, timeout=10, context=ctx) as resp:
                body = resp.read(12000)
                row["http"] = resp.status
                row["final"] = resp.geturl()[:100]
                text = body.decode("utf-8", "replace")
                if len(text.strip()) < 50:
                    text = body.decode("gbk", "replace")
                row["html_len"] = len(body)
                row["has_form"] = bool(re.search(r"<form", text, re.I))
                row["has_search"] = bool(
                    re.search(r"search|keyword|name=[\"']q[\"']|name=[\"']wd[\"']", text, re.I)
                )
                m = re.search(r"<title>(.*?)</title>", text, re.I | re.S)
                row["title"] = re.sub(r"\s+", " ", m.group(1))[:60] if m else ""
        except Exception as exc:  # noqa: BLE001
            row["http_err"] = str(exc)[:140]

        try:
            src = get_source(mcp, token, url)
            row["name"] = src.get("bookSourceName")
            row["type"] = src.get("bookSourceType")
            row["searchUrl"] = (src.get("searchUrl") or "")[:120]
            raw = extract_text(
                tools_call(mcp, token, "debug_source", {"url": url, "key": "我的"}, timeout=50)
            )
            m = re.search(r"书籍总数:(\d+)", raw) or re.search(r"列表大小:(\d+)", raw)
            row["debug_books"] = int(m.group(1)) if m else 0
            lines = [
                ln
                for ln in raw.splitlines()
                if any(x in ln for x in ("列表", "书籍总数", "未获取", "Exception", "下载链接"))
            ]
            row["debug_hint"] = " | ".join(lines[:3])[:200]
        except Exception as exc:  # noqa: BLE001
            row["src_err"] = str(exc)[:120]

        row["bucket"] = bucket(row)
        rows.append(row)
        status = row.get("http_err") or row.get("http")
        print(
            f"{status!s:>22} books={row.get('debug_books')} "
            f"{(row.get('name') or '')[:14]:14} {url[:48]}",
            flush=True,
        )

    out = _ROOT / "temp/full_fix/wave20_why.json"
    out.write_text(json.dumps(rows, ensure_ascii=False, indent=2), encoding="utf-8")
    print("\nBUCKETS", dict(Counter(r["bucket"] for r in rows)))
    for b, _ in Counter(r["bucket"] for r in rows).most_common():
        print(f"\n## {b}")
        for r in rows:
            if r["bucket"] != b:
                continue
            print(
                f"  {r['url'][:52]} form={r.get('has_form')} "
                f"searchish={r.get('has_search')} title={r.get('title')!r}"
            )
            print(f"    searchUrl={r.get('searchUrl')!r}")
            print(f"    hint={r.get('debug_hint') or r.get('http_err') or r.get('src_err')}")
    print(f"\nwrote {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
