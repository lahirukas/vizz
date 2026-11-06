use std::f32::consts::TAU;

pub struct TestSignal {
    pub name: String,
    pub sample_rate: u32,
    pub samples: Vec<f32>,
    pub beats: Vec<f64>,
    pub downbeats: Vec<f64>,
    pub final_bpm: f32,
    pub lock_from: f64,
    pub breakdown: Option<(f64, f64)>,
    pub drop_at: Option<f64>,
    pub build_from: Option<f64>,
}

impl TestSignal {
    pub fn duration(&self) -> f64 {
        self.samples.len() as f64 / self.sample_rate as f64
    }
    pub fn rhythmic(&self) -> bool {
        !self.beats.is_empty()
    }
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 23) as f32 - 1.0
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.next() * 0.5 + 0.5)
    }
}

const FIRST_BEAT: f64 = 0.3;
const CHORDS: [[f32; 3]; 4] =
    [[261.6, 329.6, 392.0], [196.0, 246.9, 293.7], [220.0, 261.6, 329.6], [174.6, 220.0, 261.6]];

struct Synth {
    sr: f32,
    buf: Vec<f32>,
    rng: Rng,
}

impl Synth {
    fn new(sr: u32, secs: f64, seed: u64) -> Self {
        Self { sr: sr as f32, buf: vec![0.0; (secs * sr as f64) as usize], rng: Rng(seed) }
    }

    fn span(&self, t: f64, len_s: f32) -> std::ops::Range<usize> {
        let a = ((t * self.sr as f64).round().max(0.0) as usize).min(self.buf.len());
        let b = (a + (len_s * self.sr) as usize).min(self.buf.len());
        a..b
    }

    fn click(&mut self, t: f64, amp: f32) {
        let r = self.span(t, 0.012);
        let a = r.start;
        for i in r {
            let tt = (i - a) as f32 / self.sr;
            let n = self.rng.next();
            self.buf[i] += amp * (-tt / 0.002).exp() * (0.6 * (TAU * 2000.0 * tt).sin() + 0.4 * n);
        }
    }

    fn kick(&mut self, t: f64, amp: f32) {
        let r = self.span(t, 0.25);
        let a = r.start;
        let mut phase = 0.0f32;
        for i in r {
            let tt = (i - a) as f32 / self.sr;
            phase += TAU * (45.0 + 110.0 * (-tt / 0.03).exp()) / self.sr;
            self.buf[i] += amp * (tt / 0.001).min(1.0) * (-tt / 0.07).exp() * phase.sin();
        }
    }

    fn snare(&mut self, t: f64, amp: f32) {
        let r = self.span(t, 0.2);
        let a = r.start;
        for i in r {
            let tt = (i - a) as f32 / self.sr;
            let n = self.rng.next();
            let tone = (TAU * 190.0 * tt).sin() * (-tt / 0.03).exp();
            self.buf[i] += amp * (tt / 0.001).min(1.0) * (0.7 * n * (-tt / 0.05).exp() + 0.5 * tone);
        }
    }

    fn hat(&mut self, t: f64, amp: f32) {
        let r = self.span(t, 0.05);
        let a = r.start;
        let mut prev = 0.0;
        for i in r {
            let tt = (i - a) as f32 / self.sr;
            let n = self.rng.next();
            self.buf[i] += amp * (n - prev) * (-tt / 0.012).exp();
            prev = n;
        }
    }

    fn tone(&mut self, t0: f64, t1: f64, freqs: &[f32], amp: f32) {
        let r = self.span(t0, (t1 - t0) as f32);
        let (a, len) = (r.start, r.len().max(1));
        for i in r {
            let k = i - a;
            let tt = k as f32 / self.sr;
            let env = (tt / 0.02).min(1.0) * ((len - k) as f32 / self.sr / 0.02).min(1.0);
            let v: f32 = freqs.iter().map(|f| (TAU * f * (i as f32 / self.sr)).sin()).sum();
            self.buf[i] += amp * env * v / freqs.len() as f32;
        }
    }

