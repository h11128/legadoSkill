---
name: legado-book-source-repair
description: >-
  Repair failing Legado (阅读) book sources after check/debug failures.
  Use when fixing 书源, 校验失败, 搜索失效, 目录失效, 正文失效, tocUrl bugs,
  or iterating save_source → start_check_sources on one URL.
---

# Legado Book Source Repair

Device MCP is authoritative. Enforce `.cursor/rules/book-source-repair-discipline.mdc`.

**Agents must follow the Deep-fix checklist in order.** After each fix/skip, refine
skill/scripts if a new trap appeared, then continue the goal loop.
Track: `python scripts/repair_progress.py status --goal 100`.

| Doc | Path |
|-----|------|
| MCP defaults (SOT) | `E:/Projects/legadoSkill/config/mcp_defaults.json` |
| MCP discover (auto) | `python scripts/mcp_discover.py` — also syncs `~/.cursor/mcp.json` |
| Pipeline (layer) | `docs/repair-pipeline-design.md` |
| Fix-agent prompt | `docs/FIX_AGENT_PROMPT.md` |
| Platform (Rust) | `docs/repair-adapter-architecture.md` — **§12 functional green**; default `source-cli`; Python shims OK; §12.6 perf cutover pending |

**Entry preference:** Default to **`source-cli`** (`diagnose` → `repair --mode oneshot`). Script names (`repair_diagnose.py`, `repair_deep_loop.py`, …) are thin shims unless `REPAIR_USE_PYTHON=1`. Orchestration (`repair_wave`, harvest, parity) stays Python.

**Do not hard-code phone IPs in prompts or skill text.** Scripts call `ensure_session`,
which rediscovers on connect failure and updates SOT. Agents must not ask the user to
“手动改 IP / 手动 rediscover” unless Cursor IDE MCP is still stale after discover
(then: reload MCP / restart agent once).

## Goal loop (toward 100)

```
while fixed_n < 100:
  A) repair_refresh_phone_index.py   # LIVE phone URLs (anti stale tagged_fails)
  B) repair_rt_queue.py --limit 100  # respondTime asc, on_phone + 搜索* only, 1/host
  C) repair_serial.py --limit 100    # L2 → require_patch → verify；每源 retro
  D) new trap? → update skill+scripts BEFORE next batch
```

**Serial efficiency rules (mandatory):**
1. Queue only from **phone index** (not stale `temp_tagged_fails` alone).
2. **No empty-probe verify** — `require_patch=True`；探针无补丁 → `no_patch_skip`（不烧设备校验）。
3. **One host one candidate** in queue; scheme-less URLs normalized; `host_key` works without `http://`.
4. migrate_to must pass L2; `search_endpoint_dead` → skip.
5. After each URL: `repair_retro` + ledger；新 trap 立刻写 skill。

**Anti-pattern (banned):** classify/probe 20–50 tagged fails to “find a good one”.
That burned minutes and violated the 2–3 min budget. Pick → diagnose → patch → verify.

## Report modes (both supported)

| Mode | When | Command / agent behavior |
|------|------|---------------------------|
| **oneshot** 修一个报一个 | 默认深修、用户要盯进度 | `source-cli diagnose` → `source-cli repair --mode oneshot --url URL`（shim: `repair_diagnose.py` / `repair_deep_loop.py --mode oneshot`）。必须走 layer；假详情不修 toc |
| **batch** 批量 | 用户明确说「批量 / 做 N 个」 | `source-cli repair --mode batch --urls-file … --limit N`（shim: `repair_deep_loop.py --mode batch`）；勿整批 AwaitShell |

## Deep-fix checklist (one URL)

Budget clock starts at **pick**. Diagnose+patch **2–3 min**; hard stop **5 min**.

