#!/usr/bin/env python3
"""Minimal streamable-HTTP MCP client for Legado phone tools."""

from __future__ import annotations

import json
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any

_SESSION: str | None = None
_ACTIVE_MCP: str | None = None
_ACTIVE_TOKEN: str | None = None
_SCRIPTS = Path(__file__).resolve().parent


def reset_session() -> None:
    global _SESSION, _ACTIVE_MCP, _ACTIVE_TOKEN
    _SESSION = None
    _ACTIVE_MCP = None
    _ACTIVE_TOKEN = None


def _set_active(mcp_url: str, token: str) -> None:
    global _ACTIVE_MCP, _ACTIVE_TOKEN
    _ACTIVE_MCP = mcp_url
    _ACTIVE_TOKEN = token


def resolve_endpoint(mcp_url: str | None = None, token: str | None = None) -> tuple[str, str]:
    """Prefer in-process active endpoint (after rediscover), else args."""
    if _ACTIVE_MCP:
        return _ACTIVE_MCP, _ACTIVE_TOKEN or (token or "")
    return mcp_url or "", token or ""


def load_endpoint() -> tuple[str, str]:
    """Read mcp_url/token from config/mcp_defaults.json (shared SOT)."""
    from mcp_discover import load_defaults

    data = load_defaults()
    return str(data.get("mcp_url") or ""), str(data.get("token") or "1234")


def ensure_endpoint(*, rediscover: bool = True) -> tuple[str, str]:
    """Return reachable (mcp_url, token); optionally rediscover on failure."""
    from mcp_discover import ensure_reachable, load_defaults

    data = load_defaults()
    url = str(data.get("mcp_url") or "")
    token = str(data.get("token") or "1234")
    if not rediscover:
        return url, token
    try:
        return ensure_reachable(url, token)
    except Exception:
        return url, token


def mcp_call(
    mcp_url: str,
    token: str,
    method: str,
    params: dict[str, Any] | None = None,
    timeout: float = 120.0,
) -> dict[str, Any]:
    global _SESSION
    mcp_url, token = resolve_endpoint(mcp_url, token)
    payload = {
        "jsonrpc": "2.0",
        "id": int(time.time() * 1000) % 1_000_000_000,
        "method": method,
        "params": params or {},
    }
    headers = {
        "Content-Type": "application/json",
        "Accept": "application/json, text/event-stream",
        "X-Legado-Token": token,
    }
    if _SESSION:
        headers["Mcp-Session-Id"] = _SESSION
    req = urllib.request.Request(
        mcp_url,
        data=json.dumps(payload).encode("utf-8"),
        method="POST",
        headers=headers,
    )
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        sid = resp.headers.get("Mcp-Session-Id")
        if sid:
            _SESSION = sid
        body = resp.read().decode("utf-8", errors="replace")
    if body.lstrip().startswith("event:") or "data:" in body:
        chunks = [
            line[5:].strip()
            for line in body.splitlines()
            if line.startswith("data:")
        ]
        if chunks:
            body = chunks[-1]
    return json.loads(body)


def ensure_session(mcp_url: str, token: str, client_name: str = "repair_source") -> str:
    """Initialize MCP session; on connection failure rediscover once and retry.

    Returns the mcp_url actually used. Subsequent mcp_call/tools_call in this
    process prefer that active endpoint even if callers keep passing the old URL.
    """
    reset_session()
    try:
        mcp_call(
            mcp_url,
            token,
            "initialize",
            {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": client_name, "version": "1.0"},
            },
        )
        _set_active(mcp_url, token)
    except (urllib.error.URLError, TimeoutError, OSError):
        from mcp_discover import ensure_reachable

        mcp_url, token = ensure_reachable(mcp_url, token)
        reset_session()
        mcp_call(
            mcp_url,
            token,
            "initialize",
            {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": client_name, "version": "1.0"},
            },
        )
        _set_active(mcp_url, token)
    try:
        mcp_call(mcp_url, token, "notifications/initialized", {})
    except Exception:
        pass
    return mcp_url


def tools_call(
    mcp_url: str,
    token: str,
    name: str,
    arguments: dict[str, Any],
    timeout: float = 120.0,
) -> Any:
    result = mcp_call(
        mcp_url,
        token,
        "tools/call",
        {"name": name, "arguments": arguments},
        timeout=timeout,
    )
    if "error" in result:
        raise RuntimeError(result["error"])
    return result.get("result", result)


def extract_text(result: Any) -> str:
    if isinstance(result, dict):
        content = result.get("content")
        if isinstance(content, list) and content:
            first = content[0]
            if isinstance(first, dict) and "text" in first:
                return str(first["text"])
        if "message" in result:
            return str(result["message"])
    return json.dumps(result, ensure_ascii=False)


def parse_json_text(text: str) -> Any:
    text = text.strip()
    if text.startswith("{") or text.startswith("["):
        return json.loads(text)
    return {"raw": text}


def get_source(mcp_url: str, token: str, book_source_url: str) -> dict[str, Any]:
    """Fetch source; retry trimmed / spaced / hash-stripped variants."""
    candidates = [book_source_url]
    trimmed = book_source_url.strip()
    if trimmed and trimmed not in candidates:
        candidates.append(trimmed)
    if trimmed and f" {trimmed}" not in candidates:
        candidates.append(f" {trimmed}")
    # fragment variants: https://host/#tag → https://host/ and https://host
    base = trimmed.split("#", 1)[0].strip()
    for v in (base, base.rstrip("/"), base.rstrip("/") + "/"):
        if v and v not in candidates:
            candidates.append(v)
    last_raw = ""
    for cand in candidates:
        raw = extract_text(tools_call(mcp_url, token, "get_source", {"url": cand}))
        last_raw = raw
        data = parse_json_text(raw)
        if isinstance(data, dict) and "bookSourceUrl" in data:
            return data
        if isinstance(data, dict) and isinstance(data.get("data"), dict):
            return data["data"]
    raise RuntimeError(f"unexpected get_source payload: {last_raw[:300]}")


def save_source(
    mcp_url: str,
    token: str,
    source: dict[str, Any],
    *,
    preserve_enabled: bool = True,
    preserve_group: bool = True,
) -> str:
    payload = json.dumps(source, ensure_ascii=False, separators=(",", ":"))
    return extract_text(
        tools_call(
            mcp_url,
            token,
            "save_source",
            {
                "source": payload,
                "preserveEnabled": preserve_enabled,
                "preserveGroup": preserve_group,
            },
            timeout=180.0,
        )
    )


def disable_source(mcp_url: str, token: str, source: dict[str, Any], tag: str = "网站失效") -> str:
    src = dict(source)
    src["enabled"] = False
    group = str(src.get("bookSourceGroup") or "")
    parts = [p.strip() for p in group.replace("，", ",").split(",") if p.strip()]
    if tag not in parts:
        parts.append(tag)
    src["bookSourceGroup"] = ",".join(parts)
    return save_source(
        mcp_url, token, src, preserve_enabled=False, preserve_group=False
    )
