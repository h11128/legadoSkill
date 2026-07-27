#!/usr/bin/env python3
"""Apply known auto-patches from rule smells (minimal, safe defaults)."""

from __future__ import annotations

import copy
import re
from typing import Any

from repair_helpers import smell_rules


def _ensure_info(source: dict[str, Any]) -> dict[str, Any]:
    info = source.get("ruleBookInfo")
    if not isinstance(info, dict):
        info = {}
        source["ruleBookInfo"] = info
    return info


def apply_auto_patches(source: dict[str, Any]) -> tuple[dict[str, Any], list[str]]:
    """Return (new_source, change_descriptions)."""
    out = copy.deepcopy(source)
    changes: list[str] = []
    smells = smell_rules(out)
    info = _ensure_info(out)
    toc = str(info.get("tocUrl") or "")
    name = str(info.get("name") or "")

    for smell in smells:
        issue = smell.get("issue")
        if issue in {"broad_a_href_regex", "maybe_content_not_catalog"} and toc:
            info["tocUrl"] = ""
            changes.append(f"clear ruleBookInfo.tocUrl ({issue})")
            toc = ""
        elif issue == "fallback_mixed_with_regex" and name and "||" in name:
            # keep selector before first || ; drop trailing ## replace on fallback side
            left = name.split("||", 1)[0].strip()
            # if left still has ## keep it (strip only)
            info["name"] = left
            changes.append("ruleBookInfo.name: drop || fallback mixed with ##")
            name = left

    # Normalize nonstandard author index often broken: .kv p.0@a → .kv a@text
    author = str(info.get("author") or "")
    if re.search(r"p\.\d+@", author):
        # .kv p.0@a → .kv a@text
        fixed = re.sub(r"\s*p\.\d+@", " ", author)
        fixed = fixed.replace("@a@text", "@text").replace("@a", "@text")
        fixed = re.sub(r"\s+", " ", fixed).strip()
        if "@" not in fixed:
            fixed = f"{fixed}@text"
        info["author"] = fixed
        changes.append(f"ruleBookInfo.author → {fixed}")

    if not str(out.get("concurrentRate") or "").strip():
        out["concurrentRate"] = "1000"
        changes.append("concurrentRate → 1000")

    # gongzicp: {'webView': true} is invalid JSON → SPA empty content
    try:
        from repair_rule_smells import apply_safe_rule_fixes

        for label in apply_safe_rule_fixes(out):
            changes.append(label)
    except ImportError:
        pass

    # Deduplicate change list while preserving order
    seen: set[str] = set()
    uniq = []
    for c in changes:
        if c not in seen:
            seen.add(c)
            uniq.append(c)
    return out, uniq


def patch_plan(source: dict[str, Any]) -> dict[str, Any]:
    smells = smell_rules(source)
    patched, changes = apply_auto_patches(source)
    return {
        "smells": smells,
        "changes": changes,
        "would_change": bool(changes),
        "patched_fields": {
            "tocUrl": (patched.get("ruleBookInfo") or {}).get("tocUrl")
            if isinstance(patched.get("ruleBookInfo"), dict)
            else None,
            "name": (patched.get("ruleBookInfo") or {}).get("name")
            if isinstance(patched.get("ruleBookInfo"), dict)
            else None,
            "concurrentRate": patched.get("concurrentRate"),
        },
    }
