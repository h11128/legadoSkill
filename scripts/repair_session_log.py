#!/usr/bin/env python3
"""Append/show repair session ledger (force process logging).

Example:
  python scripts/repair_session_log.py append \\
    --url https://ukuzy.com/ --step check --result '校验成功' \\
    --note 'downloadUrls+bookUrl'
  python scripts/repair_session_log.py show --tail 10
"""

from __future__ import annotations

import argparse
import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

_ROOT = Path(__file__).resolve().parents[1]
DEFAULT = _ROOT / "temp" / "full_fix" / "repair_session_ledger.jsonl"
MD_HINT = _ROOT / "docs" / "source-repair-session-phase-migrate-video-2026-07-26.md"


def append_row(path: Path, row: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as f:
        f.write(json.dumps(row, ensure_ascii=False) + "\n")


def show(path: Path, tail: int) -> None:
    if not path.is_file():
        print("(empty)")
        return
    lines = path.read_text(encoding="utf-8").splitlines()
    for line in lines[-max(1, tail) :]:
        print(line)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    sub = ap.add_subparsers(dest="cmd", required=True)

    a = sub.add_parser("append", help="append one ledger line")
    a.add_argument("--url", default="")
    a.add_argument("--step", required=True, help="e.g. migrate|html|patch|debug|check|divert|skip")
    a.add_argument("--result", required=True)
    a.add_argument("--note", default="")
    a.add_argument("--waste", default="", help="optional: what went wrong / minutes wasted")
    a.add_argument("--ledger", default=str(DEFAULT))

    s = sub.add_parser("show", help="print recent lines")
    s.add_argument("--tail", type=int, default=15)
    s.add_argument("--ledger", default=str(DEFAULT))

    args = ap.parse_args()
    path = Path(args.ledger)
    if args.cmd == "show":
        show(path, args.tail)
        return 0

    row = {
        "ts": datetime.now(timezone.utc).isoformat(),
        "url": args.url,
        "step": args.step,
        "result": args.result,
        "note": args.note,
        "waste": args.waste,
    }
    append_row(path, row)
    print(json.dumps(row, ensure_ascii=False))
    print(f"appended {path} (also keep phase MD in sync when closing a wave: {MD_HINT.name})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
