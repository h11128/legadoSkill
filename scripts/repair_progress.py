#!/usr/bin/env python3
"""Repair progress + pick next deep-fix URL toward a fixed-count goal.

Examples:
  python scripts/repair_progress.py status --goal 100
  python scripts/repair_progress.py next
  python scripts/repair_progress.py next --why temp/full_fix/wave20_why.json

`next` is cheap (no multi-URL HTML probe). Prefer why.json, then harvest lost,
then tagged fails — first eligible URL only.
"""

from __future__ import annotations

import argparse
import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Any
from urllib.parse import urlparse

_ROOT = Path(__file__).resolve().parents[1]
LEDGER = _ROOT / "temp" / "full_fix" / "repair_session_ledger.jsonl"
PROGRESS = _ROOT / "temp" / "full_fix" / "repair_progress.json"
HARVEST_LAST = _ROOT / "temp" / "full_fix" / "harvest_last.json"
TAGGED_FAILS = _ROOT / "legado" / "temp_tagged_fails.json"

SKIP_BUCKETS = {
    "dead_404",
    "blocked_403",
    "auth_401",
    "legal_451",
}


def norm_url(url: str) -> str:
    return (url or "").strip()


def host_key(url: str) -> str:
    raw = norm_url(url).split("##")[0].split("#")[0]
    if raw and "://" not in raw:
        raw = "http://" + raw.lstrip("/")
    host = urlparse(raw).hostname or ""
    return host.lower().removeprefix("www.")


def ledger_fixed(path: Path = LEDGER) -> set[str]:
    urls: set[str] = set()
    if not path.is_file():
        return urls
    for line in path.read_text(encoding="utf-8").splitlines():
        try:
            row = json.loads(line)
        except json.JSONDecodeError:
            continue
        result = str(row.get("result") or "")
        ok = "校验成功" in result or result.startswith(("fixed:", "fixed "))
        if row.get("step") == "check" and ok and row.get("url"):
            urls.add(norm_url(str(row["url"])))
    return urls


CLOSED_PREFIXES = ("skip:", "repurposed:", "disable:", "fail:")
SOFT_MARKERS = ("no_patch", "搜索", "verify_fail", "校验失败")
HARD_PREFIXES = (
    "disable",
    "skip:l2",
    "skip:dead",
    "skip:wall",
    "skip:park",
    "skip:jieqi",
    "skip:biquge",
    "repurposed:",
)
HARD_MARKERS = ("search_empty", "search_index_empty", "http_dead", "domain_repurposed")


def is_attempt_closed(step: str, result: str) -> bool:
    """A finished attempt: skip / disable / repurposed / explicit give-up.

    ``fail:`` belongs here, otherwise a URL the agent gave up on is offered
    again by the next ``progress next``. Mirrors ``is_attempt_closed`` in
    ``crates/source-cli/src/cmds/progress.rs``.
    """
    if step == "skip" or result.startswith(CLOSED_PREFIXES):
        return True
    return step == "check" and result.startswith(("disable", "fail"))


def is_retryable_reason(reason: str) -> bool:
    soft = any(m in reason for m in SOFT_MARKERS)
    hard = reason.startswith(HARD_PREFIXES) or any(m in reason for m in HARD_MARKERS)
    return soft and not hard


def ledger_skipped(path: Path = LEDGER) -> set[str]:
    last: dict[str, str] = {}
    forced: set[str] = set()
    if not path.is_file():
        return set()
    for line in path.read_text(encoding="utf-8").splitlines():
        try:
            row = json.loads(line)
        except json.JSONDecodeError:
            continue
        url = norm_url(str(row.get("url") or ""))
        if not url:
            continue
        result = str(row.get("result") or "")
        # `final: true` is written by repair_retro for a terminal fail/skip and
        # is not downgraded by soft wording in the reason text.
        if row.get("final") is True:
            forced.add(url)
        if is_attempt_closed(str(row.get("step") or ""), result):
            last[url] = result
    return forced | {u for u, reason in last.items() if not is_retryable_reason(reason)}


def blocked_hosts(fixed: set[str], skipped: set[str]) -> set[str]:
    return {host_key(u) for u in fixed | skipped if host_key(u)}


def is_blocked(url: str, fixed: set[str], skipped: set[str], hosts: set[str]) -> bool:
    u = norm_url(url)
    if not u:
        return True
    if u in fixed or u in skipped:
        return True
    h = host_key(u)
    return bool(h and h in hosts)


def load_why(path: Path) -> list[dict[str, Any]]:
    if not path.is_file():
        return []
    data = json.loads(path.read_text(encoding="utf-8"))
    if isinstance(data, list):
        return [x for x in data if isinstance(x, dict)]
    for key in ("rows", "items", "results"):
        if isinstance(data.get(key), list):
            return [x for x in data[key] if isinstance(x, dict)]
    return []


