#!/usr/bin/env python3
"""Repair wave: prefilter → parallel auto-patch → one batch verify (no 发现 by default).

PC patch/get/save can run with modest concurrency; device check is ONE job for all URLs
(phone threads inside start_check_sources). Do not start a second check/debug on the phone.

Example:
  python scripts/repair_wave.py --urls-file temp/full_fix/bench20_urls.txt \\
    --out temp/full_fix/wave20.json
"""

from __future__ import annotations

import argparse
import json
import sys
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

_ROOT = Path(__file__).resolve().parents[1]
_SCRIPTS = Path(__file__).resolve().parent
sys.path.insert(0, str(_SCRIPTS))

import mcp_channel  # noqa: E402
from mcp_client import (  # noqa: E402
    disable_source,
    ensure_session,
    extract_text,
    get_source,
    parse_json_text,
    save_source,
    tools_call,
)
from repair_check import check_args, is_repair_success  # noqa: E402
from repair_debug_parse import layer_from_check_message, meaningful_changes  # noqa: E402
from repair_patches import apply_auto_patches  # noqa: E402
from repair_prefilter import filter_urls  # noqa: E402
from repair_session_log import DEFAULT as LEDGER, append_row  # noqa: E402


def defaults() -> tuple[str, str]:
    from mcp_client import load_endpoint

    return load_endpoint()


def wait_check(mcp: str, token: str) -> dict[str, Any]:
    while True:
        snap = parse_json_text(
            extract_text(
                tools_call(mcp, token, "get_check_progress", {"resultOffset": 0, "resultLimit": 50})
            )
        )
        if isinstance(snap, dict) and not snap.get("running", False):
            return snap
        time.sleep(0.8)


def patch_one(mcp: str, token: str, url: str) -> dict[str, Any]:
    row: dict[str, Any] = {"url": url}
    t_i = time.perf_counter()
    try:
        src = get_source(mcp, token, url)
        patched, changes = apply_auto_patches(src)
        real = meaningful_changes(changes)
        row["name"] = src.get("bookSourceName")
        row["changes"] = changes
        row["meaningful"] = real
        # Rate-only is noise (wave20 lesson) — still save silently, do not treat as fix work
        if real:
            row["save"] = save_source(mcp, token, patched)
            row["action"] = "patched"
        elif changes:
            row["save"] = save_source(mcp, token, patched)
            row["action"] = "rate_only"
        else:
            row["action"] = "no_patch"
        row["verify"] = True
    except Exception as exc:  # noqa: BLE001
        row["action"] = "error"
        row["error"] = str(exc)
        row["verify"] = False
    row["ms"] = int((time.perf_counter() - t_i) * 1000)
    return row


def start_check(
    mcp: str,
    token: str,
    urls: list[str],
    keyword: str,
    thread_count: int,
    timeout_ms: int,
    check_discovery: bool,
) -> dict[str, Any]:
    full = check_args(
        urls,
        keyword,
        thread_count=thread_count,
        timeout_ms=timeout_ms,
        enabled_only=False,
        check_discovery=check_discovery,
    )
    # Older APKs may reject unknown bool overrides; fall back to core args only.
    core = {
        "urls": urls,
        "enabledOnly": False,
        "keyword": keyword,
        "threadCount": thread_count,
        "timeoutMs": timeout_ms,
    }
    last_exc: Exception | None = None
    for args in (full, {**core, "checkDiscovery": check_discovery}, core):
        try:
            tools_call(mcp, token, "start_check_sources", args)
            return wait_check(mcp, token)
        except Exception as exc:  # noqa: BLE001
            last_exc = exc
            continue
    raise RuntimeError(f"start_check_sources failed: {last_exc}")


def match_check(check_map: dict[str, Any], url: str) -> dict[str, Any] | None:
    cr = check_map.get(url) or check_map.get(url.rstrip("/")) or check_map.get(url + "/")
    if cr:
        return cr
    base = url.split("#", 1)[0].rstrip("/")
    for k, v in check_map.items():
        if k.split("#", 1)[0].rstrip("/") == base:
            return v
    return None


