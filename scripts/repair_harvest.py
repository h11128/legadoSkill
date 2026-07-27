#!/usr/bin/env python3
"""Harvest cheap wins: batch-verify tagged fails with discovery off.

Many tagged fails are discovery-only or already fixed on device. One batch
check converts them to ledger 校验成功 without deep HTML work.

  python scripts/repair_harvest.py --limit 20 --goal 100
"""

from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path
from typing import Any

_ROOT = Path(__file__).resolve().parents[1]
_SCRIPTS = Path(__file__).resolve().parent
sys.path.insert(0, str(_SCRIPTS))

from mcp_client import ensure_session, extract_text, reset_session, tools_call  # noqa: E402
from repair_check import check_args, is_repair_success  # noqa: E402
from repair_progress import ledger_fixed, ledger_skipped  # noqa: E402
from repair_session_log import DEFAULT as LEDGER, append_row  # noqa: E402
from repair_wait import wait_check  # noqa: E402

FAILS = _ROOT / "legado" / "temp_tagged_fails.json"


def defaults() -> tuple[str, str]:
    cfg = json.loads((_ROOT / "config" / "mcp_defaults.json").read_text(encoding="utf-8"))
    return str(cfg["mcp_url"]), str(cfg.get("token") or "1234")


def ledger_harvest_tried(path: Path = LEDGER) -> set[str]:
    """URLs already batch-checked by harvest (won or lost) — do not re-queue."""
    urls: set[str] = set()
    if not path.is_file():
        return urls
    for line in path.read_text(encoding="utf-8").splitlines():
        try:
            row = json.loads(line)
        except json.JSONDecodeError:
            continue
        if row.get("step") == "check" and row.get("note") == "harvest" and row.get("url"):
            urls.add(str(row["url"]))
    return urls


def load_fail_urls(path: Path, *, limit: int, fixed: set[str], skipped: set[str]) -> list[str]:
    data = json.loads(path.read_text(encoding="utf-8"))
    rows: list[Any]
    if isinstance(data, list):
        rows = data
    else:
        rows = data.get("items") or data.get("fails") or data.get("rows") or []
    tried = ledger_harvest_tried()
    out: list[str] = []
    seen: set[str] = set()
    for row in rows:
        if not isinstance(row, dict):
            continue
        url = str(row.get("url") or row.get("bookSourceUrl") or "")
        if not url or url in fixed or url in skipped or url in seen or url in tried:
            continue
        if "://" not in url and not url.startswith("/"):
            continue  # skip non-URL keys like local ids
        seen.add(url)
        out.append(url)
        if len(out) >= limit:
            break
    return out



def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--fails", default=str(FAILS))
    ap.add_argument("--limit", type=int, default=16)
    ap.add_argument("--keyword", default="我的")
    ap.add_argument("--threads", type=int, default=6)
    ap.add_argument(
        "--timeout-ms",
        type=int,
        default=18_000,
        help="per-source check timeout (lower = fail-fast; was 60000)",
    )
    args = ap.parse_args()

    fixed = ledger_fixed()
    skipped = ledger_skipped()
    urls = load_fail_urls(Path(args.fails), limit=args.limit, fixed=fixed, skipped=skipped)
    if not urls:
        print(json.dumps({"ok": False, "error": "no urls"}, ensure_ascii=False))
        return 2

    mcp, token = defaults()
    reset_session()
    ensure_session(mcp, token, "harvest")
    try:
        extract_text(tools_call(mcp, token, "stop_check_sources", {}))
    except Exception:
        pass

    started = time.time()
    extract_text(
        tools_call(
            mcp,
            token,
            "start_check_sources",
            check_args(urls, args.keyword, thread_count=args.threads, timeout_ms=args.timeout_ms),
        )
    )
    # Dynamic poll; wall ≈ ceil(n/threads)*timeout + slack (not fixed 2min sleep)
    batches = max(1, (len(urls) + args.threads - 1) // args.threads)
    max_wait = min(240.0, batches * (args.timeout_ms / 1000.0) + 20.0)
    snap = wait_check(
        mcp,
        token,
        poll_s=0.4,
        max_wait_s=max_wait,
        expect_n=len(urls),
    )
    results = snap.get("results") if isinstance(snap, dict) else []
    by_url = {
        str(r.get("url")): r
        for r in (results or [])
        if isinstance(r, dict) and r.get("url")
    }
    won: list[str] = []
    lost: list[str] = []
    for url in urls:
        row = by_url.get(url) or {}
        ok = is_repair_success(row)
        msg = str(row.get("message") or ("校验成功" if ok else "no result"))
        append_row(
            LEDGER,
            {
                "url": url,
                "step": "check",
                "result": "校验成功" if ok else msg,
                "note": "harvest",
                "waste": "" if ok else "needs_deep",
            },
        )
        (won if ok else lost).append(url)

    report = {
        "wall_s": round(time.time() - started, 2),
        "n": len(urls),
        "won": won,
        "lost_n": len(lost),
        "fixed_n_after": len(ledger_fixed()),
        "lost_sample": lost[:8],
    }
    out = _ROOT / "temp" / "full_fix" / "harvest_last.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False, indent=2))
    print(f"wrote {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