```
[ ] 0  channel idle
[ ] 1  progress next  (script L2-gates walls/parked; ≤~20s)
[ ] 2  if next.l2_gate.action=migrate → migrate first
[ ] 3  diagnose --url URL   # also L2-failfast BEFORE phone debug
[ ] 4  if layer=skip → ledger already done → close-out (§ below) → **立刻汇报**
[ ] 5  else patch ONLY layer → ONE verify → ledger
[ ] 6  close-out: ledger → retro（自动 gate/sync）→ progress next（自动 pending）
```

## Per-URL close-out (mandatory)

User standing preference (this repo): **every** oneshot (fixed / skip / fail) must:

1. **Document** — append ledger; add a short entry to `docs/source-repair-retrospective.md`
   (or a dated `docs/source-repair-retro-*.md` when the note is long).
2. **Reflect** — `python scripts/repair_retro.py append --url … --status … --trap … --harness …`
   (`--script-fix` / `--skill-fix 1` when you actually change code/skill).
3. **Improve（仅 novel trap / harness 缺口）** — **没有新知识点就不要改 skill 或 Rust。**
   已有 trap 在 SKILL Traps 表 → `skill_fix=0` 即可。见 `docs/repair-closeout-gate.md`。

### Close-out 决策（简化）

```
retro --trap 指向 SKILL 已有行？ 或 known:… ？
├─ YES → skill_fix=0，不改 skill/Rust（除非 diagnose 仍系统性误导 → 可选改 harness）
└─ NO（novel）→ SKILL 加一行 → skill_fix=1 → gate 通过后再 next
     Rust/Python：仅当「下次 diagnose 还会走错」才改；纯 selector 补丁不必动
```

```bash
# 有 --trap 时可选核对（novel + skill_fix=0 会失败）
python scripts/repair_closeout_check.py gate --trap SLUG --skill-fix 0|1
```

**Why 3pxs / kptxt / utexs burned time (2026-07-26):**

| URL | Real state | Waste | Now |
|-----|------------|-------|-----|
| 3pxs.xyz | why title=请输入密码 / urldance | full diagnose+rank | why title skip + L2 `wall:` |
| kptxt | intermittent「连接数据库失败」 | multi retry+patch | L2/search body `wall:连接数据库失败` → skip |
| utexs | parking `this domain` | phone debug + rank | `progress next` L2 gate before diagnose |

**Oneshot rule:** 一次用户回合只深修 **一个** live 候选。`progress next` 可自动 skip 死站，但 agent 不得在未汇报的情况下连续开 3 次 diagnose。

```bash
cd E:/Projects/legadoSkill
python scripts/repair_source.py channel
# progress next (shim → source-cli progress):
python scripts/repair_progress.py next
# Build once:
#   (cd crates && cargo build -p source_cli)
crates/target/debug/source-cli.exe diagnose --url URL --key 我的
crates/target/debug/source-cli.exe repair --mode oneshot --url URL
# Do NOT pass --l0-only on progress/diagnose/repair for live fix (skips L2 dead/wall).
# Or shim entrypoints (same flags):
python scripts/repair_diagnose.py --url URL --key 我的
python scripts/repair_deep_loop.py --mode oneshot --url URL
```

## Traps

