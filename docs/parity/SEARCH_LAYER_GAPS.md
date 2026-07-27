# Search-layer deep-repair gaps (Python ↔ Rust)

Status owner: update when closing a row. P0 blocks thorough §12 functional sign-off.

| ID | Priority | Behavior | Python | Rust target | Status |
|----|----------|----------|--------|-------------|--------|
| S1 | P0 | Live rank candidates (GET/POST fetch + score) | `rank_candidates` | `probe_search_live` + `rank_with_html` wired into `search_plan` | done |
| S2 | P0 | Form field from inputs (not hardcoded searchkey) | fixed in py | `forms.rs` keyword/q | done |
| S3 | P0 | GBK charset from meta | tips only | `append_charset_gbk` | done |
| S4 | P0 | bookList from result HTML | guess + score hints | `guess_booklist` + score hints | done |
| S5 | P0 | `search_endpoint_dead` skip on form 5xx | deep_loop skip | `SearchPlanOutcome::EndpointDead` | done |
| S6 | P0 | score≥2 / apply branches | deep_loop | `search_plan` live best | done |
| S7 | P1 | JS `data-api` search shell | `detect_js_search_api` | `js_api.rs` + search_plan | done (basic) |
| S8 | P1 | Fake-home penalty needs home_html | score_search_html | `score_search_html_with_home` | done |
| S9 | P1 | `apply_auto_patches` after probe | deep_loop | oneshot_live | done |
| S10 | P1 | diagnose embeds live probe tips | repair_diagnose | diagnose.rs | todo |
| S11 | P2 | forms_from_js | repair_search_probe | forms.rs | todo |
| S12 | P2 | scheme-less URL normalize | deep_loop | oneshot_live | todo |

Acceptance: `docs/parity/THOROUGH_ACCEPTANCE.md`.
