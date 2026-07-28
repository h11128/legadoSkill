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
from repair_session_log import DEFAULT as LEDGER
from repair_session_log import append_row as append_ledger

_ROOT = Path(__file__).resolve().parents[1]
DEFAULT = _ROOT / "temp" / "full_fix" / "repair_serial_retro.jsonl"
TERMINAL_STATUSES = frozenset({"fail", "skip"})


def append_retro(path: Path, row: dict[str, Any], *, seal: bool = False) -> dict[str, Any] | None:
    """Append one retro row; with ``seal=True`` also close the URL in the ledger.

    ``seal`` must be explicit because the two kinds of caller mean different
    things by ``status=fail``:

    * automated loops (``repair_serial``, ``repair_progress`` L2 skip) mean
      "this round's auto-patch did not work" — the URL stays retryable, so they
      pass ``seal=False`` and write their own ledger row;
    * the ``repair_retro.py append`` CLI is the agent's deliberate close-out —
      "I gave up on this one" — which must stop ``progress next`` re-picking it.
    """
    path.parent.mkdir(parents=True, exist_ok=True)
    row = dict(row)
    row.setdefault("ts", datetime.now(timezone.utc).isoformat())
    with path.open("a", encoding="utf-8") as f:
        f.write(json.dumps(row, ensure_ascii=False) + "\n")
    return seal_ledger(row) if seal else None


def seal_ledger(row: dict[str, Any]) -> dict[str, Any] | None:
    """Mirror a terminal retro into the ledger so `progress next` stops re-picking it.

    Without this the queue only sees whatever the agent typed by hand, so a
    `status=fail` retro could sit next to a `check: ok` ledger row.
    """
    status = str(row.get("status") or "").strip().lower()
    url = str(row.get("url") or "").strip()
    if status not in TERMINAL_STATUSES or not url:
        return None
    reason = str(row.get("trap") or row.get("msg") or status).strip().replace("\n", " ")
    ledger_row = {
        "ts": datetime.now(timezone.utc).isoformat(),
        "url": url,
        "step": "skip" if status == "skip" else "check",
        "result": f"{status}:{reason[:80] or status}",
        "note": "sealed by repair_retro",
        "waste": "",
        "final": True,
    }
    append_ledger(LEDGER, ledger_row)
    return ledger_row


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
    sealed = append_retro(Path(args.out), row, seal=True)
    if sealed:
        print(f"sealed ledger: {sealed['step']} {sealed['result']}")
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
