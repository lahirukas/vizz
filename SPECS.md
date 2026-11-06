# Machine specs (profiled 2026-11-05)

| Item | Value |
|---|---|
| CPU | Intel Core i3-2310M @ 2.10 GHz (Sandy Bridge, 2011), 2 cores / 4 threads |
| RAM | 11.9 GB total, **2.9 GB free** at profile time |
| Disk | C: 52 GB free |
| GPU | Intel HD Graphics 3000, driver 9.17.10.4459, WDDM 1.2, DDI 11.1 |
| D3D feature levels | **10_1**, 10_0, 9_3, 9_2, 9_1 (no 11_0) |
| OS | Windows 11 Pro 25H2, build 26200.9457 (process loopback OK: ≥ 19041) |
| winget | present |
| git | 2.56.0 present |
| rustup / cargo / rustc | missing |
| MSVC Build Tools / vswhere / cl / link / Windows SDK | missing |

## Resource budget

Free RAM is under 4 GB, so the strictest tier applies.

| Setting | Value |
|---|---|
| cargo jobs | **1** (re-check free RAM before heavy steps; 2 only if > 4 GB free) |
| dev debug info | `debug = 0` |
| LTO | none in dev; thin for milestone release; fat only for final ship build |
| Cold build timeout | 10 min (old, 2-core CPU) |
| Incremental check/test timeout | 4 min |
| GUI run | always `--exit-after`, outer timeout = exit-after + 30 s |

## Implications

- Render target is D3D11 API at **feature level 10_0/10_1**. Shaders use `vs_4_0` / `ps_4_0`, and there are no compute shaders (CS 4.x is optional on FL10). WARP is the fallback.
- WDDM 1.2 supports DirectComposition (Win8+), but flip-model swapchains for composition need to be tested on this driver.
