# Beat evaluation

Synthetic suite run with `vizz --beat-eval` (dev build, 2026-11-05). Thresholds: BPM ±1; predicted beats within ±20 ms after a 4 s lock-in (from the change point + 4 s for tempo changes); downbeat accuracy ≥ 0.8; flywheel holds ≥ 90 % of breakdown beats within ±40 ms with no IOI jump > 15 %; drop probability < 0.3 in the groove and > 0.5 in the last bar of the build, ETA within ±1.5 beats one bar out, and the drop detected ≤ 0.6 s late; confidence < 0.3 on noise and speech. Half-time is scored with AMLt ≥ 0.9 (either metrical level is accepted).

F = F-measure at ±70 ms; CMLt/AMLt = mir_eval-style continuity (17.5 % tolerance). Never let a change make these worse.

### 44100 Hz

| signal | BPM (want) | conf | F | CMLt | AMLt | max err ms | downbeat | flywheel | drop err s | result |
|---|---|---|---|---|---|---|---|---|---|---|
| click_90 | 89.94 (90) | 0.99 | 1.00 | 1.00 | 1.00 | 2.5 | – | – | – | pass |
| drums_90 | 89.93 (90) | 0.99 | 1.00 | 1.00 | 1.00 | 3.9 | 1.00 | – | – | pass |
| click_120 | 120.18 (120) | 0.99 | 1.00 | 1.00 | 1.00 | 5.9 | – | – | – | pass |
| drums_120 | 120.18 (120) | 0.99 | 1.00 | 1.00 | 1.00 | 4.2 | 1.00 | – | – | pass |
| click_128 | 128.01 (128) | 0.99 | 1.00 | 1.00 | 1.00 | 2.4 | – | – | – | pass |
| drums_128 | 128.01 (128) | 0.99 | 1.00 | 1.00 | 1.00 | 0.6 | 1.00 | – | – | pass |
| click_140 | 140.13 (140) | 0.99 | 1.00 | 1.00 | 1.00 | 3.7 | – | – | – | pass |
| drums_140 | 140.08 (140) | 0.99 | 1.00 | 1.00 | 1.00 | 2.0 | 1.00 | – | – | pass |
| click_174 | 173.76 (174) | 0.99 | 1.00 | 1.00 | 1.00 | 2.6 | – | – | – | pass |
| drums_174 | 174.04 (174) | 0.98 | 1.00 | 1.00 | 1.00 | 2.7 | 1.00 | – | – | pass |
| change_120_140 | 140.04 (140) | 0.99 | 1.00 | 1.00 | 1.00 | 2.5 | 1.00 | – | – | pass |
| swing_110 | 109.96 (110) | 0.99 | 1.00 | 1.00 | 1.00 | 2.8 | 1.00 | – | – | pass |
| synco_124 | 124.04 (124) | 0.98 | 1.00 | 1.00 | 1.00 | 0.8 | 1.00 | – | – | pass |
| halftime_140 | 140.14 (140) | 0.98 | 1.00 | 1.00 | 1.00 | 2.3 | – | – | – | pass |
| breakdown_128 | 128.03 (128) | 0.46 | 1.00 | 1.00 | 1.00 | 1.1 | – | 1.00 | – | pass |
| buildup_drop_140 | 139.82 (140) | 0.94 | 0.98 | 1.00 | 1.00 | 7.2 | – | – | 0.43 | pass |
| noise | 0.00 (–) | 0.00 | – | – | – | – | – | – | – | pass |
| speech_like | 0.00 (–) | 0.10 | – | – | – | – | – | – | – | pass |

### 48000 Hz

