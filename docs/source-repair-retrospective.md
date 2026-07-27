# Source Repair Retrospective (2026-07-26)

## Verdict

**修好一个可救书源，正常应是 2–5 分钟，不是 15 分钟。**  
「15 分钟」只是硬停上限，被误当成合理工期。壁钟时间主要被 **过程浪费** 吃掉，不是选择器本身难。

破万卷 / 爱久久有效改动都很小；设备单源校验各约 **2–3 秒**。详见会话全记录：  
`docs/source-repair-session-log-2026-07-26.md` + `temp/full_fix/repair_session_index.json`。

---

## 1. Timeline waste (observed)

| Waste | What happened | Cost |
|-------|----------------|------|
| Subagent cold start | Each agent rediscovered MCP IP/session/fail semantics | 3–8 min / agent |
| Channel contention | `full_check_runner` / `batch_check_mcp` zombies + fix agents | queue / timeout / redo |
| Fake “fixed” | `save_source` logged as fixed without single-URL check | ijjj/pow full rework |
| Rate-limit misread | debug → immediate check →「搜索失效」→ rewrite rules | wasted loop + 20s |
| Over-exploration | TOC bug but wander search/content/explore | larger blast radius |
| Ad-hoc scripts | Dozens of `legado/.local-scripts/inspect_*.py` one-offs | no reuse |
| Soft 15-min budget | Ceiling treated as target | permission to thrash |
| **Infra underuse** | legadoSkill docs/debugger/KB barely used | reinvented triage |

---

## 2. What actually broke (technical)

1. **破万卷**：`tocUrl` → content page → no `.catalog`. Fix: clear `tocUrl`.  
2. **爱久久**：broad `a@href##regex##` → homepage; `name` mixed `||`+`##`; **20s** search gap.  
3. **book18**：pagination/`name` (verified earlier in `verify_fixed.json`).

Once the *resolved* toc URL was inspected, fixes were minutes of work — not quarter-hours.

---

## 3. Gaps in the first retro (不够彻底的地方)

Earlier retro listed symptoms but under-specified:

| Gap | Missing detail | Now |
|-----|----------------|-----|
| No session ledger | Outcomes only in chat / scattered jsonl | Session log + index JSON |
| Fake-fixed not named | Which agent, which retest | Section 3 of session log; `fake_fixed_then_reworked` |
| Unverified batch “fixes” | bengben/zxcs/627txt claimed in `fix_log.jsonl` never in verify_fixed | Marked **unverified claims** |
| MCP IP drift | Skill still advertised `.43` while phone was `.139` | Standing note; repair skill default `.139` |
| Parallel policy | “don’t parallel” said late | Ban: 0 fix agents while bulk owns channel |
| **legadoSkill infra** | Almost absent from first retro | **§4 below** |
| Knowledge vs invent | Agents guessed CSS without CSS/TOC docs | Mandate doc skim before deep edit |
| Debugger unused | Local `debugger/` never in P0 path | Optional pre-check; device still authoritative |
| Logging standard | fix_log vs fix_pow shapes differ | Prefer `repair_source.py log` schema |
| Skip quality | Batch skips OK; no disable-on-device always applied | Follow with `disable_dead_sources` when skip=dead |

---

## 4. Did we use legadoSkill infra? Mostly no

### Inventory vs actual use this session

| Infra | Path | Used in repair waves? | Should have |
|-------|------|------------------------|-------------|
| Essential knowledge | `docs/ESSENTIAL_KNOWLEDGE_SUMMARY.md` | Barely | First read for HTML authenticity / rule pitfalls |
| CSS selector notes | `assets/css选择器规则.txt` | No | Before rewriting selectors |
| TOC pagination rules | `docs/TOC_PAGINATION_RULES.md` | No | Any 目录失效 |
| HTML authenticity checklist | `docs/HTML_AUTHENTICITY_CHECKLIST.md` | Partial (raw dumps yes, checklist no) | Always |
| Local debugger | `debugger/test_universal.py`, `legado_checker.py` | No | Quick PC sanity after HTML theory |
| Example sources KB | `assets/knowledge_base/book_sources/` | No | Pattern match similar sites |
| Upstream mega-skill | `skills/SKILLV0.7.md` | Only at install | Repair skill supersedes for fix loops |
| Precheck / batch check | `scripts/precheck_*.py`, `batch_check_mcp.py` | Yes (bulk) | Keep for bulk only; pause before fix |
| Disable dead | `scripts/disable_dead_sources.py` | Partial | After skip=dead |
| Full check runner | `scripts/full_check_runner.py` | Yes — **also collided** | Lockfile + exclusive with fix |
| Repair CLI (new) | `scripts/repair_source.py` | After the fact | **Default path now** |
| Throwaway probes | `legado/.local-scripts/inspect_*.py` | **Heavy** | Prefer `repair_source.py fetch` |
| Past fix writeups | `docs/歌书网书源错误分析与修复.md` etc. | No | Search docs before inventing |

