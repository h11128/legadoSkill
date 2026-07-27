#!/usr/bin/env python3
"""L0/L1/L2 pre-verify filter (PC scripts — not App).

L0: denylist in config/verify_skip_rules.json
L1: DNS + TCP connect (short)
L2: HTTP GET/HEAD with short timeout (hangs => drop from verify)

Example:
  python scripts/repair_prefilter.py --url https://a.com --url http://dead.example \\
    --out temp/full_fix/prefilter.json
"""

from __future__ import annotations

import argparse
import concurrent.futures
import json
import re
import socket
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any
from urllib.parse import urlparse

_ROOT = Path(__file__).resolve().parents[1]
_SCRIPTS = Path(__file__).resolve().parent
if str(_SCRIPTS) not in sys.path:
    sys.path.insert(0, str(_SCRIPTS))

DEFAULT_RULES = _ROOT / "config" / "verify_skip_rules.json"


def load_rules(path: Path) -> list[dict[str, str]]:
    data = json.loads(path.read_text(encoding="utf-8"))
    return list(data.get("rules") or [])


def clean_url(url: str) -> str:
    return url.split("#", 1)[0].strip()


def host_of(url: str) -> str | None:
    raw = clean_url(url)
    if "://" not in raw:
        raw = "http://" + raw
    return urlparse(raw).hostname


def match_l0(url: str, rules: list[dict[str, str]]) -> dict[str, Any] | None:
    for rule in rules:
        pat = rule.get("pattern") or ""
        if pat and re.search(pat, url, flags=re.I):
            return {
                "layer": "L0",
                "action": rule.get("action") or "skip",
                "reason": rule.get("reason") or rule.get("id") or "denylist",
                "rule_id": rule.get("id"),
            }
    return None


def probe_l1(url: str, tcp_timeout: float = 1.5) -> dict[str, Any]:
    host = host_of(url)
    if not host:
        return {"ok": False, "layer": "L1", "error": "bad_host"}
    if len(host) > 253 or any(len(p) > 63 for p in host.split(".")):
        return {"ok": False, "layer": "L1", "error": "invalid_hostname"}
    try:
        infos = socket.getaddrinfo(host, None)
    except OSError as exc:
        return {"ok": False, "layer": "L1", "error": f"dns:{exc}"}
    port = 443 if clean_url(url).startswith("https") else 80
    if "://" in clean_url(url):
        port = 443 if urlparse(clean_url(url)).scheme == "https" else 80
    addrs = []
    for info in infos:
        ip = info[4][0]
        if ip not in addrs:
            addrs.append(ip)
        if len(addrs) >= 2:
            break
    last_err = "tcp:no_addr"
    for ip in addrs:
        try:
            with socket.create_connection((ip, port), timeout=tcp_timeout):
                return {"ok": True, "layer": "L1", "ip": ip, "port": port}
        except OSError as exc:
            last_err = f"tcp:{exc}"
    return {"ok": False, "layer": "L1", "error": last_err}


# Parking / expired / sale / JS-shell — HTTP 200 but not a real book site.
DEADISH_HINTS = (
    "无法访问此网站",
    "域名到期",
    "域名已过期",
    "域名过期",
    "没有找到站点",
    "welcome to nginx",
    "广告内容 · 请自行辨别",
    "gg_card_title",
    "18+高清影视",
    "sitename suspended",
    "404 Not Found",
    "this domain",
    "domain expired",
    "domain has expired",
    "expired domain",
    "for sale",
    "buy this domain",
    "hugedomains",
    "godaddy",
    "sedo.com",
    "dan.com",
    "afternic",
    "parked",
    "parking",
    "域名出售",
    "域名买卖",
    "此域名出售",
    "该域名",
    "出售域名",
)
# Soft walls: site "alive" but not repairable without human — skip, don't diagnose.
WALL_HINTS = (
    "请输入密码",
    "输入密码访问",
    "password protected",
    "password required",
    "连接数据库失败",
    "数据库连接失败",
    "urldance.com",
)
SHELL_HINTS = (
    "redirecting...",
    "<title>redirecting",
    "inte_base64:",
    "challenge-platform",
    "cf-browser-verification",
)


