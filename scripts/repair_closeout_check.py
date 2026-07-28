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


def selftest() -> int:
    """Same cases as `mod tests` in crates/source-cli/src/cmds/progress.rs.

    Both runtimes decide which URLs `progress next` may re-pick; they must agree.
    """
    import tempfile

    from repair_progress import ledger_fixed, ledger_skipped

    cases: list[tuple[str, list[dict[str, object]], bool]] = [
        (
            "gave_up_fail_blocks_repick",
            [{"url": "https://a.test", "step": "check", "result": "fail:encrypted_chapter"}],
            True,
        ),
        (
            "transient_fail_stays_pickable",
            [{"url": "https://a.test", "step": "check", "result": "fail:verify_fail"}],
            False,
        ),
        (
            "soft_skip_stays_pickable",
            [{"url": "https://a.test", "step": "skip", "result": "skip:no_patch"}],
            False,
        ),
        (
            "hard_skip_blocks",
            [{"url": "https://a.test", "step": "skip", "result": "skip:dead_host"}],
            True,
        ),
        (
            "final_flag_beats_soft_wording",
            [
                {
                    "url": "https://a.test",
                    "step": "check",
                    "result": "fail:校验失败",
                    "final": True,
                }
            ],
            True,
        ),
    ]
    failures: list[str] = []
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "ledger.jsonl"
        for name, rows, want_blocked in cases:
            path.write_text(
                "\n".join(json.dumps(r, ensure_ascii=False) for r in rows) + "\n",
                encoding="utf-8",
            )
            got = "https://a.test" in (ledger_skipped(path) | ledger_fixed(path))
            if got != want_blocked:
                failures.append(f"{name}: blocked={got}, want {want_blocked}")
    if failures:
        for f in failures:
            print(f"selftest FAIL: {f}", file=sys.stderr)
        return 1
    print(json.dumps({"ok": True, "cases": len(cases)}, ensure_ascii=False))
    return 0


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
    sub.add_parser("selftest", help="Python queue-blocking parity with source-cli progress")
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

    if args.cmd == "selftest":
        return selftest()

    if args.cmd == "status":
        ok, errs, detail = pending_closeout()
        detail["skill_in_sync"] = skill_in_sync()
        detail["errors"] = errs
        print(json.dumps(detail, ensure_ascii=False, indent=2))
        return 0 if ok else 1

    return 2


if __name__ == "__main__":
    raise SystemExit(main())
