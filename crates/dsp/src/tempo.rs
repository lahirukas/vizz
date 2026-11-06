pub const BPM_MIN: f32 = 60.0;
pub const BPM_MAX: f32 = 200.0;
const PRIOR_CENTER_BPM: f32 = 130.0;
const DETREND_SECS: f32 = 0.25;
const TOLERANCE: f32 = 0.02;
const SPEC_STEP: f32 = 0.25;
const SPEC_RATE: f32 = 0.1;
const SPEC_FAST_RATE: f32 = 0.5;
const DISAGREE_TICKS: u32 = 1;
const SPEC_MIN_BPM: f32 = 30.0;
const PRIOR_SIGMA_OCT: f32 = 0.5;
const STRENGTH_FLOOR: f32 = 0.04;
const STRENGTH_SPAN: f32 = 0.26;

pub struct Acf {
    values: Vec<f32>,
    work: Vec<f32>,
    prefix: Vec<f32>,
    max_lag: usize,
    valid: usize,
    pub fr: f32,
    spec_lo: f32,
    spec: Vec<f32>,
    inst: Vec<f32>,
    sorted: Vec<f32>,
    spec_fresh: bool,
    median: f32,
    choose_hi: usize,
    disagree: u32,
}

impl Acf {
    pub fn new(fr: f32, window_frames: usize, max_lag_secs: f32) -> Self {
        let max_lag = (max_lag_secs * fr) as usize;
        let spec_lo = 60.0 * fr / BPM_MAX;
        let bins = ((60.0 * fr / SPEC_MIN_BPM - spec_lo) / SPEC_STEP) as usize + 1;
        let choose_hi = ((60.0 * fr / BPM_MIN - spec_lo) / SPEC_STEP) as usize;
        Self {
            values: vec![0.0; max_lag + 2],
            work: vec![0.0; window_frames],
            prefix: vec![0.0; window_frames + 1],
            max_lag,
            valid: 0,
            fr,
            spec_lo,
            spec: vec![0.0; bins],
            inst: vec![0.0; bins],
            sorted: vec![0.0; bins],
            spec_fresh: true,
            median: 0.0,
            choose_hi,
            disagree: 0,
        }
    }

    fn tolerant_comb(&self, tau: f32) -> f32 {
        let lim = self.valid as f32 - 1.0;
        let (mut s, mut n) = (0.0, 0);
        for k in 1..=4 {
            let c = tau * k as f32;
            let hi = (c * (1.0 + TOLERANCE)).ceil();
            if hi > lim {
                break;
            }
            let lo = (c * (1.0 - TOLERANCE)) as usize;
            s += self.values[lo..=hi as usize].iter().copied().fold(f32::MIN, f32::max);
            n += 1;
        }
        if n == 0 {
            0.0
        } else {
            s / n as f32
        }
    }

    pub fn update_spectrum(&mut self) {
        for i in 0..self.inst.len() {
            self.inst[i] = self.tolerant_comb(self.spec_lo + i as f32 * SPEC_STEP);
        }
        let inst_best = self.argmax_metrical(&self.inst);
        let spec_best = self.argmax_metrical(&self.spec);
        let differs = (self.tau_of(inst_best) / self.tau_of(spec_best) - 1.0).abs() > 0.04;
        self.disagree = if differs { self.disagree + 1 } else { 0 };
        let rate = if std::mem::take(&mut self.spec_fresh) {
            1.0
        } else if self.disagree >= DISAGREE_TICKS {
            SPEC_FAST_RATE
        } else {
            SPEC_RATE
        };
        for (s, &v) in self.spec.iter_mut().zip(&self.inst) {
            *s += rate * (v - *s);
        }
        let n = self.choose_hi + 1;
        self.sorted[..n].copy_from_slice(&self.spec[..n]);
        let mid = n / 2;
        self.median = *self.sorted.select_nth_unstable_by(mid, f32::total_cmp).1;
    }

    pub fn support(&self, tau: f32) -> f32 {
        let pos = (tau - self.spec_lo) / SPEC_STEP;
        if pos < 0.0 || pos > (self.spec.len() - 1) as f32 {
            return 0.0;
        }
        interp(&self.spec, pos)
    }

    fn metrical_of(&self, v: &[f32], i: usize) -> f32 {
        let tau = self.tau_of(i);
        let z = (60.0 * self.fr / tau / PRIOR_CENTER_BPM).log2() / PRIOR_SIGMA_OCT;
        let pos = (2.0 * tau - self.spec_lo) / SPEC_STEP;
        let slow = if pos <= (v.len() - 1) as f32 { interp(v, pos).max(0.0) } else { 0.0 };
        v[i].max(0.0) * (-0.5 * z * z).exp() * (1.0 + slow)
    }

