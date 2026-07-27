#!/usr/bin/env python3
"""Deprecated wrapper — use repair_deep_loop.py --mode batch|oneshot."""

from __future__ import annotations

import sys
from pathlib import Path

_SCRIPTS = Path(__file__).resolve().parent
sys.path.insert(0, str(_SCRIPTS))

# Re-export via argv rewrite for old callers
if __name__ == "__main__":
    queue = _SCRIPTS.parent / "temp" / "full_fix" / "goal15_queue.json"
    # default legacy queue if missing: write from old HARDCODED list once via deep_loop empty err
    argv = [
        sys.executable,
        str(_SCRIPTS / "repair_deep_loop.py"),
        "--mode",
        "batch",
        "--limit",
        "15",
        "--out",
        "temp/full_fix/goal15_results.json",
    ]
    if queue.is_file():
        argv[4:4] = ["--urls-file", str(queue)]
    print(
        "DEPRECATED: repair_goal15_run.py → repair_deep_loop.py --mode batch|oneshot",
        flush=True,
    )
    raise SystemExit(__import__("subprocess").call(argv))
