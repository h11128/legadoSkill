#!/usr/bin/env python3
"""Build a prioritized repair queue from check materials / fail JSONL."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

_SCRIPTS = Path(__file__).resolve().parent
if str(_SCRIPTS) not in sys.path:
    sys.path.insert(0, str(_SCRIPTS))

from repair_classify import decide, queue_sort_key  # noqa: E402


def load_items(path: Path) -> list[dict[str, Any]]:
    items: list[dict[str, Any]] = []
    if path.suffix == ".jsonl":
        for line in path.read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if not line:
                continue
            row = json.loads(line)
            if isinstance(row, dict):
                items.append(row)
        return items
    data = json.loads(path.read_text(encoding="utf-8"))
    if isinstance(data, list):
        return [x for x in data if isinstance(x, dict)]
    if isinstance(data, dict):
        for key in ("results", "failed", "items"):
            if isinstance(data.get(key), list):
                return [x for x in data[key] if isinstance(x, dict)]
        # materials dir summary style: walk values
        out = []
        for v in data.values():
            if isinstance(v, list):
                out.extend(x for x in v if isinstance(x, dict))
        return out
    return []


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--input", required=True, help="json/jsonl of failed checks")
    p.add_argument("--out", default="temp/full_fix/repair_queue.json")
    p.add_argument("--limit", type=int, default=50)
    args = p.parse_args()
    items = load_items(Path(args.input))
    enriched = []
    for it in items:
        url = str(it.get("url") or it.get("bookSourceUrl") or "")
        if not url:
            continue
        fail = str(it.get("message") or it.get("fail_msg") or it.get("group") or "")
        d = decide(fail)
        enriched.append({
            "url": url,
            "name": it.get("name") or it.get("bookSourceName"),
            "fail_msg": fail,
            "decision": d,
        })
    enriched.sort(key=queue_sort_key)
    enriched = enriched[: args.limit]
    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(enriched, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"wrote {out} n={len(enriched)}")
    for row in enriched[:10]:
        print(f"{row['decision']['priority']:3d} {row['decision']['action']:10s} {row['url']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
