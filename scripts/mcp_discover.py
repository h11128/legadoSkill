#!/usr/bin/env python3
"""Discover Legado phone MCP on LAN (_legado-mcp._tcp) and update mcp_defaults.json.

Browsers (in order):
  1. zeroconf (pip install zeroconf) — preferred mDNS
  2. dns-sd -B (Bonjour / Apple tools) if present
  3. adb: read phone wlan0 IPv4 and probe :1236/mcp

Example:
  python scripts/mcp_discover.py
  python scripts/mcp_discover.py --write
  python scripts/mcp_discover.py --timeout 5
"""

from __future__ import annotations

import argparse
import json
import re
import socket
import subprocess
import sys
import time
import urllib.error
import urllib.request
from datetime import date
from pathlib import Path
from typing import Any
from urllib.parse import urlparse

_ROOT = Path(__file__).resolve().parents[1]
_SCRIPTS = Path(__file__).resolve().parent
DEFAULTS_PATH = _ROOT / "config" / "mcp_defaults.json"
SERVICE_TYPE = "_legado-mcp._tcp.local."
DEFAULT_PORT = 1236
MCP_PATH = "/mcp"


def load_defaults(path: Path = DEFAULTS_PATH) -> dict[str, Any]:
    if not path.is_file():
        return {
            "mcp_url": f"http://127.0.0.1:{DEFAULT_PORT}{MCP_PATH}",
            "token": "1234",
            "web_api": f"http://127.0.0.1:1122",
        }
    return json.loads(path.read_text(encoding="utf-8"))


