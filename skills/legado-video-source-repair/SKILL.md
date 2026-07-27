---
name: legado-video-source-repair
description: >-
  Repair Legado 视频/影视/听书-style sources (bookSourceType video/audio),
  not novel HTML TOC sources. Use for 影视站, M3U8, 网盘片库, taopian, ukuzy,
  when check fails with 下载链接为空 on media sites.
---

# Legado Video / Media Source Repair

**Separate from** `legado-book-source-repair` (小说搜→详→目→文).

Media sources fail novel check with 「下载链接为空」 by design if rules target
M3U8 / magnet / drive links. Do not force novel TOC patches on them.

## When to use

- Host/name looks like 影视 / 资源网 / M3U8 / 片库
- `bookSourceType` is **3 (file/下载)** or **4 (video)** — not novel text `0`
- Check message: 下载链接为空, explore-only media catalogs

## Flow

```
1. Prefer: source-cli video-route --url URL  (or python scripts/video_prefilter.py)
   L0 action=video → divert from novel repair_one
2. get_source; confirm type 3|4; smell missing downloadUrls
3. PC/phone: fix search bookUrl (must be detail) + downloadUrls / play list
4. debug_source then `repair_debug_vs_check.py` if debug≠check; then start_check_sources
5. `repair_session_log.py append` + log temp/full_fix/video_repair_*.json
```

Python helpers still valid: `video_prefilter.py`, `video_repair_one.py`.

## Done criteria (video/file)

- Device `start_check_sources` **校验成功**, or `debug_source` shows non-empty `downloadUrls`/m3u8
- Common fix for type=3: set `ruleBookInfo.downloadUrls` (e.g. `input[name=copy_sel]@value`) and ensure search `bookUrl` is a **detail** URL (not search page), or infoHtml hijacks detail parse
- If debug has m3u8 but check says 下载链接为空 → **do not** thrash CSS `||`; run `repair_debug_vs_check.py`

## Do not

- Clear tocUrl / novel auto-patches from `repair_patches.py`
- Put taopian/uku into novel-only verify batches without file rules
- Claim fixed without device verify

MCP defaults: `config/mcp_defaults.json`.
Novel repair: `legado-book-source-repair`.
Domain migrate: `scripts/repair_domain_migrate.py`.
