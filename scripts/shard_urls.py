#!/usr/bin/env python3
"""Shard book-source URLs across devices with consistent hashing (client-side).

Mirrors app CheckConsistentHash behavior approximately for PC orchestration.
For authoritative sharding use the Kotlin helper; this script is for multi-phone
batch planning.

Example:
  python scripts/shard_urls.py --urls-file urls.txt --nodes phoneA,phoneB --out temp/shards.json
"""

from __future__ import annotations

import argparse
import json
from collections import defaultdict
from pathlib import Path


def mix(x: int) -> int:
    x ^= (x >> 16) & 0xFFFFFFFF
    x = (x * 0x7FEB352D) & 0xFFFFFFFF
    x ^= (x >> 15) & 0xFFFFFFFF
    x = (x * 0x846CA68B) & 0xFFFFFFFF
    x ^= (x >> 16) & 0xFFFFFFFF
    return x & 0x7FFFFFFF


def build_ring(nodes: list[str], virtual_nodes: int = 64) -> list[tuple[int, str]]:
    ring: list[tuple[int, str]] = []
    for node in nodes:
        for v in range(virtual_nodes):
            ring.append((mix(hash(f"{node}#{v}") & 0xFFFFFFFF), node))
    ring.sort(key=lambda t: t[0])
    return ring


def node_for(ring: list[tuple[int, str]], url: str) -> str:
    h = mix(hash(url) & 0xFFFFFFFF)
    for key, node in ring:
        if key >= h:
            return node
    return ring[0][1]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--urls-file", required=True)
    parser.add_argument("--nodes", required=True, help="comma-separated device ids")
    parser.add_argument("--virtual-nodes", type=int, default=64)
    parser.add_argument("--out", default="temp/shards.json")
    args = parser.parse_args()
    nodes = [n.strip() for n in args.nodes.split(",") if n.strip()]
    urls = [
        ln.strip()
        for ln in Path(args.urls_file).read_text(encoding="utf-8").splitlines()
        if ln.strip() and not ln.startswith("#")
    ]
    ring = build_ring(nodes, args.virtual_nodes)
    shards: dict[str, list[str]] = defaultdict(list)
    for url in urls:
        shards[node_for(ring, url)].append(url)
    Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    Path(args.out).write_text(
        json.dumps({k: v for k, v in shards.items()}, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )
    print(f"wrote {args.out} " + " ".join(f"{k}={len(v)}" for k, v in shards.items()))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
