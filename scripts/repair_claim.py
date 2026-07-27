#!/usr/bin/env python3
"""Validate repair claim payloads (anti fake-fixed)."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any


def load_check(path: str | Path | None) -> dict[str, Any] | None:
    if not path:
        return None
    p = Path(path)
    if not p.is_file():
        raise FileNotFoundError(f"check-json missing: {p}")
    data = json.loads(p.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise ValueError("check-json must be an object")
    return data


def assert_fixed_allowed(check: dict[str, Any] | None) -> None:
    """Refuse status=fixed unless device verify evidence says success."""
    if check is None:
        raise RuntimeError(
            "Refuse status=fixed without --check-json from repair_source.py verify"
        )
    ok = check.get("success")
    if ok is True:
        return
    # nested shapes from older logs
    nested = check.get("check") if isinstance(check.get("check"), dict) else None
    if nested and nested.get("final") == "pass":
        return
    attempts = check.get("attempts") if isinstance(check.get("attempts"), list) else []
    if any(isinstance(a, dict) and a.get("success") is True for a in attempts):
        return
    raise RuntimeError(
        f"Refuse status=fixed: check-json success!=true ({check.get('message') or check})"
    )


def append_index(index_path: Path, entry: dict[str, Any]) -> None:
    index_path.parent.mkdir(parents=True, exist_ok=True)
    if index_path.is_file():
        data = json.loads(index_path.read_text(encoding="utf-8"))
    else:
        data = {
            "session_id": "local",
            "verified_fixed": [],
            "unverified_claimed_fixed": [],
            "skipped": [],
            "failed": [],
        }
    status = entry.get("status")
    url = entry.get("url")
    item = {k: entry.get(k) for k in ("url", "name", "evidence", "agent", "root_cause") if entry.get(k)}
    if status == "fixed":
        data.setdefault("verified_fixed", [])
        data["verified_fixed"] = [x for x in data["verified_fixed"] if x.get("url") != url]
        data["verified_fixed"].append(item)
        # drop from unverified if present
        data["unverified_claimed_fixed"] = [
            x for x in data.get("unverified_claimed_fixed") or [] if x.get("url") != url
        ]
    elif status == "skipped":
        data.setdefault("skipped", []).append(item)
    elif status == "failed":
        data.setdefault("failed", []).append(item)
    index_path.write_text(json.dumps(data, ensure_ascii=False, indent=2), encoding="utf-8")
