# Decisions

- **Name:** Vizz. Crates `vizz-dsp`, `vizz-audio`, `vizz-render`, `vizz-app`; binary `vizz.exe`; AppUserModelID `Vizz.App`; config in `%APPDATA%\Vizz\`, logs in `%LOCALAPPDATA%\Vizz\logs`.
- **Toolchain:** stable MSVC Rust with the minimal profile. Removed `rust-docs`, which the winget installer had added.
- **`windows` instead of `windows-sys`** for all Win32 calls (approved): `windows` is compiled anyway for COM, so this is one crate with unified features instead of two.
- **D3D target:** feature level 10_0/10_1 with SM 4.0 and no compute shaders, because the HD 3000 has no FL 11_0. WARP is the fallback.
- **Dependencies:** `realfft` for the FFT; `hound` for WAV I/O; `serde_json` *without* `serde_derive` (approved), with config mapped by hand.
- **CLI parsing:** hand-rolled. No `clap`.
- **Snapshot transport:** a seqlock over `AtomicU32` words in `vizz-dsp`. It is lock-free and allocation-free, and stays in safe Rust.
- **Device switching:** poll the default endpoint ID once a second and reopen on `AUDCLNT_E_DEVICE_INVALIDATED`. This avoids `#[implement]` COM proc-macros for `IMMNotificationClient`.
- **Rendering:** on a dedicated render thread before milestone 5 (pinning and dragging), so Win32 modal move/size loops never freeze widgets. The UI thread handles messages only.
- **Code style:** no doc comments. Line comments only for non-obvious math, Win32/COM quirks, undocumented APIs and magic constants.
- **Widget windows:** `WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOOLWINDOW | WS_EX_LAYERED` with `SetLayeredWindowAttributes(255, LWA_ALPHA)`, plus `WS_EX_TRANSPARENT` when pinned. A probe of the widget's screen rectangle confirmed the DirectComposition content shows with the layered style. Missing content in an earlier probe was z-order (the user's active window covered a normal-level widget), so `topmost` is a config option.
- **Render beat clock:** the render thread keeps its own continuous beat position and steers it toward the engine's prediction (time constant ~170 ms, never backwards, snaps only if > 0.4 beat off). Prediction refinements therefore never cause jumps or double pulses; each beat fires exactly once. `beat_t` (seconds since the render beat) drives a damped-spring punch in the shader; `wind` is a small inhale over the last 40 % of the beat.
- **Beat drive:** beat motion scales with `drive = smoothstep(0.02, 0.3, confidence)`, smoothed over ~0.7 s. Low-confidence audio (speech, whistling alone) fades the beat motion out instead of pulsing on a guessed grid.
- **Palette:** the DWM accent (`HKCU\...\DWM\AccentColor`) is turned into a 3-stop neon gradient (accent, +43°, −61°, saturation ≥ 0.78). It is re-read on `WM_SETTINGCHANGE "ImmersiveColorSet"` / `WM_DWMCOLORIZATIONCOLORCHANGED`. A per-widget `palette` overrides it.
- **Privacy:** desktop captures can include whatever the user has open. Visibility checks use `tools/probe_rect.ps1`, which only counts ring-coloured pixels inside the widget rectangle and saves nothing; full-desktop shots are deleted after use.
- **Beat tracker v2 (replaces the 8-agent tracker):** the old tracker's thresholds were tuned on synthetic clicks (tempo comb ≈ 0.9). Real music gives 0.1–0.3, so it never phase-fitted and stayed on its first wrong tempo. New design:
  - Tempo: detrended onset envelope → autocorrelation → ±2 % tolerant comb (4 harmonics) → spectrum smoothed over ~2.5 s (fast-adapts when the instantaneous winner disagrees) → metrical score `support(τ) · prior(130 BPM, σ 0.5 oct) · (1 + support(2τ))` → exact-comb refinement, switch hysteresis (3 ticks, 10 %), phase snap on switch.
  - Phase: BTrack-style cumulative beat score (α 0.9, log-Gaussian transition, tightness 5) on `odf + odf_low` (kick-weighted to avoid backbeat lock); at mid-beat it extrapolates the score forward and predicts the next beat (sub-frame via parabolic fit).
  - Confidence: relative tempo strength × beat salience (onset peak at the beat vs. mean over the beat).
  - Real-music result in BEAT_EVAL.md. Same public API, so meter/structure/drop/tap/nudge/tempo memory are unchanged.
