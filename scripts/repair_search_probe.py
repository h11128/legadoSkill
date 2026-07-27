#!/usr/bin/env python3
"""Probe real search endpoints from homepage / JS / common paths (wmp8 + alicesw)."""

from __future__ import annotations

import json
import re
import ssl
import urllib.error
import urllib.parse
import urllib.request
from typing import Any
from urllib.parse import urljoin, urlparse

UA = {
    "User-Agent": (
        "Mozilla/5.0 (Linux; Android 13) AppleWebKit/537.36 "
        "(KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36"
    ),
    # Avoid zstd: stdlib urllib often returns empty/garbage bodies on CF zstd.
    "Accept-Encoding": "gzip, deflate",
    "Accept": "text/html,application/json,application/xhtml+xml;q=0.9,*/*;q=0.8",
}


def _decode_http_body(body: bytes) -> str:
    """Decompress gzip/deflate then decode text (probe missed 52dmshu forms otherwise)."""
    if not body:
        return ""
    raw = body
    if raw[:2] == b"\x1f\x8b":
        import gzip

        try:
            raw = gzip.decompress(raw)
        except Exception:
            pass
    elif raw[:1] in (b"\x78",) or raw[:2] == b"\x78\x9c":
        import zlib

        try:
            raw = zlib.decompress(raw)
        except Exception:
            try:
                raw = zlib.decompress(raw, -zlib.MAX_WBITS)
            except Exception:
                pass
    for enc in ("utf-8", "gbk", "gb2312"):
        try:
            return raw.decode(enc)
        except Exception:
            continue
    return raw.decode("utf-8", errors="replace")

# JS search shell: data-api="/api/v1/books/search" (paper027 / 卧龙)
JS_SEARCH_API_RE = re.compile(
    r"""data-api=["']([^"']*search[^"']*)["']""",
    re.I,
)
JS_NEEDLE_RE = re.compile(r"需要启用\s*JavaScript|实时搜索结果", re.I)

# Paths that are search endpoints, not book detail (fake-detail trap).
SEARCH_PATH_RE = re.compile(
    r"(?:/s\.php|/search\.php|/search(?:\.html)?|/so\.php|/modules/article/search\.php)"
    r"|(?:[?&](?:keyword|searchkey|q|wd)=)",
    re.I,
)

# Tried when homepage forms miss the real endpoint (alicesw: form→/?keyword= fake).
COMMON_GET_TEMPLATES = (
    "/search.php?q={{key}}",
    "/search.php?keyword={{key}}",
    "/search?q={{key}}",
    "/search?keyword={{key}}",
    "/search.html?q={{key}}",
    "/s.php?q={{key}}",
    "/so.php?q={{key}}",
    "/modules/article/search.php?searchkey={{key}}&searchtype=all",
)

PID_JS_RE = re.compile(r"javascript:[^\"\n>]*pid:\s*(\d+)|alert\(['\"]pid:\s*(\d+)", re.I)


def looks_like_search_url(url: str | None) -> bool:
    if not url:
        return False
    path = urlparse(url).path or ""
    q = urlparse(url).query or ""
    return bool(SEARCH_PATH_RE.search(path + "?" + q)) or path.rstrip("/").endswith(
        ("/s.php", "/search.php", "/so.php")
    )


def fetch_text(url: str, headers: dict[str, str] | None = None, timeout: float = 6.0) -> dict[str, Any]:
    h = dict(UA)
    if headers:
        h.update({k: v for k, v in headers.items() if v})
    # scheme-less absolute hosts
    if url and "://" not in url.split("/", 1)[0] and url.startswith("www."):
        url = "http://" + url
    ctx = ssl._create_unverified_context()
    req = urllib.request.Request(url, headers=h)
    try:
        with urllib.request.urlopen(req, timeout=timeout, context=ctx) as resp:
            body = resp.read()
            final, code = resp.geturl(), resp.status
    except urllib.error.HTTPError as exc:
        body = exc.read() if exc.fp else b""
        final, code = url, int(exc.code)
    except Exception as exc:  # noqa: BLE001
        return {"ok": False, "error": str(exc)[:160], "url": url}
    text = _decode_http_body(body)
    return {"ok": code < 400, "status": code, "final": final, "html": text, "len": len(body)}


