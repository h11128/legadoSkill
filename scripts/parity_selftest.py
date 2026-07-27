#!/usr/bin/env python3
"""Parity harness: Python baseline vs Rust §12 contract (see repair-adapter-architecture.md)."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

_SCRIPTS = Path(__file__).resolve().parent
if str(_SCRIPTS) not in sys.path:
    sys.path.insert(0, str(_SCRIPTS))

from parity_suites import SUITE_RUNNERS

ALL_SUITES = (
    "fixtures",
    "cli-help",
    "imports",
    "schemas",
    "inventory",
    "rust-cli",
    "search-parity",
)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--suite",
        action="append",
        choices=ALL_SUITES,
        help=f"run suite (default: all). choices: {', '.join(ALL_SUITES)}",
    )
    args = ap.parse_args()
    selected = args.suite or list(ALL_SUITES)

    results: list[dict] = []
    for name in selected:
        results.append(SUITE_RUNNERS[name]())

    passed = sum(1 for r in results if r.get("ok"))
    summary = {
        "ok": passed == len(results),
        "suites_run": len(results),
        "suites_passed": passed,
        "results": results,
    }
    print("SUMMARY " + json.dumps(summary, ensure_ascii=False))
    return 0 if summary["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
