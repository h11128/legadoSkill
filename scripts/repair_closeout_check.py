#!/usr/bin/env python3
"""CLI for structural close-out: gate, pending (blocks progress next), skill sync.

See docs/repair-closeout-gate.md. Core logic in repair_closeout.py.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from repair_closeout import (
    ROOT,
    ensure_ready_for_next,
    gate_trap,
    pending_closeout,
    skill_in_sync,
    sync_skill_to_cursor,
)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    sub = ap.add_subparsers(dest="cmd", required=True)

    g = sub.add_parser("gate", help="verify trap slug vs SKILL / skill_fix")
    g.add_argument("--trap", required=True)
    g.add_argument("--skill-fix", type=int, default=0, choices=(0, 1))
    g.add_argument("--harness-file", action="append", default=[])
    g.add_argument("--require-harness", action="store_true")

    sub.add_parser("pending", help="block if last ledger URL lacks valid retro/gate")
    sub.add_parser("sync-skill", help="copy skills/…/SKILL.md → ~/.cursor/skills/…")
    st = sub.add_parser("status", help="JSON: pending state + skill sync fingerprint")

    args = ap.parse_args()

    if args.cmd == "gate":
        paths = [Path(h) if Path(h).is_absolute() else ROOT / h for h in args.harness_file]
        ok, errs = gate_trap(
            args.trap,
            skill_fix=bool(args.skill_fix),
            harness_files=paths,
            require_harness=args.require_harness,
        )
        if ok:
            print(json.dumps({"ok": True, "trap": args.trap}, ensure_ascii=False))
            return 0
        for e in errs:
            print(f"close-out gate FAIL: {e}", file=sys.stderr)
        return 1

    if args.cmd == "pending":
        return ensure_ready_for_next()

    if args.cmd == "sync-skill":
        ok, msg = sync_skill_to_cursor()
        if ok:
            print(json.dumps({"ok": True, "cursor_skill": msg}, ensure_ascii=False))
            return 0
        print(f"sync-skill FAIL: {msg}", file=sys.stderr)
        return 1

    if args.cmd == "status":
        ok, errs, detail = pending_closeout()
        detail["skill_in_sync"] = skill_in_sync()
        detail["errors"] = errs
        print(json.dumps(detail, ensure_ascii=False, indent=2))
        return 0 if ok else 1

    return 2


if __name__ == "__main__":
    raise SystemExit(main())
