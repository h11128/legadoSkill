#!/usr/bin/env python3
"""Hunt replacement domains for 'dead' novel sources, then L1/L2 probe.

Does NOT use App check. Seeds come from:
  - config/domain_hunt_seeds.json (manual/web-search curated candidates)
  - optional --candidate flags

Example:
  python scripts/repair_domain_hunt.py \\
    --url http://www.zxcs.info/ --url http://book.tiexue.net/ \\
    --out temp/full_fix/domain_hunt.json
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

_ROOT = Path(__file__).resolve().parents[1]
_SCRIPTS = Path(__file__).resolve().parent
sys.path.insert(0, str(_SCRIPTS))

from repair_prefilter import classify_one, load_rules  # noqa: E402

SEEDS = _ROOT / "config" / "domain_hunt_seeds.json"
RULES = _ROOT / "config" / "verify_skip_rules.json"


def load_seeds(path: Path) -> dict[str, Any]:
    if not path.is_file():
        return {"seeds": {}}
    return json.loads(path.read_text(encoding="utf-8"))


def host_key(url: str) -> str:
    from urllib.parse import urlparse

    raw = url.split("#", 1)[0]
    if "://" not in raw:
        raw = "http://" + raw
    return (urlparse(raw).hostname or "").lower()


def hunt_one(
    url: str,
    seeds: dict[str, Any],
    rules: list[dict[str, str]],
    extra: list[str],
) -> dict[str, Any]:
    host = host_key(url)
    entry = (seeds.get("seeds") or {}).get(host) or {}
    cands = list(entry.get("candidates") or [])
    cands.extend(extra)
    # always probe original cleaned
    seen: set[str] = set()
    ordered: list[str] = []
    for c in cands:
        if c not in seen:
            seen.add(c)
            ordered.append(c)
    probes = []
    best = None
    for c in ordered:
        row = classify_one(c, rules, l2_timeout=5.0)
        probes.append(row)
        if row.get("verify") and best is None:
            best = c
    same = bool(
        best
        and best.rstrip("/").replace("https://", "http://")
        == url.split("#", 1)[0].rstrip("/").replace("https://", "http://")
    )
    low = (entry.get("confidence") or "") == "low"
    if entry.get("shutdown"):
        action = "no_mirror"
    elif best and not same and not low:
        action = "migrate"
    elif best and not same and low:
        action = "weak_candidate"
    elif best:
        action = "original_alive"
    else:
        action = "none_alive"
    return {
        "url": url,
        "host": host,
        "note": entry.get("note"),
        "shutdown": bool(entry.get("shutdown")),
        "confidence": entry.get("confidence") or "normal",
        "best_candidate": best,
        "probes": probes,
        "action": action,
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--url", action="append", default=[])
    ap.add_argument("--candidate", action="append", default=[], help="extra candidate URL")
    ap.add_argument("--seeds", default=str(SEEDS))
    ap.add_argument("--out", default="temp/full_fix/domain_hunt.json")
    args = ap.parse_args()
    if not args.url:
        print("need --url", file=sys.stderr)
        return 2
    seeds = load_seeds(Path(args.seeds))
    rules = load_rules(RULES) if RULES.is_file() else []
    results = [hunt_one(u, seeds, rules, args.candidate) for u in args.url]
    report = {
        "total": len(results),
        "migrate": [r for r in results if r["action"] == "migrate"],
        "weak_candidate": [r for r in results if r["action"] == "weak_candidate"],
        "no_mirror": [r for r in results if r["action"] == "no_mirror"],
        "none_alive": [r for r in results if r["action"] == "none_alive"],
        "original_alive": [r for r in results if r["action"] == "original_alive"],
        "results": results,
    }
    path = Path(args.out)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"wrote {path}")
    for r in results:
        print(f"{r['action']:10s} {r['url']} -> {r.get('best_candidate')}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