- **Whistle / melody line (`vizz-dsp::pitch`):** per-bin noise floor that tracks the *mean* noise (freezes while a tone sits > 3× above it) and is subtracted 3×; strongest peak 400–4500 Hz; must be tonal (≥ 50–80 % of cleaned energy), pure (little energy ±3–8 bins), not harmonic (little at f/2, 2f), no low-band (200–400 Hz) energy, and pitch-stable for ~30 ms before switching on; off after ~37 ms. Pitch from parabolic log-magnitude interpolation, glided with a critically damped spring (9 Hz). Snapshot: `pitch_hz`, `pitch_note`, `whistle` (0–1).
- **Ring widget v3:** steady radius with a spring punch on each beat (35 % bigger on beat 1 plus an expanding shockwave), spectrum-shaped thickness, calm bar-progress arc (replaces the per-beat spinning arc), beat-in-bar dots, and a whistle comet riding the ring at the pitch angle (500 Hz bottom-left → 2.5 kHz bottom-right, ~10° per semitone) with a fading 16-point trail, a center beam and a whistle-colour tint.
- **Designs × styles:** four widget kinds (`ring`, `tunnel`, `horizon`, `spectrum`) × four styles (`neon`, `fluent`, `synthwave`, `liquid`). Style is a shader uniform; each style also gets its own palette (`Theme::palette_for`): Neon/Liquid derive from the Windows accent, Fluent uses the accent with ink colours that follow dark/light mode, Synthwave uses a fixed magenta/cyan/orange set.
- **Tunnel = analytic rings + feedback trails:** one ring is born per beat (8th-note rings in between) and travels outward on an exponential path, reaching the edge in 4 beats; downbeat rings are brighter. A ping-pong RGBA16F feedback buffer (half resolution above 0.9 MP) only adds short zoomed motion trails using max-blend, so it never fogs up. A second `present` pass applies style post-processing and the edge fade.
- **Horizon:** predicted beats glide to a "now" line (downbeats larger, drop countdown marker), oscilloscope behind the lane, shader-drawn 7-segment BPM digits that breathe on the beat, confidence bar.
- **Spectrum:** Neon mirrored bars with peak caps (peak hold 0.35 s, fall 1.2/s, computed on the render thread); Fluent symmetric rounded bars; Synthwave skyline in front of a striped sun over a perspective-correct grid that scrolls at the tempo; Liquid three smoothed wave layers with a bass glow.
- **App shell:** tray icon (menu: add widget, style for all, edit layout, pin to desktop, always on top, quit), right-click any widget in edit mode for Design / Style / Size / Remove, drag edges to resize, Esc leaves edit mode. Edit mode lifts widgets to topmost so they can be arranged; leaving it restores pinned-bottom or topmost. Single instance via a named mutex. `--exit-after` is now optional.
- **Shader preview off-Windows:** `dxc -spirv` → `spirv-cross` → OpenGL (llvmpipe) renders the real HLSL headlessly, so designs can be checked without the HD 3000.
- **Flux (feedback engine, flagship):** MilkDrop-class. Each frame warps the previous frame (zoom, rotation, swirl, sinusoidal warp, drifting centre), blurs, hue-drifts and decays it, then draws the live waveform (5 O(1)-per-pixel wave modes: line, circle, spectrum ring, side walls, spiral). Post pass adds symmetry (mirror, 4-fold, 6-fold kaleido), echo and solarize. Ten presets in `app/src/presets.rs`; they crossfade over 2.5 s on phrase boundaries (or every 20 s without a beat). Zoom punches use the predicted beat envelope; rotation flips direction per bar.
- **Cosmos:** five (Low 3 … Ultra 6) parallax nebula layers built from smooth-unioned spherical puffs (real normals → lit cauliflower clouds), rim glow, filaments, star cores, glowing gas, planets every 6th layer, a distant spiral galaxy and streaking starfields. Rendered at reduced resolution and upscaled with chromatic aberration, bloom, vignette and grain. Travel speed follows tempo plus beat thrusts; drops (from the structure tracker) trigger a hyperspace jump; phrase changes advance the scene (hue rotation).
- **Whistle Sketch:** a separate widget. While a whistle is held, a glowing arrow traces one of 12 figures (star, heart, note, spiral, planet, flower, bolt, infinity, house, mountains, Vizz mark, wave); pitch deviation steers the stroke off the path and sets its colour. Completing a figure fires a sparkle burst and starts the next. The canvas holds while you keep whistling and fades 2.5 s after you stop.
- **Quality:** Low / Medium / High / Ultra, global in the tray and per widget. It sets the internal resolution of multi-pass designs (e.g. Flux 120 k → full-res pixels, Cosmos 80 k → 640 k), Cosmos layer count and octaves, bloom taps; Low also caps the frame rate at 30 fps.
- **Noise texture:** 256² RGBA, G = R shifted by (37, 17) so one bilinear fetch gives smooth 3D value noise (bound at t1/s1 for every shader).
- **Icon:** generated at runtime (beat ring with cyan→magenta gradient and a pulse line), used for the tray.
- **3D raymarch toolkit (`raymarch.hlsli`):** camera look-at with roll, distance-field primitives (capsule, rotated ellipse), smooth union, quality-scaled step count (Low 40 … Ultra 88). Scenes render at reduced internal resolution and reuse the Cosmos post pass (bloom, chromatic aberration, vignette, grain).
- **Butterfly Bloom:** you fly along a curving path through a tunnel swarm of butterflies (7 sectors × repeating rings, randomized, some cells empty) with a hero butterfly gliding ahead. The hero's wings lift with the pre-beat wind-up and slam down on the predicted beat; swarm butterflies flap at their own rates with a ripple that travels down the tunnel on beats. Wings have spectrum-driven colour, veins, eyespots, iridescence and glowing edges; glow is analytic (closest approach of the ray), so it has no banding. Background: two-layer nebula, light shaft, pollen sparkles. Scene index shifts the hues on phrase changes.
- **Logo:** "VIZZ" as one continuous line: letters rise out of a sine ripple that runs along the baseline (the pen retraces parts of its path; the gradient follows x, so overlaps are invisible). Concept A is the word alone, concept B adds audio-wave tails. Files in `assets/logo/`.
- **Stopping fast when the beat ends:** three signals now cut the beat motion. (1) True silence (> 0.12 s) releases the drive at 14/s, so motion stops in ~0.3 s. (2) Beat presence: about 15 ms after each predicted beat, the tracker measures the onset peak at the beat against the song's running beat strength; if the strongest of the last 3 beats is below 0.25, the drums are gone → fade in ~1 s even under sustained pads. (3) No onsets at all for 1.5 beats. Onset pickers also gained a floor at 30 % of the running hit strength so steady tones can't fake hits. The tracker's own confidence decays 4× faster when inactive (×0.75 per tick). The beat grid itself keeps running (flywheel), so motion snaps back the moment drums return.
- **Warp Tunnel (3D):** ray-marched tube along a curving path. Walls ripple with the bass; glowing ribs every 0.9 units carry one spectrum band each, so the passing tunnel reacts; lanes carry energy packets whose speed follows the treble; neon style adds a hex lattice. One energy ring per predicted beat is launched ahead and reaches the camera exactly on the beat (downbeat rings brighter); the light at the end pulses with the kick; the camera rolls on downbeats and kicks its field of view on beats.

