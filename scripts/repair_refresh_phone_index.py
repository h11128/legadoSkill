#!/usr/bin/env python3
"""Refresh phone book-source index via MCP list_sources (existence SOT).

Writes temp/full_fix/phone_source_index.json for queue builders.
"""

from __future__ import annotations

import argparse
import json
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

_SCRIPTS = Path(__file__).resolve().parent
_ROOT = _SCRIPTS.parent
sys.path.insert(0, str(_SCRIPTS))

from mcp_client import ensure_endpoint, ensure_session, extract_text, parse_json_text, tools_call  # noqa: E402

OUT = _ROOT / "temp" / "full_fix" / "phone_source_index.json"


def fetch_all_sources(mcp: str, token: str, *, page: int = 200) -> list[dict[str, Any]]:
    items: list[dict[str, Any]] = []
    offset = 0
    total = None
    while True:
        raw = extract_text(
            tools_call(
                mcp,
                token,
                "list_sources",
                {"offset": offset, "limit": page},
                timeout=120.0,
            )
        )
        data = parse_json_text(raw)
        if not isinstance(data, dict):
            break
        chunk = data.get("items") or []
        if not isinstance(chunk, list):
            break
        items.extend([x for x in chunk if isinstance(x, dict)])
        total = int(data.get("total") or total or 0)
        offset += len(chunk)
        if not chunk or (total and offset >= total):
            break
    return items


def refresh(out: Path = OUT) -> dict[str, Any]:
    mcp, token = ensure_endpoint()
    ensure_session(mcp, token, "refresh_phone_index")
    items = fetch_all_sources(mcp, token)
    urls = []
    by_url: dict[str, dict[str, Any]] = {}
    for it in items:
        u = str(it.get("bookSourceUrl") or "").strip()
        if not u:
            continue
        urls.append(u)
        by_url[u] = {
            "url": u,
            "name": it.get("bookSourceName") or "",
            "group": it.get("bookSourceGroup") or "",
            "enabled": it.get("enabled"),
            "isJsSource": it.get("isJsSource"),
        }
    payload = {
        "ts": datetime.now(timezone.utc).isoformat(),
        "total": len(urls),
        "urls": urls,
        "by_url": by_url,
    }
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(payload, ensure_ascii=False), encoding="utf-8")
    return payload


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--out", default=str(OUT))
    args = ap.parse_args()
    payload = refresh(Path(args.out))
    fails = [
        v
        for v in (payload.get("by_url") or {}).values()
        if v.get("enabled") is not False and "搜索" in str(v.get("group") or "")
    ]
    print(
        json.dumps(
            {
                "total": payload.get("total"),
                "search_tag_n": len(fails),
                "out": args.out,
            },
            ensure_ascii=False,
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