| Trap | Signal | Action |
|------|--------|--------|
| 假详情 (wmp8) | list-empty + books≤1 + `/s.php` | **search** |
| 真 TOC (画本) | search≥2 + 目录空 + real detail | tocUrl/ruleToc |
| 假「假详情」 | search≥2 but log shows search URL first | still toc/content |
| 空 tocUrl + JSON (长佩) | `$.data.list` + empty tocUrl | chapter API tocUrl |
| webView 单引号 (长佩) | `{'webView': true}` | `{"webView":true}` |
| debug=ok / tagged fail | flake or 发现-only | harvest/verify; don't over-patch |
| API 目录要登录 | 认证失败 / device 必填 | **skip** |
| 验证码搜索 | getcode / yzm / actyzm | **skip** |
| 域名停车/过期 | L2 GET 正文含 for sale/出售/域名到期 / Redirecting shell | **disable/skip**（勿当搜索规则坏） |
| **没有找到站点 (521danmei)** | title=`没有找到站点` / 空壳 | L2 `deadish:没有找到站点` → **skip** |
| **nginx 空站 (cstxt)** | title=`Welcome to nginx!` | L2 `deadish:welcome to nginx` → **skip** |
| **域名广告劫持 (pyzht)** | title=精选推荐 / `gg_card` / 18+广告壳 | L2 deadish 广告标记 → **skip** |
| **Empire 搜索体 (fuxsb)** | debug 有书但 check「搜索失效」；`show=a,b,c` 体 | 简化 `keyboard={{key}}&show=title&tempid=1` + Referer；正文 `.co-by`→`.conbd` |
| **webView 在 bookUrl** | `##$##,{'webView': true}` | `apply_safe_rule_fixes` 现修 ruleSearch.bookUrl（不仅 chapterUrl） |
| 主机跳转 | bookSourceUrl host ≠ final host（如 .org→.com） | **migrate** 再修搜索 |
| URL 前导空格 | `get_source` 失败但 list 能见到 | trim `bookSourceUrl` 再 get/migrate |
| bookbenx 换域 | `.item` + `/search81.html?searchkey=`（新书迷楼→shukuai99） | 固定 searchUrl，勿依赖 ajax 抽 form |
| 假首页搜索 (爱丽丝) | form=`/?keyword=` 但结果=首页壳；真入口 `/search.php?q=` | **继续修** — rank；换真 searchUrl |
| **搜索口不对** | 首页有 form，但书源 searchUrl/common_path 404/错页 | **继续修** — 先试 form 的 action（POST/GET），勿 skip |
| **搜索口挂了** | form 指向的真实入口稳定 HTTP 5xx /「连接数据库失败」 | **skip** — 非规则问题 |
| xunsearch pid 链 | `javascript:…pid: N` 无真实 href | bookUrl=`##pid:(\\d+)##/novel/$1.html###` |
| EmpireCMS keyboard | form field=`keyboard`（`/e/sch/`） | searchUrl 含 `keyboard={{key}}`；探针已收录 |
| POST 搜索未打分 | 旧 rank 跳过带 JSON 的 POST | rank 现会 POST 试抓 |
| common_path 误伤 | `/search?q=` 得虚高分压过 form POST（ixs7） | form 优先；error_page 扣分；5xx→`search_endpoint_dead` |
| 候选海选 | 多 URL classify+probe 选“好修的” | **banned** — `progress next` 只取一个 |
| 站点 DB 挂了 | 正文/搜索页仅「连接数据库失败」 | **skip**（同「搜索口挂了」） |
| 密码墙 / urldance | title=请输入密码；跳转 urldance.com | **skip**（L2 `wall:`；勿 diagnose） |
| L2 未过就 debug | phone debug + rank 打在死站上 | diagnose/`progress next` 先 L2 fail-fast |
| **`--l0-only` 误用 (dcrsu)** | progress/diagnose 带 `--l0-only` → 超时站仍进 tips/probe（~37s） | **禁止** live 挑源/深修用 `--l0-only` |
| **jieqi 搜索 0 条 (b483)** | POST/m「共有 0 条」；浏览仍有书 | **disable**。禁止首页过滤假搜索。引擎 `site:` 思路延期：`docs/engine-site-search-deferred.md` |
| rate-only | only concurrentRate | not a fix (unless verify already OK) |
| URL 无 scheme | bookSourceUrl/searchUrl=`www.foo.com` | 自动补 `http://` 并 save；get_source 试去 `#` 变体；probe 限时 5s×6 |
| 空探针仍设备校验 | notes 空 + 搜索失效（浪费 ~10s×N） | serial `require_patch`；无补丁 → `no_patch_skip` |
| 过期 tagged_fails | missing「未找到书源」 | `repair_refresh_phone_index` + 队列只取 on_phone |
| **bookUrl class-space (po18f)** | search 有书名但详情链接=search.php；`class.X a@href` | → `class.X@tag.a@href`；去掉 `\|\|@js:baseUrl`；章节在详情页则清空 tocUrl |
| **登录壳首页 (96biquge)** | 首页仅 `#loginform`+密码框、无小说搜索 | L2 `wall:login_shell_not_novel` → **skip** |
| **charset 误标 (52dmshu)** | searchUrl `,{"charset":"gbk"}` 但站点已 UTF-8 → 列表空 | 去掉 gbk / 改 utf-8；重抓结果 DOM（常为 `#sitembox dl`，bookUrl=`dt a@href`） |
| **probe 未解压 gzip** | Accept-Encoding 有 gzip 但 body 不解压 → forms=[] | `fetch_text`/`_post_fetch` 必须 gzip 解压后再 parse |
| **form 体过长** | `<form>…</form>` >800 字符被截断漏抓（52dmshu=807） | forms regex 上限调到 4000 |
| **CF 空搜索体 (qiufeng)** | debug「获取成功」但 `a`/`p` 列表亦为 0；PC POST→403 Just a moment | **skip** — WAF，非选择器 |
| **正文 textNodes 空 (biduju)** | debug 到正文步 `ContentEmptyException`；章节有 `<br/>`/`font` | `class.chapter@html`（勿死磕 textNodes） |
| **域名改行 (jinyongwang)** | title=「…专业生产厂家」；搜索 placeholder=查询的产品 | L2 deadish → **skip/disable** |
| **forms_js UTF-8 panic** | diagnose tips 拉首页时 byte 切片切到汉字中间 | `utf8_window`（char boundary） |
| **progress next 卡死** | 候选按 URL 字母序 → 永远先 `api.*`；index 无 RT | 优先 `queues/repair_serial100_queue.json` `items` |
| **App JSON 搜索空壳 (ihuaben)** | `/app/search`→`{}`；L2 首页仍小说站；listv2/CDN 可能仍活 | 试 `so.` / 站内 HTML 搜索；**勿**因 API 空就 disable；TOC/正文可继续走 CDN JSON |
| **重复 phone pull (serial)** | 每批 `refresh_phone_index` 全量 list_sources ~55s | 用 `repair_state.sqlite` + TTL；`repair_refresh_phone_index --force` 才重拉；`get_source` 走 snapshot cache |
| **Vue SSR 搜索空 (qimao miao)** | `/search/index/` 200 但无 `ul.qm-pic-txt`；`__NUXT__` 壳；phone list=0 | api-miao 无公开 search 端点 → **disable**（browse/shuku OK，§16） |
| **tocUrl 阅读链 (powanjuan)** | `tocUrl span.read a`→首章；误走 `index/1.html` 目录空 | **清空 tocUrl**；详情页 `div.catalog` + 已有 `ruleToc` |
| **COS toc 403 (tybook)** | `chapters/{bid}.json` 403 | 改 signed `/tf/chapter_list?` @js |
| **目录 href 伪装 (gaysay)** | 全部 `<a href="/book/id/">`；真 URL 在 `data-c8dcb4a` base64 | `chapterUrl` `@js:java.base64Decode(result.attr('data-c8dcb4a'))`；`chapterName` `@data-cf3b593` |
| **POST /sa 搜索空 (yoduzw)** | phone POST 200 list=0；分类页有书 | **disable** §16 |
| **小米浏览器书城 (miui)** | `reader.browser.miui.com` API 搜索 list=0；L2 body 0；需 App 签名 | **disable/skip** — 非公开 HTML 书源 |

