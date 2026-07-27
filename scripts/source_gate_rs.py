#!/usr/bin/env python3
"""Thin shim: call Rust source-cli `gate --url` (L0 classify).

Resolution order for the binary:
  1. env SOURCE_CLI — path to built `source-cli` (or `source-cli.exe`)
  2. `cargo run -p source_cli --quiet -- …` from repo `crates/`

Example:
  python scripts/source_gate_rs.py --url https://www.qidian.com/book/1
  SOURCE_CLI=./target/debug/source-cli python scripts/source_gate_rs.py --url …
"""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
from pathlib import Path

_ROOT = Path(__file__).resolve().parents[1]
_CRATES = _ROOT / "crates"


def _resolve_cmd(extra: list[str]) -> tuple[list[str], Path]:
    env_bin = (os.environ.get("SOURCE_CLI") or "").strip()
    if env_bin:
        return [env_bin, *extra], _ROOT
    cargo = shutil.which("cargo")
    if not cargo:
        print("source_gate_rs: cargo not on PATH and SOURCE_CLI unset", file=sys.stderr)
        raise SystemExit(127)
    return (
        [cargo, "run", "-p", "source_cli", "--quiet", "--", *extra],
        _CRATES,
    )


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
    # Default offline-friendly L0; --full opts into network classify.
    if not args.full:
        extra.append("--l0-only")
    argv, cwd = _resolve_cmd(extra)
    try:
        proc = subprocess.run(
            argv,
            cwd=cwd,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
    except FileNotFoundError as exc:
        print(f"source_gate_rs: failed to exec {argv[0]!r}: {exc}", file=sys.stderr)
        return 127
    return int(proc.returncode)


if __name__ == "__main__":
    raise SystemExit(main())
