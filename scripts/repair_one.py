#!/usr/bin/env python3
"""End-to-end repair for one book source: decide → patch → save → verify → log.

Example:
  python scripts/repair_one.py --url https://www.powanjuan.cc \\
    --fail-msg '校验失败:搜索目录失效' --keyword 我的
"""

from __future__ import annotations

import argparse
import json
import sys
import time
import urllib.error
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

_SCRIPTS = Path(__file__).resolve().parent
_ROOT = _SCRIPTS.parent
if str(_SCRIPTS) not in sys.path:
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
from repair_cache import (  # noqa: E402
    cooldown_for,
    note_rate_limit,
    note_verify,
    put_triage,
)
from repair_check import check_args, is_repair_success  # noqa: E402
from repair_claim import append_index, assert_fixed_allowed  # noqa: E402
from repair_classify import classify_resolved_url, decide  # noqa: E402
from repair_helpers import fetch_page, header_map, smell_rules  # noqa: E402
from repair_knowledge import search_knowledge  # noqa: E402
from repair_patches import apply_auto_patches  # noqa: E402
from repair_prefilter import classify_one, load_rules  # noqa: E402

DEFAULT_RULES = _ROOT / "config" / "verify_skip_rules.json"


def _defaults() -> tuple[str, str]:
    from mcp_client import load_endpoint

    return load_endpoint()


from repair_wait import wait_check  # noqa: E402


def run_verify(mcp: str, token: str, url: str, keyword: str, cooldown: float, timeout_ms: int) -> dict[str, Any]:
    try:
        extract_text(tools_call(mcp, token, "stop_check_sources", {}))
    except Exception:
        pass
    if cooldown > 0:
        print(f"cooldown {cooldown:.1f}s", flush=True)
        time.sleep(cooldown)
    started = time.time()
    extract_text(
        tools_call(
            mcp,
            token,
            "start_check_sources",
            check_args([url], keyword, thread_count=1, timeout_ms=timeout_ms),
        )
    )
    # Single-URL: cap wait ~ timeout + 15s; poll fast
    snap = wait_check(
        mcp,
        token,
        poll_s=0.35,
        max_wait_s=max(25.0, timeout_ms / 1000.0 + 15.0),
        expect_n=1,
    )
    results = snap.get("results") if isinstance(snap, dict) else []
    item = None
    for row in results or []:
        if isinstance(row, dict) and str(row.get("url") or "") == url:
            item = row
            break
    if item is None and results and isinstance(results[0], dict):
        item = results[0]
    ok = bool(item and item.get("success"))
    out = {
        "url": url,
        "keyword": keyword,
        "success": ok,
        "message": (item or {}).get("message") if item else snap,
        "durationMs": int((time.time() - started) * 1000),
        "cooldown_s": cooldown,
    }
    note_verify(url, ok, int(out["durationMs"]), cooldown)
    msg = str(out.get("message") or "")
    if (not ok) and any(x in msg for x in ("搜索失效", "时间间隔", "频繁")):
        note_rate_limit(url, max(20.0, cooldown + 5))
    return out


def write_log(path: Path, payload: dict[str, Any], index: Path | None) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    if index:
        append_index(index, {
            "status": payload.get("status"),
            "url": payload.get("url"),
            "name": payload.get("name"),
            "evidence": str(path),
            "agent": payload.get("agent"),
            "root_cause": payload.get("root_cause"),
        })


