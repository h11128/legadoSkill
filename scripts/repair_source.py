#!/usr/bin/env python3
"""Repair CLI: triage | fetch | verify | log | channel | index. See legado-book-source-repair skill."""

from __future__ import annotations

import argparse
import json
import re
import sys
import time
import urllib.error
from datetime import datetime, timezone
from pathlib import Path
from typing import Any
from urllib.parse import urlparse

_SCRIPTS = Path(__file__).resolve().parent
_ROOT = _SCRIPTS.parent
if str(_SCRIPTS) not in sys.path:
    sys.path.insert(0, str(_SCRIPTS))

from mcp_channel import assert_idle_for_repair, acquire, release, status as channel_status  # noqa: E402
from mcp_client import (  # noqa: E402
    ensure_session,
    extract_text,
    get_source,
    parse_json_text,
    tools_call,
)
from repair_claim import append_index, assert_fixed_allowed, load_check  # noqa: E402
from repair_helpers import fetch_page, header_map, layer_for_fail, smell_rules  # noqa: E402


def default_mcp() -> tuple[str, str]:
    cfg = _ROOT / "config" / "mcp_defaults.json"
    if cfg.is_file():
        data = json.loads(cfg.read_text(encoding="utf-8"))
        return str(data.get("mcp_url") or ""), str(data.get("token") or "1234")
    return "http://10.0.0.139:1236/mcp", "1234"


def cmd_triage(args: argparse.Namespace) -> int:
    ensure_session(args.mcp, args.token)
    source = get_source(args.mcp, args.token, args.url)
    fail = args.fail_msg or ""
    layer = layer_for_fail(fail)
    info = source.get("ruleBookInfo") if isinstance(source.get("ruleBookInfo"), dict) else {}
    report = {
        "url": args.url,
        "name": source.get("bookSourceName"),
        "group": source.get("bookSourceGroup"),
        "fail_msg": fail,
        "layer": layer,
        "action": "skip" if layer == "skip" else f"fix_{layer}",
        "smells": smell_rules(source),
        "concurrentRate": source.get("concurrentRate"),
        "tocUrl": info.get("tocUrl") if info else None,
        "budget_minutes": 5,
        "note": "Hard stop 10 min; typical TOC fix should be 2-5 min with scripts",
    }
    text = json.dumps(report, ensure_ascii=False, indent=2)
    if args.out:
        path = Path(args.out)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
        print(f"wrote {path}")
    print(text)
    return 0


def cmd_fetch(args: argparse.Namespace) -> int:
    ensure_session(args.mcp, args.token)
    source = get_source(args.mcp, args.token, args.url)
    headers = header_map(source)
    page = args.page or args.url
    result = fetch_page(page, headers)
    host = urlparse(page).netloc.replace(":", "_")
    dump_dir = Path(args.dump_dir)
    dump_dir.mkdir(parents=True, exist_ok=True)
    safe = re.sub(r"[^\w.-]+", "_", page)[:100]
    meta_path = dump_dir / f"{host}_{safe}.json"
    html_path = dump_dir / f"{host}_{safe}.html"
    body = result.pop("body", None)
    if result.get("ok") and isinstance(body, (bytes, bytearray)):
        html_path.write_bytes(body)
        result["html_path"] = str(html_path)
    meta_path.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({**result, "meta_path": str(meta_path)}, ensure_ascii=False, indent=2))
    return 0 if result.get("ok") else 1


def wait_check(mcp_url: str, token: str, poll: float) -> dict[str, Any]:
    while True:
        raw = extract_text(
            tools_call(
                mcp_url,
                token,
                "get_check_progress",
                {"resultOffset": 0, "resultLimit": 20},
            )
        )
        snap = parse_json_text(raw)
        if isinstance(snap, dict) and not snap.get("running", False):
            return snap
        time.sleep(poll)


def cmd_verify(args: argparse.Namespace) -> int:
    assert_idle_for_repair()
    acquire("repair", "verify")
    try:
        ensure_session(args.mcp, args.token)
        try:
            extract_text(tools_call(args.mcp, args.token, "stop_check_sources", {}))
        except Exception:
            pass
        if args.cooldown > 0:
            print(f"cooldown {args.cooldown}s (site search gap)", flush=True)
            time.sleep(args.cooldown)
        started = time.time()
        print(
            extract_text(
                tools_call(
                    args.mcp,
                    args.token,
                    "start_check_sources",
                    {
                        "urls": [args.url],
                        "enabledOnly": False,
                        "keyword": args.keyword,
                        "threadCount": 1,
                        "timeoutMs": args.timeout_ms,
                    },
                )
            )
        )
        snap = wait_check(args.mcp, args.token, args.poll)
        results = snap.get("results") if isinstance(snap, dict) else []
        item = None
        for row in results or []:
            if isinstance(row, dict) and str(row.get("url") or "") == args.url:
                item = row
                break
        if item is None and results and isinstance(results[0], dict):
            item = results[0]
        ok = bool(item and item.get("success"))
        out = {
            "url": args.url,
            "keyword": args.keyword,
            "success": ok,
            "message": (item or {}).get("message") if item else snap,
            "durationMs": int((time.time() - started) * 1000),
            "snap": {
                "success": snap.get("success") if isinstance(snap, dict) else None,
                "failed": snap.get("failed") if isinstance(snap, dict) else None,
                "error": snap.get("error") if isinstance(snap, dict) else None,
            },
        }
        if args.out:
            path = Path(args.out)
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps(out, ensure_ascii=False, indent=2), encoding="utf-8")
            print(f"wrote {path}")
        print(json.dumps(out, ensure_ascii=False, indent=2))
        return 0 if ok else 1
    finally:
        release("repair")


