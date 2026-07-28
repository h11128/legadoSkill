#!/usr/bin/env python3
"""CLI for repair_db: migrate, status, import ledger, export phone index."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

_SCRIPTS = Path(__file__).resolve().parent
_ROOT = _SCRIPTS.parent
sys.path.insert(0, str(_SCRIPTS))

from repair_db import (  # noqa: E402
    connect,
    db_path,
    export_phone_index_json,
    import_host_stats_file,
    import_html_cache_dir,
    import_jsonl_ledger,
    load_cfg,
    phone_index_fresh,
)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    sub = ap.add_subparsers(dest="cmd", required=True)

    sub.add_parser("migrate", help="create/open DB schema")
    exp = sub.add_parser("export-phone-index", help="write phone_source_index.json from DB")
    exp.add_argument("--out", default=str(_ROOT / "temp/full_fix/phone_source_index.json"))
    imp = sub.add_parser("import-ledger", help="import JSONL ledger into SQLite")
    imp.add_argument("--path", default=str(_ROOT / "temp/full_fix/repair_session_ledger.jsonl"))
    ihtml = sub.add_parser("import-html-cache", help="scan cache/html into html_cache_meta")
    ihtml.add_argument("--dir", default=str(_ROOT / "temp/full_fix/cache/html"))
    ihost = sub.add_parser("import-host-stats", help="import host_stats.json into SQLite")
    ihost.add_argument("--path", default=str(_ROOT / "temp/full_fix/cache/host_stats.json"))
    sub.add_parser("import-cache", help="import-html-cache + import-host-stats")
    st = sub.add_parser("status", help="show cache/db stats")

    args = ap.parse_args()
    cfg = load_cfg()
    if args.cmd == "migrate":
        with connect(cfg):
            pass
        print(json.dumps({"db": str(db_path(cfg)), "ok": True}))
        return 0
    if args.cmd == "export-phone-index":
        payload = export_phone_index_json(Path(args.out), cfg)
        print(json.dumps({"total": payload["total"], "out": args.out, "from_db": True}))
        return 0
    if args.cmd == "import-ledger":
        n = import_jsonl_ledger(Path(args.path), cfg)
        print(json.dumps({"imported": n, "path": args.path}))
        return 0
    if args.cmd == "import-html-cache":
        n = import_html_cache_dir(Path(args.dir), cfg=cfg)
        print(json.dumps({"imported": n, "dir": args.dir}))
        return 0
    if args.cmd == "import-host-stats":
        n = import_host_stats_file(Path(args.path), cfg=cfg)
        print(json.dumps({"imported": n, "path": args.path}))
        return 0
    if args.cmd == "import-cache":
        html_n = import_html_cache_dir(cfg=cfg)
        host_n = import_host_stats_file(cfg=cfg)
        print(json.dumps({"html_meta": html_n, "host_stats": host_n}))
        return 0
    if args.cmd == "status":
        with connect(cfg) as conn:
            snap = conn.execute("SELECT COUNT(*) c FROM source_snapshot").fetchone()["c"]
            led = conn.execute("SELECT COUNT(*) c FROM ledger_events").fetchone()["c"]
            html = conn.execute("SELECT COUNT(*) c FROM html_cache_meta").fetchone()["c"]
            meta = {
                r["key"]: r["value"]
                for r in conn.execute("SELECT key, value FROM schema_meta")
            }
        print(
            json.dumps(
                {
                    "db": str(db_path(cfg)),
                    "source_snapshots": snap,
                    "ledger_events": led,
                    "html_cache_meta": html,
                    "phone_pull_at": meta.get("phone_pull_at"),
                    "phone_index_fresh": phone_index_fresh(cfg=cfg),
                },
                ensure_ascii=False,
                indent=2,
            )
        )
        return 0
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
