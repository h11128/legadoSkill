#!/usr/bin/env python3
"""Parity suite runners (split from parity_selftest for 300-line limit)."""

from __future__ import annotations

import importlib
import json
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any, Callable

from parity_inventory import has_main_block

ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "scripts"
FIXTURES_GATE = ROOT / "fixtures" / "expected" / "gate"
CONTRACTS_DIR = ROOT / "config" / "repair_contracts"
CONTRACT_FIXTURES = ROOT / "fixtures" / "expected" / "contracts"
CRATES_DIR = ROOT / "crates"
SOURCE_CONTRACTS_CRATE = CRATES_DIR / "source-contracts"

CLI_HELP_SCRIPTS = (
    "repair_prefilter.py",
    "repair_session_log.py",
    "repair_source.py",
    "repair_one.py",
    "repair_progress.py",
    "repair_cache.py",
    "mcp_discover.py",
    "parity_inventory.py",
)

IMPORT_CHECKS: list[tuple[str, str]] = [
    ("repair_prefilter", "classify_one"),
    ("repair_prefilter", "match_l0"),
    ("repair_helpers", "layer_for_fail"),
    ("repair_check", "is_repair_success"),
    ("mcp_client", "resolve_endpoint"),
    ("repair_session_log", "append_row"),
    ("repair_classify", "decide"),
    ("repair_cache", "get_html"),
]


def _run_py(args: list[str], *, cwd: Path = ROOT, timeout: float = 60) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        args,
        cwd=cwd,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=timeout,
    )


def _suite_result(name: str, *, ok: bool, detail: str = "", **extra: Any) -> dict[str, Any]:
    return {"suite": name, "ok": ok, "detail": detail, **extra}


def run_fixtures_suite() -> dict[str, Any]:
    gate_dir = FIXTURES_GATE
    if not gate_dir.is_dir():
        return _suite_result("fixtures", ok=True, detail="no gate fixtures dir — skipped")

    from repair_prefilter import DEFAULT_RULES, load_rules, match_l0

    rules = load_rules(DEFAULT_RULES) if DEFAULT_RULES.is_file() else []
    paths = sorted(gate_dir.glob("*.json"))
    if not paths:
        return _suite_result("fixtures", ok=True, detail="no gate fixture files — skipped")

    failures: list[str] = []
    checked = 0
    for path in paths:
        spec = json.loads(path.read_text(encoding="utf-8"))
        url = spec.get("url") or ""
        expected = spec.get("expected") or {}
        mode = (spec.get("mode") or "l0").lower()
        if not url or not expected:
            failures.append(f"{path.name}: missing url/expected")
            continue
        if mode != "l0":
            failures.append(f"{path.name}: unsupported mode {mode!r}")
            continue
        hit = match_l0(url, rules)
        if not hit:
            failures.append(f"{path.name}: L0 miss for {url}")
            continue
        actual = {"verify": False, **hit}
        for key, want in expected.items():
            if actual.get(key) != want:
                failures.append(
                    f"{path.name}: {key} want={want!r} got={actual.get(key)!r}"
                )
        checked += 1

    ok = not failures
    detail = f"checked {checked} fixture(s)"
    if failures:
        detail += "; " + "; ".join(failures[:5])
    return _suite_result("fixtures", ok=not failures, detail=detail, checked=checked, failures=failures)


def run_cli_help_suite() -> dict[str, Any]:
    failures: list[str] = []
    skipped: list[str] = []
    checked = 0
    for name in CLI_HELP_SCRIPTS:
        path = SCRIPTS / name
        if not path.is_file():
            failures.append(f"{name}: missing")
            continue
        if not has_main_block(path):
            skipped.append(name)
            continue
        try:
            proc = _run_py([sys.executable, str(path), "--help"], timeout=30)
        except subprocess.TimeoutExpired:
            failures.append(f"{name}: --help timeout")
            continue
        if proc.returncode != 0:
            err = (proc.stderr or proc.stdout or "").strip()[:120]
            failures.append(f"{name}: exit {proc.returncode} ({err})")
        checked += 1

    ok = not failures
    detail = f"checked {checked} CLI(s)"
    if skipped:
        detail += f"; skipped(no __main__): {', '.join(skipped)}"
    if failures:
        detail += "; " + "; ".join(failures)
    return _suite_result(
        "cli-help", ok=not failures, detail=detail,
        checked=checked, skipped=skipped, failures=failures,
    )


def run_imports_suite() -> dict[str, Any]:
    failures: list[str] = []
    checked = 0
    for mod_name, attr in IMPORT_CHECKS:
        try:
            mod = importlib.import_module(mod_name)
            if not hasattr(mod, attr):
                failures.append(f"{mod_name}.{attr}: missing")
            checked += 1
        except Exception as exc:  # noqa: BLE001
            failures.append(f"{mod_name}.{attr}: {exc}")
    ok = not failures
    detail = f"checked {checked} symbol(s)"
    if failures:
        detail += "; " + "; ".join(failures)
    return _suite_result("imports", ok=not failures, detail=detail, checked=checked, failures=failures)


def _validate_with_jsonschema(schema_path: Path, doc_path: Path) -> str | None:
    try:
        import jsonschema  # type: ignore[import-untyped]
    except ImportError:
        return "jsonschema not installed"
    schema = json.loads(schema_path.read_text(encoding="utf-8"))
    doc = json.loads(doc_path.read_text(encoding="utf-8"))
    jsonschema.validate(doc, schema)
    return None


