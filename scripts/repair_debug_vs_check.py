#!/usr/bin/env python3
"""Compare debug_source vs start_check_sources using HTTP logs.

If debug gets download/m3u8 (or search hits) but check fails fast with
「下载链接为空」, look for search-without-detail — classic bookUrl→infoHtml trap.

Example:
  python scripts/repair_debug_vs_check.py --url https://ukuzy.com/ --key 隐形战队
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any
from urllib.parse import urlparse

_ROOT = Path(__file__).resolve().parents[1]
_SCRIPTS = Path(__file__).resolve().parent
sys.path.insert(0, str(_SCRIPTS))

import mcp_channel  # noqa: E402
from mcp_client import (  # noqa: E402
    ensure_session,
    extract_text,
    get_source,
    parse_json_text,
    tools_call,
)
from repair_check import check_args  # noqa: E402
from repair_session_log import append_row, DEFAULT as LEDGER  # noqa: E402


def _defaults() -> tuple[str, str]:
    from mcp_client import load_endpoint

    return load_endpoint()


def wait_check(mcp: str, token: str) -> dict[str, Any]:
    while True:
        snap = parse_json_text(
            extract_text(
                tools_call(mcp, token, "get_check_progress", {"resultOffset": 0, "resultLimit": 10})
            )
        )
        if isinstance(snap, dict) and not snap.get("running", False):
            return snap
        time.sleep(0.5)


def host_of(url: str) -> str:
    return (urlparse(url.split("#", 1)[0]).hostname or "").lower()


def parse_log_urls(raw: str, host: str) -> list[str]:
    urls = []
    for m in re.finditer(r"GET (https?://\S+)", raw):
        u = m.group(1).rstrip(",")
        if host and host not in u:
            continue
        urls.append(u)
    return urls


def classify(debug_text: str, check_msg: str, http_urls: list[str]) -> str:
    has_detail = any("/detail" in u or "/vod/detail" in u or "softdown" in u for u in http_urls)
    has_search = any("search" in u or "wd=" in u or "keyword" in u or "q=" in u for u in http_urls)
    debug_dl = ("m3u8" in debug_text) or ("下载链接" in debug_text and "下载链接为空" not in debug_text)
    check_empty = "下载链接为空" in check_msg
    if debug_dl and check_empty and has_search and not has_detail:
        return "bookUrl_infoHtml_trap"
    if debug_dl and check_empty:
        return "debug_ok_check_empty_dl"
    if "校验成功" in check_msg or check_msg.endswith("成功"):
        return "ok"
    return "other_fail"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--url", required=True)
    ap.add_argument("--key", default="我的")
    ap.add_argument("--out", default="temp/full_fix/debug_vs_check.json")
    ap.add_argument("--no-ledger", action="store_true")
    args = ap.parse_args()

    mcp, token = _defaults()
    mcp_channel.assert_idle_for_repair()
    mcp_channel.acquire("repair", "debug_vs_check")
    report: dict[str, Any] = {
        "ts": datetime.now(timezone.utc).isoformat(),
        "url": args.url,
        "key": args.key,
    }
    try:
        ensure_session(mcp, token, "debug_vs_check")
        src = get_source(mcp, token, args.url)
        report["bookSourceType"] = src.get("bookSourceType")
        report["name"] = src.get("bookSourceName")
        try:
            tools_call(mcp, token, "set_http_log_recording", {"enabled": True})
        except Exception:
            pass

        t0 = time.perf_counter()
        debug_text = extract_text(
            tools_call(mcp, token, "debug_source", {"url": args.url, "key": args.key}, timeout=120)
        )
        report["debug_ms"] = int((time.perf_counter() - t0) * 1000)
        report["debug_has_m3u8"] = "m3u8" in debug_text
        report["debug_empty_dl"] = "下载链接为空" in debug_text

        try:
            tools_call(mcp, token, "stop_check_sources", {})
        except Exception:
            pass
        t1 = time.perf_counter()
        tools_call(
            mcp,
            token,
            "start_check_sources",
            check_args([args.url], args.key, thread_count=1, timeout_ms=60000),
        )
        snap = wait_check(mcp, token)
        report["check_ms"] = int((time.perf_counter() - t1) * 1000)
        results = snap.get("results") or []
        msg = ""
        ok = False
        if results and isinstance(results[0], dict):
            msg = str(results[0].get("message") or "")
            ok = bool(results[0].get("success"))
        report["check_ok"] = ok
        report["check_msg"] = msg

        logs_raw = extract_text(tools_call(mcp, token, "get_http_logs", {"limit": 12}))
        host = host_of(args.url)
        urls = parse_log_urls(logs_raw, host)
        report["http_urls"] = urls[-8:]
        report["diagnosis"] = classify(debug_text, msg, urls)
        report["hint"] = {
            "bookUrl_infoHtml_trap": (
                "Search bookUrl likely empty/falls back to search page; "
                "infoHtml hijacks detail. Fix ruleSearch.bookUrl to detail link; "
                "avoid >-only @css if flaky."
            ),
            "debug_ok_check_empty_dl": "Compare first search hit; fix downloadUrls on real detail DOM.",
            "ok": "No action.",
            "other_fail": "Inspect debug_text + check_msg; not the classic trap.",
        }.get(report["diagnosis"], "")
    finally:
        mcp_channel.release("repair")

    path = Path(args.out)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False, indent=2))
    print(f"wrote {path}")
    if not args.no_ledger:
        append_row(
            LEDGER,
            {
                "ts": report["ts"],
                "url": args.url,
                "step": "debug_vs_check",
                "result": report.get("diagnosis"),
                "note": report.get("check_msg"),
                "waste": "" if report.get("diagnosis") == "ok" else "see hint",
            },
        )
    return 0 if report.get("check_ok") else 1


if __name__ == "__main__":
    raise SystemExit(main())
