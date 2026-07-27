#!/usr/bin/env python3
"""Serial oneshot repair from respondTime queue (with per-URL retro).

Examples:
  python scripts/repair_rt_queue.py --limit 100 --max-rt-ms 15000
  python scripts/repair_serial.py --limit 100
  python scripts/repair_serial.py --limit 5   # smoke
"""

from __future__ import annotations

import argparse
import json
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any
from urllib.parse import urlparse

_SCRIPTS = Path(__file__).resolve().parent
_ROOT = _SCRIPTS.parent
sys.path.insert(0, str(_SCRIPTS))

import mcp_channel  # noqa: E402
from mcp_client import ensure_session, load_endpoint  # noqa: E402
from repair_deep_loop import emit_report, process_one  # noqa: E402
from repair_prefilter import DEFAULT_RULES, classify_one, load_rules  # noqa: E402
from repair_progress import host_key, ledger_fixed, ledger_skipped, norm_url  # noqa: E402
from repair_retro import DEFAULT as RETRO_PATH  # noqa: E402
from repair_retro import append_retro  # noqa: E402
from repair_rt_queue import OUT_DIR, build as build_queue  # noqa: E402
from repair_session_log import DEFAULT as LEDGER  # noqa: E402
from repair_session_log import append_row  # noqa: E402

KNOWN_SKIP_NAME = ("猫眼看书", "晋江", "验证码")


def load_serial_queue(path: Path) -> list[dict[str, Any]]:
    data = json.loads(path.read_text(encoding="utf-8"))
    items = data.get("items") if isinstance(data, dict) else data
    return [x for x in (items or []) if isinstance(x, dict) and x.get("url")]