def pick_next(why: list[dict[str, Any]], fixed: set[str], skipped: set[str]) -> dict[str, Any] | None:
    hosts = blocked_hosts(fixed, skipped)
    order = ("search_ok_need_deeper", "alive_search_zero")
    for bucket in order:
        for row in why:
            url = norm_url(str(row.get("url") or ""))
            if is_blocked(url, fixed, skipped, hosts):
                continue
            if row.get("bucket") != bucket:
                continue
            if row.get("bucket") in SKIP_BUCKETS:
                continue
            out = dict(row)
            out["url"] = url
            return out
    for row in why:
        url = norm_url(str(row.get("url") or ""))
        if is_blocked(url, fixed, skipped, hosts):
            continue
        if row.get("bucket") in SKIP_BUCKETS:
            continue
        out = dict(row)
        out["url"] = url
        return out
    return None


def pick_from_url_list(
    urls: list[str], fixed: set[str], skipped: set[str], *, source: str
) -> dict[str, Any] | None:
    hosts = blocked_hosts(fixed, skipped)
    for raw in urls:
        url = norm_url(raw)
        if is_blocked(url, fixed, skipped, hosts):
            continue
        return {"url": url, "bucket": source, "name": "", "source": source}
    return None


def fallback_urls() -> list[str]:
    out: list[str] = []
    if HARVEST_LAST.is_file():
        try:
            data = json.loads(HARVEST_LAST.read_text(encoding="utf-8"))
            for u in data.get("lost_sample") or []:
                out.append(str(u))
        except json.JSONDecodeError:
            pass
    if TAGGED_FAILS.is_file():
        try:
            data = json.loads(TAGGED_FAILS.read_text(encoding="utf-8"))
            rows = data if isinstance(data, list) else data.get("items") or data.get("fails") or []
            for row in rows[:80]:
                if isinstance(row, dict):
                    u = row.get("url") or row.get("bookSourceUrl")
                    if u:
                        out.append(str(u))
        except json.JSONDecodeError:
            pass
    return out


def cmd_status(args: argparse.Namespace) -> int:
    fixed = ledger_fixed()
    skipped = ledger_skipped()
    goal = args.goal
    payload = {
        "ts": datetime.now(timezone.utc).isoformat(),
        "goal": goal,
        "fixed_n": len(fixed),
        "skipped_n": len(skipped),
        "remaining": max(0, goal - len(fixed)),
        "fixed_urls": sorted(fixed),
        "skipped_urls": sorted(skipped),
    }
    PROGRESS.parent.mkdir(parents=True, exist_ok=True)
    PROGRESS.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({k: payload[k] for k in ("goal", "fixed_n", "skipped_n", "remaining")}, ensure_ascii=False, indent=2))
    print(f"wrote {PROGRESS}")
    return 0