## Worked examples

| Source | Fix |
|--------|-----|
| 画本 | toc `/list/{id}` + `.chapter-row` |
| 御书屋 | search.php + `#sitebox dl` + 目录 + `#YiJianZhan` |
| 长佩 | `chapterGetList` + `{"webView":true}` + `.chapter-render-box` |
| 爱丽丝 | migrate `.org→.com`；`/search.php?q=`(xunsearch)；pid→`/novel/`；`article#chapterContent` |
| 新书迷楼 | trim 空格 URL；migrate→`shukuai99.net`；`/search81.html?searchkey=` + `.item` |
| 卧龙 paper027 | https；`/api/v1/books/search?q=` + `$.data.data`；toc `/chapter/$id`；`.prose` |
| PO18文学 po18f | `bookUrl=class.bookname@tag.a@href`；清空 tocUrl；`id.list-chapterAll@a`；校验成功 |
| 吾爱耽美 52dmshu | 去 gbk charset；`#sitembox dl` + `dt a@href`；目录在详情 `#list dd a`；校验成功 |
| 繁星四月 fuxsb | migrate https；Empire 搜索体简化+Referer；正文 `.conbd@html`；校验成功 |
| 笔趣 96biquge | 登录壳非小说站 → skip |
| 笔趣 bqgcn | diagnose=ok → verify |
| 猫眼 / 123du / 古诗文 | skip (auth / captcha / WAF) |
| 必读居 biduju | search `keyword`+GBK+`class.list@table`；正文 `class.chapter@html` |
| 金庸 jinyongwang | 域名改行工业风机站 → disable/skip |
| 稻草 dcrsu | L2 HTTP timeout → disable/skip |
| 免费小说 b483 | search.php 空索引 → **disable**（勿首页过滤 workaround） |
| 话本 ihuaben | `/app/search`→`{}` → `so.ihuaben.com/search?keyword=` + `.searchresult`；toc CDN `cdn/chapters`；校验成功 |
| 破万卷 powanjuan | 清空 `tocUrl`；详情页 catalog + ruleToc；keyword=斗罗 校验成功 |
| 基友 gaysay | toc `data-c8dcb4a` base64 chapterUrl；641 章 + 正文 OK |
| 淘小说 tybook | COS 403 → `/tf/chapter_list` signed tocUrl |

