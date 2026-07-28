#!/usr/bin/env python3
"""Append per-source repair reflection (skill + harness efficiency).

Examples:
  python scripts/repair_retro.py append --url URL --status fixed --msg '...' \\
      --waste-s 45 --trap 'js_search_api' --harness 'probe missed data-api' \\
      --script-fix 'repair_search_probe.js_api' --skill-fix 1
"""

from __future__ import annotations

import argparse
import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from repair_closeout import gate_trap, sync_skill_to_cursor

_ROOT = Path(__file__).resolve().parents[1]
DEFAULT = _ROOT / "temp" / "full_fix" / "repair_serial_retro.jsonl"


def append_retro(path: Path, row: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    row = dict(row)
    row.setdefault("ts", datetime.now(timezone.utc).isoformat())
    with path.open("a", encoding="utf-8") as f:
        f.write(json.dumps(row, ensure_ascii=False) + "\n")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    sub = ap.add_subparsers(dest="cmd", required=True)
    a = sub.add_parser("append")
    a.add_argument("--url", required=True)
    a.add_argument("--status", required=True)
    a.add_argument("--msg", default="")
    a.add_argument("--name", default="")
    a.add_argument("--respond-time", type=int, default=-1)
    a.add_argument("--waste-s", type=float, default=0.0)
    a.add_argument("--trap", default="")
    a.add_argument("--harness", default="", help="harness gap / inefficiency")
    a.add_argument("--script-fix", default="")
    a.add_argument("--skill-fix", type=int, default=0)
    a.add_argument("--out", default=str(DEFAULT))
    a.add_argument("--extra-json", default="", help="optional JSON object merge")
    args = ap.parse_args()
    if args.cmd != "append":
        return 2
    row: dict[str, Any] = {
        "url": args.url,
        "name": args.name,
        "status": args.status,
        "msg": args.msg[:200],
        "respondTime": args.respond_time if args.respond_time >= 0 else None,
        "waste_s": args.waste_s,
        "trap": args.trap,
        "harness": args.harness,
        "script_fix": args.script_fix,
        "skill_fix": bool(args.skill_fix),
    }
    if args.extra_json:
        try:
            extra = json.loads(args.extra_json)
            if isinstance(extra, dict):
                row.update(extra)
        except json.JSONDecodeError:
            pass
    trap = str(row.get("trap") or "").strip()
    skill_fix = bool(row.get("skill_fix"))
    if trap:
        ok, errs = gate_trap(trap, skill_fix=skill_fix)
        if not ok:
            for e in errs:
                print(f"repair_retro BLOCK: {e}", flush=True)
            return 1
    append_retro(Path(args.out), row)
    if skill_fix:
        sync_ok, sync_msg = sync_skill_to_cursor()
        if sync_ok:
            print(f"synced SKILL → {sync_msg}")
        else:
            print(f"warn: skill sync failed: {sync_msg}", flush=True)
    print(json.dumps(row, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