def save_defaults(data: dict[str, Any], path: Path = DEFAULTS_PATH) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def sync_cursor_mcp_json(mcp_url: str, token: str) -> dict[str, Any]:
    """Update ~/.cursor/mcp.json mcpServers.legado.url so Cursor IDE stops needing a hand edit."""
    mcp_json = Path.home() / ".cursor" / "mcp.json"
    out: dict[str, Any] = {"path": str(mcp_json), "updated": False}
    if not mcp_json.is_file():
        out["error"] = "missing"
        return out
    try:
        cfg = json.loads(mcp_json.read_text(encoding="utf-8"))
    except Exception as exc:  # noqa: BLE001
        out["error"] = str(exc)
        return out
    servers = cfg.get("mcpServers")
    if not isinstance(servers, dict):
        out["error"] = "no mcpServers"
        return out
    # Cursor may show the server as "legado" or historically "user-legado"
    key = "legado" if "legado" in servers else ("user-legado" if "user-legado" in servers else None)
    if key is None:
        out["error"] = "no legado server entry"
        return out
    entry = servers.get(key)
    if not isinstance(entry, dict):
        out["error"] = "legado entry not an object"
        return out
    old = str(entry.get("url") or "")
    if old == mcp_url and str((entry.get("headers") or {}).get("X-Legado-Token") or "") == token:
        out["unchanged"] = True
        return out
    entry["url"] = mcp_url
    headers = entry.get("headers")
    if not isinstance(headers, dict):
        headers = {}
        entry["headers"] = headers
    headers["X-Legado-Token"] = token
    # Bump client header so Cursor tends to refresh the HTTP MCP session.
    headers["X-Legado-Client"] = f"discover-{date.today().isoformat()}"
    servers[key] = entry
    cfg["mcpServers"] = servers
    mcp_json.write_text(json.dumps(cfg, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    out["updated"] = True
    out["old_url"] = old
    out["new_url"] = mcp_url
    return out


def sync_agent_endpoints(mcp_url: str, token: str) -> dict[str, Any]:
    """Best-effort sync of IDE MCP configs after phone discovery."""
    return {"cursor_mcp_json": sync_cursor_mcp_json(mcp_url, token)}


def mcp_url_for(host: str, port: int = DEFAULT_PORT) -> str:
    host = host.strip().strip("[]")
    return f"http://{host}:{port}{MCP_PATH}"


def probe_mcp(url: str, token: str, timeout: float = 3.0) -> bool:
    headers = {
        "Content-Type": "application/json",
        "Accept": "application/json, text/event-stream",
        "X-Legado-Token": token,
    }
    body = json.dumps(
        {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "mcp_discover", "version": "1"},
            },
        }
    ).encode()
    req = urllib.request.Request(url, data=body, headers=headers, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return 200 <= resp.status < 500
    except urllib.error.HTTPError as exc:
        # 406/401 still means something is listening on the MCP port.
        return exc.code in (401, 403, 406, 415)
    except Exception:
        return False


def discover_zeroconf(timeout: float = 4.0) -> list[dict[str, Any]]:
    try:
        from zeroconf import ServiceBrowser, ServiceStateChange, Zeroconf
    except ImportError:
        return []

    found: list[dict[str, Any]] = []

    def on_change(
        zeroconf: Any,
        service_type: str,
        name: str,
        state_change: Any,
    ) -> None:
        if state_change.name != "Added":
            return
        info = zeroconf.get_service_info(service_type, name, timeout=timeout * 1000)
        if not info or not info.addresses:
            return
        host = socket.inet_ntoa(info.addresses[0])
        port = int(info.port or DEFAULT_PORT)
        found.append(
            {
                "host": host,
                "port": port,
                "mcp_url": mcp_url_for(host, port),
                "via": "zeroconf",
                "name": name,
            }
        )

    zc = Zeroconf()
    try:
        ServiceBrowser(zc, SERVICE_TYPE, handlers=[on_change])
        deadline = time.time() + timeout
        while time.time() < deadline and not found:
            time.sleep(0.2)
    finally:
        zc.close()
    return found


def discover_dns_sd(timeout: float = 4.0) -> list[dict[str, Any]]:
    """Best-effort Bonjour browse (macOS / Windows Bonjour SDK)."""
    try:
        proc = subprocess.run(
            ["dns-sd", "-B", "_legado-mcp._tcp", "local."],
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return []
    # Resolve first instance name if browse printed one.
    names = re.findall(r"Add\s+\S+\s+\S+\s+(\S+_legado-mcp\._tcp\.)", proc.stdout or "")
    if not names:
        names = re.findall(r"(_legado-mcp\._tcp\.)", proc.stdout or "")
    out: list[dict[str, Any]] = []
    for name in names[:3]:
        try:
            r = subprocess.run(
                ["dns-sd", "-L", name.strip(), "_legado-mcp._tcp", "local."],
                capture_output=True,
                text=True,
                timeout=timeout,
                check=False,
            )
        except (FileNotFoundError, subprocess.TimeoutExpired):
            continue
        m = re.search(r"can be reached at\s+(\S+):(\d+)", r.stdout or "", re.I)
        if not m:
            continue
        host, port_s = m.group(1).rstrip("."), m.group(2)
        port = int(port_s)
        out.append(
            {
                "host": host,
                "port": port,
                "mcp_url": mcp_url_for(host, port),
                "via": "dns-sd",
                "name": name,
            }
        )
    return out


def discover_adb(token: str, port: int = DEFAULT_PORT) -> list[dict[str, Any]]:
    try:
        proc = subprocess.run(
            ["adb", "shell", "ip", "-f", "inet", "addr", "show", "wlan0"],
            capture_output=True,
            text=True,
            timeout=8,
            check=False,
        )
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return []
    text = proc.stdout or ""
    ips = re.findall(r"inet\s+(\d+\.\d+\.\d+\.\d+)/\d+", text)
    out: list[dict[str, Any]] = []
    for ip in ips:
        if ip.startswith("127."):
            continue
        url = mcp_url_for(ip, port)
        if probe_mcp(url, token):
            out.append({"host": ip, "port": port, "mcp_url": url, "via": "adb"})
    return out


def discover_all(token: str, timeout: float = 4.0) -> list[dict[str, Any]]:
    hits = discover_zeroconf(timeout=timeout)
    if not hits:
        hits = discover_dns_sd(timeout=timeout)
    if not hits:
        hits = discover_adb(token=token)
    # Dedup by mcp_url
    seen: set[str] = set()
    uniq: list[dict[str, Any]] = []
    for h in hits:
        u = h["mcp_url"]
        if u in seen:
            continue
        seen.add(u)
        uniq.append(h)
    return uniq


def pick_reachable(hits: list[dict[str, Any]], token: str) -> dict[str, Any] | None:
    for h in hits:
        if probe_mcp(h["mcp_url"], token):
            return h
    # Never write an unprobed/dead advertisement into SOT.
    return None


def apply_discovery(
    *,
    write: bool = True,
    timeout: float = 4.0,
    path: Path = DEFAULTS_PATH,
) -> dict[str, Any]:
    data = load_defaults(path)
    token = str(data.get("token") or "1234")
    hits = discover_all(token=token, timeout=timeout)
    chosen = pick_reachable(hits, token)
    result: dict[str, Any] = {
        "hits": hits,
        "chosen": chosen,
        "defaults_path": str(path),
        "wrote": False,
    }
    if not chosen:
        return result
    data["mcp_url"] = chosen["mcp_url"]
    host = chosen["host"]
    # Keep web_api on same host when it looks like the phone LAN API.
    web = str(data.get("web_api") or "")
    parsed = urlparse(web)
    if parsed.hostname and parsed.hostname.startswith(("10.", "192.168.", "172.")):
        port = parsed.port or 1122
        data["web_api"] = f"http://{host}:{port}"
    data["updated"] = date.today().isoformat()
    data["discovered_via"] = chosen.get("via")
    data["note"] = (
        "Single SOT for phone LAN endpoint. Prefer scripts/mcp_discover.py "
        "over hard-coding DHCP IPs; skills/scripts should read this file."
    )
    if write:
        save_defaults(data, path)
        result["wrote"] = True
        result["agent_sync"] = sync_agent_endpoints(
            str(data["mcp_url"]), str(data.get("token") or "1234")
        )
    result["defaults"] = data
    return result


def ensure_reachable(
    mcp_url: str | None = None,
    token: str | None = None,
    *,
    path: Path = DEFAULTS_PATH,
    timeout: float = 3.0,
) -> tuple[str, str]:
    """Return (mcp_url, token); rediscover+write if current URL is dead."""
    data = load_defaults(path)
    url = mcp_url or str(data.get("mcp_url") or "")
    tok = token or str(data.get("token") or "1234")
    if url and probe_mcp(url, tok, timeout=timeout):
        return url, tok
    discovered = apply_discovery(write=True, timeout=max(timeout, 4.0), path=path)
    chosen = discovered.get("chosen")
    if isinstance(chosen, dict) and chosen.get("mcp_url"):
        return str(chosen["mcp_url"]), tok
    if url:
        return url, tok
    raise RuntimeError("MCP unreachable and discovery found nothing")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--timeout", type=float, default=4.0)
    ap.add_argument("--write", action="store_true", help="write config/mcp_defaults.json")
    ap.add_argument("--no-write", action="store_true")
    ap.add_argument("--path", default=str(DEFAULTS_PATH))
    args = ap.parse_args()
    write = bool(args.write) or not args.no_write
    if args.no_write:
        write = False
    # Default CLI writes when hits found (discover is meant to refresh SOT).
    if not args.write and not args.no_write:
        write = True
    result = apply_discovery(write=write, timeout=args.timeout, path=Path(args.path))
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0 if result.get("chosen") else 1


if __name__ == "__main__":
    # Allow `python scripts/mcp_discover.py` without installing as package.
    if str(_SCRIPTS) not in sys.path:
        sys.path.insert(0, str(_SCRIPTS))
    raise SystemExit(main())
