#!/usr/bin/env python3
"""Exclusive MCP channel lock (bulk vs repair)."""

from __future__ import annotations

import json
import os
import time
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / "temp" / "mcp_channel.lock"
BULK_LOCK = ROOT / "temp" / "full_check" / "runner.lock"
STALE_S = 6 * 3600


def _pid_alive(pid: int) -> bool:
    if pid <= 0:
        return False
    try:
        os.kill(pid, 0)
    except OSError:
        return False
    except AttributeError:
        # Windows may still allow OpenProcess via tasklist; treat missing as stale if mtime old
        return True
    return True


def _read_lock(path: Path) -> dict[str, Any] | None:
    if not path.exists():
        return None
    try:
        raw = path.read_text(encoding="utf-8").strip()
        if raw.startswith("{"):
            return json.loads(raw)
        return {"owner": "bulk", "pid": int(raw), "mtime": path.stat().st_mtime}
    except (OSError, ValueError, json.JSONDecodeError):
        return {"owner": "unknown", "pid": 0, "mtime": path.stat().st_mtime}


def _stale(info: dict[str, Any], path: Path) -> bool:
    mtime = float(info.get("mtime") or path.stat().st_mtime)
    if time.time() - mtime > STALE_S:
        return True
    pid = int(info.get("pid") or 0)
    return bool(pid) and not _pid_alive(pid)


def status() -> dict[str, Any]:
    out: dict[str, Any] = {"idle": True, "holders": []}
    for path, default_owner in ((LOCK, "repair"), (BULK_LOCK, "bulk")):
        info = _read_lock(path)
        if not info:
            continue
        if _stale(info, path):
            try:
                path.unlink()
            except OSError:
                pass
            continue
        holder = {
            "path": str(path),
            "owner": info.get("owner") or default_owner,
            "pid": info.get("pid"),
            "role": info.get("role"),
        }
        out["holders"].append(holder)
        out["idle"] = False
    return out


def acquire(owner: str, role: str) -> None:
    """owner: bulk|repair. Raises RuntimeError if channel busy."""
    snap = status()
    if not snap["idle"]:
        raise RuntimeError(f"MCP channel busy: {snap['holders']}")
    LOCK.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "owner": owner,
        "role": role,
        "pid": os.getpid(),
        "mtime": time.time(),
    }
    LOCK.write_text(json.dumps(payload), encoding="utf-8")
    if owner == "bulk":
        BULK_LOCK.parent.mkdir(parents=True, exist_ok=True)
        BULK_LOCK.write_text(str(os.getpid()), encoding="utf-8")


def release(owner: str) -> None:
    for path in (LOCK, BULK_LOCK) if owner == "bulk" else (LOCK,):
        try:
            info = _read_lock(path)
            if not info:
                continue
            if int(info.get("pid") or 0) == os.getpid() or owner == "bulk":
                try:
                    path.unlink()
                except OSError:
                    pass
        except OSError:
            pass


def assert_idle_for_repair() -> None:
    snap = status()
    for h in snap.get("holders") or []:
        if h.get("owner") == "bulk" or "runner.lock" in str(h.get("path")):
            raise RuntimeError(
                f"Refuse repair/verify while bulk holds MCP: {h}. "
                "Stop full_check_runner / batch_check_mcp first."
            )
