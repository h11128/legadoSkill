#!/usr/bin/env python3
"""Thin shim: call Rust source-cli `gate --url` (L0 classify).

Uses scripts/source_cli_shim.py (SOURCE_CLI env or crates/target/debug/source-cli).

Example:
  python scripts/source_gate_rs.py --url https://www.qidian.com/book/1
  SOURCE_CLI=./target/debug/source-cli python scripts/source_gate_rs.py --url …
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

_SCRIPTS = Path(__file__).resolve().parent
if str(_SCRIPTS) not in sys.path:
    sys.path.insert(0, str(_SCRIPTS))

from source_cli_shim import run_source_cli  # noqa: E402


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--url", required=True)
    ap.add_argument("--rules", help="optional path to verify_skip_rules.json")
    ap.add_argument(
        "--l0-only",
        action="store_true",
        help="skip L1/L2 network (Rust gate --l0-only); default for this shim",
    )
    ap.add_argument(
        "--full",
        action="store_true",
        help="run full L0→L1→L2 classify (may hit network)",
    )
    args = ap.parse_args()

    extra = ["gate", "--url", args.url]
    if args.rules:
        extra.extend(["--rules", args.rules])
    if not args.full:
        extra.append("--l0-only")
    return run_source_cli(extra)


if __name__ == "__main__":
    raise SystemExit(main())
