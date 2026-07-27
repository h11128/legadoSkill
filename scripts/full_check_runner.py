#!/usr/bin/env python3
"""Robust full MCP batch check with line-buffered progress logging."""

from __future__ import annotations

import json
import sys
import time
from pathlib import Path

# Allow `python scripts/full_check_runner.py` from repo root.
_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(_ROOT))
sys.path.insert(0, str(_ROOT / "scripts"))

from scripts import batch_check_mcp as b  # noqa: E402
import mcp_channel  # noqa: E402


def main() -> int:
    try:
        mcp_channel.acquire("bulk", "full_check_runner")
    except RuntimeError as exc:
        print(str(exc), flush=True)
        return 3
    try:
        return _run()
    finally:
        mcp_channel.release("bulk")


def _run() -> int:
    defaults = _ROOT / "config" / "mcp_defaults.json"
    mcp = "http://10.0.0.139:1236/mcp"
    token = "1234"
    if defaults.is_file():
        cfg = json.loads(defaults.read_text(encoding="utf-8"))
        mcp = str(cfg.get("mcp_url") or mcp)
        token = str(cfg.get("token") or token)
    precheck = Path("temp/full_check/precheck.json")
    out = Path("temp/full_check/batch_check_report.json")
    materials = Path("temp/full_check/check_materials")
    log_path = Path("temp/full_check/batch_check.log")

    data = json.loads(precheck.read_text(encoding="utf-8"))
    urls = list(data.get("alive_urls") or [])
    batch_size = 80
    thread_count = 32
    timeout_ms = 45_000
    keyword = "我的"
    poll_s = 4.0

    batches = [urls[i : i + batch_size] for i in range(0, len(urls), batch_size)]
    report: dict = {
        "mcp": mcp,
        "keyword": keyword,
        "batch_size": batch_size,
        "thread_count": thread_count,
        "total_urls": len(urls),
        "batches": [],
        "success": 0,
        "failed": 0,
        "by_failure_tag": {},
    }

    def log(msg: str) -> None:
        line = f"{time.strftime('%H:%M:%S')} {msg}"
        print(line, flush=True)
        with log_path.open("a", encoding="utf-8") as fh:
            fh.write(line + "\n")

    log_path.write_text("", encoding="utf-8")
    b.ensure_session(mcp, token)
    # Clear any leftover job
    try:
        log(b.extract_text(b.tools_call(mcp, token, "stop_check_sources", {})))
    except Exception as exc:  # noqa: BLE001
        log(f"stop warn: {exc}")

    log(f"batches={len(batches)} urls={len(urls)} threads={thread_count}")
    all_results: list = []
    try:
        for index, batch in enumerate(batches, start=1):
            log(f"[{index}/{len(batches)}] start n={len(batch)}")
            msg = b.extract_text(
                b.tools_call(
                    mcp,
                    token,
                    "start_check_sources",
                    {
                        "urls": batch,
                        "enabledOnly": False,
                        "keyword": keyword,
                        "threadCount": thread_count,
                        "timeoutMs": timeout_ms,
                    },
                )
            )
            log(msg)
            snap = b.wait_batch(mcp, token, poll_s)
            batch_results = snap.get("results") or []
            all_results.extend(batch_results)
            report["batches"].append(
                {
                    "index": index,
                    "size": len(batch),
                    "success": snap.get("success"),
                    "failed": snap.get("failed"),
                    "finished": snap.get("finished"),
                    "error": snap.get("error"),
                    "result_count": len(batch_results),
                }
            )
            report["success"] += int(snap.get("success") or 0)
            report["failed"] += int(snap.get("failed") or 0)
            log(
                f"[{index}/{len(batches)}] done ok={snap.get('success')} "
                f"fail={snap.get('failed')} finished={snap.get('finished')}"
            )
            # Persist partial report after each batch
            out.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    except Exception as exc:  # noqa: BLE001
        log(f"FAILED: {exc}")
        report["error"] = str(exc)
        out.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
        return 1

    classified = b.classify_results(all_results)
    report["by_failure_tag"] = {k: len(v) for k, v in classified.items()}
    b.dump_fail_materials(classified, materials)
    out.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    log(
        f"DONE success={report['success']} failed={report['failed']} "
        f"tags={report['by_failure_tag']} -> {out}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
