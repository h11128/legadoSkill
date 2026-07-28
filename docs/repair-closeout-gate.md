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

### 1b. fail/skip 自动封口（2026-07-28）

**只有 CLI `repair_retro.py append --status fail|skip` 会封口**（内部 `append_retro(..., seal=True)`）。
`repair_serial` / `repair_progress` L2 auto-skip 这类自动循环用的是 `seal=False`：
它们说的 fail 是「这轮自动补丁没成」，应当保持可重试；agent 手动收尾说的 fail 是「我放弃了」。

CLI 收尾会往 ledger 补一行：

```json
{"url":"…","step":"check","result":"fail:<trap 或 msg>","note":"sealed by repair_retro","final":true}
```

原因：以前 retro 写 `status=fail`，但 ledger 最后一行可能还是 `check: ok`（diagnose 层的 ok），
`progress next` 只看 ledger，于是同一个源被反复挑出来。现在两边由一条命令一起写。

排队侧的判定（`crates/source-cli/src/cmds/progress.rs` + `scripts/repair_progress.py`，两处同语义）：

| ledger 行 | 结果 |
|---|---|
| `check` + `校验成功` / `fixed:` | 已修，不再挑 |
| `fail:` / `skip:` / `disable:` / `repurposed:` | 已收尾，不再挑 |
| 上面这些但含 `no_patch` / `搜索` / `verify_fail` / `校验失败` | 视为未做完，仍可重挑 |
| 任意行带 `"final": true` | **无条件**不再挑（措辞不能降级） |

`final` 就是给「我确实放弃了，别再给我了」用的，避免 msg 里带「校验失败」被当成软失败。

两边同一套用例：Rust 在 `progress.rs` 的 `mod tests`（`cargo test -p source_cli`），
Python 跑 `python scripts/repair_closeout_check.py selftest`。改任一侧都要两条都过。

已知差异（暂不改）：Rust `norm_url` 会去掉结尾 `/`，Python 只 `strip()`。
Python 侧靠 `blocked_hosts` 的 host 级屏蔽兜底，且 `progress next` 默认走 Rust。

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
