#!/usr/bin/env python3
"""Refresh phone book-source index via MCP list_sources (existence SOT).

Writes temp/full_fix/phone_source_index.json for queue builders.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

_SCRIPTS = Path(__file__).resolve().parent
_ROOT = _SCRIPTS.parent
sys.path.insert(0, str(_SCRIPTS))

from mcp_client import ensure_endpoint, ensure_session, extract_text, parse_json_text, tools_call  # noqa: E402
from repair_db import bulk_upsert_list_items, connect, export_phone_index_json, load_cfg, phone_index_fresh  # noqa: E402

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


def refresh(out: Path = OUT, *, force: bool = False, ttl_s: float | None = None) -> dict[str, Any]:
    cfg = load_cfg()
    ttl = ttl_s if ttl_s is not None else float(cfg.get("phone_index_ttl_s") or 3600)
    if not force and phone_index_fresh(ttl_s=ttl, cfg=cfg):
        payload = export_phone_index_json(out, cfg)
        payload["cache_hit"] = True
        payload["skipped_mcp_pull"] = True
        return payload

    mcp, token = ensure_endpoint()
    ensure_session(mcp, token, "refresh_phone_index")
    items = fetch_all_sources(mcp, token)
    with connect(cfg) as conn:
        bulk_upsert_list_items(conn, items)
    payload = export_phone_index_json(out, cfg)
    payload["cache_hit"] = False
    payload["skipped_mcp_pull"] = False
    return payload


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--out", default=str(OUT))
    ap.add_argument("--force", action="store_true", help="ignore DB TTL and re-pull list_sources from phone")
    ap.add_argument("--ttl-s", type=float, default=None, help="override phone_index_ttl_s from config")
    args = ap.parse_args()
    payload = refresh(Path(args.out), force=args.force, ttl_s=args.ttl_s)
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
                "cache_hit": payload.get("cache_hit"),
                "skipped_mcp_pull": payload.get("skipped_mcp_pull"),
            },
            ensure_ascii=False,
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
