#!/usr/bin/env python3
"""Disable or tag dead book sources from a precheck report via MCP save_source.

Reads precheck_sources.py JSON (dead_urls), fetches each source with get_source,
sets enabled=false and/or appends group tag「网站失效」, then save_source.

Example:
  python scripts/disable_dead_sources.py \\
    --mcp http://10.0.0.43:1236/mcp --token 1234 \\
    --precheck-json temp/precheck.json --tag --disable
"""

from __future__ import annotations

import argparse
import json
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any


DEAD_TAG = "网站失效"


def mcp_call(mcp_url: str, token: str, method: str, params: dict[str, Any]) -> dict[str, Any]:
    payload = {
        "jsonrpc": "2.0",
        "id": int(time.time() * 1000) % 1_000_000_000,
        "method": method,
        "params": params,
    }
    data = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(
        mcp_url,
        data=data,
        method="POST",
        headers={
            "Content-Type": "application/json",
            "Accept": "application/json, text/event-stream",
            "X-Legado-Token": token,
        },
    )
    with urllib.request.urlopen(req, timeout=120) as resp:
        body = resp.read().decode("utf-8", errors="replace")
    if "data:" in body:
        chunks = [ln[5:].strip() for ln in body.splitlines() if ln.startswith("data:")]
        if chunks:
            body = chunks[-1]
    return json.loads(body)


def tools_call(mcp_url: str, token: str, name: str, arguments: dict[str, Any]) -> Any:
    result = mcp_call(mcp_url, token, "tools/call", {"name": name, "arguments": arguments})
    if "error" in result:
        raise RuntimeError(result["error"])
    return result.get("result", result)


def extract_text(result: Any) -> str:
    if isinstance(result, dict):
        content = result.get("content")
        if isinstance(content, list) and content:
            first = content[0]
            if isinstance(first, dict) and "text" in first:
                return str(first["text"])
    return json.dumps(result, ensure_ascii=False)


def load_dead_urls(path: Path) -> list[str]:
    data = json.loads(path.read_text(encoding="utf-8"))
    return list(data.get("dead_urls") or [])


def ensure_tag(group: str | None, tag: str) -> str:
    parts = [p for p in (group or "").split(",") if p]
    if tag not in parts:
        parts.append(tag)
    return ",".join(parts)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mcp", default="http://10.0.0.43:1236/mcp")
    parser.add_argument("--token", default="1234")
    parser.add_argument("--precheck-json", required=True)
    parser.add_argument("--disable", action="store_true", help="set enabled=false")
    parser.add_argument("--tag", action="store_true", help=f"append group {DEAD_TAG}")
    parser.add_argument("--limit", type=int, default=0, help="max URLs to process (0=all)")
    parser.add_argument("--out", default="temp/disable_dead_report.json")
    args = parser.parse_args()
    if not args.disable and not args.tag:
        print("need --disable and/or --tag", file=sys.stderr)
        return 2

    dead = load_dead_urls(Path(args.precheck_json))
    if args.limit > 0:
        dead = dead[: args.limit]
    report: dict[str, Any] = {"total": len(dead), "ok": [], "failed": []}
    print(f"dead_urls={len(dead)} disable={args.disable} tag={args.tag}")

    for url in dead:
        try:
            raw = extract_text(tools_call(args.mcp, args.token, "get_source", {"url": url}))
            source = json.loads(raw) if raw.strip().startswith("{") else None
            if not isinstance(source, dict):
                # get_source may wrap
                maybe = json.loads(raw) if raw else {}
                source = maybe.get("source") or maybe
            if not isinstance(source, dict):
                raise RuntimeError(f"unexpected get_source payload: {raw[:200]}")
            if args.disable:
                source["enabled"] = False
            if args.tag:
                source["bookSourceGroup"] = ensure_tag(source.get("bookSourceGroup"), DEAD_TAG)
            save = tools_call(
                args.mcp,
                args.token,
                "save_source",
                {
                    "format": "json",
                    "content": json.dumps(source, ensure_ascii=False),
                    "preserveEnabled": False,
                    "preserveGroup": False,
                },
            )
            report["ok"].append({"url": url, "save": extract_text(save)[:200]})
            print(f"ok {url}")
        except (urllib.error.URLError, RuntimeError, json.JSONDecodeError, TypeError) as exc:
            report["failed"].append({"url": url, "error": str(exc)})
            print(f"fail {url}: {exc}", file=sys.stderr)

    Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    Path(args.out).write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"wrote {args.out} ok={len(report['ok'])} failed={len(report['failed'])}")
    return 0 if not report["failed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
