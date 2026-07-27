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
| Future platform design (not operational yet) | `docs/repair-adapter-architecture.md` — only after full impl + func/perf parity vs current `scripts/` |

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
| **oneshot** 修一个报一个 | 默认深修、用户要盯进度 | checklist 或 `repair_deep_loop.py --mode oneshot --url URL` → 立刻汇报 |
| **batch** 批量 | 用户明确说「批量 / 做 N 个」 | `repair_deep_loop.py --mode batch --urls-file … --limit N`；流式 `REPORT:`；勿整批 AwaitShell |

## Deep-fix checklist (one URL)

Budget clock starts at **pick**. Diagnose+patch **2–3 min**; hard stop **5 min**.

```
[ ] 0  channel idle
[ ] 1  progress next  (script L2-gates walls/parked; ≤~20s)
[ ] 2  if next.l2_gate.action=migrate → migrate first
[ ] 3  diagnose --url URL   # also L2-failfast BEFORE phone debug
[ ] 4  if layer=skip → ledger already done → **立刻汇报**（oneshot 本轮结束）
[ ] 5  else patch ONLY layer → ONE verify → ledger → 汇报
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
python scripts/repair_progress.py next
python scripts/repair_diagnose.py --url URL --key 我的
# then patch + verify; or:
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
| 笔趣 96biquge | 登录壳非小说站 → skip |
| 笔趣 bqgcn | diagnose=ok → verify |
| 猫眼 / 123du / 古诗文 | skip (auth / captcha / WAF) |

## Scripts

| Script | Role |
|--------|------|
| `repair_wait.py` | Dynamic poll `finished/total`; page all results; harvest fail-fast timeout |
| `repair_harvest.py` | Batch verify tagged fails (cheap wins) |
| `repair_progress.py` | fixed/skip/remaining + next |
| `repair_diagnose.py` | layer-first diagnose |
| `repair_rule_smells.py` | webView quotes + empty-tocUrl tip |
| `repair_debug_parse.py` | fake_detail logic |
| `repair_deep_loop.py` | **oneshot / batch** 深修自动补丁 + 流式 REPORT |
| `repair_serial.py` | respondTime 队列串行 + `require_patch` + retro |
| `repair_refresh_phone_index.py` | MCP list_sources → phone index（存在性 SOT） |
| `repair_rt_queue.py` | phone index + RT 排序；搜索标签；1 host 1 源 |
| `repair_retro.py` | 每源反思 JSONL（trap / harness / script_fix） |
| `repair_search_probe.py` | forms + **common paths** + **score/rank** + **JS data-api** → best searchUrl |
| `repair_wave.py` | triage |
| `repair_patches.py` | smells + webView auto |
| `repair_session_log.py` | ledger |