    fn metrical(&self, i: usize) -> f32 {
        self.metrical_of(&self.spec, i)
    }

    fn argmax_metrical(&self, v: &[f32]) -> usize {
        (0..=self.choose_hi).max_by(|&a, &b| self.metrical_of(v, a).total_cmp(&self.metrical_of(v, b))).unwrap_or(0)
    }

    pub fn tau_of(&self, i: usize) -> f32 {
        self.spec_lo + i as f32 * SPEC_STEP
    }

    pub fn best_tempo(&self, near: Option<(f32, f32)>) -> (f32, f32) {
        let (mut best_i, mut best) = (0, f32::MIN);
        for i in 0..=self.choose_hi {
            let tau = self.tau_of(i);
            if near.is_some_and(|(c, span)| (tau / c - 1.0).abs() > span) {
                continue;
            }
            let s = self.metrical(i);
            if s > best {
                best = s;
                best_i = i;
            }
        }
        let refine = if best_i > 0 && best_i < self.choose_hi {
            let (l, c, r) = (self.metrical(best_i - 1), best, self.metrical(best_i + 1));
            let den = l - 2.0 * c + r;
            if den < 0.0 {
                (0.5 * (l - r) / den).clamp(-0.5, 0.5)
            } else {
                0.0
            }
        } else {
            0.0
        };
        (self.tau_of(best_i) + refine * SPEC_STEP, best.max(0.0))
    }

    pub fn strength(&self, tau: f32) -> f32 {
        ((self.support(tau) - self.median - STRENGTH_FLOOR) / STRENGTH_SPAN).clamp(0.0, 1.0)
    }

    pub fn tau_range(&self) -> (f32, f32) {
        (60.0 * self.fr / BPM_MAX, (60.0 * self.fr / BPM_MIN).min(self.valid as f32 - 2.0))
    }

    pub fn compute(&mut self, src: impl Iterator<Item = f32>) -> f32 {
        let mut len = 0;
        for (w, v) in self.work.iter_mut().zip(src) {
            *w = v;
            len += 1;
        }
        let mean = self.work[..len].iter().sum::<f32>() / len.max(1) as f32;
        self.prefix[0] = 0.0;
        for i in 0..len {
            self.prefix[i + 1] = self.prefix[i] + self.work[i];
        }
        let half = ((DETREND_SECS * self.fr) as usize / 2).max(1);
        for i in 0..len {
            let (a, b) = (i.saturating_sub(half), (i + half + 1).min(len));
            let local = (self.prefix[b] - self.prefix[a]) / (b - a) as f32;
            self.work[i] = (self.work[i] - local).max(0.0);
        }
        let x = &mut self.work[..len];
        let dmean = x.iter().sum::<f32>() / len.max(1) as f32;
        for v in x.iter_mut() {
            *v -= dmean;
        }
        self.valid = self.max_lag.min(len - len / 4);
        for l in 0..=self.valid {
            let mut s = 0.0f32;
            for i in 0..len - l {
                s += x[i] * x[i + l];
            }
            self.values[l] = s / (len - l) as f32;
        }
        let a0 = self.values[0].max(1e-12);
        for v in &mut self.values[..=self.valid] {
            *v /= a0;
        }
        mean
    }

    pub fn comb(&self, tau: f32) -> f32 {
        let lim = self.valid as f32 - 1.0;
        let (mut s, mut n) = (0.0, 0);
        for k in 1..=4 {
            let lag = tau * k as f32;
            if lag > lim {
                break;
            }
            s += interp(&self.values[..=self.valid], lag);
            n += 1;
        }
        if n == 0 {
            0.0
        } else {
            s / n as f32
        }
    }

    pub fn refine(&self, center: f32, span: f32) -> f32 {
        let (lo, hi) = self.tau_range();
        let mut best_t = center;
        let mut best = self.comb(center);
        let mut t = (center - span).max(lo);
        while t <= (center + span).min(hi) {
            let s = self.comb(t);
            if s > best {
                best = s;
                best_t = t;
            }
            t += 0.02;
        }
        best_t
    }
}

pub fn interp(v: &[f32], pos: f32) -> f32 {
    if pos <= 0.0 {
        return v[0];
    }
    let i = pos as usize;
    if i + 1 >= v.len() {
        return v[v.len() - 1];
    }
    let f = pos - i as f32;
    v[i] * (1.0 - f) + v[i + 1] * f
}