def forms_from_html(html: str, base: str) -> list[dict[str, str]]:
    out: list[dict[str, str]] = []
    for m in re.finditer(r"<form\b([^>]*)>([\s\S]{0,4000}?)</form>", html, re.I):
        attrs, body = m.group(1), m.group(2)
        if not re.search(r"search|keyword|searchkey|wd|\bq\b|\bs\b|articlename", attrs + body, re.I):
            continue
        am = re.search(r'action=["\']([^"\']*)["\']', attrs, re.I)
        action = urljoin(base, am.group(1) if am else "")
        method = "POST" if re.search(r'method=["\']post["\']', attrs, re.I) else "GET"
        fields = re.findall(r'<input[^>]+name=["\']([^"\']+)["\']', body, re.I)
        out.append({"action": action, "method": method, "fields": ",".join(fields[:8])})
    return out


def forms_from_js(js: str, base: str) -> list[dict[str, str]]:
    """Parse document.writeln search forms (jieqi wap.top.js search_win)."""
    out: list[dict[str, str]] = []
    for m in re.finditer(
        r"action\\?=['\"]([^'\"]*search[^'\"]*)['\"]|"
        r"action=['\"]([^'\"]*search[^'\"]*)['\"]|"
        r"action=\\'([^\\']*search[^\\']*)\\'",
        js,
        re.I,
    ):
        action = next(g for g in m.groups() if g)
        action = action.replace("\\/", "/")
        method = (
            "POST"
            if re.search(
                r"method\\?=['\"]post['\"]|method=['\"]post['\"]",
                js[max(0, m.start() - 80) : m.end() + 200],
                re.I,
            )
            else "GET"
        )
        out.append({"action": urljoin(base, action), "method": method, "fields": "from_js", "source": "js"})
    if "/modules/article/search.php" in js and not out:
        out.append(
            {
                "action": urljoin(base, "/modules/article/search.php"),
                "method": "POST",
                "fields": "searchkey,searchtype",
                "source": "js_hint",
            }
        )
    return out


def _title(html: str) -> str:
    m = re.search(r"<title[^>]*>([^<]+)", html, re.I)
    return (m.group(1).strip() if m else "")[:120]


def score_search_html(html: str, *, home_html: str | None = None) -> dict[str, Any]:
    """Score whether HTML looks like a real search-result page (not homepage shell)."""
    low = html.lower()
    title = _title(html)
    score = 0
    signals: list[str] = []
    if "xunsearch" in low or "powered by xunsearch" in low:
        score += 8
        signals.append("xunsearch")
    if "result-list" in low or 'class="result"' in low or "class='result'" in low:
        score += 5
        signals.append("result-list")
    if re.search(r'id=["\']sitebox["\']', html) and "<dl" in low:
        score += 6
        signals.append("sitebox_dl")
    # 52dmshu / similar: #sitembox > dl (not sitebox)
    if re.search(r'id=["\']sitembox["\']', html) and "<dl" in low:
        score += 6
        signals.append("sitembox_dl")
    # common jieqi / template list markers
    for marker, pts, tag in (
        ("hot_sale", 4, "hot_sale"),
        ("bookbox", 4, "bookbox"),
        ("novelslist", 4, "novelslist"),
        ("result-item", 4, "result-item"),
        ("sone", 3, "sone"),
        ("bookname", 3, "bookname"),
        ("txt-list", 4, "txt-list"),
        ("ss_box", 3, "ss_box"),
    ):
        if marker in low:
            score += pts
            signals.append(tag)
    novel_n = len(re.findall(r"/novel/\d+", html))
    book_n = len(re.findall(r"/book/\d+", html))
    if novel_n >= 3:
        score += 4
        signals.append(f"novel_links:{novel_n}")
    if book_n >= 3:
        score += 3
        signals.append(f"book_links:{book_n}")
    pids = [g for m in PID_JS_RE.findall(html) for g in (m if isinstance(m, tuple) else (m,)) if g]
    if len(pids) >= 2:
        score += 6
        signals.append(f"pid_js:{len(pids)}")
    if re.search(r"搜索[：:]|search\s*result|找到\s*\d+|共\s*\d+\s*条", html, re.I):
        score += 2
        signals.append("search_titleish")
    # Error / soft-404 pages must not beat real form endpoints (ixs7 lesson)
    if re.search(r"出错啦|找不到了|404\s*not\s*found|页面不存在|访问的网页找不到", title + html, re.I):
        score -= 8
        signals.append("error_page")
    # Homepage shell recycled as "search" (alicesw /?keyword=)
    if home_html:
        home_t = _title(home_html)
        if home_t and title and home_t == title and "搜索" not in title and "search" not in title.lower():
            score -= 6
            signals.append("same_title_as_home")
        if "novel-list-dark" in low and "result-list" not in low and novel_n < 2:
            score -= 3
            signals.append("home_list_shell")
    book_url_hint = None
    if pids:
        book_url_hint = "a@href##pid:\\s*(\\d+)##/novel/$1.html###"
    book_list_hint = None
    if "result-list" in low:
        book_list_hint = "class.result-list@dt"
    elif re.search(r'id=["\']sitebox["\']', html):
        book_list_hint = "#sitebox dl"
    return {
        "score": score,
        "signals": signals,
        "title": title,
        "pid_count": len(pids),
        "bookUrl_hint": book_url_hint,
        "bookList_hint": book_list_hint,
    }


