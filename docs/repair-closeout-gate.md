# Repair close-out gate

## 五个根因 — 结构性修复（2026-07-27）

| 根因 | 状态 | 机制 |
|------|------|------|
| 1. Improve 不可验证 | **已结构** | `repair_retro append` 写入时跑 gate；`pending` 读 ledger+retro 复核 |
| 2. retro 可撒谎 | **已结构** | novel + `skill_fix=0` → retro 拒绝写入 + `progress next` exit 3 |
| 3. Rust 被当选修 | **按设计** | 仅 novel 且 diagnose 会误导时才改 harness；gate 不强制 Rust |
| 4. 双份 SKILL | **已结构** | `skill_fix=1` 时 `sync_skill_to_cursor()`；CLI `sync-skill` |
| 5. 吞吐 skip improve | **已结构** | `progress next`（Python + source-cli）先跑 `pending`；L2 auto-skip 自动 retro |

核心模块：`scripts/repair_closeout.py`  
CLI：`scripts/repair_closeout_check.py`（`gate` | `pending` | `sync-skill` | `status`）

---

## 原则（用户 standing）

> **没有可沉淀的新知识 → 不要改 skill，也不要改 Rust/Python。**

| 情况 | skill / harness |
|------|-----------------|
| Trap 已在 SKILL Traps 表（或 `known:…`） | `skill_fix=0`，**禁止**为改而改 |
| 新 trap（SKILL 里没有） | 先写 SKILL 一行，再 `skill_fix=1` → retro 自动 sync Cursor 副本 |
| 新 trap 且 diagnose/probe 会反复误导 | **才**改 `diagnose_tips.rs` 等 |
| 纯书源 selector 补丁 | 只改书源 + retro，`script_fix` 写改了啥 |

---

## 自动流程

### 1. `repair_retro.py append`

- 有 `--trap` → 立即 gate；失败则**不写** retro
- `--skill-fix 1` → 通过后复制 `skills/…/SKILL.md` → `~/.cursor/skills/…`

### 2. `progress next`（Python 或 source-cli）

- 先 `repair_closeout_check.py pending`
- 上一条 ledger（`check`/`skip`）必须有同 URL 的 retro
- retro 有 trap → 再跑 gate
- **blocked** → exit 1（Python）/ exit 3（Rust），不会 pick 下一 URL

### 3. L2 自动 skip（Python `repair_progress next`）

- 写 ledger skip 同时 auto-retro：`trap=known:l2_gate`, `skill_fix=0`

---

## 手动命令

```bash
python scripts/repair_closeout_check.py status    # JSON：pending + skill 指纹
python scripts/repair_closeout_check.py pending   # 模拟 progress next 闸门
python scripts/repair_closeout_check.py gate --trap SLUG --skill-fix 0|1
python scripts/repair_closeout_check.py sync-skill
```

---

## 推荐 close-out（每 URL）

```
1. ledger + retro（trap 用 SKILL 已有名或 novel slug）
2. 仅 novel → 补 SKILL Traps 一行 → retro --skill-fix 1（自动 sync）
3. 仅 harness 确有缺口 → 改 diagnose_tips / gate / probe
4. progress next（自动 pending）
```

## Agent 自检

- 这个 trap SKILL 里有了吗？有 → **不要**改 skill/Rust
- 是新模式吗？是 → SKILL；diagnose 会再误导吗？是 → 才改 Rust
- 跑 `pending` 能通过吗？
