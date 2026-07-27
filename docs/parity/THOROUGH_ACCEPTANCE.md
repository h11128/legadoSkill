# Thorough functional parity — acceptance gate (SOT)

This supersedes soft “CLI/shim green” claims for §12 functional (口径 A).

## Hard gates (all required)

1. **Suites:** `python scripts/parity_selftest.py` → all suites green, including `rust-cli` and **`search-parity`**.
2. **Golden forms:** fixture HTML → expected `searchUrl` field name (e.g. biduju `keyword` not `searchkey`); diff=0.
3. **Live search-layer E2E (device MCP):** at least one URL with diagnose `layer=search` where:
   - `source-cli diagnose --url U` → `layer=search` (or fake_detail→search)
   - `source-cli repair --mode oneshot --url U` → device **校验成功** OR evidence-backed **skip** (`search_endpoint_dead` / wall) — never silent GenericForm-only fail claimed as parity
4. **Forbidden:** claiming functional parity with only `layer=ok` verify smoke (bengben-class).

## Soft / out of scope until §12.6

- PERF_BASELINE / cutover / delete Python bodies
- Full wave parallel engine rewrite

## Gap backlog owner

See `docs/parity/SEARCH_LAYER_GAPS.md` — P0 must be closed before any new “§12 functional green” row in `ACCEPTANCE_LOG.md`.
