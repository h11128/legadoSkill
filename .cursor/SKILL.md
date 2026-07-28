---
name: legado-book-source
description: >-
  Create, debug, and iterate Legado (阅读) book sources with the local
  legadoSkill knowledge base and the device MCP at 10.0.0.43:1236.
  Use when writing 书源, book sources, Legado rules, CSS/@js selectors,
  debug_source, save_source, or when the user asks to make/fix a source
  for 阅读/legado.
version: 1.0.0
license: MIT
metadata:
  hermes:
    tags: [legado, book-source, 书源, mcp, reading-app]
    related_skills: []
---

<!-- SOT: E:\shared-skills\legado-book-source\SKILL.md
     Distributed to:
       - Claude Code: ~/.claude/skills/legado-book-source/
       - Codex: ~/.codex/skills/legado-book-source/
       - Cursor: ~/.cursor/skills/legado-book-source/
       - Hermes: %LOCALAPPDATA%\hermes\skills\software-development\legado-book-source/
     Keep agent copies identical to this file (manual sync or audit-hooks when available).
-->

# Legado Book Source

Write and verify Legado book sources against the **real app** via MCP.
Local Python debugger is only approximate; official Kotlin source and
device `debug_source` are authoritative.

Works on Claude Code, Codex, Cursor, and Hermes. Prefer the MCP tool
surface named `legado` (server may appear as `legado` / `user-legado`).

## Paths

| Role | Path |
|------|------|
| Knowledge repo | `E:/Projects/legadoSkill` |
| Official app source | `E:/Projects/legado` (junction: `legadoSkill/legado`) |
| Upstream Trae mega-skill | `legadoSkill/skills/SKILLV0.7.md` |
| Essential knowledge | `legadoSkill/docs/ESSENTIAL_KNOWLEDGE_SUMMARY.md` |
| Self-check notes | `legadoSkill/assets/智能体自我认知.md` |
| CSS rules | `legadoSkill/assets/css选择器规则.txt` |
| Example sources | `legadoSkill/assets/knowledge_base/book_sources/` |
| Local debugger | `legadoSkill/debugger/test_universal.py` |
| Local venv | `legadoSkill/.venv` |

## Device MCP (`legado`)

- URL: `http://10.0.0.43:1236/mcp`
- Header: `X-Legado-Token: 1234`
- Web UI (separate): `http://10.0.0.43:1122`

Config locations (already set when this skill was installed):

| Agent | Config |
|-------|--------|
| Cursor | `~/.cursor/mcp.json` → `mcpServers.legado` |
| Codex | `~/.codex/config.toml` → `[mcp_servers.legado]` |
| Claude Code | `~/.claude.json` → `mcpServers.legado` (`type: http`) |
| Hermes | `%LOCALAPPDATA%/hermes/config.yaml` → `mcp_servers.legado` |

### Tools

| Tool | Use |
|------|-----|
| `list_sources` | Paginated summaries (`search`, `enabledOnly`, `offset`, `limit`; default page 100, max 500) |
| `get_source` | Read full JSON by `bookSourceUrl` |
| `save_source` | Write JS/JSON; optional `preserveEnabled`/`preserveGroup` (default true) |
| `debug_source` | Single-flight step debug (`url` + `key`); not for bulk |
| `start_check_sources` | Start multi-thread batch check (App 校验书源 logic) |
| `get_check_progress` | Poll batch check progress + paged results |
| `stop_check_sources` | Cancel batch check |
| `delete_sources` | Delete by URL list |
| `set_http_log_recording` | Toggle HTTP log capture |
| `get_http_logs` / `get_http_log` | Inspect redacted request logs |

`debug_source` is single-flight. For bulk validation use `start_check_sources` then `get_check_progress`.
Prefer device MCP over local Python sim.

### Bulk check on PC (precheck + batched MCP)

Do **not** dump thousands of sources at `threadCount=100` in one MCP call.
Phone heap is limited; PC should filter and page:

