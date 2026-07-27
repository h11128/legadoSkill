#!/usr/bin/env python3
"""Disk cache for repair: HTML bodies, host EWMA cooldown, triage blobs."""

from __future__ import annotations

import hashlib
import json
import time
from pathlib import Path
from typing import Any
from urllib.parse import urlparse

ROOT = Path(__file__).resolve().parents[1]
CACHE = ROOT / "temp" / "full_fix" / "cache"
HTML_DIR = CACHE / "html"
HOST_STATS = CACHE / "host_stats.json"
TRIAGE_DIR = CACHE / "triage"
DEFAULT_GAP_S = 3.0
EWMA_ALPHA = 0.3


def _ensure() -> None:
    HTML_DIR.mkdir(parents=True, exist_ok=True)
    TRIAGE_DIR.mkdir(parents=True, exist_ok=True)


def url_key(url: str) -> str:
    return hashlib.sha256(url.encode("utf-8")).hexdigest()[:24]


def host_of(url: str) -> str:
    return urlparse(url).netloc.lower()


def get_html(url: str, max_age_s: float = 3600.0) -> dict[str, Any] | None:
    _ensure()
    meta_path = HTML_DIR / f"{url_key(url)}.json"
    bin_path = HTML_DIR / f"{url_key(url)}.bin"
    if not meta_path.is_file() or not bin_path.is_file():
        return None
    meta = json.loads(meta_path.read_text(encoding="utf-8"))
    if time.time() - float(meta.get("saved_at") or 0) > max_age_s:
        return None
    body = bin_path.read_bytes()
    return {**meta, "body": body, "cache_hit": True}


def put_html(url: str, result: dict[str, Any]) -> None:
    _ensure()
    body = result.get("body")
    if not isinstance(body, (bytes, bytearray)):
        return
    key = url_key(url)
    meta = {
        k: result.get(k)
        for k in (
            "ok",
            "status",
            "final_url",
            "content_type",
            "bytes",
            "rate_limited",
            "toc_candidate_links",
            "snippet",
        )
    }
    meta["url"] = url
    meta["saved_at"] = time.time()
    (HTML_DIR / f"{key}.json").write_text(
        json.dumps(meta, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    (HTML_DIR / f"{key}.bin").write_bytes(bytes(body))


def _load_hosts() -> dict[str, Any]:
    _ensure()
    if not HOST_STATS.is_file():
        return {}
    try:
        return json.loads(HOST_STATS.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return {}


def _save_hosts(data: dict[str, Any]) -> None:
    _ensure()
    HOST_STATS.write_text(json.dumps(data, ensure_ascii=False, indent=2), encoding="utf-8")


def note_rate_limit(url: str, suggested_gap_s: float = 20.0) -> None:
    host = host_of(url)
    data = _load_hosts()
    row = data.get(host) or {"ewma_gap_s": DEFAULT_GAP_S, "hits": 0, "rate_limits": 0}
    prev = float(row.get("ewma_gap_s") or DEFAULT_GAP_S)
    row["ewma_gap_s"] = prev * (1 - EWMA_ALPHA) + suggested_gap_s * EWMA_ALPHA
    row["rate_limits"] = int(row.get("rate_limits") or 0) + 1
    row["last_rate_limit_at"] = time.time()
    data[host] = row
    _save_hosts(data)


def note_verify(url: str, success: bool, duration_ms: int, used_cooldown_s: float) -> None:
    host = host_of(url)
    data = _load_hosts()
    row = data.get(host) or {"ewma_gap_s": DEFAULT_GAP_S, "hits": 0, "ok": 0, "fail": 0}
    row["hits"] = int(row.get("hits") or 0) + 1
    if success:
        row["ok"] = int(row.get("ok") or 0) + 1
        # successful verify after cooldown → gently decay gap toward used_cooldown
        prev = float(row.get("ewma_gap_s") or DEFAULT_GAP_S)
        target = max(DEFAULT_GAP_S, min(used_cooldown_s, 30.0))
        row["ewma_gap_s"] = prev * (1 - EWMA_ALPHA) + target * EWMA_ALPHA
    else:
        row["fail"] = int(row.get("fail") or 0) + 1
    row["last_duration_ms"] = duration_ms
    row["last_at"] = time.time()
    data[host] = row
    _save_hosts(data)


def cooldown_for(url: str, concurrent_rate: str | None = None) -> float:
    host = host_of(url)
    data = _load_hosts()
    row = data.get(host) or {}
    gap = float(row.get("ewma_gap_s") or DEFAULT_GAP_S)
    # concurrentRate in legado is often ms between requests as string int
    cr = 0.0
    if concurrent_rate and str(concurrent_rate).isdigit():
        cr = max(0.0, int(concurrent_rate) / 1000.0)
    return max(gap, cr, DEFAULT_GAP_S)


def put_triage(url: str, report: dict[str, Any]) -> None:
    _ensure()
    path = TRIAGE_DIR / f"{url_key(url)}.json"
    path.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")


def get_triage(url: str, max_age_s: float = 1800.0) -> dict[str, Any] | None:
    path = TRIAGE_DIR / f"{url_key(url)}.json"
    if not path.is_file():
        return None
    data = json.loads(path.read_text(encoding="utf-8"))
    if time.time() - float(data.get("cached_at") or 0) > max_age_s:
        return None
    return data