def main() -> int:
    mcp, token = defaults()
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--urls-file", required=True)
    ap.add_argument("--keyword", default="我的")
    ap.add_argument("--thread-count", type=int, default=8)
    ap.add_argument("--patch-workers", type=int, default=4, help="parallel get/save workers")
    ap.add_argument("--timeout-ms", type=int, default=45000)
    ap.add_argument("--disable-dropped", action="store_true")
    ap.add_argument(
        "--check-discovery",
        action="store_true",
        help="also verify 发现 (default: off)",
    )
    ap.add_argument("--out", default="temp/full_fix/wave_report.json")
    args = ap.parse_args()

    urls = [
        ln.strip()
        for ln in Path(args.urls_file).read_text(encoding="utf-8").splitlines()
        if ln.strip() and not ln.startswith("#")
    ]
    report: dict[str, Any] = {
        "ts": datetime.now(timezone.utc).isoformat(),
        "n_in": len(urls),
        "phases": {},
        "per": [],
        "policy": {
            "checkDomain": False,
            "checkSearch": True,
            "checkDiscovery": bool(args.check_discovery),
            "checkInfo": True,
            "checkCategory": True,
            "checkContent": True,
            "ignore_discovery_score": True,
        },
    }
    t0 = time.perf_counter()
    mcp_channel.assert_idle_for_repair()
    mcp_channel.acquire("repair", "repair_wave")
    try:
        t = time.perf_counter()
        pref = filter_urls(urls, concurrency=16, l2_timeout=4.0)
        report["phases"]["prefilter_s"] = round(time.perf_counter() - t, 2)
        report["prefilter"] = {
            "verify": len(pref["verify_urls"]),
            "skip": len(pref["skip"]),
            "disable": len(pref["disable"]),
            "video": len(pref.get("video") or []),
            "hunt": len(pref.get("hunt") or []),
        }
        for row in pref["skip"] + pref["disable"] + (pref.get("video") or []) + (pref.get("hunt") or []):
            report["per"].append(
                {
                    "url": row["url"],
                    "action": f"prefilter_{row.get('action')}",
                    "reason": row.get("reason"),
                }
            )
            append_row(
                LEDGER,
                {
                    "ts": datetime.now(timezone.utc).isoformat(),
                    "url": row["url"],
                    "step": "prefilter",
                    "result": row.get("action"),
                    "note": row.get("reason") or "",
                    "waste": "",
                },
            )

        ensure_session(mcp, token, "repair_wave")
        if args.disable_dropped:
            t = time.perf_counter()
            for row in pref["disable"]:
                try:
                    src = get_source(mcp, token, row["url"])
                    disable_source(mcp, token, src)
                except Exception as exc:  # noqa: BLE001
                    report.setdefault("disable_errors", []).append(str(exc))
            report["phases"]["disable_s"] = round(time.perf_counter() - t, 2)

        verify_urls: list[str] = []
        t = time.perf_counter()
        workers = max(1, min(args.patch_workers, len(pref["verify_urls"]) or 1))
        with ThreadPoolExecutor(max_workers=workers) as pool:
            futs = [pool.submit(patch_one, mcp, token, u) for u in pref["verify_urls"]]
            for fut in as_completed(futs):
                row = fut.result()
                report["per"].append(row)
                if row.get("verify"):
                    verify_urls.append(row["url"])
                print(f"{row['action']:10s} {row['ms']:5d}ms {row['url']}", flush=True)
        report["phases"]["patch_s"] = round(time.perf_counter() - t, 2)
        report["phases"]["patch_workers"] = workers

        t = time.perf_counter()
        check_map: dict[str, Any] = {}
        if verify_urls:
            try:
                tools_call(mcp, token, "stop_check_sources", {})
            except Exception:
                pass
            snap = start_check(
                mcp,
                token,
                verify_urls,
                args.keyword,
                args.thread_count,
                args.timeout_ms,
                args.check_discovery,
            )
            report["verify_snap"] = {
                "success": snap.get("success"),
                "failed": snap.get("failed"),
                "total": snap.get("total"),
                "finished": snap.get("finished"),
            }
            for r in snap.get("results") or []:
                if isinstance(r, dict) and r.get("url"):
                    check_map[r["url"]] = r
        report["phases"]["verify_s"] = round(time.perf_counter() - t, 2)

        fixed = failed = 0
        by_layer: dict[str, list[str]] = {}
        for row in report["per"]:
            if row.get("action") not in {"patched", "no_patch", "rate_only"}:
                continue
            cr = match_check(check_map, row["url"])
            row["check"] = cr
            ok = is_repair_success(cr, ignore_discovery=not args.check_discovery)
            row["fixed"] = ok
            layer = "ok" if ok else layer_from_check_message(str((cr or {}).get("message") or ""))
            row["fail_layer"] = layer
            by_layer.setdefault(layer, []).append(row["url"])
            if ok:
                fixed += 1
            else:
                failed += 1
            append_row(
                LEDGER,
                {
                    "ts": datetime.now(timezone.utc).isoformat(),
                    "url": row["url"],
                    "step": "wave_check",
                    "result": "ok" if ok else (cr or {}).get("message") or "no_result",
                    "note": f"layer={layer};" + (",".join(row.get("meaningful") or row.get("changes") or []) or row.get("action") or ""),
                    "waste": "",
                },
            )

        report["summary"] = {
            "fixed": fixed,
            "failed_verify": failed,
            "prefilter_drop": len(urls) - len(verify_urls),
            "verify_n": len(verify_urls),
            "wall_s": round(time.perf_counter() - t0, 2),
            "by_layer": {k: len(v) for k, v in by_layer.items()},
        }
        report["needs_deep"] = {
            "toc": by_layer.get("toc") or [],
            "search": by_layer.get("search") or [],
            "content": by_layer.get("content") or [],
            "other": [
                u
                for k, urls_ in by_layer.items()
                if k not in {"ok", "toc", "search", "content"}
                for u in urls_
            ],
        }
        report["next"] = (
            "For each needs_deep URL: "
            "python scripts/repair_diagnose.py --url URL  # then patch THAT layer only"
        )
    finally:
        mcp_channel.release("repair")

    path = Path(args.out)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report["summary"], ensure_ascii=False, indent=2))
    nd = report.get("needs_deep") or {}
    if isinstance(nd, dict):
        for layer, urls_ in nd.items():
            if not urls_:
                continue
            print(f"needs_deep[{layer}] ({len(urls_)}):")
            for u in urls_:
                print(" ", u)
        print(report.get("next") or "")
    else:
        print(f"needs_deep ({len(nd)}):")
        for u in nd:
            print(" ", u)
    print(f"wrote {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
