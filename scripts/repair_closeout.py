#!/usr/bin/env python3
"""Structural close-out: gate traps, block progress next, sync SKILL SOT → Cursor copy."""

from __future__ import annotations

import hashlib
import json
import shutil
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
SKILL = ROOT / "skills" / "legado-book-source-repair" / "SKILL.md"
CURSOR_SKILL = Path.home() / ".cursor" / "skills" / "legado-book-source-repair" / "SKILL.md"
LEDGER = ROOT / "temp" / "full_fix" / "repair_session_ledger.jsonl"
RETRO = ROOT / "temp" / "full_fix" / "repair_serial_retro.jsonl"
TERMINAL_STEPS = frozenset({"check", "skip"})


def norm_url(url: str) -> str:
    return (url or "").strip()


def load_skill_text() -> str:
    if SKILL.is_file():
        return SKILL.read_text(encoding="utf-8")
    if CURSOR_SKILL.is_file():
        return CURSOR_SKILL.read_text(encoding="utf-8")
    return ""


def traps_section(skill_text: str) -> str:
    start = skill_text.find("## Traps")
    if start < 0:
        return skill_text
    end = skill_text.find("\n## ", start + 8)
    if end < 0:
        return skill_text[start:]
    return skill_text[start:end]


def trap_in_skill(trap: str, skill_text: str | None = None) -> bool:
    if not trap or trap.startswith("known:"):
        return True
    text = skill_text if skill_text is not None else load_skill_text()
    section = traps_section(text).lower()
    slug = trap.strip().lower().replace("_", " ").replace("-", " ")
    if slug in section:
        return True
    first = slug.split()[0] if slug.split() else ""
    if first and (f"({first}" in section or f"({slug}" in section):
        return True
    tokens = [t for t in slug.split() if len(t) >= 4]
    if len(tokens) >= 2:
        for line in section.splitlines():
            if line.startswith("|") and all(t in line for t in tokens):
                return True
    return False


def gate_trap(
    trap: str,
    *,
    skill_fix: bool,
    harness_files: list[Path] | None = None,
    require_harness: bool = False,
) -> tuple[bool, list[str]]:
    errors: list[str] = []
    skill_text = load_skill_text()
    if not skill_text:
        errors.append(f"SKILL not found: {SKILL}")
        return False, errors
    novel = not trap_in_skill(trap, skill_text)
    if novel and not skill_fix:
        errors.append(
            f"novel trap {trap!r} not in SKILL Traps — add row to {SKILL.name} "
            f"and retro --skill-fix 1 (or use known:… if repeating a playbook)"
        )
    if novel and skill_fix and not trap_in_skill(trap, skill_text):
        errors.append(f"--skill-fix 1 but trap {trap!r} still missing from SKILL Traps")
    if require_harness and novel:
        paths = harness_files or []
        missing = [p for p in paths if not p.is_file()]
        if missing:
            errors.append(f"novel trap requires harness file(s): {missing}")
    return len(errors) == 0, errors


def skill_fingerprint(path: Path) -> str:
    if not path.is_file():
        return ""
    return hashlib.sha256(path.read_bytes()).hexdigest()[:16]


def skill_in_sync() -> bool:
    if not SKILL.is_file() or not CURSOR_SKILL.is_file():
        return not SKILL.is_file() and not CURSOR_SKILL.is_file()
    return skill_fingerprint(SKILL) == skill_fingerprint(CURSOR_SKILL)


def sync_skill_to_cursor() -> tuple[bool, str]:
    if not SKILL.is_file():
        return False, f"missing SOT: {SKILL}"
    CURSOR_SKILL.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(SKILL, CURSOR_SKILL)
    return True, str(CURSOR_SKILL)


def _read_jsonl(path: Path) -> list[dict[str, Any]]:
    if not path.is_file():
        return []
    rows: list[dict[str, Any]] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        try:
            row = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(row, dict):
            rows.append(row)
    return rows


def last_terminal_ledger(path: Path = LEDGER) -> dict[str, Any] | None:
    for row in reversed(_read_jsonl(path)):
        url = norm_url(str(row.get("url") or ""))
        step = str(row.get("step") or "")
        if url and step in TERMINAL_STEPS:
            return row
    return None


def latest_retro_for_url(url: str, path: Path = RETRO) -> dict[str, Any] | None:
    target = norm_url(url)
    for row in reversed(_read_jsonl(path)):
        if norm_url(str(row.get("url") or "")) == target:
            return row
    return None


def pending_closeout(
    *,
    ledger_path: Path = LEDGER,
    retro_path: Path = RETRO,
) -> tuple[bool, list[str], dict[str, Any] | None]:
    """Return (ok, errors, detail). Blocks progress next when last URL lacks valid retro."""
    errors: list[str] = []
    detail: dict[str, Any] = {"ok": True}
    last = last_terminal_ledger(ledger_path)
    if not last:
        detail["reason"] = "no_terminal_ledger"
        return True, errors, detail

    url = norm_url(str(last.get("url") or ""))
    detail["url"] = url
    detail["ledger_step"] = last.get("step")
    detail["ledger_result"] = last.get("result")

    retro = latest_retro_for_url(url, retro_path)
    if not retro:
        errors.append(
            f"close-out incomplete for {url!r}: ledger {last.get('step')!r} but no "
            f"repair_retro row — run repair_retro.py append --url … --status …"
        )
        detail["ok"] = False
        detail["missing"] = "retro"
        return False, errors, detail

    detail["retro_status"] = retro.get("status")
    trap = str(retro.get("trap") or "").strip()
    detail["trap"] = trap
    skill_fix = bool(retro.get("skill_fix"))
    detail["skill_fix"] = skill_fix

    if trap:
        ok, gate_errs = gate_trap(trap, skill_fix=skill_fix)
        if not ok:
            errors.extend(gate_errs)
            detail["ok"] = False
            detail["gate"] = "fail"
            return False, errors, detail
        detail["gate"] = "pass"

    if skill_fix and not skill_in_sync():
        ok, msg = sync_skill_to_cursor()
        detail["skill_sync"] = msg if ok else f"fail: {msg}"
        if not ok:
            errors.append(f"skill_fix=1 but sync failed: {msg}")
            detail["ok"] = False
            return False, errors, detail

    detail["ok"] = True
    return True, errors, detail


def ensure_ready_for_next() -> int:
    ok, errors, detail = pending_closeout()
    if ok:
        print(json.dumps({"closeout": "ready", **detail}, ensure_ascii=False))
        return 0
    for e in errors:
        print(f"close-out BLOCK: {e}", flush=True)
    print(json.dumps({"closeout": "blocked", **detail}, ensure_ascii=False))
    return 1