## Update 8
- Removed Infinite Tunnel, Cosmos, Beat Horizon, Spectrum and Butterfly Bloom (judged too basic).
  Scenes now: Flux, Warp Tunnel, Pop Tunnel, Neon Geometry, Pulse Ring, Whistle Sketch.
- Shared tunnel geometry lives in `tube.hlsli`, including `bend_screen` (gravity-well lensing, swirl,
  kick shockwave, scene kaleidoscope).
- Pop Tunnel uses analytic gradient noise instead of the 8-bit noise texture: the texture's
  quantisation made colour borders stair-step. Rays bend while marching (twist + pull to the axis).
- Neon Geometry: the tunnel cross-section itself morphs every beat (14 shapes from a generic star
  function, elastic ease, morph ripples from near to far). Every line is an exact 3D segment tested
  against the pixel ray, combined with max() so colours stay separate.
- Settings window + tray flyout built with Slint 1.18 (Fluent style, software renderer). The GUI
  thread runs Slint's event loop; widgets render on their own thread and audio on its own, so the UI
  adds no latency to visuals. Settings/flyout windows are created on open and dropped on close.
- Audio input is switchable live (Engine::set_source). Whistle Sketch can listen to the microphone
  through a second engine while the visualizers follow playback; the renderer takes whistle and
  pitch from that second stream. The saved `source` setting is now honoured at startup.
