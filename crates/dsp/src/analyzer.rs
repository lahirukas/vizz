use crate::frontend::{Frontend, CHROMA, TIMBRE};
use crate::meter::{BeatFeatures, Meter, BEATS_PER_BAR};
use crate::pitch::Whistle;
use crate::snapshot::{AnalysisSnapshot, WAVE_LEN};
use crate::structure::{BeatSummary, Structure};
use crate::tempo::Acf;
use crate::tracker::{TickCtx, Tracker};

const ODF_WINDOW_SECS: f32 = 4.5;
const ACF_MAX_LAG_SECS: f32 = 3.2;
const TICK_SECS: f32 = 0.25;
const MIN_HISTORY_SECS: f32 = 2.5;
const ACTIVITY_SECS: f32 = 1.5;
const SILENCE_RMS: f32 = 1e-4;
const NEAR_BEAT_SECS: f64 = 0.015;
const ACCENT_SECS: f64 = 0.04;

struct Ring {
    data: Vec<f32>,
    head: usize,
    len: usize,
}

impl Ring {
    fn new(cap: usize) -> Self {
        Self { data: vec![0.0; cap], head: 0, len: 0 }
    }
    fn push(&mut self, v: f32) {
        let cap = self.data.len();
        self.data[self.head] = v;
        self.head = (self.head + 1) % cap;
        self.len = (self.len + 1).min(cap);
    }
    fn back(&self, i: usize) -> f32 {
        let cap = self.data.len();
        self.data[(self.head + cap - 1 - i) % cap]
    }
    fn newest(&self, n: usize) -> impl Iterator<Item = f32> + '_ {
        let n = n.min(self.len);
        (0..n).map(move |i| self.back(n - 1 - i))
    }
}

#[derive(Default)]
struct BeatAcc {
    frames: u32,
    low: f32,
    high: f32,
    centroid: f32,
    energy: f32,
    kick: u32,
    snare: u32,
    timbre: [f32; TIMBRE],
    chroma: [f32; CHROMA],
}

impl BeatAcc {
    fn summary(&self) -> BeatSummary {
        let n = self.frames.max(1) as f32;
        let mut s = BeatSummary {
            low: self.low / n,
            high: self.high / n,
            centroid: self.centroid / n,
            energy: self.energy / n,
            snare_onsets: self.snare,
            kick_onsets: self.kick,
            ..Default::default()
        };
        for i in 0..TIMBRE {
            s.timbre[i] = self.timbre[i] / n;
        }
        let csum: f32 = self.chroma.iter().sum();
        for i in 0..CHROMA {
            s.chroma[i] = if csum > 0.0 { self.chroma[i] / csum } else { 0.0 };
        }
        s
    }
}

struct Follower {
    peak: f32,
    out: f32,
}

impl Follower {
    fn update(&mut self, v: f32, decay: f32, release: f32) -> f32 {
        self.peak = (self.peak * decay).max(v).max(1e-6);
        let x = (v / self.peak).clamp(0.0, 1.0);
        self.out = if x > self.out { x } else { self.out * release + x * (1.0 - release) };
        self.out
    }
}

pub struct Analyzer {
    fe: Frontend,
    whistle: Whistle,
    tracker: Tracker,
    meter: Meter,
    structure: Structure,
    acf: Acf,
    odf: Ring,
    odf_low: Ring,
    odf_snare: Ring,
    long_odf: f32,
    samples_done: u64,
    since_hop: usize,
    frames_done: u64,
    tick_frames: usize,
    frames_since_tick: usize,
    min_hist: usize,
    window_frames: usize,
    activity_frames: usize,
    latency: f64,
    hop_s: f64,
    beat_acc: BeatAcc,
    prev_beat_time: f64,
    kick_f: Follower,
    snare_f: Follower,
    hat_f: Follower,
    level_f: Follower,
    agc_decay: f32,
    release: f32,
    band_peak_db: f32,
    wave_chunk: usize,
    wave_acc: f32,
    wave_count: usize,
    wave_ring: [f32; WAVE_LEN],
    wave_pos: usize,
    snap: AnalysisSnapshot,
}

