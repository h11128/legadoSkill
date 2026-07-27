#!/usr/bin/env python3
"""Shared dynamic wait for start_check_sources (progress polling).

Harvest felt ~2min because batches waited for slow sources to hit timeout_ms,
not because polling was absent. This module:
  - polls get_check_progress with adaptive interval
  - prints finished/total
  - pages all results (not resultLimit=20 truncation)
  - optional max_wait_s hard stop
"""

from __future__ import annotations

import time
from typing import Any

from mcp_client import extract_text, parse_json_text, tools_call


def fetch_all_results(mcp: str, token: str, seed: dict[str, Any] | None = None) -> dict[str, Any]:
    seed = seed or {}
    all_results: list[Any] = []
    offset = 0
    total = int(seed.get("resultTotal") or 0)
    last: dict[str, Any] = dict(seed)
    while True:
        raw = extract_text(
            tools_call(
                mcp,
                token,
                "get_check_progress",
                {"resultOffset": offset, "resultLimit": 500},
            )
        )
        page = parse_json_text(raw)
        if not isinstance(page, dict):
            break
        last = page
        chunk = page.get("results") or []
        if isinstance(chunk, list):
            all_results.extend(chunk)
        total = int(page.get("resultTotal") or total)
        offset += len(chunk) if isinstance(chunk, list) else 0
        if not chunk or offset >= total:
            break
    last["results"] = all_results
    return last


def wait_check(
    mcp: str,
    token: str,
    *,
    poll_s: float = 0.4,
    poll_max_s: float = 1.2,
    max_wait_s: float = 180.0,
    expect_n: int | None = None,
    progress: bool = True,
) -> dict[str, Any]:
    """Poll until running=false (or max_wait). Adaptive sleep; full result page-in."""
    started = time.time()
    interval = max(0.2, poll_s)
    last_fin = -1
    snap: dict[str, Any] = {}
    while True:
        raw = extract_text(
            tools_call(
                mcp,
                token,
                "get_check_progress",
                {"resultOffset": 0, "resultLimit": 1},
            )
        )
        snap = parse_json_text(raw) if isinstance(parse_json_text(raw), dict) else {}
        if not isinstance(snap, dict):
            snap = {}
        running = bool(snap.get("running", False))
        finished = int(snap.get("finished") or 0)
        total = int(snap.get("total") or 0)
        if progress and finished != last_fin:
            last_fin = finished
            print(
                f"check {finished}/{total or '?'} "
                f"ok={snap.get('success')} fail={snap.get('failed')} "
                f"{time.time() - started:.1f}s",
                flush=True,
            )
        if not running:
            return fetch_all_results(mcp, token, snap)
        # Early exit if we have all expected results and job stopped producing
        if expect_n and finished >= expect_n and not running:
            return fetch_all_results(mcp, token, snap)
        if time.time() - started >= max_wait_s:
            if progress:
                print(f"check max_wait {max_wait_s}s — collecting partial", flush=True)
            try:
                extract_text(tools_call(mcp, token, "stop_check_sources", {}))
            except Exception:
                pass
            time.sleep(0.3)
            return fetch_all_results(mcp, token, snap)
        time.sleep(interval)
        # speed up when finishing, slow slightly when stalled
        if finished > last_fin:
            interval = poll_s
        else:
            interval = min(poll_max_s, interval * 1.25)