1. Export / list `bookSourceUrl`s (`list_sources` pages, or local URL file).
2. DNS precheck on PC:
   ```
   E:/Projects/legadoSkill/.venv/Scripts/python.exe scripts/precheck_sources.py \
     --urls-file urls.txt --concurrency 200 --out temp/precheck.json
   ```
3. Batch authoritative App check (50–100 URLs per call, wait until idle):
   ```
   E:/Projects/legadoSkill/.venv/Scripts/python.exe scripts/batch_check_mcp.py \
     --mcp http://10.0.0.43:1236/mcp --token 1234 \
     --precheck-json temp/precheck.json --batch-size 80 --thread-count 64 \
     --keyword 我的 --out temp/batch_check_report.json
   ```
4. Or drive the same flow via agent MCP tools (`start_check_sources` /
   `get_check_progress`) if the script’s HTTP transport does not match.

Research (why not extract JVM engine): `docs/PC_CHECK_ENGINE_RESEARCH.md`
and `E:/Projects/legado/docs/pc-check-engine-research.md`.

### Agent call notes

- **Cursor**: discover server (often `user-legado`), then call tools.
- **Claude / Codex / Hermes**: use whatever MCP invoke API the host exposes for server `legado`.
- If tools are missing: reload MCP / restart agent; confirm phone service on `:1236`.
- After updating app MCP tools, rebuild/reinstall the app and restart MCP service.
## Workflow (3 phases)

```
- [ ] Phase 1: gather (no save yet)
- [ ] Phase 2: draft rules from real HTML
- [ ] Phase 3: save + device debug + fix loop
```

### Phase 1 — Gather (do not save)

1. Read `docs/ESSENTIAL_KNOWLEDGE_SUMMARY.md`; skim CSS rules and similar
   sources under `assets/knowledge_base/book_sources/` or
   `assets/book_source_database/`.
2. Detect site charset (response header / meta / probe fetch).
3. Fetch **raw HTTP HTML** (not DevTools DOM). Save under
   `legadoSkill/temp/` if useful. Use a browser tool only when the site
   needs JS/WebView.
4. Note search URL shape (GET vs POST), list/detail/toc/content URLs.

### Phase 2 — Draft

1. Build selectors from **raw HTML** only.
2. Prefer CSS short form (`.name@text`, `#id@text`, `.a.b@href`).
3. Handle lazy cover (`data-src` / `@data-src`), merged info fields,
   pagination (`nextTocUrl` / `nextContentUrl`).
4. For JS rules: Rhino; prefer `var`; use `java.*` helpers. See
   `legadoSkill/assets/方法-JS扩展类.md` when needed.
5. When unsure, read official Kotlin under
   `legado/app/src/main/java/io/legado/app/`.

### Phase 3 — Save and verify on device

1. `save_source` with `format: "json"` (declarative) or `"js"` (script).
2. `debug_source` with a real search keyword, then detail/toc/content keys.
3. On failure: `set_http_log_recording(true)`, re-debug, read
   `get_http_logs` / `get_http_log`, fix, `save_source` again.
4. Optional local sim (hint only), from `legadoSkill`:
   `./.venv/Scripts/python.exe debugger/test_universal.py`

## Hard rules

- Real HTML > browser rendered DOM.
- Official Kotlin in `E:/Projects/legado` > Python debugger.
- Device MCP debug > local simulation.
- Do not invent unsupported BookSource fields; mirror working examples.
- Keep tokens out of committed book-source JSON; MCP auth lives in agent configs.
- After meaningful book-source work, optionally write L3 memory via
  `audit-hooks l3 write "..."` (see `l3-memory-client` skill).

## When MCP is missing

Fallback: write JSON under `legadoSkill/temp/` and tell the user to import
via Web `:1122` or the app UI.

## Extra reference

- Upstream Trae skill (long, custom tools): `skills/SKILLV0.7.md`
- Architecture: `docs/PROJECT_ARCHITECTURE.md`
- Charset / POST encoding: `docs/MCP编码使用指南.md`
- Local debugger: `docs/LEGADO_DEBUGGER.md`
- Install notes: `E:/Projects/legadoSkill/MULTI_AGENT_SETUP.md`
