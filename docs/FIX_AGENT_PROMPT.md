# Fix-agent prompt template (paste into Task tool)

Use this verbatim when spawning a book-source repair subagent.

```
You fix ONE Legado book source on device MCP. Hard rules:

1. Read E:/Projects/legadoSkill/config/mcp_defaults.json for mcp_url + token (do not invent IP).
2. Read skill legado-book-source-repair and .cursor/rules/book-source-repair-discipline.mdc.
3. Before any MCP check/debug: `python scripts/repair_source.py channel` from E:/Projects/legadoSkill — must be idle. If bulk holds lock, STOP.
4. Workflow only:
   repair_source.py triage --url URL --fail-msg '...'
   → skim docs/ESSENTIAL_KNOWLEDGE_SUMMARY.md + docs/TOC_PAGINATION_RULES.md if layer is toc/content
   → repair_source.py fetch --url URL --page ...
   → minimal save_source (preserveEnabled/group true unless clearing fail tags)
   → repair_source.py verify --url URL --keyword 我的 --cooldown 20 --out temp/full_fix/verify_<id>.json
   → repair_source.py log --status fixed|skipped|failed --check-json <verify> --out temp/full_fix/fix_<id>.json
5. NEVER claim fixed without verify success=true. log --status fixed REFUSES without check-json.
6. NEVER create legado/.local-scripts/inspect_*.py; use fetch.
7. Budget: 2–5 min target, 10 min hard stop → skipped/failed + reason.
8. If HTML shows 搜索时间间隔/请稍后再搜索: cooldown, do not rewrite searchUrl.
9. One source only. Do not start_check_sources on other URLs. Do not start full_check_runner.

URL: <BOOK_SOURCE_URL>
Fail message: <FAIL_MSG>
Keyword: 我的
```
