#!/usr/bin/env python3
"""Migrate bookSourceUrl (+ rewrite absolute old-host URLs) then optional verify.

MCP has no rename: save new URL, delete old URL.

Example:
  python scripts/repair_domain_migrate.py \\
    --from-url http://www.zxcs.info --to-url https://www.zxcs.click/ \\
    --verify --keyword 我的
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
    save_source,
    tools_call,
)
from repair_check import check_args  # noqa: E402


def _defaults() -> tuple[str, str]:
    from mcp_client import load_endpoint

    return load_endpoint()


def split_comment(url: str) -> tuple[str, str]:
    if "##" in url:
        base, comment = url.split("##", 1)
        return base, "##" + comment
    return url, ""


def host_forms(host: str) -> list[str]:
    host = host.lower().rstrip(".")
    forms = {host}
    if host.startswith("www."):
        forms.add(host[4:])
    else:
        forms.add("www." + host)
    return sorted(forms, key=len, reverse=True)


def rewrite_hosts(text: str, old_host: str, new_base: str) -> str:
    """Replace scheme://[www.]old_host with new_base (no trailing slash issues)."""
    new = new_base.rstrip("/")
    out = text
    for h in host_forms(old_host):
        out = re.sub(
            rf"https?://{re.escape(h)}(?=[:/?#\"'\s,]|$)",
            new,
            out,
            flags=re.I,
        )
    return out


def migrate_payload(src: dict[str, Any], from_url: str, to_url: str) -> dict[str, Any]:
    old_base, old_cmt = split_comment(from_url)
    new_base, new_cmt = split_comment(to_url)
    old_host = urlparse(old_base if "://" in old_base else "http://" + old_base).hostname or ""
    new_host = urlparse(new_base if "://" in new_base else "https://" + new_base).hostname or ""
    if not old_host or not new_host:
        raise ValueError(f"bad hosts old={old_host!r} new={new_host!r}")

    blob = json.dumps(src, ensure_ascii=False)
    blob2 = rewrite_hosts(blob, old_host, new_base.rstrip("/"))
    out = json.loads(blob2)
    if new_cmt:
        out["bookSourceUrl"] = to_url
    elif old_cmt:
        out["bookSourceUrl"] = new_base.rstrip("/") + old_cmt
    else:
        out["bookSourceUrl"] = to_url
    return out


def wait_check(mcp: str, token: str, poll: float = 1.0) -> dict[str, Any]:
    while True:
        raw = extract_text(
            tools_call(mcp, token, "get_check_progress", {"resultOffset": 0, "resultLimit": 20})
        )
        snap = parse_json_text(raw)
        if isinstance(snap, dict) and not snap.get("running", False):
            return snap
        time.sleep(poll)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--from-url", required=True)
    ap.add_argument("--to-url", required=True)
    ap.add_argument("--verify", action="store_true")
    ap.add_argument("--keyword", default="我的")
    ap.add_argument("--enable", action="store_true", help="set enabled=true on migrated")
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--keep-old", action="store_true", help="do not delete old URL")
    ap.add_argument("--out", default="temp/full_fix/domain_migrate.json")
    args = ap.parse_args()

    mcp, token = _defaults()
    mcp_channel.assert_idle_for_repair()
    mcp_channel.acquire("repair", "domain_migrate")
    report: dict[str, Any] = {
        "ts": datetime.now(timezone.utc).isoformat(),
        "from": args.from_url,
        "to": args.to_url,
    }
    try:
        ensure_session(mcp, token, "domain_migrate")
        src = get_source(mcp, token, args.from_url)
        migrated = migrate_payload(src, args.from_url, args.to_url)
        if args.enable:
            migrated["enabled"] = True
        report["name"] = migrated.get("bookSourceName")
        report["new_url"] = migrated.get("bookSourceUrl")
        # show host rewrite count
        old_h = urlparse(split_comment(args.from_url)[0] if "://" in args.from_url else "http://" + args.from_url).hostname
        report["rewrote_host"] = old_h
        if args.dry_run:
            report["dry_run"] = True
            report["preview_keys"] = {
                k: migrated.get(k)
                for k in ("bookSourceUrl", "searchUrl", "exploreUrl")
                if migrated.get(k)
            }
        else:
            report["save"] = save_source(mcp, token, migrated, preserve_enabled=False, preserve_group=True)
            if not args.keep_old and split_comment(args.from_url)[0].rstrip("/") != split_comment(args.to_url)[0].rstrip("/"):
                report["delete_old"] = extract_text(
                    tools_call(mcp, token, "delete_sources", {"urls": [args.from_url]})
                )
            if args.verify:
                try:
                    tools_call(mcp, token, "stop_check_sources", {})
                except Exception:
                    pass
                tools_call(
                    mcp,
                    token,
                    "start_check_sources",
                    check_args(
                        [migrated["bookSourceUrl"]],
                        args.keyword,
                        thread_count=1,
                        timeout_ms=45000,
                    ),
                )
                snap = wait_check(mcp, token)
                report["verify"] = snap
                results = snap.get("results") or snap.get("items") or []
                ok = False
                for r in results:
                    if isinstance(r, dict) and (
                        r.get("success") is True or "成功" in str(r.get("message") or "")
                    ):
                        ok = True
                report["verify_ok"] = ok
    finally:
        mcp_channel.release("repair")

    path = Path(args.out)
    path.parent.mkdir(parents=True, exist_ok=True)
    # append-friendly: write unique file if default
    if args.out.endswith("domain_migrate.json"):
        path = path.with_name(
            f"domain_migrate_{datetime.now().strftime('%Y%m%d_%H%M%S')}.json"
        )
    path.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False, indent=2))
    print(f"wrote {path}", flush=True)
    if report.get("verify") is not None and not report.get("verify_ok"):
        return 1
    return 0 if "error" not in report else 1


if __name__ == "__main__":
    import os

    if os.environ.get("REPAIR_USE_PYTHON", "") != "1":
        from source_cli_shim import run_source_cli

        ap = argparse.ArgumentParser(description=__doc__)
        ap.add_argument("--from-url", required=True)
        ap.add_argument("--to-url", required=True)
        ap.add_argument("--verify", action="store_true")
        ap.add_argument("--keyword", default="我的")
        ap.add_argument("--enable", action="store_true")
        ap.add_argument("--dry-run", action="store_true")
        ap.add_argument("--keep-old", action="store_true")
        ap.add_argument("--out", default="temp/full_fix/domain_migrate.json")
        args = ap.parse_args()
        extra = [
            "migrate",
            "--from-url",
            args.from_url,
            "--to-url",
            args.to_url,
        ]
        if args.dry_run:
            extra.append("--dry-run")
        if args.keep_old:
            extra.append("--keep-old")
        # --verify/--enable still Python path for now
        if args.verify or args.enable:
            raise SystemExit(main())
        raise SystemExit(run_source_cli(extra))
    raise SystemExit(main())
