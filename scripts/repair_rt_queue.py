#!/usr/bin/env python3
"""Build respondTime-sorted queue from LIVE phone index (not stale tagged fails).

Requires: python scripts/repair_refresh_phone_index.py
Optional RT join: temp/all_sources.json
"""

from __future__ import annotations

import argparse
import json
import shutil
from datetime import datetime, timezone
from pathlib import Path
from typing import Any
from urllib.parse import urlparse

_ROOT = Path(__file__).resolve().parents[1]
ALL = _ROOT / "temp" / "all_sources.json"
PHONE = _ROOT / "temp" / "full_fix" / "phone_source_index.json"
LEDGER = _ROOT / "temp" / "full_fix" / "repair_session_ledger.jsonl"
OUT_DIR = _ROOT / "temp" / "full_fix" / "queues"
DEAD_GROUP = ("网站失效", "域名失效")
SEARCH_HINTS = ("搜索失效", "搜索目录失效", "搜索正文失效")


def norm(u: str) -> str:
    return (u or "").strip()


def host_key(url: str) -> str:
    raw = norm(url).split("##")[0].split("#")[0]
    if raw and "://" not in raw:
        raw = "http://" + raw.lstrip("/")
    host = urlparse(raw).hostname or ""
    return host.lower().removeprefix("www.")


def with_scheme(u: str) -> str:
    u = norm(u)
    if u and "://" not in u.split("##")[0]:
        return "http://" + u.lstrip("/")
    return u


DEAD_SKIP_PREFIXES = (
    "l2_",
    "l1_",
    "missing:",
    "search_endpoint_dead",
    "known_auth",
    "captcha",
    "api_signature",
    "migrate_target_dead",
    "non_book",
    "repurposed",
    "waf_",
    "dead_site",
    "why_title",
    "bad_bookSourceUrl",
)


def ledger_sets(path: Path) -> tuple[set[str], set[str], set[str]]:
    """Returns (fixed, hard_skipped, retryable_skipped)."""
    fixed: set[str] = set()
    hard: set[str] = set()
    retryable: set[str] = set()
    if not path.is_file():
        return fixed, hard, retryable
    # last skip reason per url
    last_skip: dict[str, str] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        try:
            row = json.loads(line)
        except json.JSONDecodeError:
            continue
        u = norm(str(row.get("url") or ""))
        if not u:
            continue
        result = str(row.get("result") or "")
        if row.get("step") == "check" and (
            "校验成功" in result
            or result.startswith("fixed")
            or result.startswith("fixed:")
        ):
            fixed.add(u)
        step = str(row.get("step") or "")
        # hard outcomes: skip / disable / domain repurposed
        if (
            step == "skip"
            or result.startswith("skip:")
            or result.startswith("repurposed:")
            or result.startswith("disable:")
            or result.startswith("disable")
        ):
            last_skip[u] = result if result else step
    for u, reason in last_skip.items():
        if u in fixed:
            continue
        if reason.startswith(DEAD_SKIP_PREFIXES) or any(
            reason.startswith(p) for p in DEAD_SKIP_PREFIXES
        ):
            hard.add(u)
        elif (
            reason.startswith("skip:")
            or reason.startswith("repurposed:")
            or reason.startswith("disable:")
            or reason.startswith("disable")
        ):
            hard.add(u)
        elif "no_patch" in reason or "搜索" in reason or "verify_fail" in reason:
            retryable.add(u)
        else:
            hard.add(u)
    return fixed, hard, retryable


