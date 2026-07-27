#!/usr/bin/env python3
"""Layer-first diagnose one source (ihuaben + wmp8 lessons).

1) debug_source → parse layer (search|toc|content|…)
2) reclassify fake-detail (search page parsed as 1 book) → search
3) fetch failing page; if search, also probe homepage/JS forms
4) print hints + candidate searchUrl

Example:
  python scripts/repair_diagnose.py --url 'https://m.wmp8.com' --key 我的
"""

from __future__ import annotations

import argparse
import json
import re
import ssl
import sys
import urllib.request
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path
from typing import Any
from urllib.parse import urljoin

_ROOT = Path(__file__).resolve().parents[1]
_SCRIPTS = Path(__file__).resolve().parent
sys.path.insert(0, str(_SCRIPTS))

from mcp_client import ensure_session, extract_text, get_source, tools_call  # noqa: E402
from repair_debug_parse import looks_like_search_url, parse_debug_text  # noqa: E402
from repair_helpers import header_map  # noqa: E402
from repair_prefilter import DEFAULT_RULES, classify_one, load_rules  # noqa: E402
from repair_search_probe import probe_search_forms  # noqa: E402
from repair_session_log import DEFAULT as LEDGER, append_row  # noqa: E402

UA = {
    "User-Agent": (
        "Mozilla/5.0 (Linux; Android 13) AppleWebKit/537.36 "
        "(KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36"
    )
}


def defaults() -> tuple[str, str]:
    cfg = _ROOT / "config" / "mcp_defaults.json"
    data = json.loads(cfg.read_text(encoding="utf-8"))
    return str(data["mcp_url"]), str(data.get("token") or "1234")


def fetch(url: str, headers: dict[str, str] | None = None, timeout: float = 15.0) -> dict[str, Any]:
    import gzip

    h = dict(UA)
    if headers:
        h.update({k: v for k, v in headers.items() if v})
    h.setdefault("Accept-Encoding", "gzip")
    ctx = ssl._create_unverified_context()
    req = urllib.request.Request(url, headers=h)
    with urllib.request.urlopen(req, timeout=timeout, context=ctx) as resp:
        body = resp.read()
        final = resp.geturl()
        code = resp.status
    if body[:2] == b"\x1f\x8b":
        try:
            body = gzip.decompress(body)
        except Exception:
            pass
    text = None
    for enc in ("utf-8", "gbk", "gb2312"):
        try:
            text = body.decode(enc)
            break
        except Exception:
            continue
    text = text or body.decode("utf-8", errors="replace")
    # anti-bot shells
    if text.lstrip().startswith("inte_base64:") or (
        len(text) < 200 and "Redirecting" in text
    ):
        return {
            "ok": False,
            "status": code,
            "final": final,
            "html": text[:500],
            "len": len(body),
            "error": "anti_bot_shell",
        }
    return {"ok": True, "status": code, "final": final, "html": text, "len": len(body)}



def html_hints(html: str) -> dict[str, Any]:
    classes = Counter(re.findall(r'class=["\']([^"\']+)["\']', html))
    chapterish = [
        (c, n)
        for c, n in classes.most_common(40)
        if re.search(r"chapter|catalog|mulu|toc|list|searchresult|book-|sitebox|head-book", c, re.I)
    ]
    list_links = re.findall(
        r'href=["\']([^"\']*(?:list|mulu|catalog|chapter|/index/)[^"\']*)["\']', html, re.I
    )[:8]
    forms = []
    for m in re.finditer(r"<form[^>]*>([\s\S]{0,400}?)</form>", html, re.I):
        b = m.group(0)
        if re.search(r"search|keyword|wd|q=", b, re.I):
            am = re.search(r'action=["\']([^"\']*)["\']', b, re.I)
            forms.append(am.group(1) if am else "")
    return {
        "top_classes": classes.most_common(12),
        "chapterish_classes": chapterish[:12],
        "listish_hrefs": list_links,
        "search_forms": forms[:5],
        "has_chapter_row": "chapter-row" in html,
        "has_sitebox_dl": bool(re.search(r'id=["\']sitebox["\']', html)) and "<dl" in html,
        "has_yijianzhan": "YiJianZhan" in html,
        "title": (
            re.sub(r"\s+", " ", m.group(1))[:80]
            if (m := re.search(r"<title>(.*?)</title>", html, re.I | re.S))
            else ""
        ),
    }


