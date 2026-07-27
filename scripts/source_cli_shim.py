#!/usr/bin/env python3
"""Shared resolver for SOURCE_CLI / cargo run -p source_cli."""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path

_ROOT = Path(__file__).resolve().parents[1]
_CRATES = _ROOT / "crates"


def resolve_cmd(extra: list[str]) -> tuple[list[str], Path]:
    env_bin = (os.environ.get("SOURCE_CLI") or "").strip()
    if env_bin:
        return [env_bin, *extra], _ROOT
    # Prefer prebuilt binary
    for name in ("source-cli.exe", "source-cli"):
        cand = _CRATES / "target" / "debug" / name
        if cand.is_file():
            return [str(cand), *extra], _ROOT
    cargo = shutil.which("cargo")
    if not cargo:
        print("source_cli_shim: cargo not on PATH and SOURCE_CLI unset", file=sys.stderr)
        raise SystemExit(127)
    return (
        [cargo, "run", "-p", "source_cli", "--quiet", "--", *extra],
        _CRATES,
    )


def run_source_cli(extra: list[str]) -> int:
    argv, cwd = resolve_cmd(extra)
    try:
        proc = subprocess.run(
            argv,
            cwd=cwd,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
    except FileNotFoundError as exc:
        print(f"source_cli_shim: failed to exec {argv[0]!r}: {exc}", file=sys.stderr)
        return 127
    return int(proc.returncode)
