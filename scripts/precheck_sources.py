#!/usr/bin/env python3
"""DNS + lightweight HTTP precheck for Legado book-source URLs.

Does not run App check logic. Filters hosts that fail DNS so MCP batches
do not waste phone heap on dead domains.

Examples:
  python scripts/precheck_sources.py --urls-file urls.txt --out temp/precheck.json
  python scripts/precheck_sources.py --url https://example.com --concurrency 200
"""

from __future__ import annotations

import argparse
import concurrent.futures
import json
import socket
import sys
import time
import urllib.error
import urllib.request
from dataclasses import asdict, dataclass
from typing import Iterable
from urllib.parse import urlparse


@dataclass
class ProbeResult:
    url: str
    host: str
    dns_ok: bool = False
    http_ok: bool = False
    error: str | None = None
    duration_ms: int = 0


def parse_host(url: str) -> str | None:
    raw = url.split("#", 1)[0].strip()
    if not raw:
        return None
    if "://" not in raw:
        raw = "http://" + raw
    return urlparse(raw).hostname


def probe_one(url: str, timeout: float) -> ProbeResult:
    start = time.perf_counter()
    host = parse_host(url) or ""
    result = ProbeResult(url=url, host=host)
    if not host:
        result.error = "invalid_url"
        result.duration_ms = int((time.perf_counter() - start) * 1000)
        return result
    # Skip pathological hosts (emoji / huge labels break idna).
    if len(host) > 253 or any(len(label) > 63 for label in host.split(".")):
        result.error = "dns:invalid_hostname"
        result.duration_ms = int((time.perf_counter() - start) * 1000)
        return result
    try:
        socket.getaddrinfo(host, None)
        result.dns_ok = True
    except (OSError, UnicodeError, ValueError) as exc:
        result.error = f"dns:{exc}"
        result.duration_ms = int((time.perf_counter() - start) * 1000)
        return result
    except Exception as exc:  # noqa: BLE001
        result.error = f"dns:{type(exc).__name__}:{exc}"
        result.duration_ms = int((time.perf_counter() - start) * 1000)
        return result

    probe_url = url if "://" in url else f"http://{url}"
    probe_url = probe_url.split("#", 1)[0]
    headers = {"User-Agent": "legadoSkill-precheck/1.0"}
    try:
        req = urllib.request.Request(probe_url, method="HEAD", headers=headers)
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            result.http_ok = 200 <= getattr(resp, "status", 200) < 500
    except urllib.error.HTTPError as exc:
        result.http_ok = exc.code < 500
        result.error = f"http:{exc.code}"
    except Exception:
        try:
            req = urllib.request.Request(probe_url, method="GET", headers=headers)
            with urllib.request.urlopen(req, timeout=timeout) as resp:
                result.http_ok = 200 <= getattr(resp, "status", 200) < 500
        except Exception as get_exc:  # noqa: BLE001
            result.error = f"http:{get_exc}"
    result.duration_ms = int((time.perf_counter() - start) * 1000)
    return result


def load_urls(args: argparse.Namespace) -> list[str]:
    urls: list[str] = []
    if args.url:
        urls.extend(args.url)
    if args.urls_file:
        with open(args.urls_file, encoding="utf-8") as fh:
            for line in fh:
                line = line.strip()
                if line and not line.startswith("#"):
                    urls.append(line)
    seen: set[str] = set()
    out: list[str] = []
    for item in urls:
        if item not in seen:
            seen.add(item)
            out.append(item)
    return out


def main(argv: Iterable[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--url", action="append", default=[], help="bookSourceUrl (repeatable)")
    parser.add_argument("--urls-file", help="one URL per line")
    parser.add_argument("--concurrency", type=int, default=200)
    parser.add_argument("--timeout", type=float, default=5.0)
    parser.add_argument("--out", help="write JSON report path")
    args = parser.parse_args(list(argv) if argv is not None else None)

    urls = load_urls(args)
    if not urls:
        print("no urls", file=sys.stderr)
        return 2

    results: list[ProbeResult] = []
    workers = max(1, args.concurrency)
    with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as pool:
        futs = [pool.submit(probe_one, u, args.timeout) for u in urls]
        for fut in concurrent.futures.as_completed(futs):
            results.append(fut.result())

    alive = [r.url for r in results if r.dns_ok]
    dead = [r.url for r in results if not r.dns_ok]
    report = {
        "total": len(results),
        "dns_ok": len(alive),
        "dns_fail": len(dead),
        "alive_urls": alive,
        "dead_urls": dead,
        "results": [asdict(r) for r in sorted(results, key=lambda x: x.url)],
    }
    text = json.dumps(report, ensure_ascii=False, indent=2)
    if args.out:
        with open(args.out, "w", encoding="utf-8") as fh:
            fh.write(text)
        print(f"wrote {args.out} dns_ok={len(alive)} dns_fail={len(dead)}")
    else:
        print(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