def suggest(
    layer: str,
    hints: dict[str, Any],
    parsed: dict[str, Any],
    probe: dict[str, Any],
    src: dict[str, Any] | None = None,
) -> list[str]:
    tips: list[str] = []
    src = src or {}
    if parsed.get("fake_detail"):
        tips.append("TRAP fake_detail: detail_url is search page / list-empty fallback — fix SEARCH first")
    try:
        from repair_rule_smells import suggest_api_toc

        tips.extend(
            suggest_api_toc(
                src.get("ruleBookInfo") if isinstance(src.get("ruleBookInfo"), dict) else None,
                src.get("ruleToc") if isinstance(src.get("ruleToc"), dict) else None,
            )
        )
    except ImportError:
        pass
    toc = src.get("ruleToc") if isinstance(src.get("ruleToc"), dict) else {}
    cu = str((toc or {}).get("chapterUrl") or "")
    if "webView" in cu and ("'webView'" in cu or cu.find("{'") >= 0):
        tips.append("TRAP webView quotes: use {\"webView\":true} not {'webView': true}")
    try:
        from repair_rule_smells import suggest_bookurl_selector

        tips.extend(
            suggest_bookurl_selector(
                src.get("ruleSearch") if isinstance(src.get("ruleSearch"), dict) else None,
                parsed,
            )
        )
    except ImportError:
        pass
    if layer == "search":
        tips.append("Fix searchUrl + ruleSearch (bookList/name/bookUrl). Probe forms + common paths + score.")
        su = str(src.get("searchUrl") or "")
        if re.search(r'charset["\']?\s*:\s*["\']?gbk', su, re.I):
            tips.append(
                "TRAP charset gbk: if site meta is utf-8, remove `,{\"charset\":\"gbk\"}` "
                "(52dmshu) — else keyword encodes wrong → empty list"
            )
        if probe.get("search_endpoint_dead"):
            tips.append("TRAP 搜索口挂了: form endpoint HTTP 5xx — SKIP (not a selector bug)")
        elif probe.get("search_endpoint_ok"):
            tips.append("搜索口不对但 form 可用 — keep fixing with probe.best (prefer form over common_path)")
        best = probe.get("best") or (probe.get("candidates") or [{}])[0]
        if best.get("searchUrl"):
            tips.append(
                f"best searchUrl (score={best.get('score', '?')}): "
                f"{str(best.get('searchUrl'))[:120]}"
            )
        if best.get("signals"):
            tips.append(f"best signals: {best.get('signals')}")
        if best.get("bookList_hint"):
            tips.append(f"bookList hint: {best['bookList_hint']}")
        if best.get("bookUrl_hint"):
            tips.append(f"bookUrl hint (xunsearch pid): {best['bookUrl_hint']}")
        ranked = probe.get("ranked") or []
        if ranked and int(ranked[0].get("score") or 0) <= 0 and not probe.get("search_endpoint_dead"):
            tips.append("TRAP: form candidates scored ≤0 (homepage shell?) — try /search.php?q= etc.")
        if any("same_title_as_home" in (r.get("signals") or []) for r in ranked[:3]):
            tips.append("TRAP fake_home_search: /?keyword= returns homepage — not real results")
        if hints.get("has_sitebox_dl"):
            tips.append("jieqi mobile results: bookList=#sitebox dl, name/bookUrl=h3 a")
        if hints.get("search_forms"):
            tips.append(f"form actions: {hints['search_forms']}")
    elif layer == "toc":
        tips.append("Search OK — do NOT rewrite search. Fix tocUrl + ruleToc (detail/list HTML).")
        if parsed.get("fake_detail"):
            tips.append("TOC empty but detail_url=search — still a SEARCH/bookUrl bug, not chapterList")
        if hints.get("has_chapter_row"):
            tips.append("DOM has chapter-row → chapterList=.chapter-row")
        if hints.get("listish_hrefs"):
            tips.append(f"list-like hrefs: {hints['listish_hrefs'][:5]}")
        tips.append("Common: tocUrl=text.目录@href ; chapterList=ul:not(.nav_s) li")
    elif layer == "content":
        tips.append("TOC OK — fix ruleContent.content against chapter HTML")
        if hints.get("has_yijianzhan"):
            tips.append("content=#YiJianZhan@html (wmp8-style)")
        if "webView" in cu:
            tips.append("content + webView: prefer class.chapter-render-box@html||class.reader@html")
    elif layer == "file_download":
        tips.append("type=3: downloadUrls; bookUrl must be detail not search page")
    return tips