def materialize_search_url(template: str, key: str) -> str:
    """Turn Legado searchUrl template into a fetchable URL (GET only)."""
    enc = urllib.parse.quote(key)
    # strip POST JSON suffix if present
    head = template.split(",", 1)[0].strip()
    return (
        head.replace("{{key}}", enc)
        .replace("{{key:urlEncode}}", enc)
        .replace("%7B%7Bkey%7D%7D", enc)
    )


def _post_fetch(url: str, body: str, headers: dict[str, str] | None = None) -> dict[str, Any]:
    h = dict(UA)
    if headers:
        h.update({k: v for k, v in headers.items() if v})
    h.setdefault("Content-Type", "application/x-www-form-urlencoded")
    ctx = ssl._create_unverified_context()
    data = body.encode("utf-8", errors="replace")
    req = urllib.request.Request(url, data=data, headers=h, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=12.0, context=ctx) as resp:
            raw = resp.read()
            final, code = resp.geturl(), resp.status
    except urllib.error.HTTPError as exc:
        raw = exc.read() if exc.fp else b""
        final, code = url, int(exc.code)
    except Exception as exc:  # noqa: BLE001
        return {"ok": False, "error": str(exc)[:160], "url": url}
    text = _decode_http_body(raw)
    return {"ok": code < 400, "status": code, "final": final, "html": text, "len": len(raw)}


def rank_candidates(
    base: str,
    candidates: list[dict[str, str]],
    *,
    keyword: str = "我的",
    headers: dict[str, str] | None = None,
    home_html: str | None = None,
    max_fetch: int = 6,
) -> list[dict[str, Any]]:
    """Fetch GET/POST candidates (+ common paths), score, return best-first."""
    seen: set[str] = set()
    work: list[dict[str, str]] = []
    for c in candidates:
        su = c.get("searchUrl") or ""
        if not su:
            continue
        if su not in seen:
            seen.add(su)
            work.append(dict(c))
    # Cap common_path tries to keep serial under budget (was 10×12s≈2min)
    common_budget = max(0, min(4, max_fetch - len(work)))
    for tmpl in COMMON_GET_TEMPLATES:
        if common_budget <= 0:
            break
        if tmpl not in seen:
            seen.add(tmpl)
            work.append({"searchUrl": tmpl, "from": "common_path"})
            common_budget -= 1
    ranked: list[dict[str, Any]] = []
    enc_key = urllib.parse.quote(keyword)
    for c in work[:max_fetch]:
        su = c["searchUrl"]
        if "," in su and '"method"' in su and "POST" in su:
            head, _, rest = su.partition(",")
            try:
                meta = json.loads(rest)
            except Exception:
                meta = {}
            body = str(meta.get("body") or "").replace("{{key}}", enc_key)
            url = urljoin(base.rstrip("/") + "/", head.lstrip("/"))
            page = _post_fetch(url, body, headers)
        else:
            path = materialize_search_url(su, keyword)
            url = urljoin(base.rstrip("/") + "/", path.lstrip("/"))
            page = fetch_text(url, headers, timeout=5.0)
        scored = score_search_html(page.get("html") or "", home_html=home_html)
        status = page.get("status")
        # Form endpoint 5xx = 搜索口挂了 (not "wrong URL")
        if c.get("from") != "common_path" and status and int(status) >= 500:
            scored["signals"] = list(scored.get("signals") or []) + ["endpoint_5xx"]
            scored["score"] = min(int(scored.get("score") or 0), -5)
            scored["endpoint_dead"] = True
        ranked.append(
            {
                **c,
                "fetch_url": url,
                "fetch_ok": page.get("ok"),
                "fetch_status": status,
                "fetch_final": page.get("final"),
                **scored,
            }
        )
    # Prefer real form/js over common_path when scores tie-ish (wrong-path vs hung-form)
    def _rank_key(x: dict[str, Any]) -> tuple:
        from_form = 1 if x.get("from") != "common_path" else 0
        return (int(x.get("score") or 0), from_form)

    ranked.sort(key=_rank_key, reverse=True)
    return ranked