**Conclusion:** We treated legadoSkill as a **temp dump + MCP scratchpad**, not as the repair toolchain. That forced every subagent to re-derive session glue, HTML fetch, and rule folklore — the real reason wall time exploded.

### What “full utilization” looks like

```
1. repair_source.py triage          # fail layer + smells
2. Skim ESSENTIAL + TOC_PAGINATION if layer=toc/content
3. repair_source.py fetch           # headers + dump + toc candidates
4. Optional: debugger/test_universal on saved JSON (approx only)
5. Minimal save_source via MCP
6. repair_source.py verify --cooldown N
7. repair_source.py log → temp/full_fix/fix_*.json
8. Append one line to session index / fix_log.jsonl
```

Bulk path stays: `precheck` → **one** `full_check_runner` → classify materials → then **serial** repair with above loop.

---

## 5. Why it *felt* like 15 minutes

Waste path:

```
spawn → rediscover MCP → hand-roll session → blind debug → bare curl
→ save → claim fixed → parent retest fail → spawn again
→ rate-limit false search fail → sleep → verify
```

Effective path (with infra):

```
triage → fetch → 1–2 field edit → cooldown verify → log
```

---

## 6. Process bans

1. No “fixed” without `repair_source.py verify` (or equivalent single-URL check).  
2. No fix agent while bulk runner owns MCP.  
3. No rewriting searchUrl on rate-limit HTML.  
4. No `||` + `##` on the same field.  
5. No broad `a@href##…##` tocUrl without checking resolved URL.  
6. No treating 15 min as target (≤5 target, 10 hard stop).  
7. No new `inspect_*.py` for a one-off if `fetch` covers it.  
8. No ignoring legadoSkill docs on TOC/CSS failures.

---

## 7. Tooling now

| Script | Role |
|--------|------|
| `scripts/mcp_client.py` | MCP session + get_source |
| `scripts/repair_helpers.py` | layer / smells / headered fetch |
| `scripts/repair_source.py` | triage \| fetch \| verify \| log \| channel \| index |
| `scripts/mcp_channel.py` | Exclusive bulk↔repair lock |
| `scripts/repair_claim.py` | Anti fake-fixed + index append |
| `config/mcp_defaults.json` | MCP URL/token SOT |
| `docs/FIX_AGENT_PROMPT.md` | Subagent paste template |

Skill SOT: `E:/shared-skills/legado-book-source-repair/SKILL.md`

---

## 8. SLOs

| Metric | Target |
|--------|--------|
| Wall time / fixable source | **2–5 min** |
| Hard stop | **10 min** → skip + log |
| Device verify | Always |
| Parallel fix on one phone | **0** during verify/debug |
| Docs skim on toc/content fail | Required |
| Session ledger update | Required every verified/skip |

If another session burns 15+ minutes on a one-line tocUrl bug, failure mode is **process + infra neglect**, not the site.

---

## 9. Mitigations shipped (2026-07-26)

| Problem | Layer | Fix |
|---------|-------|-----|
| Fake `fixed` | Script | `log --status fixed` refuses without verify `success=true` |
| Channel contention | Script | `mcp_channel.py`; verify asserts idle; bulk runner acquires lock |
| Stale MCP IP | Config | `config/mcp_defaults.json` SOT |
| Ad-hoc inspect_* | Hook + MDC | beforeShell prompt; `book-source-repair-discipline.mdc`; audit warn |
| Subagent cold start | Doc | `docs/FIX_AGENT_PROMPT.md` |
| Infra underuse | Skill + MDC | Mandatory docs + repair CLI |
| 15 min as target | MDC + Skill | Target 2–5 / hard stop 10 |
| No ledger | Script | `log --index` + `index` subcommand |
| Parallel fix agents | MDC + prompt | Explicit ban |

