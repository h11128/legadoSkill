#!/usr/bin/env python3
"""Batch-orchestrate device MCP start_check_sources / get_check_progress.

Uses JSON-RPC streamable HTTP against the phone MCP endpoint.
Requires: pip install httpx (or use requests). Prefers stdlib urllib only.

Examples:
  python scripts/batch_check_mcp.py \\
    --mcp http://10.0.0.43:1236/mcp --token 1234 \\
    --urls-file alive.txt --batch-size 80 --thread-count 64 --keyword 我的
"""

from __future__ import annotations

import argparse
import json
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any


_SESSION: str | None = None


def mcp_call(
    mcp_url: str,
    token: str,
    method: str,
    params: dict[str, Any] | None = None,
    timeout: float = 120.0,
) -> dict[str, Any]:
    """Minimal JSON-RPC POST for streamable HTTP MCP (with session header)."""
    global _SESSION
    payload = {
        "jsonrpc": "2.0",
        "id": int(time.time() * 1000) % 1_000_000_000,
        "method": method,
        "params": params or {},
    }
    data = json.dumps(payload).encode("utf-8")
    headers = {
        "Content-Type": "application/json",
        "Accept": "application/json, text/event-stream",
        "X-Legado-Token": token,
    }
    if _SESSION:
        headers["Mcp-Session-Id"] = _SESSION
    req = urllib.request.Request(
        mcp_url,
        data=data,
        method="POST",
        headers=headers,
    )
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        sid = resp.headers.get("Mcp-Session-Id")
        if sid:
            _SESSION = sid
        body = resp.read().decode("utf-8", errors="replace")
    # SSE: take last data: line if present
    if body.lstrip().startswith("event:") or "data:" in body:
        chunks = []
        for line in body.splitlines():
            if line.startswith("data:"):
                chunks.append(line[5:].strip())
        if chunks:
            body = chunks[-1]
    return json.loads(body)


def ensure_session(mcp_url: str, token: str) -> None:
    """Open a streamable-HTTP MCP session (required by Legado Kotlin MCP)."""
    global _SESSION
    _SESSION = None
    mcp_call(
        mcp_url,
        token,
        "initialize",
        {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "batch_check_mcp", "version": "1.0"},
        },
    )
    try:
        mcp_call(mcp_url, token, "notifications/initialized", {})
    except Exception:
        pass


