# Repair close-out gate

## Rust SOT (2026-07-28 cutover)

Core: `crates/source-closeout/`  
CLI: `source-cli closeout` · `source-cli retro`

| 根因 | 机制 |
|------|------|
| Improve 不可验证 | `retro append` gate + `closeout pending` |
| retro 可撒谎 | novel + no skill_fix → retro 拒绝 |
| 双份 SKILL | `skill_fix` → `closeout sync-skill` |
| progress 跳过收尾 | `progress next` 先跑 `closeout pending` |
| fail 重挑 | `retro append --status fail` seals `final:true` ledger |

## 命令

```bash
source-cli closeout status
source-cli closeout pending
source-cli closeout gate --trap SLUG --skill-fix
source-cli closeout sync-skill
source-cli retro append --url URL --status fixed --trap known:… --skill-fix false
```

## fail/skip 封口

`source-cli retro append --status fail|skip`（默认 `--seal`）写 ledger：

```json
{"url":"…","step":"check","result":"fail:…","final":true,"note":"sealed by source-cli retro"}
```

排队判定：`crates/source-cli/src/cmds/progress.rs`（Rust 唯一实现，无 Python 镜像）。

自测：`cargo test -p source_closeout` · `cargo test -p source_cli progress::tests`