def should_pre_skip(item: dict[str, Any]) -> str | None:
    name = str(item.get("name") or "")
    group = str(item.get("group") or "")
    if any(k in name for k in KNOWN_SKIP_NAME) and "搜索目录" in group:
        return "known_auth_or_fragile"
    if "验证码" in group or "人机" in group:
        return "captcha_group"
    return None


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--limit", type=int, default=100)
    ap.add_argument("--max-rt-ms", type=int, default=30_000)
    ap.add_argument("--timeout-ms", type=int, default=20_000)
    ap.add_argument("--rebuild-queue", action="store_true", default=True)
    ap.add_argument("--no-rebuild-queue", action="store_true")
    ap.add_argument(
        "--queue",
        default=str(OUT_DIR / "repair_serial100_queue.json"),
    )
    ap.add_argument("--out", default="temp/full_fix/serial_last.json")
    args = ap.parse_args()

    if not args.no_rebuild_queue:
        from repair_refresh_phone_index import OUT as PHONE_IDX
        from repair_refresh_phone_index import refresh as refresh_phone

        phone_path = Path(PHONE_IDX)
        need_refresh = True
        if phone_path.is_file():
            age = time.time() - phone_path.stat().st_mtime
            need_refresh = age > 3600
        if need_refresh:
            print("REPORT: [meta] refreshing phone_source_index", flush=True)
            try:
                payload = refresh_phone()
                print(
                    "REPORT_JSON:"
                    + json.dumps(
                        {"status": "meta", "phone_total": payload.get("total")},
                        ensure_ascii=False,
                    ),
                    flush=True,
                )
            except Exception as exc:  # noqa: BLE001
                print(f"REPORT: [blocked] phone index refresh failed: {exc}", flush=True)
                return 4
        build_queue(max_rt_ms=args.max_rt_ms, limit=args.limit, enabled_only=True)

    queue_path = Path(args.queue)
    items = load_serial_queue(queue_path)[: args.limit]
    mcp, token = load_endpoint()
    ensure_session(mcp, token, "repair_serial")
    try:
        mcp_channel.assert_idle_for_repair()
    except Exception as exc:  # noqa: BLE001
        print(f"REPORT: [blocked] channel {exc}", flush=True)
        return 3

    rules = load_rules(DEFAULT_RULES) if Path(str(DEFAULT_RULES)).is_file() else []
    # Soft-block: hard ledger skips only (allow no_patch / search verify_fail retry)
    from repair_rt_queue import ledger_sets as rt_ledger_sets

    fixed = ledger_fixed()
    _hard, _retry = set(), set()
    try:
        _fixed2, _hard, _retry = rt_ledger_sets(LEDGER)
        fixed |= _fixed2
    except Exception:
        _hard = ledger_skipped()
    skipped = _hard
    blocked = {host_key(u) for u in fixed | skipped if host_key(u)}
    summary = {"fixed": 0, "skip": 0, "fail": 0, "missing": 0, "rows": []}
    t0_all = time.time()

    for idx, item in enumerate(items, 1):
        url = norm_url(str(item.get("url") or ""))
        if url and "://" not in url.split("##")[0]:
            url = "http://" + url.lstrip("/")
        name = str(item.get("name") or "")
        rt = item.get("respondTime")
        if not url or url in fixed or host_key(url) in blocked:
            continue
        t0 = time.time()
        pre = should_pre_skip(item)
        if pre:
            row = {
                "ts": datetime.now(timezone.utc).isoformat(),
                "url": url,
                "status": "skip",
                "msg": pre,
                "notes": ["pre_skip"],
                "fixed_n": len(ledger_fixed()),
            }
            append_row(
                LEDGER,
                {"ts": row["ts"], "url": url, "step": "skip", "result": pre, "note": "serial_pre"},
            )
        else:
            try:
                gate = classify_one(url, rules)
            except Exception as exc:  # noqa: BLE001
                gate = {"action": "skip", "reason": f"l2_error:{exc}"[:80]}
            act = gate.get("action")
            if act in ("skip", "disable"):
                row = {
                    "ts": datetime.now(timezone.utc).isoformat(),
                    "url": url,
                    "status": "skip",
                    "msg": str(gate.get("reason") or act),
                    "notes": ["l2_gate"],
                    "fixed_n": len(ledger_fixed()),
                }
                append_row(
                    LEDGER,
                    {
                        "ts": row["ts"],
                        "url": url,
                        "step": "skip",
                        "result": row["msg"],
                        "note": "serial_l2",
                    },
                )
            else:
                work = {"url": url, "kind": "fix", "migrate_to": None}
                if act == "migrate" and gate.get("migrate_to"):
                    target = str(gate.get("migrate_to") or "")
                    # Validate migrate target with a cheap L2 before spending probe
                    try:
                        tg = classify_one(target if "://" in target else f"https://{target}", rules)
                    except Exception:
                        tg = {"action": "skip", "reason": "migrate_target_l2_error"}
                    if tg.get("action") in ("skip", "disable"):
                        row = {
                            "ts": datetime.now(timezone.utc).isoformat(),
                            "url": url,
                            "status": "skip",
                            "msg": f"migrate_target_dead:{tg.get('reason')}",
                            "notes": ["migrate_target_l2"],
                            "fixed_n": len(ledger_fixed()),
                        }
                        append_row(
                            LEDGER,
                            {
                                "ts": row["ts"],
                                "url": url,
                                "step": "skip",
                                "result": row["msg"],
                                "note": "serial_migrate_gate",
                            },
                        )
                        # fall through to common reporting below — set flag
                        work = None  # type: ignore[assignment]
                    else:
                        work = {
                            "url": url,
                            "kind": "migrate",
                            "migrate_to": gate.get("migrate_to"),
                        }
                if work is not None:
                    try:
                        row = process_one(
                            mcp, token, work, timeout_ms=args.timeout_ms, require_patch=True
                        )
                    except Exception as exc:  # noqa: BLE001
                        row = {
                            "ts": datetime.now(timezone.utc).isoformat(),
                            "url": url,
                            "status": "fail",
                            "msg": f"exception:{exc}"[:120],
                            "notes": ["process_crash"],
                            "fixed_n": len(ledger_fixed()),
                        }
                        append_row(
                            LEDGER,
                            {
                                "ts": row["ts"],
                                "url": url,
                                "step": "skip",
                                "result": row["msg"],
                                "note": "serial_exception",
                            },
                        )
                # else: row already set for migrate_target_dead

        waste = round(time.time() - t0, 2)
        st = str(row.get("status") or "fail")
        if st == "fixed":
            summary["fixed"] += 1
            fixed.add(norm_url(str(row.get("url") or url)))
            blocked.add(host_key(str(row.get("url") or url)))
        elif st in ("skip", "missing"):
            summary["skip" if st == "skip" else "missing"] += 1
            skipped.add(url)
            blocked.add(host_key(url))
        else:
            summary["fail"] += 1
            # Don't retry same host variants in this serial run
            skipped.add(norm_url(str(row.get("url") or url)))
            blocked.add(host_key(str(row.get("url") or url)))

        harness = ""
        trap = ""
        script_fix = ""
        if "js_api" in " ".join(row.get("notes") or []):
            trap = "js_search_api"
            script_fix = "repair_search_probe.detect_js_search_api"
        if waste > 120:
            harness = "over_budget_>2min"
        elif st == "fail" and waste > 60:
            harness = "fail_after_long_probe"
        elif st == "skip" and "l2" in " ".join(row.get("notes") or []):
            harness = "ok_failfast_l2"
            script_fix = script_fix or "repair_prefilter"

        append_retro(
            RETRO_PATH,
            {
                "n": idx,
                "url": row.get("url") or url,
                "name": name,
                "status": st,
                "msg": str(row.get("msg") or "")[:160],
                "respondTime": rt,
                "waste_s": waste,
                "trap": trap,
                "harness": harness,
                "script_fix": script_fix,
                "notes": row.get("notes") or [],
                "fixed_n": row.get("fixed_n"),
            },
        )
        emit_report(
            {
                **row,
                "n": idx,
                "name": name,
                "respondTime": rt,
                "waste_s": waste,
                "mode": "serial",
            }
        )
        summary["rows"].append(
            {"url": row.get("url") or url, "status": st, "waste_s": waste, "name": name}
        )

    summary["elapsed_s"] = round(time.time() - t0_all, 1)
    summary["fixed_n"] = len(ledger_fixed())
    out = _ROOT / args.out
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding="utf-8")
    print(
        "SUMMARY_JSON:"
        + json.dumps(
            {
                k: summary[k]
                for k in ("fixed", "skip", "fail", "missing", "elapsed_s", "fixed_n")
            },
            ensure_ascii=False,
        ),
        flush=True,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