def _candidates_from_forms(uniq: list[dict[str, str]]) -> list[dict[str, str]]:
    candidates: list[dict[str, str]] = []
    for f in uniq:
        action = f["action"]
        fields = (f.get("fields") or "").lower()
        path = urlparse(action).path or action
        rel = path if path.startswith("/") else "/" + path.lstrip("/")
        if "search.php" in action or "modules/article/search" in action:
            # Prefer real form field (biduju=keyword; jieqi=searchkey). Do NOT hardcode searchkey.
            field = "keyword"
            for cand in ("searchkey", "keyword", "keyboard", "q", "wd", "s"):
                if cand in fields.split(","):
                    field = cand
                    break
            if "modules/article/search" in action and "searchkey" not in fields.split(","):
                field = "searchkey"
            body = f"{field}={{{{key}}}}&searchtype=all"
            use_post = f.get("method", "POST").upper() != "GET" or "modules/article/search" in action
            if use_post:
                candidates.append(
                    {
                        "searchUrl": (
                            f'{rel},{{\n  "method": "POST",\n  "body": "{body}"\n}}'
                        ),
                        "from": f.get("source") or "html",
                    }
                )
            elif field == "searchkey":
                candidates.append(
                    {
                        "searchUrl": f"{rel}?searchkey={{{{key}}}}&searchtype=all",
                        "from": f.get("source") or "html",
                    }
                )
            else:
                candidates.append(
                    {
                        "searchUrl": f"{rel}?{field}={{{{key}}}}",
                        "from": f.get("source") or "html",
                    }
                )
        elif any(x in fields for x in ("keyword", "searchkey", "q", "wd", "s", "keyboard")):
            field = "keyword"
            for cand in ("searchkey", "keyword", "keyboard", "q", "wd", "s"):
                if cand in fields.split(","):
                    field = cand
                    break
            # EmpireCMS /e/sch/ often needs show=title&classid=0
            extra = ""
            if "/e/sch" in action or field == "keyboard":
                extra = "&show=title&tempid=1&classid=0"
            if f.get("method", "GET").upper() == "POST":
                body = f"{field}={{{{key}}}}"
                if "searchtype" in fields:
                    body += "&searchtype=all"
                if "t" in fields.split(",") and "searchkey" in fields:
                    body += "&t=1"
                candidates.append(
                    {
                        "searchUrl": f'{rel},{{\n  "method": "POST",\n  "body": "{body}"\n}}',
                        "from": f.get("source") or "html",
                    }
                )
            else:
                if action.startswith("http"):
                    rel = urlparse(action).path or "/"
                    q = urlparse(action).query
                    if q:
                        rel = rel + "?" + q
                sep = "&" if "?" in rel else "?"
                candidates.append(
                    {
                        "searchUrl": f"{rel}{sep}{field}={{{{key}}}}{extra}",
                        "from": f.get("source") or "html",
                    }
                )
    return candidates


def detect_js_search_api(html: str) -> dict[str, Any] | None:
    """Detect SPA/JS search shells (paper027): data-api + empty HTML results."""
    if not html:
        return None
    m = JS_SEARCH_API_RE.search(html)
    if not m and not JS_NEEDLE_RE.search(html):
        return None
    api = (m.group(1).strip() if m else "/api/v1/books/search").rstrip("?")
    if not api.startswith("/"):
        api = "/" + api
    sep = "&" if "?" in api else "?"
    search_url = f"{api}{sep}q={{{{key}}}}&page={{{{page}}}}"
    return {
        "searchUrl": search_url,
        "from": "js_data_api",
        "score": 12,
        "signals": ["js_search_api"],
        "bookList_hint": "$.data.data",
        "bookUrl_hint": "/book/{{$.id}}",
        "name_hint": "$.title",
        "author_hint": "$.author",
        "json_rules": True,
    }


