#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod autostart;
mod beatclock;
mod beateval;
mod cli;
mod config;
mod desktop;
mod engine;
mod gui;
mod icon;
mod presets;
mod renderer;
mod sketch;
mod theme;
mod ui;
mod wav;

use serde_json::json;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    #[cfg(not(debug_assertions))]
    unsafe {
        use windows::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }
    install_panic_log();
    let args = match cli::parse(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };

    if let Some(path) = &args.headless_analyze {
        return headless_analyze(path);
    }
    if let Some(dir) = &args.gen_test_wavs {
        return gen_test_wavs(dir);
    }
    if args.beat_eval {
        return beateval::synthetic_report();
    }
    if let Some(dir) = &args.eval_dir {
        return beateval::eval_dir(dir);
    }
    let exit_after = args.exit_after.unwrap_or(f64::INFINITY);
    let config_path = args.config.as_ref().map(PathBuf::from).unwrap_or_else(config::default_path);
    let cfg = config::Config::load(&config_path);
    let saved = if cfg.source == "mic" { cli::Source::Mic } else { cli::Source::Loopback };
    let source = match to_audio_source(args.source.clone().unwrap_or(saved)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    if args.console {
        return console(source, exit_after);
    }
    if let Some(out) = &args.screenshot {
        let mut wc = cfg.widgets.first().cloned().unwrap_or(config::WidgetCfg {
            kind: "warp".into(),
            style: "neon".into(),
            quality: None,
            x: 0,
            y: 0,
            w: 480,
            h: 270,
            palette: None,
            opacity: 1.0,
        });
        if let Some(k) = &args.kind {
            wc.kind = k.clone();
        }
        if let Some(s) = &args.style {
            wc.style = s.clone();
        }
        return screenshot(source, &cfg, &wc, out, args.frames, exit_after.min(120.0), args.warp);
    }
    let engine = match engine::Engine::start(source) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let opts = gui::GuiOpts {
        exit_after: args.exit_after,
        edit: args.edit,
        warp: args.warp,
        background: args.background,
        config_path,
    };
    match gui::run(engine, cfg, &opts) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("gui: {e}");
            ExitCode::FAILURE
        }
    }
}

const SHEET_PHASES: [f32; 6] = [0.0, 0.12, 0.4, 0.7, 0.85, 0.97];

fn beat_sheet(
    gpu: &vizz_render::Gpu,
    kind: vizz_render::WidgetKind,
    w: u32,
    h: u32,
    live: &vizz_render::Params,
) -> windows::core::Result<(u32, u32, Vec<u8>)> {
    let n = SHEET_PHASES.len() as u32;
    let (sw, sh) = (w * n, h);
    let mut sheet = vec![0u8; (sw * sh * 4) as usize];
    let bpm = if live.bpm > 0.0 { live.bpm } else { 120.0 };
    for (i, &phase) in SHEET_PHASES.iter().enumerate() {
        let p = beatclock::at_phase(live, phase, bpm, (i as u32 % 4) as f32 + 1.0);
        let px = gpu.render_offscreen(w, h, kind, |f| warm_up(&p, f), warm_frames(kind))?;
        for y in 0..h {
            for x in 0..w {
                let s = ((y * w + x) * 4) as usize;
                let d = ((y * sw + i as u32 * w + x) * 4) as usize;
                let bg = dark_background(i as u32 * w + x, y, sw, sh);
                let a = px[s + 3] as f32 / 255.0;
                for c in 0..3 {
                    sheet[d + c] = (px[s + c] as f32 * a + bg[c] * (1.0 - a)).round().min(255.0) as u8;
                }
                sheet[d + 3] = 255;
            }
        }
    }
    Ok((sw, sh, sheet))
}

fn warm_frames(kind: vizz_render::WidgetKind) -> u32 {
    if kind.two_pass() {
        90
    } else {
        1
    }
}

fn warm_up(p: &vizz_render::Params, frame: u32) -> vizz_render::Params {
    let mut q = *p;
    q.time += frame as f32 / 60.0;
    q.dt = 1.0 / 60.0;
    q
}

