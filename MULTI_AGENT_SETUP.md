# Multi-agent setup — legado-book-source

## Source of truth

`E:/shared-skills/legado-book-source/SKILL.md`

Keep agent copies byte-identical to the SOT (same pattern as `pe-task-runner`).

## Installed copies

| Agent | Path |
|-------|------|
| Claude Code | `~/.claude/skills/legado-book-source/SKILL.md` |
| Codex | `~/.codex/skills/legado-book-source/SKILL.md` (+ `agents/openai.yaml`) |
| Cursor | `~/.cursor/skills/legado-book-source/SKILL.md` |
| Hermes | `%LOCALAPPDATA%/hermes/skills/software-development/legado-book-source/SKILL.md` |
| Hermes (import mirror) | `%LOCALAPPDATA%/hermes/skills/agent-memory-imports/legado-book-source/SKILL.md` |

## Device MCP (`legado`)

- **SOT:** `config/mcp_defaults.json` (update when phone LAN IP changes)
- Header: `X-Legado-Token` (same as Web token)
- Repair workflow: skill `legado-book-source-repair` + `scripts/repair_source.py`

| Agent | Config entry |
|-------|----------------|
| Cursor | `~/.cursor/mcp.json` → `mcpServers.legado` |
| Codex | `~/.codex/config.toml` → `[mcp_servers.legado]` |
| Claude Code | `~/.claude.json` → `mcpServers.legado` (`type: http`) |
| Hermes | `%LOCALAPPDATA%/hermes/config.yaml` → `mcp_servers.legado` |

## Knowledge / debugger

- Repo: `E:/Projects/legadoSkill`
- Official app junction: `legadoSkill/legado` → `E:/Projects/legado`
- Local venv: `legadoSkill/.venv`

## Re-sync after SOT edits

```bash
SOT="E:/shared-skills/legado-book-source/SKILL.md"
cp "$SOT" "$HOME/.claude/skills/legado-book-source/SKILL.md"
cp "$SOT" "$HOME/.codex/skills/legado-book-source/SKILL.md"
cp "$SOT" "$HOME/.cursor/skills/legado-book-source/SKILL.md"
cp "$SOT" "$LOCALAPPDATA/hermes/skills/software-development/legado-book-source/SKILL.md"
cp "$SOT" "$LOCALAPPDATA/hermes/skills/agent-memory-imports/legado-book-source/SKILL.md"
```

When `audit-hooks sync` / `audit-hooks codex sync` is healthy again, prefer that for Claude/Cursor/Codex; Hermes still needs the manual copy above.
