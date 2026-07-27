#!/usr/bin/env python3
"""Deep-fix a few wave failures under a hard per-URL wall budget.

Steps per URL (stop when budget hit):
  1. debug_source search
  2. if books>0 and fail looks like toc → clear tocUrl + verify
  3. else fetch search HTML; if form action found → set searchUrl once + verify
  4. else skip

Example:
  python scripts/repair_deep_wave.py --urls-file temp/full_fix/wave20_deep.txt --budget-s 90
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import time
import urllib.request
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
from repair_helpers import header_map  # noqa: E402
from repair_session_log import DEFAULT as LEDGER, append_row  # noqa: E402


def defaults() -> tuple[str, str]:
    cfg = _ROOT / "config" / "mcp_defaults.json"
    data = json.loads(cfg.read_text(encoding="utf-8"))
    return str(data["mcp_url"]), str(data.get("token") or "1234")


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


def debug_search(mcp: str, token: str, url: str, key: str) -> tuple[str, int]:
    raw = extract_text(
        tools_call(mcp, token, "debug_source", {"url": url, "key": key}, timeout=60)
    )
    n = 0
    m = re.search(r"书籍总数:(\d+)", raw)
    if m:
        n = int(m.group(1))
    else:
        m = re.search(r"列表大小:(\d+)", raw)
        if m:
            n = int(m.group(1))
    return raw, n


def fetch(url: str, headers: dict[str, str], timeout: float = 12.0) -> str:
    req = urllib.request.Request(url, headers=headers or {"User-Agent": "Mozilla/5.0"})
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        body = resp.read()
    for enc in ("utf-8", "gbk", "gb2312"):
        try:
            return body.decode(enc)
        except Exception:
            continue
    return body.decode("utf-8", errors="replace")


def find_search_action(html: str, base: str) -> str | None:
    for m in re.finditer(r"<form[^>]*>([\s\S]{0,1200}?)</form>", html, re.I):
        block = m.group(0)
        if not re.search(r"search|keyword|key|wd|q=", block, re.I):
            continue
        am = re.search(r'action=["\']([^"\']*)["\']', block, re.I)
        action = am.group(1) if am else ""
        name = "q"
        for nm in ("keyword", "key", "wd", "q", "searchkey", "searchKey"):
            if re.search(rf'name=["\']{nm}["\']', block, re.I):
                name = nm
                break
        abs_u = urljoin(base if base.endswith("/") else base + "/", action or "")
        # relative search patterns common in legado
        path = urlparse(abs_u).path or "/"
        qs = urlparse(abs_u).query
        if qs:
            return f"{path}?{qs}&{name}={{{{key}}}}" if name not in qs else f"{path}?{qs}".replace(
                f"{name}=", f"{name}={{{{key}}}}"
            )
        return f"{path}?{name}={{{{key}}}}"
    # also /search?q=
    if re.search(r"/search\?q=", html):
        return "/search?q={{key}}"
    return None


def verify(mcp: str, token: str, url: str, key: str) -> dict[str, Any]:
    try:
        tools_call(mcp, token, "stop_check_sources", {})
    except Exception:
        pass
    tools_call(
        mcp,
        token,
        "start_check_sources",
        {
            "urls": [url],
            "enabledOnly": False,
            "keyword": key,
            "threadCount": 1,
            "timeoutMs": 45000,
        },
    )
    snap = wait_check(mcp, token)
    results = snap.get("results") or []
    return results[0] if results and isinstance(results[0], dict) else {"success": False, "message": "no_result"}


def deep_one(mcp: str, token: str, url: str, key: str, budget_s: float) -> dict[str, Any]:
    t0 = time.perf_counter()
    row: dict[str, Any] = {"url": url, "steps": []}

    def left() -> float:
        return budget_s - (time.perf_counter() - t0)

    if left() < 5:
        row["result"] = "skip_budget"
        return row
    src = get_source(mcp, token, url)
    row["name"] = src.get("bookSourceName")
    dbg, n = debug_search(mcp, token, url, key)
    row["steps"].append({"debug_books": n, "ms": int((time.perf_counter() - t0) * 1000)})

    # toc-ish path
    if n > 0 and left() > 10:
        info = src.get("ruleBookInfo") if isinstance(src.get("ruleBookInfo"), dict) else {}
        toc = str((info or {}).get("tocUrl") or "")
        if toc:
            info = dict(info or {})
            info["tocUrl"] = ""
            src["ruleBookInfo"] = info
            src["concurrentRate"] = src.get("concurrentRate") or "1000"
            save_source(mcp, token, src)
            row["steps"].append({"patch": "clear_tocUrl"})
            if left() > 8:
                chk = verify(mcp, token, url, key)
                row["check"] = chk
                row["result"] = "fixed" if chk.get("success") else "failed_after_toc_clear"
                return row

    # search form path
    if n == 0 and left() > 15:
        base = url.split("#", 1)[0]
        if "://" not in base:
            base = "http://" + base
        try:
            html = fetch(base, header_map(src), timeout=min(12.0, left() - 5))
            action = find_search_action(html, base)
            row["steps"].append({"search_action": action})
            if action:
                src["searchUrl"] = action
                src["concurrentRate"] = src.get("concurrentRate") or "1000"
                save_source(mcp, token, src)
                row["steps"].append({"patch": f"searchUrl={action}"})
                if left() > 8:
                    chk = verify(mcp, token, url, key)
                    row["check"] = chk
                    row["result"] = "fixed" if chk.get("success") else "failed_after_searchUrl"
                    return row
        except Exception as exc:  # noqa: BLE001
            row["steps"].append({"fetch_err": str(exc)[:160]})

    row["result"] = row.get("result") or "skip_no_quick_fix"
    row["wall_s"] = round(time.perf_counter() - t0, 2)
    return row


def main() -> int:
    mcp, token = defaults()
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--urls-file", required=True)
    ap.add_argument("--keyword", default="我的")
    ap.add_argument("--budget-s", type=float, default=90.0)
    ap.add_argument("--out", default="temp/full_fix/deep_wave.json")
    args = ap.parse_args()
    urls = [
        ln.strip()
        for ln in Path(args.urls_file).read_text(encoding="utf-8").splitlines()
        if ln.strip() and not ln.startswith("#")
    ]
    mcp_channel.assert_idle_for_repair()
    mcp_channel.acquire("repair", "deep_wave")
    out_rows = []
    try:
        ensure_session(mcp, token, "deep_wave")
        for url in urls:
            print(f"== deep {url} budget={args.budget_s}s", flush=True)
            row = deep_one(mcp, token, url, args.keyword, args.budget_s)
            out_rows.append(row)
            print(json.dumps({"url": url, "result": row.get("result"), "wall_s": row.get("wall_s"), "steps": row.get("steps")}, ensure_ascii=False))
            append_row(
                LEDGER,
                {
                    "ts": datetime.now(timezone.utc).isoformat(),
                    "url": url,
                    "step": "deep_wave",
                    "result": str(row.get("result")),
                    "note": json.dumps(row.get("steps"), ensure_ascii=False)[:200],
                    "waste": "",
                },
            )
    finally:
        mcp_channel.release("repair")
    report = {
        "ts": datetime.now(timezone.utc).isoformat(),
        "fixed": sum(1 for r in out_rows if r.get("result") == "fixed"),
        "rows": out_rows,
    }
    path = Path(args.out)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"fixed={report['fixed']} wrote {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