def _sniff_dead_html(text: str, *, final_url: str = "", title: str = "") -> str | None:
    """Return reason tag if HTML looks like parking/expired/bot-shell/password wall."""
    low = (text or "").lower()
    title_l = (title or "").lower()
    final_l = (final_url or "").lower()
    blob = f"{title_l}\n{final_l}\n{low[:8000]}"
    for h in WALL_HINTS:
        if h.lower() in blob:
            return f"wall:{h}"
    for h in DEADISH_HINTS:
        if h.lower() in blob:
            return f"deadish:{h}"
    for h in SHELL_HINTS:
        if h in blob:
            return f"shell:{h}"
    # Tiny non-site bodies often parking/JS redirectors
    if len(text) < 6000 and ("for sale" in low or "出售" in text or title_l == "redirecting..."):
        return "deadish:tiny_sale_or_redirect"
    # Login/account shell home — not a novel site (96biquge lesson)
    has_login = bool(
        re.search(r'id=["\']?loginform|name=["\']?loginform|action=["\'][^"\']*login', low)
    )
    has_pwd = 'type="password"' in low or "type='password'" in low
    has_novel_search = bool(
        re.search(r"searchkey|name=[\"']q[\"']|name=[\"']wd[\"']|search\.php|/search\?", low)
    )
    if has_login and has_pwd and not has_novel_search and len(text) < 40_000:
        return "wall:login_shell_not_novel"
    return None


def probe_l2(url: str, timeout: float = 4.0) -> dict[str, Any]:
    """GET homepage (not HEAD-only) and sniff parking/expired shells.

    HEAD+200 used to mark parked domains as alive — missed shu05 for-sale pages.
    """
    probe = clean_url(url)
    if "://" not in probe:
        probe = "http://" + probe
    headers = {
        "User-Agent": (
            "Mozilla/5.0 (Linux; Android 13) AppleWebKit/537.36 "
            "(KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36"
        ),
        "Accept": "text/html,application/xhtml+xml",
    }
    start = time.perf_counter()
    ctx = None
    try:
        import ssl

        ctx = ssl._create_unverified_context()
    except Exception:
        ctx = None

    def _read(resp) -> tuple[int, str, str]:
        code = getattr(resp, "status", 200)
        final = resp.geturl()
        body = resp.read(12000)
        if body[:2] == b"\x1f\x8b":
            import gzip

            try:
                body = gzip.decompress(body)
            except Exception:
                pass
        text = body.decode("utf-8", errors="replace")
        return code, final, text

    try:
        req = urllib.request.Request(probe, method="GET", headers=headers)
        with urllib.request.urlopen(req, timeout=timeout, context=ctx) as resp:
            code, final, text = _read(resp)
    except urllib.error.HTTPError as exc:
        try:
            body = exc.read(8000) if exc.fp else b""
            text = body.decode("utf-8", errors="replace")
        except Exception:
            text = ""
        code, final = int(exc.code), probe
        ms = int((time.perf_counter() - start) * 1000)
        if code >= 500:
            return {"ok": False, "layer": "L2", "status": code, "ms": ms, "error": f"http_{code}"}
        # still sniff 4xx bodies
        title_m = re.search(r"<title[^>]*>([^<]+)", text, re.I)
        title = title_m.group(1).strip() if title_m else ""
        reason = _sniff_dead_html(text, final_url=final, title=title)
        return {
            "ok": reason is None and code < 400,
            "layer": "L2",
            "status": code,
            "ms": ms,
            "final": final,
            "title": title[:80],
            "deadish": reason,
        }
    except Exception as exc:  # noqa: BLE001
        return {
            "ok": False,
            "layer": "L2",
            "error": str(exc)[:200],
            "ms": int((time.perf_counter() - start) * 1000),
        }

    ms = int((time.perf_counter() - start) * 1000)
    title_m = re.search(r"<title[^>]*>([^<]+)", text, re.I)
    title = title_m.group(1).strip() if title_m else ""
    reason = _sniff_dead_html(text, final_url=final, title=title)
    # Host jumped (org→com etc.) — still "reachable" but flag migrate
    src_host = host_of(probe)
    fin_host = host_of(final)
    migrated = bool(src_host and fin_host and src_host.lower().rstrip(".") != fin_host.lower().rstrip("."))
    ok = (200 <= code < 400) and reason is None
    out: dict[str, Any] = {
        "ok": ok,
        "layer": "L2",
        "status": code,
        "ms": ms,
        "final": final,
        "title": title[:80],
        "bytes": len(text),
    }
    if reason:
        out["deadish"] = reason
    if migrated:
        out["host_migrated"] = True
        out["from_host"] = src_host
        out["to_host"] = fin_host
    return out


