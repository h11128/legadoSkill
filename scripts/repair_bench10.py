#!/usr/bin/env python3
"""Timed bench: patch/save up to N sources on PC+MCP, then ONE batch verify.

Writes temp/full_fix/bench10_report.json with phase timings.
"""

from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path
from typing import Any

_SCRIPTS = Path(__file__).resolve().parent
_ROOT = _SCRIPTS.parent
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
from repair_classify import decide  # noqa: E402
from repair_check import check_args  # noqa: E402
from repair_helpers import smell_rules  # noqa: E402
from repair_knowledge import search_knowledge  # noqa: E402
from repair_patches import apply_auto_patches  # noqa: E402
from repair_prefilter import filter_urls  # noqa: E402

DEFAULT_URLS = [
    "https://www.bengben.com#🎃",
    "https://www.627txt.com##@尐哖",
    "http://www.zxcs.info/",
    "https://www.book18.org/",
    "https://www.powanjuan.cc",
    "https://www.ijjjxsw.com",
    "https://api.9yread.com/",
    "http://book.tiexue.net/",
    "http://wap.wangshugu.net#",
    "https://ifun.cool",
]


def defaults() -> tuple[str, str]:
    cfg = _ROOT / "config" / "mcp_defaults.json"
    data = json.loads(cfg.read_text(encoding="utf-8"))
    return str(data["mcp_url"]), str(data.get("token") or "1234")