def load_rt_map() -> dict[str, int]:
    if not ALL.is_file():
        return {}
    try:
        data = json.loads(ALL.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return {}
    sources = data.get("data") or []
    out: dict[str, int] = {}
    for s in sources:
        if not isinstance(s, dict):
            continue
        u = norm(str(s.get("bookSourceUrl") or ""))
        if u and s.get("respondTime") is not None:
            try:
                out[u] = int(s["respondTime"])
            except (TypeError, ValueError):
                pass
    return out


def build(
    *,
    max_rt_ms: int = 30_000,
    limit: int = 100,
    enabled_only: bool = True,
    search_tag_only: bool = True,
) -> dict[str, Any]:
    if not PHONE.is_file():
        raise SystemExit(
            f"missing {PHONE} — run: python scripts/repair_refresh_phone_index.py"
        )
    phone = json.loads(PHONE.read_text(encoding="utf-8"))
    by_url: dict[str, Any] = phone.get("by_url") or {}
    rt_map = load_rt_map()
    fixed, hard_skipped, retryable = ledger_sets(LEDGER)
    blocked_hosts = {host_key(u) for u in fixed | hard_skipped if host_key(u)}
    # retryable hosts are allowed back into the queue once

    rows: list[dict[str, Any]] = []
    seen_hosts: set[str] = set()
    for u, meta in by_url.items():
        if not isinstance(meta, dict):
            continue
        url = with_scheme(norm(u))
        hk = host_key(url)
        if not url or not hk:
            continue
        if url in fixed or u in fixed or hk in blocked_hosts:
            continue
        if url in hard_skipped or u in hard_skipped:
            continue
        if hk in seen_hosts:
            continue  # one source per host in serial queue
        group = str(meta.get("group") or "")
        if any(x in group for x in DEAD_GROUP):
            continue
        if search_tag_only and not any(h in group for h in SEARCH_HINTS):
            continue
        enabled = meta.get("enabled")
        if enabled_only and enabled is False:
            continue
        rt = rt_map.get(u)
        if rt is None:
            rt = rt_map.get(url)
        if rt is None:
            rt = 8_000  # unknown → mid priority, not first
        if rt > max_rt_ms:
            continue
        seen_hosts.add(hk)
        rows.append(
            {
                "url": url,
                "name": str(meta.get("name") or ""),
                "group": group,
                "enabled": enabled,
                "respondTime": rt,
                "bookSourceType": 0,
                "status": "candidate",
                "on_phone": True,
                "retry": url in retryable or u in retryable,
            }
        )
    rows.sort(key=lambda r: (int(r.get("respondTime") or 10**9), r["url"]))
    selected = rows[:limit]
    ts = datetime.now(timezone.utc)
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    payload = {
        "ts": ts.isoformat(),
        "max_rt_ms": max_rt_ms,
        "limit": limit,
        "phone_total": phone.get("total"),
        "n_candidate_all": len(rows),
        "n_selected": len(selected),
        "n_retryable_ledger": len(retryable),
        "sort": "respondTime asc",
        "source": "phone_index+rt_join",
        "items": selected,
    }
    out_latest = OUT_DIR / "repair_candidates_fast_latest.json"
    out_serial = OUT_DIR / "repair_serial100_queue.json"
    out_latest.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    out_serial.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    if ALL.is_file():
        snap = OUT_DIR / "snapshots"
        snap.mkdir(parents=True, exist_ok=True)
        stamp = ts.strftime("%Y%m%d_%H%M%S")
        shutil.copy2(PHONE, snap / f"{stamp}_phone_source_index.json")
    return payload


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--max-rt-ms", type=int, default=30_000)
    ap.add_argument("--limit", type=int, default=100)
    ap.add_argument("--include-disabled", action="store_true")
    ap.add_argument("--all-groups", action="store_true", help="do not require 搜索* tag")
    args = ap.parse_args()
    payload = build(
        max_rt_ms=args.max_rt_ms,
        limit=args.limit,
        enabled_only=not args.include_disabled,
        search_tag_only=not args.all_groups,
    )
    print(
        json.dumps(
            {
                "n_selected": payload["n_selected"],
                "n_candidate_all": payload["n_candidate_all"],
                "phone_total": payload.get("phone_total"),
                "max_rt_ms": payload["max_rt_ms"],
                "top5": payload["items"][:5],
                "out": str(OUT_DIR / "repair_serial100_queue.json"),
            },
            ensure_ascii=False,
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