fn dark_background(x: u32, y: u32, w: u32, h: u32) -> [f32; 3] {
    let t = (x as f32 / w as f32 * 0.6 + y as f32 / h as f32 * 0.4).clamp(0.0, 1.0);
    [16.0 + 14.0 * t, 20.0 + 12.0 * t, 28.0 + 26.0 * t]
}

fn screenshot(
    source: vizz_audio::Source,
    cfg: &config::Config,
    wc: &config::WidgetCfg,
    out: &str,
    frames: u32,
    exit_after: f64,
    warp: bool,
) -> ExitCode {
    let kind = vizz_render::WidgetKind::from_name(&wc.kind).unwrap_or(vizz_render::WidgetKind::Ring);
    let style = vizz_render::Style::from_name(&wc.style).unwrap_or(vizz_render::Style::Neon);
    let gpu = match vizz_render::Gpu::new(warp) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("gpu: {e}");
            return ExitCode::FAILURE;
        }
    };
    eprintln!("renderer: {}", gpu.describe());
    let engine = match engine::Engine::start(source) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let spec = renderer::WidgetSpec {
        hwnd: 0,
        kind,
        style,
        quality: vizz_render::Quality::High,
        width: wc.w as u32,
        height: wc.h as u32,
        palette: wc.palette,
        opacity: wc.opacity,
    };
    let base = renderer::base_params(&spec, &theme::Theme::current(), false);
    let mut clock = beatclock::BeatClock::new(cfg.av_offset_ms);
    let mut snap = vizz_dsp::AnalysisSnapshot::EMPTY;
    let start = std::time::Instant::now();
    let mut prev_pulse = 0.0f32;
    let mut fired = Vec::new();
    let frame_dt = std::time::Duration::from_micros(16_667);
    let n = frames.max(1);
    let mut last = vizz_render::Params::default();
    for i in 0..n {
        if start.elapsed().as_secs_f64() >= exit_after {
            eprintln!("exit-after reached at frame {i}");
            break;
        }
        let due = start + frame_dt * i;
        if let Some(d) = due.checked_duration_since(std::time::Instant::now()) {
            std::thread::sleep(d);
        }
        engine.bus.read(&mut snap);
        let now = std::time::Instant::now();
        last = clock.params(&snap, now, base);
        if last.pulse > prev_pulse + 0.3 {
            fired.push(clock.stream_now(&snap, now));
        }
        prev_pulse = last.pulse;
    }
    drop(engine);
    let frozen = last;
    let rgba = match gpu.render_offscreen(wc.w as u32, wc.h as u32, kind, |f| warm_up(&frozen, f), warm_frames(kind)) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("render: {e}");
            return ExitCode::FAILURE;
        }
    };
    if let Some(dir) = std::path::Path::new(out).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = std::fs::write(out, vizz_render::png::encode_rgba(wc.w as u32, wc.h as u32, &rgba)) {
        eprintln!("{out}: {e}");
        return ExitCode::FAILURE;
    }
    let sheet_path = out.trim_end_matches(".png").to_string() + "_sheet.png";
    match beat_sheet(&gpu, kind, wc.w as u32, wc.h as u32, &last) {
        Ok((w, h, px)) => {
            if let Err(e) = std::fs::write(&sheet_path, vizz_render::png::encode_rgba(w, h, &px)) {
                eprintln!("{sheet_path}: {e}");
            }
        }
        Err(e) => eprintln!("sheet: {e}"),
    }
    let fired: Vec<f64> = fired.iter().map(|t| (t * 1000.0).round() / 1000.0).collect();
    println!(
        "{}",
        json!({"png": out, "frames": n, "bpm": snap.bpm, "confidence": snap.confidence,
               "pulse": last.pulse, "phase": last.phase, "pulse_fired_at": fired})
    );
    ExitCode::SUCCESS
}

fn to_audio_source(s: cli::Source) -> Result<vizz_audio::Source, String> {
    Ok(match s {
        cli::Source::Loopback => vizz_audio::Source::Loopback,
        cli::Source::Mic => vizz_audio::Source::Mic,
        cli::Source::Process(pid) => vizz_audio::Source::Process(pid),
        cli::Source::Wav(p) => {
            let (sr, mono) = wav::read_mono(&p)?;
            vizz_audio::Source::Samples { sample_rate: sr, data: mono.into(), looped: false }
        }
    })
}