def cmd_log(args: argparse.Namespace) -> int:
    check = load_check(args.check_json) if args.check_json else None
    if args.status == "fixed":
        assert_fixed_allowed(check)
    payload = {
        "url": args.url,
        "name": args.name or "",
        "status": args.status,
        "keyword": args.keyword,
        "root_cause": args.root_cause or "",
        "changes": args.change or [],
        "check": check,
        "saved_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "agent": args.agent or "repair_source",
    }
    path = Path(args.out)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"wrote {path}")
    if args.index:
        append_index(
            Path(args.index),
            {
                "status": args.status,
                "url": args.url,
                "name": args.name,
                "evidence": str(path),
                "agent": payload["agent"],
                "root_cause": payload["root_cause"],
            },
        )
        print(f"updated index {args.index}")
    return 0


def cmd_channel(args: argparse.Namespace) -> int:
    snap = channel_status()
    print(json.dumps(snap, ensure_ascii=False, indent=2))
    return 0 if snap.get("idle") else 1


def cmd_index(args: argparse.Namespace) -> int:
    entry = json.loads(Path(args.from_log).read_text(encoding="utf-8"))
    append_index(Path(args.index), {
        "status": entry.get("status"),
        "url": entry.get("url"),
        "name": entry.get("name"),
        "evidence": args.from_log,
        "agent": entry.get("agent"),
        "root_cause": entry.get("root_cause"),
    })
    print(f"updated {args.index}")
    return 0


def build_parser() -> argparse.ArgumentParser:
    mcp_url, token = default_mcp()
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--mcp", default=mcp_url)
    p.add_argument("--token", default=token)
    sub = p.add_subparsers(dest="cmd", required=True)

    t = sub.add_parser("triage", help="classify fail + static rule smells")
    t.add_argument("--url", required=True)
    t.add_argument("--fail-msg", default="")
    t.add_argument("--out")
    t.set_defaults(func=cmd_triage)

    f = sub.add_parser("fetch", help="GET page with source headers; list toc candidates")
    f.add_argument("--url", required=True, help="bookSourceUrl")
    f.add_argument("--page", help="page to fetch (default: bookSourceUrl)")
    f.add_argument("--dump-dir", default="temp/full_fix/html")
    f.set_defaults(func=cmd_fetch)

    v = sub.add_parser("verify", help="single-URL device check")
    v.add_argument("--url", required=True)
    v.add_argument("--keyword", default="我的")
    v.add_argument("--timeout-ms", type=int, default=60_000)
    v.add_argument("--poll", type=float, default=2.0)
    v.add_argument("--cooldown", type=float, default=0.0, help="sleep before check")
    v.add_argument("--out")
    v.set_defaults(func=cmd_verify)

    g = sub.add_parser("log", help="write standardized fix log JSON")
    g.add_argument("--url", required=True)
    g.add_argument("--status", choices=["fixed", "skipped", "failed"], required=True)
    g.add_argument("--out", required=True)
    g.add_argument("--name", default="")
    g.add_argument("--keyword", default="我的")
    g.add_argument("--root-cause", default="")
    g.add_argument("--change", action="append", default=[])
    g.add_argument("--check-json", help="required when --status fixed")
    g.add_argument("--agent", default="")
    g.add_argument(
        "--index",
        default="temp/full_fix/repair_session_index.json",
        help="update session index (empty string to skip)",
    )
    g.set_defaults(func=cmd_log)

    c = sub.add_parser("channel", help="show MCP channel lock status")
    c.set_defaults(func=cmd_channel)

    i = sub.add_parser("index", help="register an existing fix_*.json into session index")
    i.add_argument("--from-log", required=True)
    i.add_argument("--index", default="temp/full_fix/repair_session_index.json")
    i.set_defaults(func=cmd_index)
    return p


def main() -> int:
    args = build_parser().parse_args()
    if getattr(args, "index", None) == "":
        args.index = None
    try:
        return int(args.func(args))
    except (urllib.error.URLError, RuntimeError, json.JSONDecodeError, OSError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
