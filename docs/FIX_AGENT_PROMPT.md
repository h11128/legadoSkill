# Fix-agent prompt template (paste into Task tool)

Modes (user picks; default oneshot for deep):
- oneshot: fix ONE URL via checklist. Pick with `repair_progress.py next` only (NO multi-URL 海选).
- batch: N URLs streaming REPORT. `repair_deep_loop.py --mode batch ...`
  Do not AwaitShell the whole batch in chat.
Budget: pick≤15s (progress next L2-gates walls); diagnose+patch 2–3min; hard stop 5min.
Never phone-debug a site that L2 already marked wall/parked/DB-fail.
Oneshot: one deep attempt per user turn, then report (even if skip).

```
You fix Legado book source(s). Follow Deep-fix checklist in
legadoSkill/skills/legado-book-source-repair/SKILL.md.

Prefer Rust for gate/smoke when available: crates/target/debug/source-cli.exe
  gate --url URL | repair-dry --url URL [--html file]
Live seed-adapter oneshot: repair --url URL [--dry-run] [--no-prefetch]
Full deep-fix (diagnose→layer→branch) still Python until §12 cutover:
  repair_diagnose.py / repair_deep_loop.py --mode oneshot

0. Read config/mcp_defaults.json + book-source-repair-discipline.mdc
1. cd E:/Projects/legadoSkill && python scripts/repair_source.py channel  # must idle
2. DIAGNOSE FIRST (do not patch before this):
   Prefer gate smoke: source-cli gate --url URL
   Required for full deep-fix: python scripts/repair_diagnose.py --url URL --key 我的
   Use output.layer. If fake_detail=true → SEARCH (wmp8 trap), not toc.
   「搜索目录失效」is ambiguous — trust diagnose, not the Chinese alone.
3. BRANCH:
   - Full deep: python scripts/repair_deep_loop.py --mode oneshot --url URL
   - Optional seed-adapter live: source-cli repair --url URL (not a diagnose substitute)
   - search: apply probe.best / ranked[0] (score>0), not raw forms[0]; use bookList_hint / bookUrl_hint
     (假首页搜索 / xunsearch pid traps — see skill)
   - toc: only if real detail_url; tocUrl + ruleToc from HTML
   - content: ruleContent from chapter HTML
4. ONE device verify (checkDiscovery=false). Claim fixed only on 校验成功.
5. Ledger:
   python scripts/repair_session_log.py append --url URL --step patch|check --result '…'
6. Budget 2–3 min diagnose+patch; hard stop 5 min → skip + ledger.
7. NEVER rate-only as fixed. NEVER new inspect_*.py. NEVER two check/debug jobs.

Mode: <oneshot|batch>
URL: <BOOK_SOURCE_URL>
Fail message: <FAIL_MSG>
Keyword: 我的
```