def probe_search_forms(
    home_url: str,
    headers: dict[str, str] | None = None,
    *,
    keyword: str = "我的",
    rank: bool = True,
) -> dict[str, Any]:
    """Homepage + JS forms → candidates; optionally rank via live GET + common paths."""
    page = fetch_text(home_url, headers)
    if not page.get("ok") and page.get("status") not in (200,):
        return {"home": page, "forms": [], "candidates": [], "ranked": []}
    html = page.get("html") or ""
    base = page.get("final") or home_url
    js_api = detect_js_search_api(html)
    if js_api:
        # Prefer API over HTML form ranking (form page is empty without JS).
        out: dict[str, Any] = {
            "home_status": page.get("status"),
            "home_final": base,
            "forms": forms_from_html(html, base),
            "candidates": [js_api],
            "best": js_api,
            "ranked": [js_api],
            "js_search_api": True,
            "search_endpoint_ok": True,
        }
        return out
    forms = forms_from_html(html, base)
    scripts = re.findall(r'<script[^>]+src=["\']([^"\']+)["\']', html, re.I)
    for src in scripts[:6]:
        if not re.search(r"top|search|wap|main|header", src, re.I):
            continue
        js_page = fetch_text(urljoin(base, src), headers)
        if js_page.get("html"):
            forms.extend(forms_from_js(js_page["html"], base))
    seen: set[str] = set()
    uniq: list[dict[str, str]] = []
    for f in forms:
        key = f.get("action") or ""
        if key and key not in seen:
            seen.add(key)
            uniq.append(f)
    candidates = _candidates_from_forms(uniq)
    out: dict[str, Any] = {
        "home_status": page.get("status"),
        "home_final": base,
        "forms": uniq,
        "candidates": candidates,
    }
    if rank:
        ranked = rank_candidates(
            base, candidates, keyword=keyword, headers=headers, home_html=html
        )
        out["ranked"] = ranked
        form_rows = [r for r in ranked if r.get("from") != "common_path"]
        # 搜索口挂了: homepage has a search form, but that endpoint is 5xx
        if form_rows and all(r.get("endpoint_dead") or int(r.get("fetch_status") or 0) >= 500 for r in form_rows):
            out["search_endpoint_dead"] = True
            out["suggest"] = [
                "搜索口挂了 (form endpoint HTTP 5xx) — skip; do NOT keep trying common_path"
            ]
        # Pick best: never prefer common_path error page over a live form URL
        best = None
        for r in ranked:
            if int(r.get("score") or 0) <= 0 and r.get("from") == "common_path":
                continue
            if r.get("endpoint_dead"):
                continue
            if int(r.get("score") or 0) > 0 or r.get("from") != "common_path":
                best = r
                break
        if best is None and ranked and int(ranked[0].get("score") or 0) > 0:
            best = ranked[0]
        if best and not best.get("endpoint_dead"):
            prefer = {
                "searchUrl": best.get("searchUrl"),
                "from": best.get("from"),
                "score": best.get("score"),
                "signals": best.get("signals"),
                "bookList_hint": best.get("bookList_hint"),
                "bookUrl_hint": best.get("bookUrl_hint"),
            }
            rest = [c for c in candidates if c.get("searchUrl") != prefer.get("searchUrl")]
            out["candidates"] = [prefer] + rest
            out["best"] = prefer
            # 搜索口不对: form exists and ranks above common_path — keep fixing
            if prefer.get("from") != "common_path":
                out["search_endpoint_ok"] = True
    return out


if __name__ == "__main__":
    import argparse
    import os
    import sys

    if os.environ.get("REPAIR_USE_PYTHON", "") != "1":
        # Offline HTML probe via Rust; network form probe still needs REPAIR_USE_PYTHON=1
        from pathlib import Path

        _SCRIPTS = Path(__file__).resolve().parent
        if str(_SCRIPTS) not in sys.path:
            sys.path.insert(0, str(_SCRIPTS))
        from source_cli_shim import run_source_cli

        ap = argparse.ArgumentParser(
            description="Probe search forms (Rust offline). For live fetch set REPAIR_USE_PYTHON=1."
        )
        ap.add_argument("--base-url", required=True)
        ap.add_argument("--html-file", required=True)
        ap.add_argument("--key", default="我的")
        args = ap.parse_args()
        raise SystemExit(
            run_source_cli(
                [
                    "probe",
                    "--base-url",
                    args.base_url,
                    "--html-file",
                    args.html_file,
                    "--key",
                    args.key,
                ]
            )
        )
    print(
        "repair_search_probe: library module; CLI offline probe needs --base-url/--html-file "
        "or REPAIR_USE_PYTHON=1 for legacy network probe helpers",
        file=sys.stderr,
    )
    raise SystemExit(2)