impl Analyzer {
    pub fn new(sample_rate: u32) -> Self {
        let fe = Frontend::new(sample_rate);
        let fr = fe.fr;
        let hop = fe.hop;
        let hop_s = hop as f64 / sample_rate as f64;
        let window_frames = (ODF_WINDOW_SECS * fr) as usize;
        let follower = || Follower { peak: 1e-6, out: 0.0 };
        Self {
            acf: Acf::new(fr, window_frames, ACF_MAX_LAG_SECS),
            odf: Ring::new(window_frames),
            odf_low: Ring::new(window_frames),
            odf_snare: Ring::new(window_frames),
            long_odf: 0.0,
            tracker: Tracker::new(fr, hop_s, 1.5 * hop_s),
            meter: Meter::new(),
            structure: Structure::new(),
            samples_done: 0,
            since_hop: 0,
            frames_done: 0,
            tick_frames: ((TICK_SECS * fr) as usize).max(1),
            frames_since_tick: 0,
            min_hist: (MIN_HISTORY_SECS * fr) as usize,
            window_frames,
            activity_frames: (ACTIVITY_SECS * fr) as usize,
            // Calibrated on synthetic clicks: the log-flux peak trails the onset by ~1.5 hops.
            latency: 1.5 * hop_s,
            hop_s,
            beat_acc: BeatAcc::default(),
            prev_beat_time: f64::MIN,
            kick_f: follower(),
            snare_f: follower(),
            hat_f: follower(),
            level_f: follower(),
            agc_decay: (-(hop_s as f32) / 4.0).exp(),
            release: (-(hop_s as f32) / 0.12).exp(),
            band_peak_db: -40.0,
            wave_chunk: (hop / 2).max(1),
            wave_acc: 0.0,
            wave_count: 0,
            wave_ring: [0.0; WAVE_LEN],
            wave_pos: 0,
            snap: AnalysisSnapshot::EMPTY,
            whistle: Whistle::new(fe.sr, fe.n, fe.hop),
            fe,
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.fe.sr as u32
    }

    pub fn snapshot(&self) -> &AnalysisSnapshot {
        &self.snap
    }

    pub fn remembered_bpm(&self) -> Option<f32> {
        (self.tracker.locked && self.tracker.confidence > 0.3).then(|| self.tracker.bpm())
    }

    pub fn restore_bpm(&mut self, bpm: f32) {
        if (40.0..=240.0).contains(&bpm) {
            self.tracker.seed(60.0 * self.fe.fr / bpm);
        }
    }

    pub fn tap(&mut self) -> bool {
        let t = self.samples_done as f64 / self.fe.sr as f64;
        self.tracker.tap(t)
    }

    pub fn nudge_ms(&mut self, ms: f32) {
        self.tracker.nudge(ms as f64 / 1000.0);
    }

    pub fn tempo_switches(&self) -> u32 {
        self.tracker.tempo_switches
    }

    pub fn process(&mut self, samples: &[f32], on_beat: &mut impl FnMut(f64)) {
        for &x in samples {
            self.fe.push_sample(x);
            self.samples_done += 1;
            if x.abs() > self.wave_acc.abs() {
                self.wave_acc = x;
            }
            self.wave_count += 1;
            if self.wave_count == self.wave_chunk {
                self.wave_ring[self.wave_pos] = self.wave_acc.clamp(-1.0, 1.0);
                self.wave_pos = (self.wave_pos + 1) % WAVE_LEN;
                self.wave_acc = 0.0;
                self.wave_count = 0;
            }
            self.since_hop += 1;
            if self.since_hop == self.fe.hop {
                self.since_hop = 0;
                self.frame(on_beat);
            }
        }
    }

    fn t_now(&self) -> f64 {
        self.samples_done as f64 / self.fe.sr as f64
    }

    fn near_beat(&self) -> bool {
        if !self.tracker.locked {
            return false;
        }
        let t = self.frames_done as f64 * self.hop_s - self.latency;
        let p = self.tracker.period_s;
        let d = t - self.tracker.next_beat;
        (d - p * (d / p).round()).abs() < NEAR_BEAT_SECS
    }

    fn frame(&mut self, on_beat: &mut impl FnMut(f64)) {
        let f = self.fe.frame(self.near_beat());
        let w = self.whistle.process(self.fe.spectrum());
        self.snap.pitch_hz = w.hz;
        self.snap.pitch_note = w.note;
        self.snap.whistle = w.strength;
        self.odf.push(f.odf);
        self.tracker.push(f.odf + f.odf_low);
        self.odf_low.push(f.odf_low);
        self.odf_snare.push(f.odf_snare);
        self.frames_done += 1;

        let a = &mut self.beat_acc;
        a.frames += 1;
        a.low += f.low;
        a.high += f.high;
        a.centroid += f.centroid;
        a.energy += f.rms;
        a.kick += f.kick_onset as u32;
        a.snare += f.snare_onset as u32;
        for i in 0..TIMBRE {
            a.timbre[i] += self.fe.timbre[i];
        }
        if f.chroma_ready {
            for i in 0..CHROMA {
                a.chroma[i] += self.fe.chroma[i];
            }
        }
        self.update_display(&f);

        self.frames_since_tick += 1;
        if self.frames_since_tick >= self.tick_frames {
            self.frames_since_tick = 0;
            self.tick();
        }

        let t_now = self.t_now();
        let mut fired = [0.0f64; 4];
        let mut nf = 0;
        let mut first_index = 0;
        self.tracker.emit(t_now, |t, idx| {
            if nf < fired.len() {
                if nf == 0 {
                    first_index = idx;
                }
                fired[nf] = t;
                nf += 1;
            }
        });
        for (i, &t) in fired[..nf].iter().enumerate() {
            on_beat(t);
            self.on_beat(t, first_index + i as u64);
        }
        self.update_snapshot(t_now);
    }

    fn accent(&self, ring: &Ring, t: f64) -> f32 {
        let center = (t + self.latency) / self.hop_s - 1.0;
        let span = ACCENT_SECS / self.hop_s;
        let newest = self.frames_done as f64 - 1.0;
        let mut m = 0.0f32;
        let mut fi = (center - span).ceil();
        while fi <= center + span {
            let back = newest - fi;
            if back >= 0.0 && (back as usize) < ring.len {
                m = m.max(ring.back(back as usize));
            }
            fi += 1.0;
        }
        m
    }

    fn on_beat(&mut self, t: f64, index: u64) {
        let summary = self.beat_acc.summary();
        self.beat_acc = BeatAcc::default();
        if self.prev_beat_time > f64::MIN {
            let feats = BeatFeatures {
                low_accent: self.accent(&self.odf_low, self.prev_beat_time),
                snare_accent: self.accent(&self.odf_snare, self.prev_beat_time),
                chroma: summary.chroma,
            };
            self.meter.on_beat(index, &feats);
            let bib_prev = self.meter.beat_in_bar(index - 1);
            if self.structure.on_beat(&summary, bib_prev) {
                self.snap.drops = self.snap.drops.wrapping_add(1);
            }
        }
        self.prev_beat_time = t;
    }

    fn tick(&mut self) {
        let avail = self.odf.len.min(self.window_frames);
        if avail < self.min_hist {
            return;
        }
        let mean = self.acf.compute(self.odf.newest(avail));
        self.acf.update_spectrum();
        let recent: f32 = self.odf.newest(self.activity_frames).sum::<f32>() / self.activity_frames.min(avail) as f32;
        let active = mean > 1e-3 && self.snap.silent_secs < 0.5 && recent > 0.35 * self.long_odf;
        if self.snap.silent_secs < 0.5 {
            let rate = if recent > self.long_odf { 0.2 } else { 0.01 };
            self.long_odf += rate * (recent - self.long_odf);
        }

        let ctx = TickCtx { t_now: self.t_now(), active };
        let switches = self.tracker.tempo_switches;
        self.tracker.tick(&self.acf, &ctx);
        if self.tracker.tempo_switches != switches {
            self.meter.reset();
        }
    }

    fn update_display(&mut self, f: &crate::frontend::FrameOut) {
        let s = &mut self.snap;
        let hop_s = self.hop_s as f32;
        let peak_db = self.fe.band_db.iter().copied().fold(-120.0f32, f32::max);
        self.band_peak_db = (self.band_peak_db - 6.0 * hop_s).max(peak_db).max(-50.0);
        let floor = self.band_peak_db - 60.0;
        for (o, db) in s.bands.iter_mut().zip(self.fe.band_db) {
            let v = ((db - floor) / 60.0).clamp(0.0, 1.0);
            *o = if v > *o { v } else { *o * self.release + v * (1.0 - self.release) };
        }
        let (d, r) = (self.agc_decay, self.release);
        s.kick = self.kick_f.update(f.low, d, r);
        s.snare = self.snare_f.update(f.mid, d, r);
        s.hat = self.hat_f.update(f.high, d, r);
        s.level = self.level_f.update(f.rms, d, r);
        s.kick_onsets = s.kick_onsets.wrapping_add(f.kick_onset as u32);
        s.snare_onsets = s.snare_onsets.wrapping_add(f.snare_onset as u32);
        s.hat_onsets = s.hat_onsets.wrapping_add(f.hat_onset as u32);
        if f.rms < SILENCE_RMS {
            s.silent_secs += hop_s;
        } else {
            s.silent_secs = 0.0;
        }
    }

    fn update_snapshot(&mut self, t_now: f64) {
        let tr = &self.tracker;
        let s = &mut self.snap;
        s.t_audio = t_now;
        s.last_beat = tr.last_beat;
        s.next_beat = tr.next_beat;
        s.beat_count = tr.beat_index as u32;
        s.bpm = tr.bpm();
        s.confidence = tr.confidence;
        s.beat_presence = tr.presence;
        if tr.locked {
            s.beat_phase = ((t_now - tr.last_beat) / tr.period_s).clamp(0.0, 1.0) as f32;
            s.next_beat_eta_ms = ((tr.next_beat - t_now) * 1000.0) as f32;
        } else {
            s.beat_phase = 0.0;
            s.next_beat_eta_ms = 0.0;
        }
        s.beat_in_bar = self.meter.beat_in_bar(tr.beat_index);
        s.bar_phase = ((s.beat_in_bar - 1) as f32 + s.beat_phase) / BEATS_PER_BAR as f32;
        s.phrase_pos = self.structure.phrase_pos(s.beat_in_bar, s.bar_phase);
        s.phrase_bars = self.structure.phrase_len as f32;
        s.drop_probability = self.structure.drop_probability;
        s.drop_eta_beats =
            if s.drop_probability > 0.3 { self.structure.drop_eta_beats(s.beat_in_bar, s.beat_phase) } else { -1.0 };
        for i in 0..WAVE_LEN {
            s.wave[i] = self.wave_ring[(self.wave_pos + i) % WAVE_LEN];
        }
    }
}
