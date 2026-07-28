# Domain hunt trial (2026-07-26)

PC script: `scripts/repair_domain_hunt.py` + seeds in `config/domain_hunt_seeds.json`.
Probes = same L1/L2 as prefilter (not App check).

## Results

| Source | Action | Best | Notes |
|--------|--------|------|-------|
| zxcs.info | **migrate** | https://www.zxcs.click/ | also live: zxcs.live, www.zxcs.info; zxcs.zip SSL fail |
| 627txt / 爱去 | **migrate** | https://www.aiqu226.com/ | aiqu225 also L2 OK |
| tiexue book | **no_mirror** | — | official shutdown ~2026-03; correct to disable |
| dddw.net | **none / weak** | — | random bxwx clones ≠ successor; do not auto-migrate |

## Policy

1. Run hunt before L0 hard-disable when reason is timeout/dead_site (except confirmed shutdown).
2. `migrate` = propose `bookSourceUrl` rewrite + re-verify; do not claim fixed on L2 alone.
3. Video hosts use `action: video` → `legado-video-source-repair`, not novel disable.
