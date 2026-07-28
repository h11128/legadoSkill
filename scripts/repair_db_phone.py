"""Phone index snapshot helpers (list_sources bulk cache)."""

from __future__ import annotations

import json
import sqlite3
import time
from pathlib import Path
from typing import Any

from repair_db import _iso_now, _parse_iso, connect, load_cfg, upsert_source_snapshot

_ROOT = Path(__file__).resolve().parents[1]


def bulk_upsert_list_items(conn: sqlite3.Connection, items: list[dict[str, Any]]) -> int:
    n = 0
    for it in items:
        if upsert_source_snapshot(conn, it):
            n += 1
    conn.execute(
        "INSERT INTO schema_meta(key,value) VALUES('phone_pull_at', ?)"
        " ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        (_iso_now(),),
    )
    conn.execute(
        "INSERT INTO schema_meta(key,value) VALUES('phone_pull_total', ?)"
        " ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        (str(n),),
    )
    return n


def phone_index_fresh(ttl_s: float | None = None, cfg: dict[str, Any] | None = None) -> bool:
    cfg = cfg or load_cfg()
    ttl = ttl_s if ttl_s is not None else float(cfg.get("phone_index_ttl_s") or 3600)
    with connect(cfg) as conn:
        row = conn.execute(
            "SELECT value FROM schema_meta WHERE key='phone_pull_at'"
        ).fetchone()
        if not row:
            return False
        pulled = _parse_iso(str(row["value"]))
        if pulled is None or time.time() - pulled > ttl:
            return False
        n = conn.execute("SELECT COUNT(*) c FROM source_snapshot").fetchone()
        return int(n["c"] if n else 0) > 0


def export_phone_index_json(out: Path, cfg: dict[str, Any] | None = None) -> dict[str, Any]:
    cfg = cfg or load_cfg()
    with connect(cfg) as conn:
        rows = conn.execute(
            """SELECT source_key, name, group_name, enabled, respond_time_ms, pulled_at
               FROM source_snapshot ORDER BY respond_time_ms ASC NULLS LAST"""
        ).fetchall()
        meta = conn.execute(
            "SELECT value FROM schema_meta WHERE key='phone_pull_at'"
        ).fetchone()
    by_url: dict[str, dict[str, Any]] = {}
    urls: list[str] = []
    for r in rows:
        u = str(r["source_key"])
        urls.append(u)
        by_url[u] = {
            "url": u,
            "name": r["name"] or "",
            "group": r["group_name"] or "",
            "enabled": bool(r["enabled"]),
            "respondTime": r["respond_time_ms"],
        }
    payload = {
        "ts": str(meta["value"]) if meta else _iso_now(),
        "total": len(urls),
        "urls": urls,
        "by_url": by_url,
        "from_db": True,
    }
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(payload, ensure_ascii=False), encoding="utf-8")
    return payload