    fn riser(&mut self, t0: f64, t1: f64, amp: f32) {
        let r = self.span(t0, (t1 - t0) as f32);
        let len = r.len().max(1) as f32;
        let a = r.start;
        let mut lp = 0.0f32;
        for i in r {
            let x = (i - a) as f32 / len;
            let fc = 300.0 * (8000.0f32 / 300.0).powf(x);
            let alpha = 1.0 - (-TAU * fc / self.sr).exp();
            lp += alpha * (self.rng.next() - lp);
            self.buf[i] += amp * x * lp;
        }
    }

    fn silence(&mut self, t0: f64, t1: f64) {
        let r = self.span(t0, (t1 - t0) as f32);
        self.buf[r].fill(0.0);
    }

    fn finish(mut self) -> Vec<f32> {
        let peak = self.buf.iter().fold(0.0f32, |m, v| m.max(v.abs())).max(1e-9);
        for v in &mut self.buf {
            *v *= 0.9 / peak;
        }
        self.buf
    }
}

fn grid(secs: f64, segments: &[(f64, f32)]) -> Vec<f64> {
    let mut out = Vec::new();
    let mut t = FIRST_BEAT;
    while t < secs - 0.05 {
        out.push(t);
        let bpm = segments.iter().rev().find(|(s, _)| t + 1e-9 >= *s).map_or(120.0, |s| s.1);
        t += 60.0 / bpm as f64;
    }
    out
}

fn downbeats(beats: &[f64]) -> Vec<f64> {
    beats.iter().copied().step_by(4).collect()
}

#[derive(Clone, Copy)]
enum Pattern {
    Basic,
    Swing(f64),
    Syncopated,
    HalfTime,
    FourFloor,
}

fn groove(s: &mut Synth, beats: &[f64], range: std::ops::Range<usize>, pattern: Pattern) {
    for i in range {
        let Some(&t) = beats.get(i) else { break };
        let p = beats.get(i + 1).map_or(beats[1] - beats[0], |n| n - t);
        let pos = i % 4;
        match pattern {
            Pattern::Basic | Pattern::Swing(_) => {
                if pos % 2 == 0 {
                    s.kick(t, if pos == 0 { 1.0 } else { 0.75 });
                } else {
                    s.snare(t, 0.6);
                }
            }
            Pattern::Syncopated => {
                match pos {
                    0 => s.kick(t, 1.0),
                    1 | 3 => s.snare(t, 0.6),
                    _ => {}
                }
                if pos == 1 {
                    s.kick(t + 0.5 * p, 0.8);
                }
                if pos == 2 {
                    s.kick(t + 0.75 * p, 0.7);
                }
            }
            Pattern::HalfTime => {
                if pos == 0 {
                    s.kick(t, 1.0);
                }
                if pos == 2 {
                    s.snare(t, 0.7);
                }
            }
            Pattern::FourFloor => s.kick(t, 1.0),
        }
        let off = if let Pattern::Swing(r) = pattern { r } else { 0.5 };
        s.hat(t, 0.25);
        s.hat(t + off * p, 0.18);
    }
}

fn chords(s: &mut Synth, beats: &[f64], range: std::ops::Range<usize>, amp: f32) {
    for bar_start in range.clone().step_by(4) {
        let Some(&t0) = beats.get(bar_start) else { break };
        let t1 = beats.get((bar_start + 4).min(range.end)).copied().unwrap_or(t0 + 4.0 * (beats[1] - beats[0]));
        s.tone(t0, t1, &CHORDS[(bar_start / 4) % 4], amp);
    }
}

fn signal(name: String, sr: u32, s: Synth, beats: Vec<f64>, bpm: f32) -> TestSignal {
    TestSignal {
        name,
        sample_rate: sr,
        samples: s.finish(),
        downbeats: downbeats(&beats),
        beats,
        final_bpm: bpm,
        lock_from: 4.0,
        breakdown: None,
        drop_at: None,
        build_from: None,
    }
}

