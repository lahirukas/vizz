#![forbid(unsafe_code)]

mod analyzer;
pub mod eval;
mod frontend;
mod meter;
pub mod pitch;
mod snapshot;
mod structure;
mod tempo;
pub mod testgen;
mod tracker;

pub use analyzer::Analyzer;
pub use snapshot::{AnalysisSnapshot, SnapshotBus, NUM_BANDS, WAVE_LEN};

pub struct OfflineResult {
    pub final_snapshot: AnalysisSnapshot,
    pub max_confidence: f32,
    pub beats: Vec<f64>,
    pub beat_in_bar: Vec<u32>,
    pub trace: Vec<AnalysisSnapshot>,
    pub drop_times: Vec<f64>,
}

pub fn analyze_offline(sample_rate: u32, samples: &[f32]) -> OfflineResult {
    let mut a = Analyzer::new(sample_rate);
    let mut beats = Vec::new();
    let mut bib = Vec::new();
    let mut trace = Vec::new();
    let mut drops = Vec::new();
    let mut max_conf = 0.0f32;
    let (mut last_count, mut last_drops, mut next_trace) = (0u32, 0u32, 0.0f64);
    let chunk = (sample_rate / 100).max(1) as usize;
    for c in samples.chunks(chunk) {
        a.process(c, &mut |t| beats.push(t));
        let s = *a.snapshot();
        if s.beat_count != last_count {
            last_count = s.beat_count;
            bib.resize(beats.len(), s.beat_in_bar);
        }
        if s.drops != last_drops {
            last_drops = s.drops;
            drops.push(s.t_audio);
        }
        if s.t_audio > 3.0 {
            max_conf = max_conf.max(s.confidence);
        }
        if s.t_audio >= next_trace {
            next_trace += 0.05;
            trace.push(s);
        }
    }
    bib.resize(beats.len(), 1);
    OfflineResult {
        final_snapshot: *a.snapshot(),
        max_confidence: max_conf,
        beats,
        beat_in_bar: bib,
        trace,
        drop_times: drops,
    }
}

pub struct EvalRow {
    pub name: String,
    pub bpm: f32,
    pub want_bpm: f32,
    pub confidence: f32,
    pub max_confidence: f32,
    pub scores: eval::BeatScores,
    pub downbeat_acc: Option<f64>,
    pub flywheel: Option<f64>,
    pub drop_err: Option<f64>,
    pub failures: Vec<String>,
}

fn at(trace: &[AnalysisSnapshot], t: f64) -> Option<&AnalysisSnapshot> {
    trace.iter().find(|s| s.t_audio >= t)
}

