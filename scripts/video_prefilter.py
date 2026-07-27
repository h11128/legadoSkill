#!/usr/bin/env python3
"""Classify URL into novel vs video repair flow; optional L2 probe for media hosts."""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

_ROOT = Path(__file__).resolve().parents[1]
_SCRIPTS = Path(__file__).resolve().parent
sys.path.insert(0, str(_SCRIPTS))

from repair_prefilter import classify_one, load_rules  # noqa: E402

ROUTES = _ROOT / "config" / "video_source_routes.json"
SKIP = _ROOT / "config" / "verify_skip_rules.json"


def load_routes(path: Path) -> list[dict[str, str]]:
    if not path.is_file():
        return []
    return list(json.loads(path.read_text(encoding="utf-8")).get("routes") or [])


def route_url(url: str, routes: list[dict[str, str]]) -> dict[str, Any]:
    for rule in routes:
        pat = rule.get("pattern") or ""
        if pat and re.search(pat, url, flags=re.I):
            return {
                "url": url,
                "flow": rule.get("flow") or "video",
                "reason": rule.get("reason") or rule.get("id"),
                "route_id": rule.get("id"),
            }
    return {"url": url, "flow": "novel", "reason": "default_novel"}


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--url", action="append", default=[])
    ap.add_argument("--probe", action="store_true", help="also run L0-L2 probe")
    ap.add_argument("--out", default="temp/full_fix/video_route.json")
    args = ap.parse_args()
    if not args.url:
        print("need --url", file=sys.stderr)
        return 2
    routes = load_routes(ROUTES)
    rules = load_rules(SKIP) if SKIP.is_file() else []
    rows = []
    for u in args.url:
        row = route_url(u, routes)
        if args.probe:
            row["probe"] = classify_one(u, rules, l2_timeout=5.0)
        rows.append(row)
    report = {
        "video": [r for r in rows if r["flow"] == "video"],
        "novel": [r for r in rows if r["flow"] == "novel"],
        "results": rows,
    }
    path = Path(args.out)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"wrote {path} video={len(report['video'])} novel={len(report['novel'])}")
    for r in rows:
        print(f"{r['flow']:6s} {r['reason']:16s} {r['url']}")
    return 0


if __name__ == "__main__":
    import os

    if os.environ.get("REPAIR_USE_PYTHON", "") != "1":
        from source_cli_shim import run_source_cli

        ap = argparse.ArgumentParser(description=__doc__)
        ap.add_argument("--url", action="append", default=[])
        ap.add_argument("--probe", action="store_true")
        ap.add_argument("--out", default="temp/full_fix/video_route.json")
        args = ap.parse_args()
        if len(args.url) == 1 and not args.probe:
            raise SystemExit(run_source_cli(["video-route", "--url", args.url[0]]))
    raise SystemExit(main())
