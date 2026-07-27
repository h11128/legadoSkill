# Parity acceptance log

Record sign-off only when **thorough** gates pass (`docs/parity/THOROUGH_ACCEPTANCE.md`).

| date (UTC) | git sha | suites | operator | notes |
|------------|---------|--------|----------|-------|
| 2026-07-27 | _(wip)_ | search-parity + rust-cli expanding | agent | **Retracted soft §12 functional green.** Gap: live rank / score / dead-endpoint / JS API were incomplete. Hard gate: search-layer E2E + `THOROUGH_ACCEPTANCE.md`. Biduju later fixed via Rust live path (`keyword`+GBK+table) — evidence only, not full parity close. |
| 2026-07-27 | `87a431e` | 6/6 soft suites | agent | **SUPERSEDED** — CLI/shim inventory only; insufficient for thorough functional parity. |
| 2026-07-27 | e4d5d20 | 5/5 | agent | Phase A harness |

## Thorough gate (required)

See `THOROUGH_ACCEPTANCE.md`. Forbidden: claim green with only `layer=ok` smoke.

## Evidence (partial)

```
source-cli repair --url http://www.biduju.net
→ keyword+GBK+class.list@table → 校验成功 (after forms.rs field fix)
```

Gap backlog: `SEARCH_LAYER_GAPS.md`.