pub fn evaluate(sig: &testgen::TestSignal) -> EvalRow {
    let r = analyze_offline(sig.sample_rate, &sig.samples);
    let s = r.final_snapshot;
    let dur = sig.duration();
    let mut row = EvalRow {
        name: sig.name.clone(),
        bpm: s.bpm,
        want_bpm: sig.final_bpm,
        confidence: s.confidence,
        max_confidence: r.max_confidence,
        scores: Default::default(),
        downbeat_acc: None,
        flywheel: None,
        drop_err: None,
        failures: Vec::new(),
    };
    let fail = |row: &mut EvalRow, msg: String| row.failures.push(msg);

    if !sig.rhythmic() {
        if r.max_confidence >= 0.3 {
            fail(&mut row, format!("confidence {:.2} >= 0.3 on non-rhythmic input", r.max_confidence));
        }
        return row;
    }

    let halftime = sig.name.starts_with("halftime");
    let bpm_ok = (s.bpm - sig.final_bpm).abs() <= 1.0 || (halftime && (s.bpm - sig.final_bpm / 2.0).abs() <= 1.0);
    if !bpm_ok {
        fail(&mut row, format!("bpm {:.2}, want {}", s.bpm, sig.final_bpm));
    }

    let (truth, est): (Vec<f64>, Vec<f64>) = match sig.breakdown {
        Some((a, b)) => (
            sig.beats.iter().copied().filter(|&t| t < a - 0.1 || t >= b + 4.0).collect(),
            r.beats.iter().copied().filter(|&t| t < a - 0.1 || t >= b + 4.0).collect(),
        ),
        None => (sig.beats.clone(), r.beats.clone()),
    };
    row.scores = eval::score(&truth, &est, sig.lock_from, dur - 0.1);
    let sc = row.scores;
    if halftime {
        if sc.aml_t < 0.9 {
            fail(&mut row, format!("AMLt {:.2} < 0.9", sc.aml_t));
        }
    } else if sc.missed_20ms > 0 || sc.extra_20ms > 0 || sc.matched_20ms == 0 {
        fail(
            &mut row,
            format!("±20ms: missed {} extra {} max_err {:.1}ms", sc.missed_20ms, sc.extra_20ms, sc.max_err * 1e3),
        );
    }

    if !sig.name.starts_with("click") && !halftime && sig.drop_at.is_none() {
        let (mut hits, mut total) = (0, 0);
        for &d in sig.downbeats.iter().filter(|&&t| t >= sig.lock_from + 2.0 && t < dur - 0.1) {
            if sig.breakdown.is_some_and(|b| d >= b.0 && d < b.1 + 4.0) {
                continue;
            }
            total += 1;
            if let Some(i) = r.beats.iter().position(|&b| (b - d).abs() < 0.07) {
                hits += (r.beat_in_bar[i] == 1) as usize;
            }
        }
        if total > 0 {
            let acc = hits as f64 / total as f64;
            row.downbeat_acc = Some(acc);
            if acc < 0.8 {
                fail(&mut row, format!("downbeat accuracy {acc:.2} < 0.8"));
            }
        }
    }

    if let Some((a, b)) = sig.breakdown {
        let inside: Vec<f64> = sig.beats.iter().copied().filter(|&t| t >= a && t < b).collect();
        let held = inside.iter().filter(|&&t| r.beats.iter().any(|&p| (p - t).abs() <= 0.04)).count();
        let frac = held as f64 / inside.len().max(1) as f64;
        row.flywheel = Some(frac);
        if frac < 0.9 {
            fail(&mut row, format!("flywheel held {frac:.2} of breakdown beats (< 0.9 within ±40ms)"));
        }
        let p = 60.0 / sig.final_bpm as f64;
        let jumps = r.beats.windows(2).filter(|w| w[0] >= a && ((w[1] - w[0]) / p - 1.0).abs() > 0.15).count();
        if jumps > 0 {
            fail(&mut row, format!("{jumps} phase jumps (IOI off by >15%) after breakdown"));
        }
    }

    if let (Some(drop), Some(build)) = (sig.drop_at, sig.build_from) {
        let bar = 4.0 * 60.0 / sig.final_bpm as f64;
        let in_range = |lo: f64, hi: f64| {
            r.trace
                .iter()
                .filter(move |s| s.t_audio >= lo && s.t_audio < hi)
                .map(|s| s.drop_probability)
                .fold(0.0, f32::max)
        };
        let groove_max = in_range(3.0, build);
        let build_max = in_range(drop - bar, drop);
        if groove_max >= 0.3 {
            fail(&mut row, format!("drop probability {groove_max:.2} during groove"));
        }
        if build_max <= 0.5 {
            fail(&mut row, format!("drop probability only {build_max:.2} in the last bar of the build"));
        }
        if let Some(sn) = at(&r.trace, drop - bar) {
            if (sn.drop_eta_beats - 4.0).abs() > 1.5 {
                fail(&mut row, format!("drop eta {:.1} beats one bar before the drop (want 4)", sn.drop_eta_beats));
            }
        }
        match r.drop_times.iter().find(|&&t| t >= drop - 0.1) {
            Some(&t) => {
                row.drop_err = Some(t - drop);
                if t > drop + 0.6 {
                    fail(&mut row, format!("drop detected {:.2}s late", t - drop));
                }
            }
            None => fail(&mut row, "drop not detected".into()),
        }
    }
    row
}
