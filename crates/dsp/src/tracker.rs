use crate::tempo::Acf;

const ALPHA: f32 = 0.9;
const TIGHTNESS: f32 = 5.0;
const DETREND_SECS: f32 = 0.25;
const HISTORY_SECS: f32 = 2.6;
const TRACK_SPAN: f32 = 0.03;
const SWITCH_RATIO: f32 = 1.1;
const SWITCH_TICKS: u32 = 3;
const LOCK_STRENGTH: f32 = 0.1;
const LOCK_TICKS: u32 = 2;
const MANUAL_SECS: f64 = 8.0;
const PRESENCE_TRACK: f32 = 0.4;
const PRESENCE_RATE: f32 = 0.1;
const PRESENCE_SAG: f32 = 0.98;
const INACTIVE_DECAY: f32 = 0.75;

pub struct TickCtx {
    pub t_now: f64,
    pub active: bool,
}

struct Hist {
    data: Vec<f32>,
}

impl Hist {
    fn at(&self, frame: u64) -> f32 {
        self.data[(frame % self.data.len() as u64) as usize]
    }
    fn set(&mut self, frame: u64, v: f32) {
        let n = self.data.len() as u64;
        self.data[(frame % n) as usize] = v;
    }
}

pub struct Tracker {
    fr: f32,
    hop_s: f64,
    latency: f64,
    frames: u64,
    cum: Hist,
    det: Hist,
    trail: Vec<f32>,
    trail_pos: usize,
    trail_sum: f32,
    weights: Vec<f32>,
    w_lo: usize,
    w_tau: f32,
    fut: Vec<f32>,
    tau: f32,
    predicted: bool,
    last_frame: f64,
    salience: f32,
    lock_ticks: u32,
    challenger: f32,
    challenge: u32,
    nudge: f64,
    taps: [f64; 8],
    tap_count: usize,
    manual_until: f64,
    pub locked: bool,
    pub period_s: f64,
    pub next_beat: f64,
    pub last_beat: f64,
    pub beat_index: u64,
    pub confidence: f32,
    pub presence: f32,
    recent_presence: [f32; 3],
    beat_peak_avg: f32,
    pending_score: Option<f64>,
    pub tempo_switches: u32,
}

impl Tracker {
    pub fn new(fr: f32, hop_s: f64, latency: f64) -> Self {
        let hist = (HISTORY_SECS * fr) as usize + 4;
        let trail = ((DETREND_SECS * fr) as usize).max(1);
        Self {
            fr,
            hop_s,
            latency,
            frames: 0,
            cum: Hist { data: vec![0.0; hist] },
            det: Hist { data: vec![0.0; hist] },
            trail: vec![0.0; trail],
            trail_pos: 0,
            trail_sum: 0.0,
            weights: Vec::with_capacity(hist),
            w_lo: 1,
            w_tau: 0.0,
            fut: vec![0.0; hist],
            tau: 0.0,
            predicted: false,
            last_frame: 0.0,
            salience: 0.0,
            lock_ticks: 0,
            challenger: 0.0,
            challenge: 0,
            nudge: 0.0,
            taps: [0.0; 8],
            tap_count: 0,
            manual_until: f64::MIN,
            locked: false,
            period_s: 0.5,
            next_beat: 0.0,
            last_beat: 0.0,
            beat_index: 0,
            confidence: 0.0,
            presence: 0.0,
            recent_presence: [0.0; 3],
            beat_peak_avg: 0.0,
            pending_score: None,
            tempo_switches: 0,
        }
    }

    pub fn bpm(&self) -> f32 {
        if self.locked {
            (60.0 / self.period_s) as f32
        } else {
            0.0
        }
    }

    fn frame_time(&self, frame: f64) -> f64 {
        (frame + 1.0) * self.hop_s - self.latency
    }

    fn time_frame(&self, t: f64) -> f64 {
        (t + self.latency) / self.hop_s - 1.0
    }

    fn set_tau(&mut self, tau: f32) {
        self.tau = tau;
        self.period_s = (tau / self.fr) as f64;
        if (tau - self.w_tau).abs() < 0.1 {
            return;
        }
        self.w_tau = tau;
        self.w_lo = ((0.5 * tau).round() as usize).max(1);
        let hi = ((2.0 * tau).round() as usize).min(self.cum.data.len() - 2);
        self.weights.clear();
        for j in self.w_lo..=hi {
            let x = TIGHTNESS * (j as f32 / tau).ln();
            self.weights.push((-0.5 * x * x).exp());
        }
    }

    fn lock_at(&mut self, frame: f64) {
        self.locked = true;
        self.last_frame = frame;
        self.last_beat = self.frame_time(frame) + self.nudge;
        self.next_beat = self.last_beat + self.period_s;
        self.predicted = false;
    }