def home_base(url: str) -> str:
    base = url.split("#", 1)[0]
    if "://" not in base:
        base = "http://" + base
    return base.rstrip("/") + "/"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--url", required=True)
    ap.add_argument("--key", default="我的")
    ap.add_argument("--out", default="temp/full_fix/diagnose.json")
    args = ap.parse_args()

    # Fail-fast L2 BEFORE phone debug / search rank (password/parked/DB wall).
    rules = load_rules(DEFAULT_RULES) if Path(str(DEFAULT_RULES)).is_file() else []
    try:
        gate = classify_one(args.url, rules)
    except Exception as exc:  # noqa: BLE001
        gate = {"action": "verify", "reason": f"l2_error_continue:{exc}"[:80]}
    if gate.get("action") in ("disable", "skip"):
        report = {
            "ts": datetime.now(timezone.utc).isoformat(),
            "url": args.url,
            "layer": "skip",
            "l2_gate": gate,
            "suggest": [
                f"L2 fail-fast: {gate.get('action')} / {gate.get('reason')} — do NOT diagnose further",
            ],
        }
        out = Path(args.out)
        if args.out.endswith("diagnose.json"):
            out = out.with_name(
                f"diagnose_{datetime.now().strftime('%Y%m%d_%H%M%S')}.json"
            )
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
        print(json.dumps(report, ensure_ascii=False, indent=2))
        print(f"wrote {out}")
        append_row(
            LEDGER,
            {
                "ts": report["ts"],
                "url": args.url,
                "step": "skip",
                "result": str(gate.get("reason") or gate.get("action")),
                "note": "diagnose_l2_failfast",
            },
        )
        return 3

    mcp, token = defaults()
    ensure_session(mcp, token, "diagnose")
    try:
        tools_call(mcp, token, "stop_check_sources", {})
    except Exception:
        pass

    src = get_source(mcp, token, args.url)
    raw = extract_text(
        tools_call(mcp, token, "debug_source", {"url": args.url, "key": args.key}, timeout=120)
    )
    parsed = parse_debug_text(raw)
    layer = parsed["layer"]
    headers = header_map(src)
    probe: dict[str, Any] = {}

    # Reclassify: toc + fake detail or detail looks like search
    if layer == "toc" and (parsed.get("fake_detail") or looks_like_search_url(parsed.get("detail_url"))):
        layer = "search"
        parsed["layer"] = "search"
        parsed["reclassified_from"] = "toc"

    fetch_url = None
    if layer == "search":
        fetch_url = home_base(args.url)
        try:
            probe = probe_search_forms(fetch_url, headers, keyword=args.key, rank=True)
        except Exception as exc:  # noqa: BLE001
            probe = {"error": str(exc)[:160]}
    elif layer in {"toc", "content", "ok"}:
        fetch_url = parsed.get("toc_url") or parsed.get("detail_url") or home_base(args.url)

    page: dict[str, Any] = {}
    hints: dict[str, Any] = {}
    if fetch_url:
        try:
            page = fetch(fetch_url, headers)
            hints = html_hints(page.get("html") or "")
            if layer == "toc" and hints.get("listish_hrefs"):
                abs_list = urljoin(page.get("final") or fetch_url, hints["listish_hrefs"][0])
                try:
                    lp = fetch(abs_list, headers)
                    hints["list_page"] = {
                        "url": abs_list,
                        **{k: v for k, v in html_hints(lp.get("html") or "").items() if k != "search_forms"},
                    }
                except Exception as exc:  # noqa: BLE001
                    hints["list_page_err"] = str(exc)[:120]
            # Ranked probe already fetched; mirror top hit into hints for humans
            if layer == "search" and probe.get("best"):
                b = probe["best"]
                hints["search_best"] = {
                    "searchUrl": b.get("searchUrl"),
                    "score": b.get("score"),
                    "signals": b.get("signals"),
                    "bookList_hint": b.get("bookList_hint"),
                    "bookUrl_hint": b.get("bookUrl_hint"),
                }
            elif layer == "search" and probe.get("forms"):
                act = (probe["forms"][0] or {}).get("action")
                if act and "search" in act:
                    try:
                        sample = act + ("&" if "?" in act else "?") + "searchkey=%E6%88%91%E7%9A%84&searchtype=all"
                        sp = fetch(sample, headers)
                        sh = html_hints(sp.get("html") or "")
                        hints["search_result"] = {
                            "url": sample,
                            "status": sp.get("status"),
                            "has_sitebox_dl": sh.get("has_sitebox_dl"),
                            "chapterish_classes": sh.get("chapterish_classes"),
                        }
                    except Exception as exc:  # noqa: BLE001
                        hints["search_result_err"] = str(exc)[:120]
        except Exception as exc:  # noqa: BLE001
            page = {"ok": False, "error": str(exc)[:200]}

    report = {
        "ts": datetime.now(timezone.utc).isoformat(),
        "url": args.url,
        "name": src.get("bookSourceName"),
        "layer": layer,
        "parsed": parsed,
        "fetch_url": fetch_url,
        "page": {k: page.get(k) for k in ("ok", "status", "final", "len", "error") if k in page or page},
        "probe": {
            k: probe.get(k)
            for k in ("home_status", "home_final", "forms", "candidates", "best", "ranked", "error")
            if k in probe
        },
        "hints": hints,
        "suggest": suggest(layer, hints, parsed, probe, src),
        "checklist": [
            "1 channel idle",
            "2 diagnose (this)",
            "3 patch ONLY layer (search may need bookList+tocUrl+content if site theme changed)",
            "4 one verify",
            "5 ledger append",
        ],
        "rules": {
            "searchUrl": (src.get("searchUrl") or "")[:160],
            "ruleSearch": src.get("ruleSearch"),
            "tocUrl": (src.get("ruleBookInfo") or {}).get("tocUrl")
            if isinstance(src.get("ruleBookInfo"), dict)
            else None,
            "ruleToc": src.get("ruleToc"),
        },
    }
    path = Path(args.out)
    if args.out.endswith("diagnose.json"):
        path = path.with_name(f"diagnose_{datetime.now().strftime('%Y%m%d_%H%M%S')}.json")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(
        json.dumps(
            {k: report[k] for k in ("url", "name", "layer", "suggest", "parsed", "probe")},
            ensure_ascii=False,
            indent=2,
        )
    )
    print(f"wrote {path}")
    append_row(
        LEDGER,
        {
            "ts": report["ts"],
            "url": args.url,
            "step": "diagnose",
            "result": layer,
            "note": "; ".join(report["suggest"][:2]),
            "waste": "",
        },
    )
    return 0 if layer != "busy" else 2


if __name__ == "__main__":
    import os

    if os.environ.get("REPAIR_USE_PYTHON", "") != "1":
        from source_cli_shim import run_source_cli

        ap = argparse.ArgumentParser(description=__doc__)
        ap.add_argument("--url", required=True)
        ap.add_argument("--key", default="我的")
        ap.add_argument("--out", default="temp/full_fix/diagnose.json")
        ap.add_argument("--l0-only", action="store_true")
        ap.add_argument("--debug-file")
        args = ap.parse_args()
        extra = ["diagnose", "--url", args.url, "--key", args.key]
        if args.l0_only:
            extra.append("--l0-only")
        if args.debug_file:
            extra.extend(["--debug-file", args.debug_file])
        raise SystemExit(run_source_cli(extra))
    raise SystemExit(main())