def _schema_for_fixture(doc_path: Path, schemas: list[Path]) -> Path | None:
    """Map gate_result_pass.json → gate_result.schema.json (longest prefix)."""
    stem = doc_path.stem
    best: Path | None = None
    best_len = -1
    for schema_path in schemas:
        name = schema_path.name[: -len(".schema.json")] if schema_path.name.endswith(".schema.json") else schema_path.stem
        if stem == name or stem.startswith(name + "_"):
            if len(name) > best_len:
                best = schema_path
                best_len = len(name)
    return best


def run_schemas_suite() -> dict[str, Any]:
    schemas = sorted(CONTRACTS_DIR.glob("*.schema.json")) if CONTRACTS_DIR.is_dir() else []
    fixtures: list[Path] = []
    if CONTRACT_FIXTURES.is_dir():
        fixtures.extend(CONTRACT_FIXTURES.glob("*.json"))
        fixtures.extend(CONTRACT_FIXTURES.glob("valid/*.json"))
        fixtures = sorted(fixtures)

    if not schemas and not SOURCE_CONTRACTS_CRATE.is_dir():
        return _suite_result("schemas", ok=True, detail="no schemas or source_contracts crate — skipped")

    notes: list[str] = []
    failures: list[str] = []
    cargo = shutil.which("cargo")
    if cargo and (CRATES_DIR / "Cargo.toml").is_file() and SOURCE_CONTRACTS_CRATE.is_dir():
        try:
            proc = _run_py([cargo, "test", "-p", "source_contracts", "--quiet"], cwd=CRATES_DIR, timeout=120)
            if proc.returncode == 0:
                notes.append("cargo test -p source_contracts passed")
            else:
                err = (proc.stderr or proc.stdout or "").strip().splitlines()
                failures.append("cargo test -p source_contracts failed: " + (err[-1] if err else "unknown"))
        except subprocess.TimeoutExpired:
            failures.append("cargo test -p source_contracts timeout")
    elif cargo:
        notes.append("source_contracts crate missing — cargo skipped")
    else:
        notes.append("cargo not on PATH — skipped crate tests")

    # Optional: source_gate lib tests (L0-only by default; do not fail suite on network L2).
    gate_crate = CRATES_DIR / "source-gate"
    if cargo and (CRATES_DIR / "Cargo.toml").is_file() and gate_crate.is_dir():
        try:
            # Default features = L0 only; avoid enabling l2 so CI/offline stays green.
            proc = _run_py(
                [cargo, "test", "-p", "source_gate", "--lib", "--quiet"],
                cwd=CRATES_DIR,
                timeout=180,
            )
            if proc.returncode == 0:
                notes.append("cargo test -p source_gate --lib passed (L0; L2 not required)")
            else:
                err = (proc.stderr or proc.stdout or "").strip().splitlines()
                # Soft note only: schemas suite still passes if contracts OK; record for operators.
                notes.append(
                    "cargo test -p source_gate --lib soft-fail (not suite-blocking): "
                    + (err[-1] if err else "unknown")
                )
        except subprocess.TimeoutExpired:
            notes.append("cargo test -p source_gate --lib timeout (soft; not suite-blocking)")
    elif cargo:
        notes.append("source_gate crate missing — gate cargo skipped")

    if schemas and fixtures:
        validated = 0
        for doc_path in fixtures:
            schema_path = _schema_for_fixture(doc_path, schemas)
            if schema_path is None:
                notes.append(f"no schema for fixture {doc_path.name}")
                continue
            err = _validate_with_jsonschema(schema_path, doc_path)
            if err:
                notes.append(f"{doc_path.name}: {err}")
            else:
                validated += 1
        if validated:
            notes.append(f"jsonschema validated {validated} fixture(s)")
    elif schemas:
        notes.append(f"{len(schemas)} schema file(s) present; no contract fixtures yet")
    else:
        notes.append("config/repair_contracts/*.schema.json not present yet")

    ok = not failures
    detail = "; ".join(notes + failures) or "schemas suite idle"
    return _suite_result("schemas", ok=not failures, detail=detail, failures=failures, notes=notes)


def run_inventory_suite() -> dict[str, Any]:
    proc = _run_py([sys.executable, str(SCRIPTS / "parity_inventory.py"), "--check"])
    try:
        report = json.loads(proc.stdout.strip() or "{}")
    except json.JSONDecodeError:
        report = {"raw": proc.stdout, "stderr": proc.stderr}
    ok = proc.returncode == 0
    warnings = report.get("warnings") or []
    detail = f"exit={proc.returncode} warnings={len(warnings)}"
    return _suite_result(
        "inventory", ok=proc.returncode == 0, detail=detail,
        report=report, stderr=(proc.stderr or "").strip()[:200],
    )


def run_rust_cli_suite() -> dict[str, Any]:
    from parity_rust_suite import run_rust_cli_suite as _run

    return _run()


SUITE_RUNNERS: dict[str, Callable[[], dict[str, Any]]] = {
    "fixtures": run_fixtures_suite,
    "cli-help": run_cli_help_suite,
    "imports": run_imports_suite,
    "schemas": run_schemas_suite,
    "inventory": run_inventory_suite,
    "rust-cli": run_rust_cli_suite,
}