    pub fn seed(&mut self, period_frames: f32) {
        self.set_tau(period_frames);
        let now = self.frames as f64 - 1.0;
        self.lock_at(now - period_frames as f64);
        self.confidence = 0.3;
    }

    pub fn nudge(&mut self, secs: f64) {
        self.nudge += secs;
        self.next_beat += secs;
        self.last_beat += secs;
    }

    pub fn tap(&mut self, t: f64) -> bool {
        if self.tap_count > 0 && t - self.taps[(self.tap_count - 1) % 8] > 2.0 {
            self.tap_count = 0;
        }
        self.taps[self.tap_count % 8] = t;
        self.tap_count += 1;
        let n = self.tap_count.min(8);
        if n < 4 {
            return false;
        }
        let first = self.taps[(self.tap_count - n) % 8];
        let period = (t - first) / (n - 1) as f64;
        if !(0.25..=1.0).contains(&period) {
            return false;
        }
        self.set_tau((period * self.fr as f64) as f32);
        let frame = self.time_frame(t - self.nudge);
        self.lock_at(frame);
        self.manual_until = t + MANUAL_SECS;
        self.confidence = self.confidence.max(0.8);
        true
    }

    pub fn push(&mut self, odf: f32) {
        let n = self.frames;
        self.frames += 1;
        self.trail_sum += odf - self.trail[self.trail_pos];
        self.trail[self.trail_pos] = odf;
        self.trail_pos = (self.trail_pos + 1) % self.trail.len();
        let d = (odf - self.trail_sum / self.trail.len() as f32).max(0.0);
        self.det.set(n, d);
        if let Some(f) = self.pending_score {
            if n as f64 >= f + 3.0 {
                self.pending_score = None;
                self.score_presence(f);
            }
        }

        let c = if self.tau > 0.0 { (1.0 - ALPHA) * d + ALPHA * self.past_max(n, |f| self.cum.at(f)) } else { d };
        self.cum.set(n, c);

        if self.locked && !self.predicted && n as f64 >= self.last_frame + 0.5 * self.tau as f64 {
            self.predict(n);
        }
    }

    fn past_max(&self, n: u64, value: impl Fn(u64) -> f32) -> f32 {
        let mut m = 0.0f32;
        for (k, &w) in self.weights.iter().enumerate() {
            let j = (self.w_lo + k) as u64;
            if j > n {
                break;
            }
            m = m.max(w * value(n - j));
        }
        m
    }

    fn predict(&mut self, n: u64) {
        let tau = self.tau as f64;
        let horizon = (tau.round() as usize).min(self.fut.len() - 1);
        let expect = self.last_frame + tau;
        let (mut best_m, mut best) = (1usize, f32::MIN);
        for m in 1..=horizon {
            let f = n + m as u64;
            let v = ALPHA * self.past_max(f, |g| if g > n { self.fut[(g - n) as usize] } else { self.cum.at(g) });
            self.fut[m] = v;
            let z = (f as f64 - expect) / (0.5 * tau);
            let s = v * (-0.5 * z * z).exp() as f32;
            if s > best {
                best = s;
                best_m = m;
            }
        }
        let score = |m: usize| {
            let z = ((n + m as u64) as f64 - expect) / (0.5 * tau);
            self.fut[m] * (-0.5 * z * z).exp() as f32
        };
        let mut frac = 0.0;
        if best_m > 1 && best_m < horizon {
            let (l, r) = (score(best_m - 1), score(best_m + 1));
            let den = l - 2.0 * best + r;
            if den < 0.0 {
                frac = (0.5 * (l - r) / den).clamp(-0.5, 0.5) as f64;
            }
        }
        let frame = n as f64 + best_m as f64 + frac;
        let t = self.frame_time(frame) + self.nudge;
        self.next_beat = t.max(self.last_beat + 0.4 * self.period_s);
        self.predicted = true;
    }

    fn snap_phase(&mut self) {
        let n = self.frames as i64 - 1;
        let tau = self.tau as f64;
        let span = (HISTORY_SECS as f64 * self.fr as f64 * 0.9) as i64;
        let (mut best_phi, mut best) = (0.0f64, f32::MIN);
        let mut phi = 0.0;
        while phi < tau {
            let (mut s, mut k) = (0.0f32, 0.0f64);
            loop {
                let back = (phi + k * tau).round() as i64;
                if back > span || back > n {
                    break;
                }
                s += self.det.at((n - back) as u64) * (0.85f32).powi(k as i32);
                k += 1.0;
            }
            if s > best {
                best = s;
                best_phi = phi;
            }
            phi += 0.5;
        }
        let mut frame = n as f64 - best_phi;
        while frame + tau <= n as f64 {
            frame += tau;
        }
        self.last_frame = frame;
        self.predicted = false;
        self.next_beat =
            (self.frame_time(frame) + self.nudge + self.period_s).max(self.last_beat + 0.4 * self.period_s);
    }

