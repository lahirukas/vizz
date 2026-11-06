# Performance

Targets: **< 30 MB RAM, ~1–2 % CPU with one widget, 60 fps on the HD 3000** (i3-2310M, 2C/4T).
Measured with `tools/perf.ps1` (3 s warm-up, 10 s window). CPU % is of the whole machine (4 logical CPUs); "of one core" is ×4.

| Date | Milestone | Build | Scenario | Working set | Private | CPU (total) | CPU (one core) | Notes |
|---|---|---|---|---|---|---|---|---|
| 2026-11-05 | 2 | release | `vizz-dsp` bench, 56 s of 48 kHz drums offline | – | – | 0.29 % | 1.17 % | DSP only |
| 2026-11-05 | 3 | dev (opt 1) | `--source loopback --console`, 128 BPM drums playing | 13.0 MB | 3.9 MB | 0.94 % | 3.76 % | includes console printer polling every 10 ms |
| 2026-11-05 | 4 | dev (opt 1) | GUI, 1 ring widget 260×260 @60 fps, WAV source, render thread + DComp | 21.5 MB | 8.5 MB | 1.13 % | 4.53 % | HD 3000 FL 10_1 |