def main() -> int:
    mcp, token = defaults()
    ap = argparse.ArgumentParser()
    ap.add_argument("--mcp", default=mcp)
    ap.add_argument("--token", default=token)
    ap.add_argument("--keyword", default="我的")
    ap.add_argument("--thread-count", type=int, default=8)
    ap.add_argument("--timeout-ms", type=int, default=45_000)
    ap.add_argument("--out", default="temp/full_fix/bench10_report.json")
    ap.add_argument("--url", action="append", default=[])
    ap.add_argument("--urls-file")
    ap.add_argument(
        "--disable-dropped",
        action="store_true",
        help="MCP-disable L0/L1/L2 disable-bucket before verify",
    )
    args = ap.parse_args()
    urls = list(args.url)
    if args.urls_file:
        for line in Path(args.urls_file).read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if line and not line.startswith("#"):
                urls.append(line)
    if not urls:
        urls = list(DEFAULT_URLS)

    mcp_channel.assert_idle_for_repair()
    mcp_channel.acquire("repair", "bench10")
    t0 = time.perf_counter()
    phases: dict[str, Any] = {}
    per: list[dict[str, Any]] = []
    verify_urls: list[str] = []

    try:
        # L0/L1/L2 prefilter (PC) — drop dead/non-book before device verify
        t_pf = time.perf_counter()
        pref = filter_urls(urls, concurrency=16, l2_timeout=4.0)
        phases["prefilter_s"] = round(time.perf_counter() - t_pf, 2)
        phases["prefilter"] = {
            "verify": len(pref["verify_urls"]),
            "skip": len(pref["skip"]),
            "disable": len(pref["disable"]),
            "video": len(pref.get("video") or []),
            "hunt": len(pref.get("hunt") or []),
        }
        work_urls = list(pref["verify_urls"])
        for row in pref["disable"] + pref["skip"] + (pref.get("video") or []) + (pref.get("hunt") or []):
            per.append({
                "url": row["url"],
                "action": f"prefilter_{row.get('action')}",
                "reason": row.get("reason"),
                "ms_total": 0,
            })
        print(
            f"prefilter verify={len(work_urls)} "
            f"skip={len(pref['skip'])} disable={len(pref['disable'])} "
            f"video={len(pref.get('video') or [])} hunt={len(pref.get('hunt') or [])} "
            f"in {phases['prefilter_s']}s",
            flush=True,
        )

        ensure_session(args.mcp, args.token, "bench10")

        # Optional: disable dropped sources on device
        t_dis = time.perf_counter()
        disabled_n = 0
        if args.disable_dropped:
            for row in pref["disable"]:
                u = row["url"]
                try:
                    src = get_source(args.mcp, args.token, u)
                    msg = disable_source(args.mcp, args.token, src)
                    disabled_n += 1
                    print(f"disable {u} -> {msg[:60]}", flush=True)
                except Exception as exc:  # noqa: BLE001
                    print(f"disable fail {u}: {exc}", flush=True)
        phases["disable_dropped_s"] = round(time.perf_counter() - t_dis, 2)
        phases["disabled_n"] = disabled_n

        # --- Phase patch/save (only survivors) ---
        t_patch = time.perf_counter()
        for url in work_urls:
            row: dict[str, Any] = {"url": url}
            t_i = time.perf_counter()
            try:
                t_get = time.perf_counter()
                src = get_source(args.mcp, args.token, url)
                row["ms_get"] = int((time.perf_counter() - t_get) * 1000)
                fail = ""
                smells = smell_rules(src)
                t_k = time.perf_counter()
                knowledge = search_knowledge(url, "toc")
                row["ms_knowledge"] = int((time.perf_counter() - t_k) * 1000)
                row["knowledge_hits"] = len(knowledge)
                decision = decide(fail or "校验失败:搜索目录失效", smells)
                row["decision"] = decision
                patched, changes = apply_auto_patches(src)
                row["changes"] = changes
                t_save = time.perf_counter()
                if decision["action"] == "disable":
                    row["save_msg"] = disable_source(args.mcp, args.token, src)
                    row["action"] = "disable"
                elif changes:
                    row["save_msg"] = save_source(args.mcp, args.token, patched)
                    row["action"] = "patched"
                    verify_urls.append(url)
                else:
                    row["action"] = "no_patch"
                    verify_urls.append(url)  # still measure check cost
                row["ms_save"] = int((time.perf_counter() - t_save) * 1000)
            except Exception as exc:  # noqa: BLE001
                row["error"] = str(exc)
                row["action"] = "error"
            row["ms_total"] = int((time.perf_counter() - t_i) * 1000)
            per.append(row)
            print(f"patch {row.get('action')} {row['ms_total']}ms {url}", flush=True)
        phases["patch_save_s"] = round(time.perf_counter() - t_patch, 2)

        # --- Phase one batch verify ---
        t_ver = time.perf_counter()
        if verify_urls:
            extract_text(tools_call(args.mcp, args.token, "stop_check_sources", {}))
            extract_text(
                tools_call(
                    args.mcp,
                    args.token,
                    "start_check_sources",
                    check_args(
                        verify_urls,
                        args.keyword,
                        thread_count=args.thread_count,
                        timeout_ms=args.timeout_ms,
                    ),
                )
            )
            while True:
                raw = extract_text(
                    tools_call(
                        args.mcp,
                        args.token,
                        "get_check_progress",
                        {"resultOffset": 0, "resultLimit": 50},
                    )
                )
                snap = parse_json_text(raw)
                if isinstance(snap, dict) and not snap.get("running", False):
                    phases["verify_snap"] = {
                        "success": snap.get("success"),
                        "failed": snap.get("failed"),
                        "finished": snap.get("finished"),
                        "error": snap.get("error"),
                        "results": snap.get("results") or [],
                    }
                    break
                time.sleep(2)
        phases["verify_s"] = round(time.perf_counter() - t_ver, 2)
        phases["wall_s"] = round(time.perf_counter() - t0, 2)
        phases["verify_url_count"] = len(verify_urls)

        # per-URL verify ms if present
        by_url = {
            str(r.get("url")): r
            for r in (phases.get("verify_snap") or {}).get("results") or []
            if isinstance(r, dict)
        }
        for row in per:
            r = by_url.get(row["url"])
            if r:
                row["verify_success"] = r.get("success")
                row["verify_message"] = r.get("message")
                row["verify_respondTime"] = r.get("respondTime")

        report = {
            "phases": phases,
            "per_source": per,
            "bottleneck_hint": _bottleneck(phases, per),
        }
        out = Path(args.out)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
        print(json.dumps({"phases": phases, "bottleneck": report["bottleneck_hint"]}, ensure_ascii=False, indent=2))
        print(f"wrote {out}")
        return 0
    finally:
        mcp_channel.release("repair")


def _bottleneck(phases: dict[str, Any], per: list[dict[str, Any]]) -> dict[str, Any]:
    patch_s = float(phases.get("patch_save_s") or 0)
    verify_s = float(phases.get("verify_s") or 0)
    get_ms = [int(r.get("ms_get") or 0) for r in per]
    save_ms = [int(r.get("ms_save") or 0) for r in per]
    know_ms = [int(r.get("ms_knowledge") or 0) for r in per]
    dominant = "verify_batch" if verify_s >= patch_s else "patch_save"
    return {
        "dominant_phase": dominant,
        "patch_save_s": patch_s,
        "verify_s": verify_s,
        "avg_ms_get": round(sum(get_ms) / max(len(get_ms), 1), 1),
        "avg_ms_save": round(sum(save_ms) / max(len(save_ms), 1), 1),
        "avg_ms_knowledge": round(sum(know_ms) / max(len(know_ms), 1), 1),
        "max_ms_get": max(get_ms or [0]),
        "max_ms_save": max(save_ms or [0]),
    }


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:  # noqa: BLE001
        print(f"error: {exc}", file=sys.stderr)
        raise SystemExit(1)