| signal | BPM (want) | conf | F | CMLt | AMLt | max err ms | downbeat | flywheel | drop err s | result |
|---|---|---|---|---|---|---|---|---|---|---|
| click_90 | 90.00 (90) | 0.99 | 1.00 | 1.00 | 1.00 | 3.6 | – | – | – | pass |
| drums_90 | 90.00 (90) | 0.99 | 1.00 | 1.00 | 1.00 | 1.2 | 1.00 | – | – | pass |
| click_120 | 120.00 (120) | 0.99 | 1.00 | 1.00 | 1.00 | 2.0 | – | – | – | pass |
| drums_120 | 120.00 (120) | 0.99 | 1.00 | 1.00 | 1.00 | 0.3 | 1.00 | – | – | pass |
| click_128 | 127.84 (128) | 0.99 | 1.00 | 1.00 | 1.00 | 2.0 | – | – | – | pass |
| drums_128 | 127.84 (128) | 0.99 | 1.00 | 1.00 | 1.00 | 3.7 | 1.00 | – | – | pass |
| click_140 | 140.04 (140) | 0.99 | 1.00 | 1.00 | 1.00 | 2.2 | – | – | – | pass |
| drums_140 | 140.04 (140) | 0.99 | 1.00 | 1.00 | 1.00 | 0.6 | 1.00 | – | – | pass |
| click_174 | 173.97 (174) | 0.99 | 1.00 | 1.00 | 1.00 | 1.5 | – | – | – | pass |
| drums_174 | 173.97 (174) | 0.99 | 1.00 | 1.00 | 1.00 | 0.5 | 1.00 | – | – | pass |
| change_120_140 | 140.04 (140) | 0.99 | 1.00 | 1.00 | 1.00 | 1.2 | 1.00 | – | – | pass |
| swing_110 | 110.02 (110) | 0.99 | 1.00 | 1.00 | 1.00 | 0.7 | 1.00 | – | – | pass |
| synco_124 | 123.97 (124) | 0.99 | 1.00 | 1.00 | 1.00 | 1.2 | 1.00 | – | – | pass |
| halftime_140 | 140.03 (140) | 0.99 | 1.00 | 1.00 | 1.00 | 2.9 | – | – | – | pass |
| breakdown_128 | 127.84 (128) | 0.50 | 1.00 | 1.00 | 1.00 | 2.8 | – | 1.00 | – | pass |
| buildup_drop_140 | 140.05 (140) | 0.94 | 0.98 | 1.00 | 1.00 | 3.4 | – | – | 0.43 | pass |
| noise | 0.00 (–) | 0.00 | – | – | – | – | – | – | – | pass |
| speech_like | 66.37 (–) | 0.19 | – | – | – | – | – | – | – | pass |

Note: the synthetic table above was produced by the old 8-agent tracker. Tracker v2 passes every synthetic case at 44.1 and 48 kHz (`cargo test -p vizz-dsp`); rerun `vizz --beat-eval` to refresh the table.

## Real music (tracker v2)

Scored against librosa's beat tracker (bias-corrected by its measured +20 ms lateness on clean kicks), F-measure ±70 ms from 15 s on, any metrical level. Clips: librosa example set + beat_this test clip, 60 s each, 48 kHz.

| Clip | Genre | Vizz BPM | F (≥15 s) | Conf | Lock |
|---|---|---|---|---|---|
| choice | hip-hop | 136.2 | 0.93 | 0.86 | 4.2 s |
| vibeace | electronic | 130.1 | 0.76 | 0.46 | 20 s |
| fishin | country/pop | 117.8 | 0.57 | 0.07 | 12 s |
| swing | big-band swing | 111.9 | 0.88 | 0.09 | – |
| ragtime | piano ragtime | 143.8 | 0.91 | 0.40 | 9.6 s |
| sugarplum | orchestral pop | 108.4 | 0.99 | 0.32 | 5.9 s |
| hungarian | string orchestra | 128.3 | 0.42 | 0.00 | – |
| waltz | waltz | 150.0 | 1.00 | 1.00 | 4.0 s |
| **mean** | | | **0.81** | | |

"Lock" = first time confidence > 0.3. Beats are emitted before that and drawn with intensity `smoothstep(0.02, 0.3, confidence)`, so acoustic tracks with low confidence (fishin, swing) still pulse, just softer. Their confidence is low because their onsets look statistically close to speech; raising it without letting speech through needs real annotated data.

Old 8-agent tracker on the same clips: 4 of 8 never produced a BPM, mean F ≈ 0.3. Speech (3 LibriSpeech clips), whale song, solo trumpet: max confidence ≤ 0.11. Strictly orchestral clips (Hungarian Dance, Brahms-like) remain weak (F 0.0–0.45), which is expected for causal trackers.

Whistle tracker: synthetic whistle melody in loud fan noise is tracked note-for-note (recall > 90 %, median error < 25 cents); real robin song tracked; speech triggers it on 1–3 % of frames.

CPU (release, i7-class container): full engine incl. whistle tracker 0.47 % of one core.
