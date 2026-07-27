#!/usr/bin/env python3
"""Search-layer parity golden: form field extraction (Rust source-cli probe)."""

from __future__ import annotations

import json
import os
import subprocess
import tempfile
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
CRATES = ROOT / "crates"
BIN = CRATES / "target" / "debug" / ("source-cli.exe" if os.name == "nt" else "source-cli")
FIXTURES = ROOT / "fixtures" / "expected" / "probe"


def _cli(args: list[str]) -> subprocess.CompletedProcess[str]:
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
        timeout=60,
    )


def run_search_parity_suite() -> dict[str, Any]:
    failures: list[str] = []
    checked = 0
    if not FIXTURES.is_dir():
        return {"suite": "search-parity", "ok": True, "detail": "no probe fixtures"}

    for path in sorted(FIXTURES.glob("*.json")):
        spec = json.loads(path.read_text(encoding="utf-8"))
        html = spec.get("html") or ""
        base = spec.get("base_url") or "https://example.com/"
        with tempfile.NamedTemporaryFile(
            "w", suffix=".html", delete=False, encoding="utf-8"
        ) as fh:
            fh.write(html)
            html_path = fh.name
        try:
            proc = _cli(
                ["probe", "--base-url", base, "--html-file", html_path, "--key", "我的"]
            )
        finally:
            Path(html_path).unlink(missing_ok=True)
        if proc.returncode != 0:
            failures.append(f"{path.name}: probe exit {proc.returncode}")
            continue
        raw = (proc.stdout or "").strip()
        try:
            out = json.loads(raw)
        except json.JSONDecodeError:
            failures.append(f"{path.name}: not JSON ({raw[:80]!r})")
            continue
        best = ((out.get("best") or {}).get("search_url")) or ""
        for needle in spec.get("expected_best_contains") or []:
            if needle not in best:
                failures.append(f"{path.name}: expected {needle!r} in {best!r}")
        for bad in spec.get("forbidden_best_contains") or []:
            if bad in best:
                failures.append(f"{path.name}: forbidden {bad!r} in {best!r}")
        checked += 1

    ok = not failures
    return {
        "suite": "search-parity",
        "ok": ok,
        "detail": f"checked {checked}" + ("; " + "; ".join(failures[:5]) if failures else ""),
        "failures": failures,
        "checked": checked,
    }


if __name__ == "__main__":
    r = run_search_parity_suite()
    print(json.dumps(r, ensure_ascii=False, indent=2))
    raise SystemExit(0 if r["ok"] else 1)