def tools_call(mcp_url: str, token: str, name: str, arguments: dict[str, Any]) -> Any:
    result = mcp_call(
        mcp_url,
        token,
        "tools/call",
        {"name": name, "arguments": arguments},
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


def load_urls(args: argparse.Namespace) -> list[str]:
    urls: list[str] = []
    if args.precheck_json:
        data = json.loads(Path(args.precheck_json).read_text(encoding="utf-8"))
        urls.extend(data.get("alive_urls") or [])
    if args.urls_file:
        for line in Path(args.urls_file).read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if line and not line.startswith("#"):
                urls.append(line)
    if args.url:
        urls.extend(args.url)
    seen: set[str] = set()
    out: list[str] = []
    for item in urls:
        if item not in seen:
            seen.add(item)
            out.append(item)
    return out


def wait_batch(mcp_url: str, token: str, poll_s: float) -> dict[str, Any]:
    while True:
        raw = extract_text(tools_call(mcp_url, token, "get_check_progress", {
            "resultOffset": 0,
            "resultLimit": 1,
        }))
        snap = json.loads(raw) if raw.strip().startswith("{") else {"raw": raw}
        if not snap.get("running", False):
            return fetch_all_results(mcp_url, token, snap)
        time.sleep(poll_s)


def fetch_all_results(mcp_url: str, token: str, seed: dict[str, Any]) -> dict[str, Any]:
    """Page through get_check_progress until all stored results are collected."""
    all_results: list[Any] = []
    offset = 0
    total = int(seed.get("resultTotal") or 0)
    while True:
        raw = extract_text(tools_call(mcp_url, token, "get_check_progress", {
            "resultOffset": offset,
            "resultLimit": 500,
        }))
        page = json.loads(raw) if raw.strip().startswith("{") else {}
        chunk = page.get("results") or []
        all_results.extend(chunk)
        total = int(page.get("resultTotal") or total)
        offset += len(chunk)
        if not chunk or offset >= total:
            page["results"] = all_results
            return page


FAIL_TAGS = (
    "网站失效",
    "域名失效",
    "搜索失效",
    "发现失效",
    "校验超时",
    "js失效",
    "搜索目录失效",
    "发现目录失效",
    "搜索正文失效",
    "发现正文失效",
    "搜索链接规则为空",
    "发现规则为空",
)


def classify_results(results: list[Any]) -> dict[str, list[dict[str, Any]]]:
    buckets: dict[str, list[dict[str, Any]]] = {tag: [] for tag in FAIL_TAGS}
    buckets["other_fail"] = []
    buckets["success"] = []
    for item in results:
        if not isinstance(item, dict):
            continue
        if item.get("success"):
            buckets["success"].append(item)
            continue
        group = str(item.get("group") or "")
        message = str(item.get("message") or "")
        hit = None
        for tag in FAIL_TAGS:
            if tag in group or tag in message:
                hit = tag
                break
        buckets[hit or "other_fail"].append(item)
    return {k: v for k, v in buckets.items() if v}


def dump_fail_materials(classified: dict[str, list[dict[str, Any]]], out_dir: Path) -> None:
    out_dir.mkdir(parents=True, exist_ok=True)
    summary_lines: list[str] = []
    for tag, items in classified.items():
        if tag == "success":
            continue
        tag_dir = out_dir / tag.replace("/", "_")
        tag_dir.mkdir(parents=True, exist_ok=True)
        summary_lines.append(f"{tag}\t{len(items)}")
        for item in items:
            url = str(item.get("url") or "unknown")
            safe = "".join(ch if ch.isalnum() or ch in "-._" else "_" for ch in url)[:120]
            path = tag_dir / f"{safe}.json"
            path.write_text(json.dumps(item, ensure_ascii=False, indent=2), encoding="utf-8")
    (out_dir / "summary.tsv").write_text("\n".join(summary_lines) + "\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mcp", default="http://10.0.0.43:1236/mcp")
    parser.add_argument("--token", default="1234")
    parser.add_argument("--url", action="append", default=[])
    parser.add_argument("--urls-file")
    parser.add_argument("--precheck-json", help="precheck_sources.py output")
    parser.add_argument("--batch-size", type=int, default=80)
    parser.add_argument("--thread-count", type=int, default=64)
    parser.add_argument("--timeout-ms", type=int, default=60_000)
    parser.add_argument("--keyword", default="我的")
    parser.add_argument("--enabled-only", action="store_true", default=False)
    parser.add_argument("--poll-seconds", type=float, default=3.0)
    parser.add_argument("--out", default="temp/batch_check_report.json")
    parser.add_argument(
        "--materials-dir",
        default="temp/check_materials",
        help="dump failed result JSON by failure tag",
    )
    args = parser.parse_args()

    urls = load_urls(args)
    if not urls:
        print("no urls", file=sys.stderr)
        return 2

    try:
        ensure_session(args.mcp, args.token)
    except Exception as exc:  # noqa: BLE001
        print(f"MCP session init failed: {exc}", file=sys.stderr)
        return 1

    batches = [
        urls[i : i + args.batch_size]
        for i in range(0, len(urls), args.batch_size)
    ]
    all_results: list[Any] = []
    report: dict[str, Any] = {
        "mcp": args.mcp,
        "keyword": args.keyword,
        "batch_size": args.batch_size,
        "thread_count": args.thread_count,
        "total_urls": len(urls),
        "batches": [],
        "success": 0,
        "failed": 0,
        "by_failure_tag": {},
    }

    print(
        f"batches={len(batches)} urls={len(urls)} "
        f"threadCount={args.thread_count} keyword={args.keyword}"
    )
    try:
        for index, batch in enumerate(batches, start=1):
            print(f"[{index}/{len(batches)}] start_check_sources n={len(batch)}")
            msg = extract_text(
                tools_call(
                    args.mcp,
                    args.token,
                    "start_check_sources",
                    {
                        "urls": batch,
                        "enabledOnly": args.enabled_only,
                        "keyword": args.keyword,
                        "threadCount": args.thread_count,
                        "timeoutMs": args.timeout_ms,
                    },
                )
            )
            print(msg)
            snap = wait_batch(args.mcp, args.token, args.poll_seconds)
            batch_results = snap.get("results") or []
            all_results.extend(batch_results)
            report["batches"].append({
                "index": index,
                "size": len(batch),
                "success": snap.get("success"),
                "failed": snap.get("failed"),
                "finished": snap.get("finished"),
                "error": snap.get("error"),
                "result_count": len(batch_results),
            })
            report["success"] += int(snap.get("success") or 0)
            report["failed"] += int(snap.get("failed") or 0)
            print(
                f"[{index}/{len(batches)}] done "
                f"ok={snap.get('success')} fail={snap.get('failed')}"
            )
    except (urllib.error.URLError, RuntimeError, json.JSONDecodeError) as exc:
        print(
            "MCP call failed. Prefer agent MCP tools if streamable HTTP "
            f"shape differs.\nError: {exc}",
            file=sys.stderr,
        )
        report["error"] = str(exc)
        Path(args.out).parent.mkdir(parents=True, exist_ok=True)
        Path(args.out).write_text(
            json.dumps(report, ensure_ascii=False, indent=2),
            encoding="utf-8",
        )
        return 1

    classified = classify_results(all_results)
    report["by_failure_tag"] = {k: len(v) for k, v in classified.items()}
    report["results_sample_failed"] = [
        item for tag, items in classified.items() if tag != "success" for item in items[:20]
    ]
    dump_fail_materials(classified, Path(args.materials_dir))

    Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    Path(args.out).write_text(
        json.dumps(report, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )
    print(
        f"wrote {args.out} success={report['success']} failed={report['failed']} "
        f"tags={report['by_failure_tag']} materials={args.materials_dir}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
