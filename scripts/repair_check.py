#!/usr/bin/env python3
"""Shared check-result helpers for repair scripts.

Default repair policy: ignore 发现 (explore) failures unless user asks.
"""

from __future__ import annotations

import re
from typing import Any

# Strip these when deciding if a source still needs repair.
_DISCOVERY_TOKENS = (
    "发现正文失效",
    "发现目录失效",
    "发现规则为空",
    "发现失效",
)

# Repair-wave defaults for start_check_sources (must match App MCP overrides).
REPAIR_CHECK_DEFAULTS: dict[str, Any] = {
    "checkDomain": False,
    "checkSearch": True,
    "checkDiscovery": False,
    "checkInfo": True,
    "checkCategory": True,
    "checkContent": True,
}


def strip_discovery_failures(message: str) -> str:
    msg = message or ""
    for tok in _DISCOVERY_TOKENS:
        msg = msg.replace(tok, "")
    msg = re.sub(r"校验失败\s*:?", "", msg)
    msg = re.sub(r"[，,\s]+", " ", msg).strip(" ,，")
    return msg.strip()


def is_repair_success(check: dict[str, Any] | None, *, ignore_discovery: bool = True) -> bool:
    """True if device check is good enough for default repair waves."""
    if not isinstance(check, dict) or not check:
        return False
    if check.get("success") is True:
        return True
    msg = str(check.get("message") or "")
    if not msg:
        return False
    if not ignore_discovery:
        return False
    # Only 发现* left after strip → treat as OK for default repair
    return strip_discovery_failures(msg) == ""


def check_args(
    urls: list[str],
    keyword: str,
    *,
    thread_count: int = 8,
    timeout_ms: int = 45_000,
    enabled_only: bool = False,
    check_domain: bool = False,
    check_search: bool = True,
    check_discovery: bool = False,
    check_info: bool = True,
    check_category: bool = True,
    check_content: bool = True,
    w_source_comment: bool | None = None,
) -> dict[str, Any]:
    """Arguments for start_check_sources (repair default: no discovery/domain)."""
    args: dict[str, Any] = {
        "urls": urls,
        "enabledOnly": enabled_only,
        "keyword": keyword,
        "threadCount": thread_count,
        "timeoutMs": timeout_ms,
        "checkDomain": check_domain,
        "checkSearch": check_search,
        "checkDiscovery": check_discovery,
        "checkInfo": check_info,
        "checkCategory": check_category,
        "checkContent": check_content,
    }
    if w_source_comment is not None:
        args["wSourceComment"] = w_source_comment
    return args
