#!/usr/bin/env python3
"""Deep-fix loop with two report modes.

Modes:
  oneshot  — process exactly one URL, print REPORT, exit (agent reports to user).
  batch    — process up to --limit URLs; print REPORT after each; summary at end.

Examples:
  python scripts/repair_deep_loop.py --mode oneshot --url https://example.com
  python scripts/repair_deep_loop.py --mode oneshot --url http://old.com --migrate-to https://new.com
  python scripts/repair_deep_loop.py --mode batch --urls-file temp/full_fix/deep_queue.json --limit 15

Queue JSON items: {"url": "...", "kind": "fix|migrate|skip", "migrate_to": "..."}
or plain URL strings.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any
from urllib.parse import urljoin

_SCRIPTS = Path(__file__).resolve().parent
_ROOT = _SCRIPTS.parent
sys.path.insert(0, str(_SCRIPTS))

import mcp_channel  # noqa: E402
from bs4 import BeautifulSoup  # noqa: E402
from mcp_client import (  # noqa: E402
    ensure_session,
    extract_text,
    get_source,
    load_endpoint,
    save_source,
    tools_call,
)
from repair_check import check_args, is_repair_success  # noqa: E402
from repair_domain_migrate import migrate_payload  # noqa: E402
from repair_progress import ledger_fixed  # noqa: E402
from repair_search_probe import fetch_text, materialize_search_url, probe_search_forms  # noqa: E402
from repair_session_log import DEFAULT as LEDGER  # noqa: E402
from repair_session_log import append_row  # noqa: E402
from repair_wait import wait_check  # noqa: E402


def emit_report(row: dict[str, Any]) -> None:
    """One-line machine + human report (safe for streaming batch)."""
    print("REPORT_JSON:" + json.dumps(row, ensure_ascii=False), flush=True)
    st = row.get("status")
    url = row.get("url")
    msg = (row.get("msg") or "")[:80]
    print(f"REPORT: [{st}] {url} {msg}", flush=True)


def guess_booklist(html: str) -> dict[str, Any]:
    soup = BeautifulSoup(html, "html.parser")
    hints: dict[str, Any] = {}
    for sel in (
        ".hot_sale",
        "#sitebox dl",
        ".result-list dt",
        ".txt-list li",
        ".bookbox",
        ".sone",
        "div.result-item",
        ".item.fiction",
    ):
        n = len(soup.select(sel))
        if n >= 2:
            hints[sel] = n
    hrefs = []
    for a in soup.select("a"):
        h = a.get("href") or ""
        t = a.get_text(strip=True)
        if t and len(t) > 1 and re.search(
            r"/book/|/novel/|/info/|/chapter|/xiaoshuo|/shu/\d|/\d+\.html", h
        ):
            hrefs.append((h, t[:40]))
    hints["bookish_n"] = len(hrefs)
    hints["bookish_sample"] = hrefs[:5]
    return hints


def pick_booklist_rule(hints: dict[str, Any]) -> str | None:
    for sel, rule in [
        (".hot_sale", "class.hot_sale"),
        ("#sitebox dl", "#sitebox dl"),
        (".bookbox", "class.bookbox"),
        (".txt-list li", "class.txt-list@li!0"),
        (".item.fiction", "class.item.fiction"),
        (".sone", "class.sone"),
        ("div.result-item", "div.result-item"),
        (".result-list dt", "class.result-list@dt"),
    ]:
        if hints.get(sel, 0) >= 2:
            return rule
    return None


def load_queue(path: Path | None, url: str | None, migrate_to: str | None) -> list[dict[str, Any]]:
    if url:
        kind = "migrate" if migrate_to else "fix"
        return [{"url": url, "kind": kind, "migrate_to": migrate_to}]
    if not path or not path.is_file():
        return []
    data = json.loads(path.read_text(encoding="utf-8"))
    items = data if isinstance(data, list) else data.get("items") or data.get("queue") or []
    out: list[dict[str, Any]] = []
    for it in items:
        if isinstance(it, str):
            out.append({"url": it, "kind": "fix", "migrate_to": None})
        elif isinstance(it, (list, tuple)) and len(it) >= 2:
            # legacy goal15 tuple: (kind, url, migrate_to)
            out.append(
                {
                    "url": it[1],
                    "kind": it[0],
                    "migrate_to": it[2] if len(it) > 2 else None,
                }
            )
        elif isinstance(it, dict) and it.get("url"):
            out.append(
                {
                    "url": str(it["url"]),
                    "kind": str(it.get("kind") or ("migrate" if it.get("migrate_to") else "fix")),
                    "migrate_to": it.get("migrate_to") or it.get("to"),
                }
            )
    return out


def process_one(
    mcp: str,
    token: str,
    item: dict[str, Any],
    *,
    timeout_ms: int = 20_000,
    require_patch: bool = False,
) -> dict[str, Any]:
    kind = str(item.get("kind") or "fix")
    url = str(item.get("url") or "")
    to = item.get("migrate_to")
    if kind == "skip":
        row = {
            "ts": datetime.now(timezone.utc).isoformat(),
            "url": url,
            "status": "skip",
            "msg": str(item.get("reason") or "skipped"),
            "notes": [],
            "fixed_n": len(ledger_fixed()),
        }
        append_row(
            LEDGER,
            {
                "ts": row["ts"],
                "url": url,
                "step": "skip",
                "result": row["msg"],
                "note": "deep_loop",
            },
        )
        return row

    try:
        src = get_source(mcp, token, url)
    except Exception:
        try:
            src = get_source(mcp, token, url.rstrip("/"))
        except Exception as exc:
            row = {
                "ts": datetime.now(timezone.utc).isoformat(),
                "url": url,
                "status": "missing",
                "msg": str(exc)[:120],
                "notes": [],
                "fixed_n": len(ledger_fixed()),
            }
            append_row(
                LEDGER,
                {
                    "ts": row["ts"],
                    "url": url,
                    "step": "skip",
                    "result": f"missing:{row['msg'][:80]}",
                    "note": "deep_loop",
                },
            )
            return row

    note_bits: list[str] = []
    # Normalize scheme-less bookSourceUrl (www.foo.com → http://www.foo.com)
    bsu = str(src.get("bookSourceUrl") or "").strip()
    if bsu and "://" not in bsu.split("##")[0]:
        fixed_bsu = "http://" + bsu.lstrip("/")
        src["bookSourceUrl"] = fixed_bsu
        note_bits.append("scheme_http")
        try:
            save_source(mcp, token, src, preserve_enabled=False, preserve_group=True)
        except Exception:
            pass
        url = fixed_bsu

    # Fix scheme-less absolute searchUrl / exploreUrl (device check rejects)
    changed_fields = False
    for field in ("searchUrl", "exploreUrl"):
        val = str(src.get(field) or "")
        head = val.split(",", 1)[0].strip()
        if head and "://" not in head and (head.startswith("www.") or head.startswith("m.")):
            src[field] = "http://" + val.lstrip("/")
            note_bits.append(f"{field}_scheme")
            changed_fields = True
    if changed_fields:
        try:
            save_source(mcp, token, src, preserve_enabled=False, preserve_group=True)
        except Exception:
            pass

    if kind == "migrate" and to:
        try:
            src = migrate_payload(src, url, str(to))
            note_bits.append(f"migrate->{to}")
        except Exception as exc:
            return {
                "ts": datetime.now(timezone.utc).isoformat(),
                "url": url,
                "status": "migrate_fail",
                "msg": str(exc)[:120],
                "notes": [],
                "fixed_n": len(ledger_fixed()),
            }

    raw_host = str(src.get("bookSourceUrl") or url).split("##")[0].strip()
    if raw_host and "://" not in raw_host:
        raw_host = "http://" + raw_host.lstrip("/")
    host = raw_host.rstrip("/") + "/"
    if not host.startswith(("http://", "https://")):
        return {
            "ts": datetime.now(timezone.utc).isoformat(),
            "url": url,
            "status": "skip",
            "msg": f"bad_bookSourceUrl:{raw_host[:60]}",
            "notes": ["bad_url"],
            "fixed_n": len(ledger_fixed()),
        }
    key = (src.get("ruleSearch") or {}).get("checkKeyWord") or "我的"
    probe = probe_search_forms(host, keyword=str(key), rank=True)
    best = probe.get("best") or {}
    patched = False
    if probe.get("search_endpoint_dead"):
        row = {
            "ts": datetime.now(timezone.utc).isoformat(),
            "url": url,
            "status": "skip",
            "msg": "search_endpoint_dead",
            "notes": ["search_endpoint_dead"],
            "fixed_n": len(ledger_fixed()),
        }
        append_row(
            LEDGER,
            {
                "ts": row["ts"],
                "url": url,
                "step": "skip",
                "result": "search_endpoint_dead",
                "note": "deep_loop",
            },
        )
        return row
    # JS/API search shell (paper027): apply JSON rules immediately
    if probe.get("js_search_api") and best.get("searchUrl"):
        src["searchUrl"] = best["searchUrl"]
        rs = dict(src.get("ruleSearch") or {})
        rs["bookList"] = best.get("bookList_hint") or "$.data.data"
        rs["name"] = best.get("name_hint") or "$.title"
        rs["author"] = best.get("author_hint") or "$.author"
        rs["bookUrl"] = best.get("bookUrl_hint") or "/book/{{$.id}}"
        if "coverUrl" not in rs:
            rs["coverUrl"] = "$.cover"
        if "intro" not in rs:
            rs["intro"] = "$.intro"
        src["ruleSearch"] = rs
        info = dict(src.get("ruleBookInfo") or {})
        if not info.get("tocUrl"):
            info["tocUrl"] = "##.*/book/(\\d+).*##/chapter/$1###"
        src["ruleBookInfo"] = info
        toc = dict(src.get("ruleToc") or {})
        if not toc.get("chapterList") or "chapters" in str(toc.get("chapterList")):
            toc["chapterList"] = "article nav a[href*=/chapter/]"
            toc["chapterName"] = "div@text"
            toc["chapterUrl"] = "href"
        src["ruleToc"] = toc
        content = dict(src.get("ruleContent") or {})
        if not content.get("content") or "contentsource" in str(content.get("content")).lower():
            content["content"] = ".prose@html"
        src["ruleContent"] = content
        # prefer https if home redirected
        final = str(probe.get("home_final") or "")
        if final.startswith("https://") and str(src.get("bookSourceUrl") or "").startswith("http://"):
            from_url = str(src.get("bookSourceUrl"))
            src = migrate_payload(src, from_url, final if final.endswith("/") else final + "/")
            note_bits.append(f"migrate->{src.get('bookSourceUrl')}")
            kind = "migrate"
            url = from_url
        note_bits.append(f"js_api={str(best['searchUrl'])[:60]}")
        patched = True
    elif best.get("searchUrl") and int(best.get("score") or 0) >= 2:
        if src.get("searchUrl") != best["searchUrl"]:
            src["searchUrl"] = best["searchUrl"]
            note_bits.append(f"searchUrl={str(best['searchUrl'])[:60]}")
            patched = True
        if best.get("bookList_hint") or best.get("bookUrl_hint"):
            rs = dict(src.get("ruleSearch") or {})
            if best.get("bookList_hint"):
                rs["bookList"] = best["bookList_hint"]
                note_bits.append(f"bookList={best['bookList_hint']}")
            if best.get("bookUrl_hint"):
                rs["bookUrl"] = best["bookUrl_hint"]
            src["ruleSearch"] = rs
            patched = True
    else:
        cands = probe.get("candidates") or []
        cand = cands[0] if cands else None
        su = (cand or {}).get("searchUrl") or ""
        if cand and su and "," not in su:
            fetch_u = urljoin(host, materialize_search_url(su, str(key)).lstrip("/"))
            page = fetch_text(fetch_u)
            hints = guess_booklist(page.get("html") or "")
            if hints.get("bookish_n", 0) >= 2 or any(
                isinstance(v, int) and v >= 2
                for k, v in hints.items()
                if k not in ("bookish_n", "bookish_sample")
            ):
                src["searchUrl"] = su
                rs = dict(src.get("ruleSearch") or {})
                rule = pick_booklist_rule(hints)
                if rule:
                    rs["bookList"] = rule
                    note_bits.append(f"bookList={rule}")
                src["ruleSearch"] = rs
                note_bits.append(f"form_search={su[:50]}")
                patched = True
        elif cand and "," in su:
            src["searchUrl"] = su
            note_bits.append("post_search")
            patched = True

    if not patched:
        try:
            from repair_patches import apply_auto_patches
            from repair_rule_smells import apply_safe_rule_fixes

            smell_changes = apply_safe_rule_fixes(src)
            src2, auto_changes = apply_auto_patches(src)
            src = src2
            if smell_changes or auto_changes:
                note_bits.extend(smell_changes or [])
                note_bits.extend(auto_changes or [])
                patched = True
                note_bits.append("auto_smells")
        except Exception:
            pass

    if kind == "migrate" or patched:
        src["enabled"] = True
        save_source(mcp, token, src, preserve_enabled=False, preserve_group=True)
        if kind == "migrate":
            try:
                extract_text(tools_call(mcp, token, "delete_sources", {"urls": [url]}))
            except Exception:
                pass

    # Meaningful change? scheme-only counts; empty probe must not burn device verify.
    meaningful = bool(patched or kind == "migrate" or any(
        n.startswith(("scheme_", "searchUrl_scheme", "exploreUrl_scheme")) for n in note_bits
    ))
    if require_patch and not meaningful:
        row = {
            "ts": datetime.now(timezone.utc).isoformat(),
            "url": str(src.get("bookSourceUrl") or url),
            "status": "skip",
            "msg": "no_patch_skip",
            "notes": note_bits + ["no_patch_skip"],
            "best_score": best.get("score"),
            "best_searchUrl": best.get("searchUrl"),
            "fixed_n": len(ledger_fixed()),
        }
        append_row(
            LEDGER,
            {
                "ts": row["ts"],
                "url": row["url"],
                "step": "skip",
                "result": "no_patch_skip",
                "note": ";".join(note_bits) or "no_patch",
            },
        )
        return row

    verify_url = str(src.get("bookSourceUrl"))
    try:
        tools_call(mcp, token, "stop_check_sources", {})
    except Exception:
        pass
    tools_call(
        mcp,
        token,
        "start_check_sources",
        check_args([verify_url], str(key), thread_count=1, timeout_ms=timeout_ms),
    )
    snap = wait_check(mcp, token, expect_n=1, max_wait_s=max(30.0, timeout_ms / 1000 + 15), progress=True)
    ok = False
    msg = ""
    for r in snap.get("results") or []:
        msg = str(r.get("message") or r.get("msg") or "")
        if is_repair_success(r) or r.get("success") is True or "校验成功" in msg:
            ok = True
    append_row(
        LEDGER,
        {
            "ts": datetime.now(timezone.utc).isoformat(),
            "url": verify_url,
            "step": "check" if ok else "skip",
            "result": "校验成功" if ok else (msg[:120] or "verify_fail"),
            "note": ";".join(note_bits) or kind,
        },
    )
    return {
        "ts": datetime.now(timezone.utc).isoformat(),
        "url": verify_url,
        "status": "fixed" if ok else "fail",
        "msg": msg[:120],
        "notes": note_bits,
        "best_score": best.get("score"),
        "best_searchUrl": best.get("searchUrl"),
        "fixed_n": len(ledger_fixed()),
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--mode", choices=("oneshot", "batch"), required=True)
    ap.add_argument("--url", default="")
    ap.add_argument("--migrate-to", default="")
    ap.add_argument("--urls-file", default="")
    ap.add_argument("--limit", type=int, default=15, help="batch: max URLs")
    ap.add_argument("--timeout-ms", type=int, default=20_000)
    ap.add_argument("--out", default="temp/full_fix/deep_loop_last.json")
    args = ap.parse_args()

    queue = load_queue(
        Path(args.urls_file) if args.urls_file else None,
        args.url or None,
        args.migrate_to or None,
    )
    if not queue:
        print(json.dumps({"ok": False, "error": "empty queue — pass --url or --urls-file"}, ensure_ascii=False))
        return 2
    if args.mode == "oneshot":
        queue = queue[:1]
    else:
        queue = queue[: max(1, args.limit)]

    start = len(ledger_fixed())
    print(
        json.dumps(
            {"mode": args.mode, "n": len(queue), "fixed_start": start},
            ensure_ascii=False,
        ),
        flush=True,
    )

    mcp, token = load_endpoint()
    mcp_channel.assert_idle_for_repair()
    mcp_channel.acquire("repair", f"deep_loop_{args.mode}")
    results: list[dict[str, Any]] = []
    try:
        ensure_session(mcp, token, f"deep_loop_{args.mode}")
        for item in queue:
            print(f"\n######## {item.get('kind')} {item.get('url')}", flush=True)
            row = process_one(mcp, token, item, timeout_ms=args.timeout_ms)
            results.append(row)
            emit_report(row)
            if args.mode == "oneshot":
                break
    finally:
        mcp_channel.release("repair")

    out = {
        "mode": args.mode,
        "fixed_start": start,
        "fixed_end": len(ledger_fixed()),
        "results": results,
    }
    path = _ROOT / args.out
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(out, ensure_ascii=False, indent=2), encoding="utf-8")
    print("DONE", json.dumps({k: out[k] for k in ("mode", "fixed_start", "fixed_end")}, ensure_ascii=False), flush=True)
    print(f"wrote {path}", flush=True)
    if args.mode == "oneshot":
        return 0 if results and results[0].get("status") == "fixed" else 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