- Logo A (no trailing sine waves) is the official logo: `assets/logo/vizz_logo.{svg,png}`.

## Update 9 (0.1.0-alpha.1)
- Licence GPL-3.0-or-later; repo github.com/lahirukas/vizz. Slint is used under its GPL-3.0 option.
- Release builds use the Windows GUI subsystem (no terminal) and attach to a parent console when
  launched from one; debug builds keep a console. Panics append to %APPDATA%\Vizz\crash.log.
- Launch opens the main window (Showcase). `--background` (used by Start with Windows) starts in the
  tray only. A second launch posts WM_APP_SHOW to the running instance instead of erroring.
- Start with Windows = HKCU\...\Run "Vizz" -> "<exe>" --background, refreshed at every launch so a
  moved portable folder keeps working.
- Pin to desktop now embeds widgets into the desktop layer: on 24H2+ (raised desktop) as layered
  WS_CHILD of Progman placed directly below SHELLDLL_DefView; on older builds inside the WorkerW behind
  the icons (after Progman message 0x052C). Falls back to HWND_BOTTOM popups. Entering edit mode
  re-creates widgets as top-level popups; leaving it re-embeds them. Explorer restarts (TaskbarCreated)
  re-embed. Layered child windows need the exe manifest (supportedOS Win8-11), embedded via
  /MANIFESTINPUT; icon and version info come from res/vizz.rc (embed-resource, Windows hosts only).
- The dashed edit-mode border is gone; edit mode shows a faint tint instead.
- Whistle Sketch is fully interactive: steady tone = straight with speed ramp, pitch above/below a
  slowly adapting reference = turn (~2.4 rad/s per semitone, quarter-semitone dead zone), two short
  whistles within 0.45 s = explosion (stamped into the canvas), lines fade within ~1 s, bounces off
  the edges. Sketch values are applied only to sketch widgets (Pulse Ring shares the trail array).
- Showcase: the renderer draws a 960x540 off-screen preview of the selected design each frame and
  hands RGB frames to the UI through a shared slot; it only runs while the Showcase page is visible.
- Styles are presented as palette swatches.
- New default config has no desktop widgets; the Showcase is the first-run experience.

## Update 10
- Pop Tunnel II (`pop2.hlsl`): own gentle path with strong curvature so the vanishing point is mostly
  hidden, slow camera, breathing cross-section, scene-cycled materials (agate with quartz pockets,
  malachite, peacock eyes, leaf veins) from analytic noise and seamless tube Voronoi. Bump from the
  material height in the tube tangent frame, wrap lighting, translucency, roughness-driven gloss,
  distance-based detail fade into the palette mean, fog into the palette's deep tone (no light).
- Palette presets: 12 named palettes in `theme::PRESETS` on top of the 4 style palettes. A preset sets
  `WidgetCfg.palette`; `c1.a = 2` flags it for shaders (`custom_pal()`), so Pop, Neon Geometry and Warp
  switch from their built-in colour sets to the preset. One swatch index covers styles then presets.
