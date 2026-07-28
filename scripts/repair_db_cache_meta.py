"""Cache metadata sync into SQLite (HTML + host EWMA)."""

from __future__ import annotations

import json
import time
from pathlib import Path
from typing import Any

from repair_db import connect, host_key, load_cfg

_ROOT = Path(__file__).resolve().parents[1]
HTML_DIR = _ROOT / "temp" / "full_fix" / "cache" / "html"
HOST_STATS = _ROOT / "temp" / "full_fix" / "cache" / "host_stats.json"


def upsert_html_meta(
    cache_key: str,
    url: str,
    meta: dict[str, Any],
    bin_rel: str,
    cfg: dict[str, Any] | None = None,
) -> None:
    cfg = cfg or load_cfg()
    if not cfg.get("sync_html_meta_on_put", True):
        return
    with connect(cfg) as conn:
        conn.execute(
            """INSERT INTO html_cache_meta(
                 cache_key, url, host_key, saved_at, status, final_url,
                 content_type, bytes, rate_limited, bin_path
               ) VALUES (?,?,?,?,?,?,?,?,?,?)
               ON CONFLICT(cache_key) DO UPDATE SET
                 url=excluded.url, host_key=excluded.host_key,
                 saved_at=excluded.saved_at, status=excluded.status,
                 final_url=excluded.final_url, content_type=excluded.content_type,
                 bytes=excluded.bytes, rate_limited=excluded.rate_limited,
                 bin_path=excluded.bin_path""",
            (
                cache_key,
                url,
                host_key(url),
                float(meta.get("saved_at") or time.time()),
                meta.get("status"),
                meta.get("final_url"),
                meta.get("content_type"),
                meta.get("bytes"),
                1 if meta.get("rate_limited") else 0,
                bin_rel,
            ),
        )


def upsert_host_stats(host: str, row: dict[str, Any], cfg: dict[str, Any] | None = None) -> None:
    cfg = cfg or load_cfg()
    if not cfg.get("sync_host_stats_on_put", True):
        return
    with connect(cfg) as conn:
        conn.execute(
            """INSERT INTO host_stats(
                 host_key, ewma_gap_s, hits, ok, fail, rate_limits,
                 last_rate_limit_at, last_duration_ms, last_at
               ) VALUES (?,?,?,?,?,?,?,?,?)
               ON CONFLICT(host_key) DO UPDATE SET
                 ewma_gap_s=excluded.ewma_gap_s, hits=excluded.hits,
                 ok=excluded.ok, fail=excluded.fail,
                 rate_limits=excluded.rate_limits,
                 last_rate_limit_at=excluded.last_rate_limit_at,
                 last_duration_ms=excluded.last_duration_ms,
                 last_at=excluded.last_at""",
            (
                host,
                float(row.get("ewma_gap_s") or 3.0),
                int(row.get("hits") or 0),
                int(row.get("ok") or 0),
                int(row.get("fail") or 0),
                int(row.get("rate_limits") or 0),
                row.get("last_rate_limit_at"),
                row.get("last_duration_ms"),
                row.get("last_at"),
            ),
        )


def import_html_cache_dir(
    html_dir: Path | None = None,
    *,
    cfg: dict[str, Any] | None = None,
) -> int:
    """Scan disk HTML cache (*.json + *.bin) into html_cache_meta."""
    cfg = cfg or load_cfg()
    root = html_dir or HTML_DIR
    if not root.is_dir():
        return 0
    n = 0
    with connect(cfg) as conn:
        for meta_path in sorted(root.glob("*.json")):
            key = meta_path.stem
            bin_path = root / f"{key}.bin"
            if not bin_path.is_file():
                continue
            try:
                meta = json.loads(meta_path.read_text(encoding="utf-8"))
            except json.JSONDecodeError:
                continue
            url = str(meta.get("url") or "")
            if not url:
                continue
            conn.execute(
                """INSERT INTO html_cache_meta(
                     cache_key, url, host_key, saved_at, status, final_url,
                     content_type, bytes, rate_limited, bin_path
                   ) VALUES (?,?,?,?,?,?,?,?,?,?)
                   ON CONFLICT(cache_key) DO UPDATE SET
                     url=excluded.url, host_key=excluded.host_key,
                     saved_at=excluded.saved_at, status=excluded.status,
                     final_url=excluded.final_url, content_type=excluded.content_type,
                     bytes=excluded.bytes, rate_limited=excluded.rate_limited,
                     bin_path=excluded.bin_path""",
                (
                    key,
                    url,
                    host_key(url),
                    float(meta.get("saved_at") or time.time()),
                    meta.get("status"),
                    meta.get("final_url"),
                    meta.get("content_type"),
                    meta.get("bytes") or bin_path.stat().st_size,
                    1 if meta.get("rate_limited") else 0,
                    f"html/{key}.bin",
                ),
            )
            n += 1
    return n


def import_host_stats_file(
    path: Path | None = None,
    *,
    cfg: dict[str, Any] | None = None,
) -> int:
    """Import host_stats.json EWMA rows into SQLite."""
    cfg = cfg or load_cfg()
    p = path or HOST_STATS
    if not p.is_file():
        return 0
    try:
        data = json.loads(p.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return 0
    n = 0
    for host, row in data.items():
        if isinstance(row, dict):
            upsert_host_stats(str(host), row, cfg)
            n += 1
    return n
