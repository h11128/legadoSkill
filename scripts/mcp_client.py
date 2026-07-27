#!/usr/bin/env python3
"""Minimal streamable-HTTP MCP client for Legado phone tools."""

from __future__ import annotations

import json
import time
import urllib.error
import urllib.request
from typing import Any

_SESSION: str | None = None


def reset_session() -> None:
    global _SESSION
    _SESSION = None


def mcp_call(
    mcp_url: str,
    token: str,
    method: str,
    params: dict[str, Any] | None = None,
    timeout: float = 120.0,
) -> dict[str, Any]:
    global _SESSION
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


def ensure_session(mcp_url: str, token: str, client_name: str = "repair_source") -> None:
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
    try:
        mcp_call(mcp_url, token, "notifications/initialized", {})
    except Exception:
        pass


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
    raw = extract_text(
        tools_call(mcp_url, token, "get_source", {"bookSourceUrl": book_source_url})
    )
    data = parse_json_text(raw)
    if isinstance(data, dict) and "bookSourceUrl" in data:
        return data
    if isinstance(data, dict) and isinstance(data.get("data"), dict):
        return data["data"]
    raise RuntimeError(f"unexpected get_source payload: {raw[:300]}")