def main() -> int:
    mcp_url, token = _defaults()
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--mcp", default=mcp_url)
    p.add_argument("--token", default=token)
    p.add_argument("--url", required=True)
    p.add_argument("--fail-msg", default="")
    p.add_argument("--keyword", default="我的")
    p.add_argument("--timeout-ms", type=int, default=45_000)
    p.add_argument("--dry-run", action="store_true", help="do not save/disable/verify")
    p.add_argument("--no-verify", action="store_true")
    p.add_argument("--out-dir", default="temp/full_fix")
    p.add_argument("--index", default="temp/full_fix/repair_session_index.json")
    args = p.parse_args()
    args.apply = not args.dry_run

    mcp_channel.assert_idle_for_repair()
    mcp_channel.acquire("repair", "repair_one")
    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    try:
        ensure_session(args.mcp, args.token, "repair_one")
        # L0/L1/L2 before any expensive device verify
        rules = load_rules(DEFAULT_RULES) if DEFAULT_RULES.is_file() else []
        pre = classify_one(args.url, rules)
        if not pre.get("verify"):
            source = get_source(args.mcp, args.token, args.url)
            status = "skipped"
            action = pre.get("action")
            if action == "video":
                status = "divert_video"
                print(
                    "divert: use video flow — "
                    f"python scripts/video_repair_one.py --url {args.url}",
                    flush=True,
                )
            elif action == "hunt":
                status = "divert_hunt"
                print(
                    "divert: domain hunt — "
                    f"python scripts/repair_domain_hunt.py --url {args.url}",
                    flush=True,
                )
            elif action == "disable" and args.apply:
                print(disable_source(args.mcp, args.token, source))
            payload = {
                "url": args.url,
                "name": source.get("bookSourceName"),
                "status": status,
                "root_cause": pre.get("reason"),
                "prefilter": pre,
                "saved_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
                "agent": "repair_one",
            }
            write_log(out_dir / "fix_auto.json", payload, Path(args.index) if args.index else None)
            print(json.dumps({"prefilter": pre}, ensure_ascii=False))
            return 0

        source = get_source(args.mcp, args.token, args.url)
        smells = smell_rules(source)
        decision = decide(args.fail_msg, smells)
        knowledge = search_knowledge(args.fail_msg or args.url, decision["layer"])
        triage = {
            "url": args.url,
            "name": source.get("bookSourceName"),
            "fail_msg": args.fail_msg,
            "decision": decision,
            "smells": smells,
            "knowledge": knowledge,
            "cached_at": time.time(),
        }
        put_triage(args.url, triage)
        (out_dir / "last_triage.json").write_text(
            json.dumps(triage, ensure_ascii=False, indent=2), encoding="utf-8"
        )
        print(json.dumps({"decision": decision, "smells": smells}, ensure_ascii=False))

        if decision["action"] in {"disable", "skip"}:
            status = "skipped"
            if decision["action"] == "disable" and args.apply:
                msg = disable_source(args.mcp, args.token, source)
                print(msg)
            payload = {
                "url": args.url,
                "name": source.get("bookSourceName"),
                "status": status,
                "root_cause": decision["reason"],
                "decision": decision,
                "knowledge": knowledge,
                "saved_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
                "agent": "repair_one",
            }
            write_log(out_dir / "fix_auto.json", payload, Path(args.index) if args.index else None)
            return 0

        patched, changes = apply_auto_patches(source)
        # Probe book homepage for URL class if we still have tocUrl smell residue
        headers = header_map(patched)
        probe = fetch_page(args.url, headers, use_cache=True)
        html = ""
        if probe.get("ok") and isinstance(probe.get("body"), (bytes, bytearray)):
            html = bytes(probe["body"]).decode("utf-8", errors="replace")
        resolved = classify_resolved_url(str(probe.get("final_url") or args.url), html)
        if resolved["kind"] == "homepage" and (patched.get("ruleBookInfo") or {}).get("tocUrl"):
            # Extra safety: broad toc on sites that redirect home
            info = patched.setdefault("ruleBookInfo", {})
            if isinstance(info, dict) and info.get("tocUrl"):
                info["tocUrl"] = ""
                changes.append("clear tocUrl after homepage classification")

        if changes and args.apply:
            print(save_source(args.mcp, args.token, patched))
        elif not changes:
            print("no auto patches; manual edit may be required", flush=True)

        check = None
        if not args.no_verify and args.apply:
            cool = cooldown_for(args.url, str(patched.get("concurrentRate") or ""))
            check = run_verify(
                args.mcp, args.token, args.url, args.keyword, cool, args.timeout_ms
            )
            (out_dir / "verify_auto.json").write_text(
                json.dumps(check, ensure_ascii=False, indent=2), encoding="utf-8"
            )
            print(json.dumps(check, ensure_ascii=False))

        status = "fixed" if is_repair_success(check) else (
            "failed" if check is not None else ("skipped" if not changes else "failed")
        )
        if status == "fixed":
            assert_fixed_allowed(check)
        payload = {
            "url": args.url,
            "name": patched.get("bookSourceName"),
            "status": status,
            "keyword": args.keyword,
            "root_cause": decision["reason"],
            "changes": changes,
            "decision": decision,
            "resolved": resolved,
            "knowledge": knowledge,
            "check": check,
            "saved_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "agent": "repair_one",
        }
        write_log(out_dir / "fix_auto.json", payload, Path(args.index) if args.index else None)
        try:
            from repair_session_log import DEFAULT as _LEDGER, append_row as _ledger_append

            _ledger_append(
                _LEDGER,
                {
                    "ts": payload["saved_at"],
                    "url": args.url,
                    "step": "repair_one",
                    "result": status,
                    "note": decision.get("reason") or "",
                    "waste": "",
                },
            )
        except Exception:
            pass
        return 0 if status in {"fixed", "skipped"} else 1
    finally:
        mcp_channel.release("repair")


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (urllib.error.URLError, RuntimeError, OSError, json.JSONDecodeError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        raise SystemExit(1)