pub fn click_track(sr: u32, bpm: f32, secs: f64) -> TestSignal {
    let beats = grid(secs, &[(0.0, bpm)]);
    let mut s = Synth::new(sr, secs, 0x1234_5678_9abc_def1);
    for (i, &t) in beats.iter().enumerate() {
        s.click(t, if i % 4 == 0 { 1.0 } else { 0.7 });
    }
    signal(format!("click_{bpm}"), sr, s, beats, bpm)
}

fn pattern_track(name: &str, sr: u32, bpm: f32, secs: f64, pattern: Pattern, seed: u64) -> TestSignal {
    let beats = grid(secs, &[(0.0, bpm)]);
    let mut s = Synth::new(sr, secs, seed);
    groove(&mut s, &beats, 0..beats.len(), pattern);
    chords(&mut s, &beats, 0..beats.len(), 0.08);
    s.tone(0.0, secs, &[55.0], 0.04);
    signal(format!("{name}_{bpm}"), sr, s, beats, bpm)
}

pub fn drum_pattern(sr: u32, bpm: f32, secs: f64) -> TestSignal {
    pattern_track("drums", sr, bpm, secs, Pattern::Basic, 0xdead_beef_cafe_f00d)
}

pub fn swing(sr: u32, bpm: f32, secs: f64) -> TestSignal {
    pattern_track("swing", sr, bpm, secs, Pattern::Swing(0.64), 0x5111_6000_0000_0001)
}

pub fn syncopated(sr: u32, bpm: f32, secs: f64) -> TestSignal {
    pattern_track("synco", sr, bpm, secs, Pattern::Syncopated, 0x5ca1_ab1e_0000_0002)
}

pub fn half_time(sr: u32, bpm: f32, secs: f64) -> TestSignal {
    pattern_track("halftime", sr, bpm, secs, Pattern::HalfTime, 0x4a1f_0000_0000_0003)
}

pub fn tempo_change(sr: u32, bpm_a: f32, bpm_b: f32, change_at: f64, secs: f64) -> TestSignal {
    let beats = grid(secs, &[(0.0, bpm_a), (change_at, bpm_b)]);
    let mut s = Synth::new(sr, secs, 0x0bad_c0de_1234_5678);
    groove(&mut s, &beats, 0..beats.len(), Pattern::Basic);
    chords(&mut s, &beats, 0..beats.len(), 0.08);
    let mut sig = signal(format!("change_{bpm_a}_{bpm_b}"), sr, s, beats, bpm_b);
    sig.lock_from = change_at + 4.0;
    sig
}

pub fn breakdown(sr: u32, bpm: f32) -> TestSignal {
    let secs = 16.0;
    let beats = grid(secs, &[(0.0, bpm)]);
    let quiet_from = beats.iter().position(|&t| t >= 8.0).unwrap_or(0) / 4 * 4;
    let back_at = beats.iter().position(|&t| t >= 12.0).unwrap_or(beats.len()).div_ceil(4) * 4;
    let mut s = Synth::new(sr, secs, 0xb4ea_0000_0000_0004);
    groove(&mut s, &beats, 0..quiet_from, Pattern::Basic);
    groove(&mut s, &beats, back_at..beats.len(), Pattern::Basic);
    chords(&mut s, &beats, 0..beats.len(), 0.08);
    let span = (beats[quiet_from], beats.get(back_at).copied().unwrap_or(secs));
    let mut sig = signal(format!("breakdown_{bpm}"), sr, s, beats, bpm);
    sig.breakdown = Some(span);
    sig
}

