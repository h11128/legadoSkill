#!/usr/bin/env python3
"""SQLite repair state (§9): phone source snapshots, ledger dual-write, cache meta.

Avoid repeated MCP list_sources/get_source when TTL-fresh rows exist in DB.
"""

from __future__ import annotations

import json
import sqlite3
import time
from contextlib import contextmanager
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterator
from urllib.parse import urlparse

_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_DB = _ROOT / "temp" / "full_fix" / "repair_state.sqlite"
DEFAULT_CFG = _ROOT / "config" / "repair_db_defaults.json"
_SCHEMA = _ROOT / "config" / "repair_db_schema.sql"


def load_cfg() -> dict[str, Any]:
    if DEFAULT_CFG.is_file():
        return json.loads(DEFAULT_CFG.read_text(encoding="utf-8"))
    return {}


def db_path(cfg: dict[str, Any] | None = None) -> Path:
    cfg = cfg or load_cfg()
    raw = str(cfg.get("db_path") or DEFAULT_DB)
    p = Path(raw)
    return p if p.is_absolute() else _ROOT / p


def norm_source_key(url: str) -> str:
    return (url or "").strip()


def host_key(url: str) -> str:
    raw = norm_source_key(url).split("##")[0].split("#")[0]
    if raw and "://" not in raw:
        raw = "http://" + raw.lstrip("/")
    host = (urlparse(raw).hostname or "").lower()
    return host.removeprefix("www.")


def _iso_now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _parse_iso(ts: str) -> float | None:
    try:
        return datetime.fromisoformat(ts.replace("Z", "+00:00")).timestamp()
    except ValueError:
        return None


@contextmanager
def connect(cfg: dict[str, Any] | None = None) -> Iterator[sqlite3.Connection]:
    path = db_path(cfg)
    path.parent.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(path, timeout=5.0)
    conn.row_factory = sqlite3.Row
    try:
        conn.execute("PRAGMA journal_mode=WAL")
        conn.execute("PRAGMA foreign_keys=ON")
        conn.executescript(_SCHEMA.read_text(encoding="utf-8"))
        conn.execute(
            "INSERT INTO schema_meta(key,value) VALUES('version','1')"
            " ON CONFLICT(key) DO NOTHING"
        )
        yield conn
        conn.commit()
    finally:
        conn.close()


def upsert_source_snapshot(conn: sqlite3.Connection, source: dict[str, Any]) -> str:
    key = norm_source_key(str(source.get("bookSourceUrl") or ""))
    if not key:
        return ""
    payload = json.dumps(source, ensure_ascii=False, separators=(",", ":"))
    rt = source.get("respondTime")
    respond_ms = int(rt) if isinstance(rt, (int, float)) else None
    conn.execute(
        """INSERT INTO source_snapshot(
             source_key, host_key, name, type, enabled, group_name,
             respond_time_ms, payload_json, pulled_at
           ) VALUES (?,?,?,?,?,?,?,?,?)
           ON CONFLICT(source_key) DO UPDATE SET
             host_key=excluded.host_key, name=excluded.name, type=excluded.type,
             enabled=excluded.enabled, group_name=excluded.group_name,
             respond_time_ms=excluded.respond_time_ms,
             payload_json=excluded.payload_json, pulled_at=excluded.pulled_at""",
        (
            key,
            host_key(key),
            source.get("bookSourceName"),
            int(source.get("bookSourceType") or 0),
            1 if source.get("enabled") is not False else 0,
            source.get("bookSourceGroup"),
            respond_ms,
            payload,
            _iso_now(),
        ),
    )
    return key


def get_source_snapshot(
    source_key: str,
    *,
    max_age_s: float | None = None,
    cfg: dict[str, Any] | None = None,
) -> dict[str, Any] | None:
    key = norm_source_key(source_key)
    if not key:
        return None
    cfg = cfg or load_cfg()
    ttl = max_age_s if max_age_s is not None else float(cfg.get("source_snapshot_ttl_s") or 600)
    with connect(cfg) as conn:
        row = conn.execute(
            "SELECT payload_json, pulled_at FROM source_snapshot WHERE source_key=?",
            (key,),
        ).fetchone()
        if not row:
            return None
        pulled = _parse_iso(str(row["pulled_at"]))
        if pulled is None or time.time() - pulled > ttl:
            return None
        return json.loads(str(row["payload_json"]))


def append_ledger_row(row: dict[str, Any], cfg: dict[str, Any] | None = None) -> None:
    cfg = cfg or load_cfg()
    if not cfg.get("dual_write_ledger", True):
        return
    url = norm_source_key(str(row.get("url") or ""))
    if not url:
        url = "https://invalid.local/"
    line = json.dumps(row, ensure_ascii=False)
    with connect(cfg) as conn:
        conn.execute(
            """INSERT INTO ledger_events(
                 ts, source_key, step, result, note, waste, row_json
               ) VALUES (?,?,?,?,?,?,?)""",
            (
                row.get("ts") or _iso_now(),
                url,
                str(row.get("step") or ""),
                str(row.get("result") or ""),
                row.get("note") or "",
                row.get("waste") or "",
                line,
            ),
        )


def import_jsonl_ledger(path: Path, cfg: dict[str, Any] | None = None) -> int:
    if not path.is_file():
        return 0
    seen: set[str] = set()
    n = 0
    with connect(cfg) as conn:
        for line in path.read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if not line:
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError:
                continue
            url = norm_source_key(str(row.get("url") or ""))
            if not url:
                continue
            dedupe = f"{row.get('ts')}|{url}|{row.get('step')}|{row.get('result')}"
            if dedupe in seen:
                continue
            seen.add(dedupe)
            conn.execute(
                """INSERT INTO ledger_events(
                     ts, source_key, step, result, note, waste, row_json
                   ) VALUES (?,?,?,?,?,?,?)""",
                (
                    row.get("ts") or _iso_now(),
                    url,
                    str(row.get("step") or ""),
                    str(row.get("result") or ""),
                    row.get("note") or "",
                    row.get("waste") or "",
                    line,
                ),
            )
            n += 1
    return n


# Re-export split modules (backward compat for callers).
from repair_db_cache_meta import (  # noqa: E402
    import_host_stats_file,
    import_html_cache_dir,
    upsert_host_stats,
    upsert_html_meta,
)
from repair_db_phone import bulk_upsert_list_items, export_phone_index_json, phone_index_fresh  # noqa: E402

__all__ = [
    "append_ledger_row",
    "bulk_upsert_list_items",
    "connect",
    "db_path",
    "export_phone_index_json",
    "get_source_snapshot",
    "host_key",
    "import_host_stats_file",
    "import_html_cache_dir",
    "import_jsonl_ledger",
    "load_cfg",
    "norm_source_key",
    "phone_index_fresh",
    "upsert_host_stats",
    "upsert_html_meta",
    "upsert_source_snapshot",
]
