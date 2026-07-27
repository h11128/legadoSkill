---
name: legado-book-source-repair
description: >-
  Repair failing Legado (阅读) book sources after check/debug failures.
  Use when fixing 书源, 校验失败, 搜索失效, 目录失效, 正文失效, tocUrl bugs,
  or iterating save_source → start_check_sources on one URL.
---

# Legado Book Source Repair

Narrow skill for **fixing existing sources**. Device MCP is authoritative.
Also enforce `.cursor/rules/book-source-repair-discipline.mdc`.

| Doc | Path |
|-----|------|
| Retro | `E:/Projects/legadoSkill/docs/source-repair-retrospective.md` |
| Session ledger | `…/docs/source-repair-session-log-2026-07-26.md` |
| Fix-agent prompt | `E:/Projects/legadoSkill/docs/FIX_AGENT_PROMPT.md` |
| MCP defaults | `E:/Projects/legadoSkill/config/mcp_defaults.json` |

## Enforcement layers (do not bypass)

| Layer | Mechanism |
|-------|-----------|
| MDC | `book-source-repair-discipline.mdc` alwaysApply |
| Script | `log --status fixed` **requires** verify `--check-json` with `success=true` |
| Channel | `mcp_channel.py` — verify refuses if bulk lock held; `repair_source.py channel` |
| Hooks | beforeShell deny/ask on `inspect_*.py` and bulk runners |
| Prompt | Paste `FIX_AGENT_PROMPT.md` into every fix Task |

## Defaults

Read MCP from `config/mcp_defaults.json` (scripts do this automatically). Do not hardcode LAN IPs.

| Item | Value |
|------|--------|
| Verify | one URL, `threadCount=1` |
| Save | `preserveEnabled=true`, `preserveGroup=true` unless clearing fail tags |
| PC | `E:/Projects/legadoSkill/.venv` |

## Required CLI

```bash
cd E:/Projects/legadoSkill
python scripts/repair_source.py channel   # must idle before fix/verify
python scripts/repair_source.py triage --url URL --fail-msg '...'
python scripts/repair_source.py fetch --url URL --page PAGE
# save_source via MCP (minimal fields)
python scripts/repair_source.py verify --url URL --keyword 我的 --cooldown 20 \
  --out temp/full_fix/verify_x.json
python scripts/repair_source.py log --url URL --status fixed \
  --check-json temp/full_fix/verify_x.json \
  --out temp/full_fix/fix_x.json
```

Subcommands: `triage` | `fetch` | `verify` | `log` | `channel` | `index`.

## Infra before guessing selectors

| Doc / tool | When |
|------------|------|
| `docs/ESSENTIAL_KNOWLEDGE_SUMMARY.md` | Any serious rule edit |
| `docs/TOC_PAGINATION_RULES.md` | 目录失效 |
| `docs/HTML_AUTHENTICITY_CHECKLIST.md` | Before trusting browser DOM |
| `assets/css选择器规则.txt` | CSS rules |
| `debugger/test_universal.py` | Optional PC dry-run only |

## Time budget

**Target 2–5 min. Hard stop 10 min → skip/fail + log.**  
Do not treat 15 min as normal.

## Critical patterns

- **A** tocUrl → content page → clear or retarget catalog  
- **B** broad `a@href##…##` → may resolve homepage  
- **C** never mix `||` and `##` on same field  
- Rate-limit HTML → cooldown, do not rewrite searchUrl  

## Done criteria

- `verify` → `success=true` + `log --status fixed`, **or**
- `log --status skipped|failed` with reason — never “saved only”