def cmd_next(args: argparse.Namespace) -> int:
    """Pick one URL. L2-gate before return — skip walls/parked without diagnose."""
    from datetime import datetime, timezone

    from repair_closeout import ensure_ready_for_next
    from repair_prefilter import DEFAULT_RULES, classify_one, load_rules
    from repair_retro import DEFAULT as RETRO_PATH
    from repair_retro import append_retro
    from repair_session_log import DEFAULT as LEDGER_PATH
    from repair_session_log import append_row

    if ensure_ready_for_next() != 0:
        return 1

    fixed = ledger_fixed()
    skipped = ledger_skipped()
    why = load_why(Path(args.why))
    rules = load_rules(DEFAULT_RULES) if Path(str(DEFAULT_RULES)).is_file() else []
    max_try = max(1, int(args.l2_tries))
    rejected: list[dict[str, Any]] = []

    def auto_skip_retro(url: str, msg: str) -> None:
        append_retro(
            RETRO_PATH,
            {
                "url": url,
                "status": "skip",
                "msg": msg[:200],
                "trap": "known:l2_gate",
                "skill_fix": False,
                "harness": "",
                "script_fix": "",
            },
        )

    def candidates() -> list[dict[str, Any]]:
        out: list[dict[str, Any]] = []
        seen: set[str] = set()
        hosts = blocked_hosts(fixed, skipped)
        # Prefer respondTime-sorted queue (fast sites first)
        rt_path = _ROOT / "temp" / "full_fix" / "queues" / "repair_candidates_fast_latest.json"
        if rt_path.is_file():
            try:
                payload = json.loads(rt_path.read_text(encoding="utf-8"))
                for r in payload.get("items") or []:
                    if not isinstance(r, dict):
                        continue
                    url = norm_url(str(r.get("url") or ""))
                    if is_blocked(url, fixed, skipped, hosts) or url in seen:
                        continue
                    seen.add(url)
                    item = dict(r)
                    item["url"] = url
                    item["bucket"] = "respondTime_asc"
                    item["source"] = "rt_queue"
                    out.append(item)
                    if len(out) >= max_try * 3:
                        break
            except json.JSONDecodeError:
                pass
        for bucket in ("search_ok_need_deeper", "alive_search_zero", None):
            for r in why:
                url = norm_url(str(r.get("url") or ""))
                if is_blocked(url, fixed, skipped, hosts) or url in seen:
                    continue
                if bucket and r.get("bucket") != bucket:
                    continue
                if r.get("bucket") in SKIP_BUCKETS:
                    continue
                # why already knew password wall / parking title — skip without L2
                title = str(r.get("title") or "")
                final = str(r.get("final") or "")
                blob = f"{title}\n{final}".lower()
                if any(
                    x in blob
                    for x in ("请输入密码", "password", "urldance", "for sale", "域名出售")
                ):
                    append_row(
                        LEDGER_PATH,
                        {
                            "ts": datetime.now(timezone.utc).isoformat(),
                            "url": url,
                            "step": "skip",
                            "result": "why_title_wall_or_parked",
                            "note": title[:80],
                        },
                    )
                    auto_skip_retro(url, f"why_title_wall_or_parked: {title[:80]}")
                    skipped.add(url)
                    rejected.append({"url": url, "reason": "why_title_wall_or_parked"})
                    continue
                seen.add(url)
                item = dict(r)
                item["url"] = url
                out.append(item)
                if len(out) >= max_try * 2:
                    return out
        for u in fallback_urls():
            url = norm_url(u)
            if is_blocked(url, fixed, skipped, hosts) or url in seen:
                continue
            seen.add(url)
            out.append({"url": url, "bucket": "fallback_tagged_or_harvest", "source": "fallback"})
            if len(out) >= max_try * 2:
                break
        return out

    picked: dict[str, Any] | None = None
    for cand in candidates()[:max_try]:
        url = cand["url"]
        # cheap title wall already handled; L2 gate
        try:
            gate = classify_one(url, rules)
        except Exception as exc:  # noqa: BLE001
            gate = {"action": "skip", "reason": f"l2_error:{exc}"[:80]}
        act = gate.get("action")
        if act in ("disable", "skip"):
            append_row(
                LEDGER_PATH,
                {
                    "ts": datetime.now(timezone.utc).isoformat(),
                    "url": url,
                    "step": "skip",
                    "result": str(gate.get("reason") or act),
                    "note": "progress_next_l2_gate",
                },
            )
            auto_skip_retro(url, str(gate.get("reason") or act))
            skipped.add(url)
            rejected.append({"url": url, "reason": gate.get("reason"), "action": act})
            continue
        picked = dict(cand)
        picked["l2_gate"] = {
            "action": act,
            "reason": gate.get("reason"),
            "migrate_to": gate.get("migrate_to"),
        }
        break

    out = {
        "fixed_n": len(fixed),
        "skipped_n": len(skipped),
        "rejected_n": len(rejected),
        "rejected_sample": rejected[:5],
        "next": picked,
        "hint": (
            f"python scripts/repair_diagnose.py --url {picked['url']} --key 我的"
            if picked
            else "no live candidate after L2 gate — refresh why / tagged fails"
        ),
    }
    print(json.dumps(out, ensure_ascii=False, indent=2))
    return 0 if picked else 2


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    sub = ap.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("status")
    s.add_argument("--goal", type=int, default=100)
    s.set_defaults(func=cmd_status)
    n = sub.add_parser("next")
    n.add_argument("--why", default=str(_ROOT / "temp/full_fix/wave20_why.json"))
    n.add_argument(
        "--l2-tries",
        type=int,
        default=5,
        help="max candidates to L2-gate before giving up (default 5)",
    )
    n.set_defaults(func=cmd_next)
    args = ap.parse_args()
    return int(args.func(args))


if __name__ == "__main__":
    import os

    if os.environ.get("REPAIR_USE_PYTHON", "") != "1":
        from repair_closeout import ensure_ready_for_next
        from source_cli_shim import run_source_cli

        if ensure_ready_for_next() != 0:
            raise SystemExit(1)
        ap = argparse.ArgumentParser(description=__doc__)
        sub = ap.add_subparsers(dest="cmd", required=True)
        s = sub.add_parser("status")
        s.add_argument("--goal", type=int, default=100)
        n = sub.add_parser("next")
        n.add_argument("--why", default=str(_ROOT / "temp/full_fix/wave20_why.json"))
        n.add_argument("--l2-tries", type=int, default=5)
        args = ap.parse_args()
        if args.cmd == "status":
            raise SystemExit(run_source_cli(["progress", "--cmd", "status"]))
        if args.cmd == "next":
            raise SystemExit(run_source_cli(["progress", "--cmd", "next"]))
    raise SystemExit(main())
