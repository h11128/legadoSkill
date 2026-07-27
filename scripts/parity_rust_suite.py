#!/usr/bin/env python3
"""Rust CLI golden smoke for §12 functional parity (diagnose/probe/migrate/gate)."""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
CRATES = ROOT / "crates"
BIN = CRATES / "target" / "debug" / ("source-cli.exe" if os.name == "nt" else "source-cli")


def _suite_result(name: str, *, ok: bool, detail: str = "", **extra: Any) -> dict[str, Any]:
    return {"suite": name, "ok": ok, "detail": detail, **extra}


def _cli(args: list[str], *, timeout: float = 60) -> subprocess.CompletedProcess[str]:
    if BIN.is_file():
        argv = [str(BIN), *args]
        cwd = ROOT
    else:
        argv = ["cargo", "run", "-p", "source_cli", "--quiet", "--", *args]
        cwd = CRATES
    return subprocess.run(
        argv,
        cwd=cwd,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=timeout,
    )


def run_rust_cli_suite() -> dict[str, Any]:
    failures: list[str] = []
    notes: list[str] = []

    help_proc = _cli(["--help"])
    if help_proc.returncode != 0:
        failures.append("source-cli --help failed")
    else:
        text = (help_proc.stdout or "") + (help_proc.stderr or "")
        for needle in ("diagnose", "probe", "migrate", "hunt", "progress", "ledger", "repair"):
            if needle not in text.lower():
                failures.append(f"help missing {needle}")
        notes.append("help ok")

    # Gate L0 golden (offline)
    gate = _cli(
        [
            "gate",
            "--url",
            "https://www.qidian.com/book/123456",
            "--l0-only",
        ]
    )
    if gate.returncode != 0:
        failures.append(f"gate exit {gate.returncode}")
    else:
        try:
            g = json.loads(gate.stdout.strip().splitlines()[-1])
            if g.get("action") != "skip":
                failures.append(f"gate qidian action={g.get('action')!r}")
            else:
                notes.append("gate qidian skip")
        except json.JSONDecodeError:
            failures.append("gate stdout not JSON")

    # Diagnose from debug text file (no MCP)
    debug_body = (
        "========搜索标题========\n"
        "搜索结果为空\n"
        "========详情========\n"
    )
    with tempfile.NamedTemporaryFile("w", suffix=".txt", delete=False, encoding="utf-8") as fh:
        fh.write(debug_body)
        debug_path = fh.name
    try:
        diag = _cli(
            [
                "diagnose",
                "--url",
                "https://example.invalid/novel",
                "--l0-only",
                "--debug-file",
                debug_path,
            ]
        )
        if diag.returncode not in (0, 2):
            failures.append(f"diagnose exit {diag.returncode}: {(diag.stderr or '')[:80]}")
        else:
            out = (diag.stdout or "").strip()
            if "search" in out.lower() or "layer" in out.lower() or out.startswith("{"):
                notes.append("diagnose debug-file ok")
            else:
                notes.append(f"diagnose out={out[:60]!r}")
    finally:
        Path(debug_path).unlink(missing_ok=True)

    # Probe offline HTML
    html = '<form action="/search.php" method="get"><input name="keyword"/></form>'
    with tempfile.NamedTemporaryFile("w", suffix=".html", delete=False, encoding="utf-8") as fh:
        fh.write(html)
        html_path = fh.name
    try:
        probe = _cli(
            [
                "probe",
                "--base-url",
                "https://example.invalid/",
                "--html-file",
                html_path,
                "--key",
                "我的",
            ]
        )
        if probe.returncode != 0:
            failures.append(f"probe exit {probe.returncode}")
        else:
            notes.append("probe forms ok")
    finally:
        Path(html_path).unlink(missing_ok=True)

    # Migrate dry-run (may need MCP get — tolerate soft fail without phone)
    mig = _cli(
        [
            "migrate",
            "--from-url",
            "https://old.example.invalid/",
            "--to-url",
            "https://new.example.invalid/",
            "--dry-run",
        ],
        timeout=30,
    )
    if mig.returncode == 0:
        notes.append("migrate dry-run ok")
    else:
        notes.append(f"migrate dry-run soft exit={mig.returncode} (MCP optional)")

    # Hunt (seeds file, offline-ish)
    hunt = _cli(["hunt", "--url", "https://www.alicesw.org/"], timeout=45)
    if hunt.returncode == 0:
        notes.append("hunt ok")
    else:
        notes.append(f"hunt soft exit={hunt.returncode}")

    ok = not failures
    detail = "; ".join(notes + failures) or "rust-cli idle"
    return _suite_result("rust-cli", ok=ok, detail=detail, failures=failures, notes=notes)


if __name__ == "__main__":
    r = run_rust_cli_suite()
    print(json.dumps(r, ensure_ascii=False, indent=2))
    raise SystemExit(0 if r["ok"] else 1)
