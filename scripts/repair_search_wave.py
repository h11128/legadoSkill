#!/usr/bin/env python3
"""Parallel PC search-form hunt + one batch verify (no 发现 by default).

Fetches many homepages in parallel, patches searchUrl when a form is found,
then ONE start_check_sources for all patched URLs.

Example:
  python scripts/repair_search_wave.py --urls-file temp/full_fix/wave20_needs.txt
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import time
import urllib.request
from concurrent.futures import ThreadPoolExecutor, as_completed
from datetime import datetime, timezone
from pathlib import Path
from typing import Any
from urllib.parse import urljoin, urlparse

_ROOT = Path(__file__).resolve().parents[1]
_SCRIPTS = Path(__file__).resolve().parent
sys.path.insert(0, str(_SCRIPTS))

import mcp_channel  # noqa: E402
from mcp_client import (  # noqa: E402
    ensure_session,
    extract_text,
    get_source,
    parse_json_text,
    save_source,
    tools_call,
)
from repair_check import is_repair_success  # noqa: E402
from repair_helpers import header_map  # noqa: E402
from repair_session_log import DEFAULT as LEDGER, append_row  # noqa: E402
from repair_wave import match_check, start_check, wait_check  # noqa: E402


def defaults() -> tuple[str, str]:
    from mcp_client import load_endpoint

    return load_endpoint()


def fetch(url: str, headers: dict[str, str], timeout: float = 12.0) -> str:
    req = urllib.request.Request(url, headers=headers or {"User-Agent": "Mozilla/5.0"})
    ctx = None
    try:
        import ssl

        ctx = ssl._create_unverified_context()
    except Exception:
        pass
    with urllib.request.urlopen(req, timeout=timeout, context=ctx) as resp:
        body = resp.read()
    for enc in ("utf-8", "gbk", "gb2312"):
        try:
            return body.decode(enc)
        except Exception:
            continue
    return body.decode("utf-8", errors="replace")


def find_search_action(html: str, base: str) -> str | None:
    for m in re.finditer(r"<form[^>]*>([\s\S]{0,1500}?)</form>", html, re.I):
        block = m.group(0)
        if not re.search(r"search|keyword|key|wd|q=|sosuo", block, re.I):
            continue
        am = re.search(r'action=["\']([^"\']*)["\']', block, re.I)
        action = am.group(1) if am else ""
        name = "q"
        for nm in ("searchkey", "searchKey", "keyword", "key", "wd", "q"):
            if re.search(rf'name=["\']{nm}["\']', block, re.I):
                name = nm
                break
        abs_u = urljoin(base if base.endswith("/") else base + "/", action or ".")
        path = urlparse(abs_u).path or "/"
        return f"{path}?{name}={{{{key}}}}"
    if re.search(r'action=["\'][^"\']*search', html, re.I):
        m = re.search(r'action=["\']([^"\']*search[^"\']*)["\']', html, re.I)
        if m:
            path = urlparse(urljoin(base, m.group(1))).path
            return f"{path}?q={{{{key}}}}"
    return None


def work_one(mcp: str, token: str, url: str) -> dict[str, Any]:
    row: dict[str, Any] = {"url": url}
    t0 = time.perf_counter()
    try:
        src = get_source(mcp, token, url)
        row["name"] = src.get("bookSourceName")
        base = url.split("#", 1)[0]
        if "://" not in base:
            base = "http://" + base
        html = fetch(base, header_map(src))
        action = find_search_action(html, base)
        row["search_action"] = action
        if action and action != (src.get("searchUrl") or "").split(",", 1)[0].strip():
            src["searchUrl"] = action
            src["concurrentRate"] = src.get("concurrentRate") or "1000"
            row["save"] = save_source(mcp, token, src)
            row["action"] = "patched_search"
        else:
            row["action"] = "no_form" if not action else "search_unchanged"
    except Exception as exc:  # noqa: BLE001
        row["action"] = "error"
        row["error"] = str(exc)[:200]
    row["ms"] = int((time.perf_counter() - t0) * 1000)
    return row


def main() -> int:
    mcp, token = defaults()
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--urls-file", required=True)
    ap.add_argument("--keyword", default="我的")
    ap.add_argument("--workers", type=int, default=6)
    ap.add_argument("--thread-count", type=int, default=8)
    ap.add_argument("--out", default="temp/full_fix/search_wave.json")
    args = ap.parse_args()
    urls = [
        ln.strip()
        for ln in Path(args.urls_file).read_text(encoding="utf-8").splitlines()
        if ln.strip() and not ln.startswith("#")
    ]
    report: dict[str, Any] = {
        "ts": datetime.now(timezone.utc).isoformat(),
        "n": len(urls),
        "per": [],
    }
    t0 = time.perf_counter()
    mcp_channel.assert_idle_for_repair()
    mcp_channel.acquire("repair", "search_wave")
    try:
        ensure_session(mcp, token, "search_wave")
        with ThreadPoolExecutor(max_workers=max(1, args.workers)) as pool:
            futs = [pool.submit(work_one, mcp, token, u) for u in urls]
            for fut in as_completed(futs):
                row = fut.result()
                report["per"].append(row)
                print(f"{row['action']:16s} {row['ms']:5d}ms {row.get('search_action')} {row['url']}", flush=True)

        verify_urls = [r["url"] for r in report["per"] if r.get("action") == "patched_search"]
        # also re-check unchanged that we still want status on? only patched
        report["verify_urls"] = verify_urls
        if verify_urls:
            try:
                tools_call(mcp, token, "stop_check_sources", {})
            except Exception:
                pass
            snap = start_check(
                mcp, token, verify_urls, args.keyword, args.thread_count, 45000, False
            )
            check_map = {
                r["url"]: r for r in (snap.get("results") or []) if isinstance(r, dict) and r.get("url")
            }
            fixed = 0
            for row in report["per"]:
                if row.get("action") != "patched_search":
                    continue
                cr = match_check(check_map, row["url"])
                row["check"] = cr
                ok = is_repair_success(cr)
                row["fixed"] = ok
                if ok:
                    fixed += 1
                append_row(
                    LEDGER,
                    {
                        "ts": datetime.now(timezone.utc).isoformat(),
                        "url": row["url"],
                        "step": "search_wave",
                        "result": "ok" if ok else (cr or {}).get("message") or "fail",
                        "note": row.get("search_action") or "",
                        "waste": "",
                    },
                )
            report["fixed"] = fixed
        else:
            report["fixed"] = 0
        report["wall_s"] = round(time.perf_counter() - t0, 2)
    finally:
        mcp_channel.release("repair")

    path = Path(args.out)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({"fixed": report.get("fixed"), "patched": len(report.get("verify_urls") or []), "wall_s": report.get("wall_s")}, ensure_ascii=False))
    print(f"wrote {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