pub fn buildup_drop(sr: u32, bpm: f32) -> TestSignal {
    let bar = 4.0 * 60.0 / bpm as f64;
    let secs = FIRST_BEAT + 10.0 * bar + 0.1;
    let beats = grid(secs, &[(0.0, bpm)]);
    let p = 60.0 / bpm as f64;
    let mut s = Synth::new(sr, secs, 0xd209_0000_0000_0005);
    groove(&mut s, &beats, 0..16, Pattern::Basic);
    s.tone(beats[0], beats[16], &[55.0], 0.25);
    chords(&mut s, &beats, 0..beats.len(), 0.06);
    for (bar_i, subdiv) in [(4usize, 1usize), (5, 1), (6, 2), (7, 4)] {
        for b in 0..4 {
            let t = beats[bar_i * 4 + b];
            for k in 0..subdiv {
                let amp = 0.3 + 0.08 * (bar_i - 4) as f32 + 0.02 * b as f32;
                s.snare(t + k as f64 * p / subdiv as f64, amp);
            }
        }
    }
    let drop_at = beats[32];
    s.riser(beats[16], drop_at - p, 0.45);
    s.silence(drop_at - p, drop_at);
    groove(&mut s, &beats, 32..beats.len(), Pattern::FourFloor);
    s.tone(drop_at, secs, &[50.0, 100.0], 0.5);
    let build = beats[16];
    let mut sig = signal(format!("buildup_drop_{bpm}"), sr, s, beats, bpm);
    sig.drop_at = Some(drop_at);
    sig.build_from = Some(build);
    sig
}

pub fn noise(sr: u32, secs: f64) -> TestSignal {
    let mut s = Synth::new(sr, secs, 0x5eed_5eed_5eed_5eed);
    let mut lp = 0.0f32;
    for i in 0..s.buf.len() {
        lp += 0.3 * (s.rng.next() - lp);
        s.buf[i] = lp;
    }
    signal("noise".into(), sr, s, vec![], 0.0)
}

pub fn speech_like(sr: u32, secs: f64) -> TestSignal {
    let mut s = Synth::new(sr, secs, 0x0517_ab1e_5eed_0042);
    let srf = s.sr;
    let mut t = 0.1f32;
    while (t as f64) < secs - 0.3 {
        let dur = s.rng.range(0.06, 0.28);
        let pitch = s.rng.range(95.0, 190.0);
        let formants = [s.rng.range(300.0, 900.0), s.rng.range(900.0, 2500.0)];
        let voiced = s.rng.next() > -0.6;
        let amp = s.rng.range(0.3, 1.0);
        let start = (t * srf) as usize;
        let len = (dur * srf) as usize;
        let coefs = formants.map(|f| (2.0 * 0.97 * (TAU * f / srf).cos(), -0.97f32 * 0.97));
        let mut st = [[0.0f32; 2]; 2];
        let mut ph = 0.0f32;
        for i in 0..len {
            if start + i >= s.buf.len() {
                break;
            }
            ph = (ph + pitch / srf) % 1.0;
            let exc = if voiced { ph - 0.5 } else { 0.5 * s.rng.next() };
            let mut out = 0.0;
            for (k, &(a1, a2)) in coefs.iter().enumerate() {
                let y = exc * 0.05 + a1 * st[k][0] + a2 * st[k][1];
                st[k][1] = st[k][0];
                st[k][0] = y;
                out += y;
            }
            s.buf[start + i] += amp * (std::f32::consts::PI * i as f32 / len as f32).sin() * out;
        }
        t += dur + s.rng.range(0.03, 0.35) + if s.rng.next() > 0.85 { s.rng.range(0.2, 0.6) } else { 0.0 };
    }
    signal("speech_like".into(), sr, s, vec![], 0.0)
}

pub fn standard_set(sr: u32) -> Vec<TestSignal> {
    let mut v = Vec::new();
    for bpm in [90.0, 120.0, 128.0, 140.0, 174.0] {
        v.push(click_track(sr, bpm, 14.0));
        v.push(drum_pattern(sr, bpm, 14.0));
    }
    v.push(tempo_change(sr, 120.0, 140.0, 6.0, 15.0));
    v.push(swing(sr, 110.0, 14.0));
    v.push(syncopated(sr, 124.0, 14.0));
    v.push(half_time(sr, 140.0, 14.0));
    v.push(breakdown(sr, 128.0));
    v.push(buildup_drop(sr, 140.0));
    v.push(noise(sr, 12.0));
    v.push(speech_like(sr, 12.0));
    v
}