    fn score_salience(&mut self, frame: f64) {
        let f = frame.round() as i64;
        let tau = self.tau.round() as i64;
        let newest = self.frames as i64 - 1;
        if f - tau < 0 || f + 2 > newest || tau < 4 {
            return;
        }
        let peak = (f - 2..=f + 2).map(|g| self.det.at(g as u64)).fold(0.0f32, f32::max);
        let mean = (f - tau + 1..=f).map(|g| self.det.at(g as u64)).sum::<f32>() / tau as f32;
        let s = if mean > 1e-6 { peak / mean } else { 0.0 };
        self.salience += 0.2 * (s - self.salience);
    }

    fn score_presence(&mut self, frame: f64) {
        let f = frame.round() as i64;
        let newest = self.frames as i64 - 1;
        if f < 2 || f + 2 > newest {
            return;
        }
        let peak = (f - 2..=f + 2).map(|g| self.det.at(g as u64)).fold(0.0f32, f32::max);
        if peak > PRESENCE_TRACK * self.beat_peak_avg || self.beat_peak_avg <= 0.0 {
            self.beat_peak_avg += PRESENCE_RATE * (peak - self.beat_peak_avg);
        } else {
            self.beat_peak_avg *= PRESENCE_SAG;
        }
        let now = if self.beat_peak_avg > 1e-6 { peak / self.beat_peak_avg } else { 0.0 };
        self.recent_presence.rotate_right(1);
        self.recent_presence[0] = now;
        self.presence = self.recent_presence.iter().copied().fold(0.0, f32::max);
    }

    pub fn tick(&mut self, acf: &Acf, ctx: &TickCtx) {
        if !ctx.active {
            self.confidence *= INACTIVE_DECAY;
            self.lock_ticks = 0;
            return;
        }
        if ctx.t_now >= self.manual_until {
            self.update_tempo(acf);
        }
        if self.tau <= 0.0 {
            return;
        }
        let strength = acf.strength(self.tau);
        if !self.locked {
            self.lock_ticks = if strength > LOCK_STRENGTH { self.lock_ticks + 1 } else { 0 };
            if self.lock_ticks >= LOCK_TICKS {
                let n = self.frames as f64 - 1.0;
                self.lock_at(n - self.tau as f64 * 0.5);
            }
            return;
        }
        let sal = ((self.salience - 2.0) / 2.5).clamp(0.0, 1.0);
        let target = strength * (0.4 + 0.6 * sal);
        self.confidence += 0.15 * (target - self.confidence);
    }

    fn update_tempo(&mut self, acf: &Acf) {
        let (cand, cand_s) = acf.best_tempo(None);
        if cand_s <= 0.0 {
            return;
        }
        if self.tau <= 0.0 {
            self.set_tau(acf.refine(cand, cand * TRACK_SPAN));
            return;
        }
        let (local, local_s) = acf.best_tempo(Some((self.tau, TRACK_SPAN)));
        if (cand / self.tau - 1.0).abs() > TRACK_SPAN && cand_s > SWITCH_RATIO * local_s {
            if (cand / self.challenger - 1.0).abs() < TRACK_SPAN {
                self.challenge += 1;
            } else {
                self.challenger = cand;
                self.challenge = 1;
            }
            if self.challenge >= SWITCH_TICKS {
                self.set_tau(acf.refine(cand, cand * TRACK_SPAN));
                self.challenge = 0;
                self.tempo_switches += 1;
                if self.locked {
                    self.snap_phase();
                }
                return;
            }
        } else {
            self.challenge = 0;
        }
        let exact = acf.refine(local, local * TRACK_SPAN);
        self.set_tau(self.tau + 0.5 * (exact - self.tau));
    }

    pub fn emit(&mut self, t_now: f64, mut on_beat: impl FnMut(f64, u64)) {
        if !self.locked {
            return;
        }
        while self.next_beat <= t_now {
            self.last_beat = self.next_beat;
            self.beat_index += 1;
            on_beat(self.next_beat, self.beat_index);
            self.score_salience(self.last_frame);
            self.last_frame = self.time_frame(self.last_beat - self.nudge);
            self.pending_score = Some(self.last_frame);
            self.predicted = false;
            self.next_beat = self.last_beat + self.period_s;
        }
    }
}