## Scripts / CLI

| Entry | Role |
|--------|------|
| **`source-cli diagnose`** | L2 fail-fast + debug layer / fake_detail |
| **`source-cli repair`** | Live oneshot/batch: gate → diagnose → patch → save/verify |
| **`source-cli repair-dry`** | Mem ports + adapters; no MCP writes（冒烟） |
| **`source-cli gate`** / `source_gate_rs.py` | L0(/L1/L2) classify |
| **`source-cli probe` / `migrate` / `hunt` / `progress` / `ledger`** | Probe forms, domain migrate/hunt, queue, session ledger |
| `repair_*.py` shims | Same flags → `source-cli` (`REPAIR_USE_PYTHON=1` = legacy) |
| `repair_wait.py` | Dynamic poll `finished/total`; page all results; harvest fail-fast timeout |
| `repair_harvest.py` | Batch verify tagged fails (cheap wins) — orchestration |
| `repair_serial.py` | respondTime 队列串行 + `require_patch` + retro |
| `repair_refresh_phone_index.py` | MCP list_sources → phone index（存在性 SOT） |
| `repair_rt_queue.py` | phone index + RT 排序；搜索标签；1 host 1 源 |
| `repair_retro.py` | 每源反思 JSONL（trap / harness / script_fix） |
| `repair_closeout.py` | 核心：gate / pending / SKILL sync |
| `repair_closeout_check.py` | CLI：`gate` · `pending`（progress next 闸门）· `sync-skill` · `status` |
| `repair_db.py` / `repair_db_cli.py` | SQLite §9：`source_snapshot` / ledger dual-write / cache meta；`status` / `import-ledger` |
| `repair_wave.py` | triage（单源调 `source-cli repair`） |
| `mcp_discover.py` / `parity_*` | MCP discover + parity harness（长期 Python） |