def classify_one(
    url: str,
    rules: list[dict[str, str]],
    *,
    l2_timeout: float = 4.0,
    tcp_timeout: float = 1.5,
) -> dict[str, Any]:
    hit = match_l0(url, rules)
    if hit:
        return {"url": url, "verify": False, **hit}
    l1 = probe_l1(url, tcp_timeout=tcp_timeout)
    if not l1.get("ok"):
        return {
            "url": url,
            "verify": False,
            "action": "disable",
            "reason": "l1_unreachable",
            "l1": l1,
        }
    l2 = probe_l2(url, timeout=l2_timeout)
    if not l2.get("ok"):
        reason = "l2_http_dead"
        action = "disable"
        dead = str(l2.get("deadish") or "")
        if dead.startswith("wall:"):
            reason = "l2_password_or_db_wall"
            action = "skip"
        elif dead.startswith("deadish:"):
            reason = "l2_domain_parked_or_expired"
            action = "disable"
        elif dead.startswith("shell:"):
            reason = "l2_bot_shell"
            action = "skip"
        return {
            "url": url,
            "verify": False,
            "action": action,
            "reason": reason,
            "l1": l1,
            "l2": l2,
        }
    out = {
        "url": url,
        "verify": True,
        "action": "verify",
        "reason": "passed_l0_l1_l2",
        "l1": l1,
        "l2": l2,
    }
    if l2.get("host_migrated"):
        out["action"] = "migrate"
        out["reason"] = "l2_host_redirect"
        out["migrate_to"] = l2.get("to_host")
    return out


def filter_urls(
    urls: list[str],
    rules_path: Path = DEFAULT_RULES,
    *,
    concurrency: int = 32,
    l2_timeout: float = 4.0,
) -> dict[str, Any]:
    rules = load_rules(rules_path) if rules_path.is_file() else []
    rows: list[dict[str, Any]] = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=max(1, concurrency)) as pool:
        futs = [pool.submit(classify_one, u, rules, l2_timeout=l2_timeout) for u in urls]
        for fut in concurrent.futures.as_completed(futs):
            rows.append(fut.result())
    rows.sort(key=lambda r: r.get("url") or "")
    verify = [r["url"] for r in rows if r.get("verify")]
    skip = [r for r in rows if r.get("action") == "skip"]
    disable = [r for r in rows if r.get("action") == "disable"]
    video = [r for r in rows if r.get("action") == "video"]
    hunt = [r for r in rows if r.get("action") == "hunt"]
    return {
        "total": len(rows),
        "verify_urls": verify,
        "skip": skip,
        "disable": disable,
        "video": video,
        "hunt": hunt,
        "results": rows,
    }


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--url", action="append", default=[])
    p.add_argument("--urls-file")
    p.add_argument("--rules", default=str(DEFAULT_RULES))
    p.add_argument("--concurrency", type=int, default=32)
    p.add_argument("--l2-timeout", type=float, default=4.0)
    p.add_argument("--out")
    args = p.parse_args()
    urls = list(args.url)
    if args.urls_file:
        for line in Path(args.urls_file).read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if line and not line.startswith("#"):
                urls.append(line)
    if not urls:
        print("no urls", file=sys.stderr)
        return 2
    report = filter_urls(
        urls,
        Path(args.rules),
        concurrency=args.concurrency,
        l2_timeout=args.l2_timeout,
    )
    text = json.dumps(report, ensure_ascii=False, indent=2)
    if args.out:
        path = Path(args.out)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
        print(
            f"wrote {path} verify={len(report['verify_urls'])} "
            f"skip={len(report['skip'])} disable={len(report['disable'])} "
            f"video={len(report.get('video') or [])} hunt={len(report.get('hunt') or [])}"
        )
    else:
        print(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