fn console(source: vizz_audio::Source, exit_after: f64) -> ExitCode {
    let engine = match engine::Engine::start(source) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let start = std::time::Instant::now();
    let mut snap = vizz_dsp::AnalysisSnapshot::EMPTY;
    let mut last_beat = 0u32;
    let mut last_status = 0.0f64;
    while start.elapsed().as_secs_f64() < exit_after && !engine.finished() {
        std::thread::sleep(std::time::Duration::from_millis(10));
        engine.bus.read(&mut snap);
        let el = start.elapsed().as_secs_f64();
        if snap.beat_count != last_beat {
            last_beat = snap.beat_count;
            let mark = if snap.confidence >= 0.3 { "BEAT" } else { "beat" };
            let drop = if snap.drop_eta_beats >= 0.0 {
                format!("drop {:.2} in {:.1}b", snap.drop_probability, snap.drop_eta_beats)
            } else {
                format!("drop {:.2}", snap.drop_probability)
            };
            println!(
                "{el:7.2}s {mark} {}/4 #{:<4} bpm {:6.1} conf {:.2} phase {:.2} phrase {:.2}/{} next {:4.0}ms {drop}",
                snap.beat_in_bar,
                snap.beat_count,
                snap.bpm,
                snap.confidence,
                snap.beat_phase,
                snap.phrase_pos,
                snap.phrase_bars,
                snap.next_beat_eta_ms
            );
        } else if el - last_status >= 1.0 {
            last_status = el;
            println!(
                "{el:7.2}s status    stream {:7.2}s  bpm {:6.1}  conf {:.2}  level {:.2}  silent {:.1}s",
                snap.t_audio, snap.bpm, snap.confidence, snap.level, snap.silent_secs
            );
        }
    }
    drop(engine);
    println!("exit after {:.1}s", start.elapsed().as_secs_f64());
    ExitCode::SUCCESS
}

fn headless_analyze(path: &str) -> ExitCode {
    let (sr, mono) = match wav::read_mono(path) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let r = vizz_dsp::analyze_offline(sr, &mono);
    let beats: Vec<f64> = r.beats.iter().map(|t| (t * 1e4).round() / 1e4).collect();
    let out = json!({
        "file": path,
        "sample_rate": sr,
        "duration": mono.len() as f64 / sr as f64,
        "bpm": (r.final_snapshot.bpm * 100.0).round() / 100.0,
        "confidence": (r.final_snapshot.confidence * 1000.0).round() / 1000.0,
        "max_confidence": (r.max_confidence * 1000.0).round() / 1000.0,
        "beats": beats,
        "beat_in_bar": r.beat_in_bar,
        "drops": r.drop_times,
    });
    println!("{out}");
    ExitCode::SUCCESS
}

fn gen_test_wavs(dir: &str) -> ExitCode {
    let dir = std::path::Path::new(dir);
    if let Err(e) = std::fs::create_dir_all(dir) {
        eprintln!("{e}");
        return ExitCode::FAILURE;
    }
    for sig in vizz_dsp::testgen::standard_set(44100) {
        let wav = dir.join(format!("{}.wav", sig.name));
        if let Err(e) = wav::write_mono16(&wav, sig.sample_rate, &sig.samples) {
            eprintln!("{}: {e}", wav.display());
            return ExitCode::FAILURE;
        }
        let truth = json!({ "bpm": sig.final_bpm, "beats": sig.beats, "lock_from": sig.lock_from });
        let _ = std::fs::write(dir.join(format!("{}.json", sig.name)), truth.to_string());
    }
    println!("wrote test set to {}", dir.display());
    ExitCode::SUCCESS
}

fn install_panic_log() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let path = config::default_path().with_file_name("crash.log");
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let line = format!("{} {info}\n", env!("CARGO_PKG_VERSION"));
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .and_then(|mut f| std::io::Write::write_all(&mut f, line.as_bytes()));
        default(info);
    }));
}
