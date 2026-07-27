#!/usr/bin/env python3
"""Inventory scripts/*.py with __main__ and compare to §12.2 parity matrix.

Examples:
  python scripts/parity_inventory.py --write
  python scripts/parity_inventory.py --check
  python scripts/parity_inventory.py --check --strict
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "scripts"
INVENTORY_PATH = ROOT / "docs" / "parity" / "SCRIPT_INVENTORY.json"
MATRIX_DOC = ROOT / "docs" / "repair-adapter-architecture.md"
MATRIX_SECTION = re.compile(r"^#{2,3}\s+12\.2\s+Parity matrix", re.I | re.M)


def git_sha() -> str:
    try:
        out = subprocess.check_output(
            ["git", "rev-parse", "--short", "HEAD"],
            cwd=ROOT,
            stderr=subprocess.DEVNULL,
            text=True,
        )
        return out.strip()
    except (subprocess.CalledProcessError, FileNotFoundError):
        return "unknown"


def has_main_block(path: Path) -> bool:
    try:
        text = path.read_text(encoding="utf-8")
    except OSError:
        return False
    return bool(re.search(r'if\s+__name__\s*==\s*["\']__main__["\']', text))


def list_main_scripts() -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for path in sorted(SCRIPTS.glob("*.py")):
        if path.name.startswith("_"):
            continue
        rows.append(
            {
                "name": path.name,
                "path": str(path.relative_to(ROOT)).replace("\\", "/"),
                "has_main": has_main_block(path),
            }
        )
    return rows


def parse_matrix_scripts() -> tuple[list[str], str | None]:
    """Return script names from §12.2 table and optional parse note."""
    if not MATRIX_DOC.is_file():
        return [], "matrix doc missing"
    text = MATRIX_DOC.read_text(encoding="utf-8")
    start = MATRIX_SECTION.search(text)
    if not start:
        return [], "§12.2 section not found"
    chunk = text[start.end() :]
    next_h2 = re.search(r"\n#{2,3}\s+\d", chunk)
    if next_h2:
        chunk = chunk[: next_h2.start()]
    names: list[str] = []
    for line in chunk.splitlines():
        m = re.match(r"^\|\s*`([^`]+\.py)`\s*\|", line)
        if m:
            names.append(m.group(1))
    if not names:
        return [], "§12.2 table empty or unparsed"
    return sorted(set(names)), None


def build_inventory() -> dict[str, Any]:
    scripts = list_main_scripts()
    matrix, matrix_note = parse_matrix_scripts()
    main_names = sorted(s["name"] for s in scripts if s["has_main"])
    matrix_set = set(matrix)
    main_set = set(main_names)
    return {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "git_sha": git_sha(),
        "matrix_doc": str(MATRIX_DOC.relative_to(ROOT)).replace("\\", "/"),
        "matrix_note": matrix_note,
        "scripts": scripts,
        "main_scripts": main_names,
        "matrix_scripts": matrix,
        "coverage": {
            "main_not_in_matrix": sorted(main_set - matrix_set),
            "matrix_not_on_disk": sorted(matrix_set - {s["name"] for s in scripts}),
            "main_in_matrix": sorted(main_set & matrix_set),
        },
    }


def write_inventory(path: Path = INVENTORY_PATH) -> dict[str, Any]:
    inv = build_inventory()
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(inv, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return inv


def check_inventory(*, strict: bool = False) -> tuple[dict[str, Any], int]:
    inv = build_inventory()
    cov = inv["coverage"]
    issues: list[str] = []
    warnings: list[str] = []

    if inv.get("matrix_note"):
        warnings.append(str(inv["matrix_note"]))

    missing_matrix = cov.get("main_not_in_matrix") or []
    if missing_matrix:
        msg = f"{len(missing_matrix)} __main__ scripts not in §12.2 matrix"
        if strict and inv.get("matrix_scripts"):
            issues.append(msg + ": " + ", ".join(missing_matrix[:8]))
        else:
            warnings.append(msg + ": " + ", ".join(missing_matrix[:8]))

    ghost = cov.get("matrix_not_on_disk") or []
    if ghost:
        warnings.append(
            f"{len(ghost)} matrix rows missing on disk: " + ", ".join(ghost[:8])
        )

    if INVENTORY_PATH.is_file():
        try:
            saved = json.loads(INVENTORY_PATH.read_text(encoding="utf-8"))
            saved_main = set(saved.get("main_scripts") or [])
            live_main = set(inv.get("main_scripts") or [])
            added = sorted(live_main - saved_main)
            removed = sorted(saved_main - live_main)
            if added or removed:
                warnings.append(f"inventory drift: +{len(added)} -{len(removed)} main scripts")
        except (json.JSONDecodeError, OSError) as exc:
            warnings.append(f"could not read saved inventory: {exc}")
    else:
        warnings.append("SCRIPT_INVENTORY.json missing — run --write to seed")

    report = {
        "ok": not issues,
        "strict": strict,
        "main_count": len(inv.get("main_scripts") or []),
        "matrix_count": len(inv.get("matrix_scripts") or []),
        "issues": issues,
        "warnings": warnings,
        "coverage": cov,
    }
    exit_code = 1 if issues else 0
    return report, exit_code


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--write", action="store_true", help="write docs/parity/SCRIPT_INVENTORY.json")
    ap.add_argument("--check", action="store_true", help="compare live scripts to matrix / inventory")
    ap.add_argument(
        "--strict",
        action="store_true",
        help="fail --check when __main__ scripts are missing from a non-empty matrix",
    )
    args = ap.parse_args()

    if not args.write and not args.check:
        inv = build_inventory()
        print(json.dumps(inv, ensure_ascii=False, indent=2))
        return 0

    if args.write:
        inv = write_inventory()
        print(
            f"wrote {INVENTORY_PATH} "
            f"main={len(inv['main_scripts'])} matrix={len(inv['matrix_scripts'])}"
        )

    if args.check:
        report, code = check_inventory(strict=args.strict)
        print(json.dumps(report, ensure_ascii=False, indent=2))
        if args.write and code == 0:
            write_inventory()
        return code

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
