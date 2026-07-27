# Parity acceptance log

Record §12.3 sign-off rows after `parity_selftest.py` suites pass.

| date (UTC) | git sha | suites passed | operator | notes |
|------------|---------|---------------|----------|-------|
| 2026-07-27 | `87a431e` | inventory,fixtures,cli-help,imports,schemas,**rust-cli** (6/6) | agent | **§12 functional (口径 A) sign-off.** Diagnose/probe/patch/migrate/hunt/progress/ledger in Rust; Python shims → `source-cli`. Live-smoke OK: `bengben` layer=ok→校验成功. **Broken-source diagnose accept:** `http://www.biduju.net` → `layer=search` `fake_detail=true`; device verify **校验失败:搜索失效** (diagnose layer matches). **Not** §12.6 perf cutover. |
| 2026-07-27 | _pending commit_ | inventory,fixtures,cli-help,imports,schemas (5/5) + spine registry oneshot | agent | Adapters wired into spine; `source-cli repair`/`repair-dry`; dual-path skill; **not a cutover** (no live-smoke/perf sign-off) |
| 2026-07-27 | e4d5d20 (wip tree) | inventory,fixtures,cli-help,imports,schemas (5/5) | agent | Phase A harness; matrix 44; Rust contracts/types/ports/gate-L0/db green; not a cutover |
| _pending_ | _pending_ | inventory,fixtures,cli-help,imports,schemas | _name_ | initial harness seeded |

## Live-smoke evidence (2026-07-27)

```
source-cli diagnose --url 'https://www.bengben.com#🎃' --key 我的
→ layer=ok, gate passed_l0_l1_l2

source-cli repair --mode oneshot --url 'https://www.bengben.com#🎃'
→ REPORT status=fixed message="diagnose layer=ok; device verify ok"
→ verify.success=true message=校验成功
```

## Broken-source diagnose accept (2026-07-27, after `87a431e`)

```
source-cli diagnose --url 'http://www.biduju.net' --key 我的
→ layer=search, fake_detail=true, gate passed_l0_l1_l2

python scripts/repair_source.py verify --url 'http://www.biduju.net' --keyword 我的
→ success=false, message=校验失败:搜索失效
```

Also same pattern: `http://www.yqk.net/` → search + fake_detail.

Forbidden: claim cutover / delete Python without §12.6 PERF_BASELINE.
