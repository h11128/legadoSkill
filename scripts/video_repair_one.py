#!/usr/bin/env python3
"""Skeleton end-to-end path for one video/media source (not novel TOC).

Does NOT call novel start_check_sources as success gate.
Steps: route → L1/L2 probe → get_source → print type/smells → optional save.

Example:
  python scripts/video_repair_one.py --url https://taopianzy.com/ --dry-run
"""

from __future__ import annotations

import argparse
import json
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

_ROOT = Path(__file__).resolve().parents[1]
_SCRIPTS = Path(__file__).resolve().parent
sys.path.insert(0, str(_SCRIPTS))

from mcp_client import ensure_session, extract_text, get_source, parse_json_text  # noqa: E402
from repair_prefilter import classify_one, load_rules  # noqa: E402
from video_prefilter import load_routes, route_url  # noqa: E402

ROUTES = _ROOT / "config" / "video_source_routes.json"
SKIP = _ROOT / "config" / "verify_skip_rules.json"
OUT_DIR = _ROOT / "temp" / "full_fix"


def _defaults() -> tuple[str, str]:
    cfg = _ROOT / "config" / "mcp_defaults.json"
    if cfg.is_file():
        data = json.loads(cfg.read_text(encoding="utf-8"))
        return str(data.get("mcp_url") or ""), str(data.get("token") or "1234")
    return "http://10.0.0.139:1236/mcp", "1234"


def smell_video(src: dict[str, Any]) -> list[str]:
    smells: list[str] = []
    bst = src.get("bookSourceType")
    # 3 = video in this library; 0/text on a media host is the smell
    if bst in (0, "0", None):
        smells.append(f"bookSourceType={bst!r}_should_be_video")
    if not (src.get("exploreUrl") or src.get("searchUrl")):
        smells.append("no_explore_or_search")
    bi = src.get("ruleBookInfo") or {}
    if isinstance(bi, dict) and not bi.get("downloadUrls"):
        smells.append("missing_downloadUrls")
    return smells


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--url", required=True)
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--mcp")
    ap.add_argument("--token")
    args = ap.parse_args()

    route = route_url(args.url, load_routes(ROUTES))
    rules = load_rules(SKIP) if SKIP.is_file() else []
    probe = classify_one(args.url, rules, l2_timeout=5.0)
    report: dict[str, Any] = {
        "ts": datetime.now(timezone.utc).isoformat(),
        "url": args.url,
        "route": route,
        "probe": probe,
        "flow": "video",
    }
    if route.get("flow") != "video":
        report["warning"] = "url_not_in_video_routes_continue_anyway"

    mcp, token = args.mcp or _defaults()[0], args.token or _defaults()[1]
    try:
        ensure_session(mcp, token)
        raw = extract_text(get_source(mcp, token, args.url))
        src = parse_json_text(raw)
    except Exception as exc:  # noqa: BLE001
        report["error"] = str(exc)
        src = None
    if isinstance(src, dict):
        report["bookSourceName"] = src.get("bookSourceName")
        report["bookSourceType"] = src.get("bookSourceType")
        report["smells"] = smell_video(src)
        report["next"] = (
            "PC-fetch explore/search HTML; patch playUrl/downloadUrls; "
            "debug_source one title — do not claim fixed via novel 校验成功"
        )
    elif "error" not in report:
        report["error"] = "get_source_failed"
        report["raw"] = raw[:500] if isinstance(raw, str) else raw

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    path = OUT_DIR / f"video_repair_{datetime.now().strftime('%Y%m%d_%H%M%S')}.json"
    path.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False, indent=2))
    print(f"wrote {path}", flush=True)
    if args.dry_run:
        return 0
    return 0 if "error" not in report else 1


if __name__ == "__main__":
    raise SystemExit(main())
