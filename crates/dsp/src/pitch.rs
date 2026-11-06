use realfft::num_complex::Complex;

const F_LO: f32 = 400.0;
const F_HI: f32 = 4500.0;
const WARMUP_SECS: f32 = 0.3;
const FLOOR_RISE_DB_PER_SEC: f32 = 1.5;
const FLOOR_RATE: f32 = 0.03;
const FLOOR_GATE: f32 = 3.0;
const OVERSUBTRACT: f32 = 3.0;
const SNR_ON_DB: f32 = 14.0;
const SNR_SPAN_DB: f32 = 8.0;
const TONAL_LO: f32 = 0.5;
const TONAL_HI: f32 = 0.8;
const LOW_LO: f32 = 0.1;
const LOW_HI: f32 = 0.5;
const MIN_POWER: f32 = 1e-9;
const PURITY_LO: f32 = 0.55;
const PURITY_HI: f32 = 0.8;
const HARMONIC_LO: f32 = 0.04;
const HARMONIC_HI: f32 = 0.25;
const ON_FRAMES: u32 = 6;
const ONSET_STABLE_SEMITONES: f32 = 0.8;
const OFF_FRAMES: u32 = 7;
const JUMP_SEMITONES: f32 = 7.0;
const GLIDE_HZ: f32 = 9.0;
const ATTACK_SECS: f32 = 0.015;
const RELEASE_SECS: f32 = 0.18;

#[derive(Clone, Copy, Default, Debug)]
pub struct WhistleOut {
    pub hz: f32,
    pub note: f32,
    pub strength: f32,
    pub voiced: bool,
}

pub struct Whistle {
    track_lo: usize,
    lo: usize,
    hi: usize,
    bin_hz: f32,
    dt: f32,
    floor: Vec<f32>,
    clean: Vec<f32>,
    rise: f32,
    warm_frames: u32,
    seen: u32,
    on: u32,
    off: u32,
    voiced: bool,
    jump: u32,
    onset_note: f32,
    note: f32,
    vel: f32,
    strength: f32,
    attack: f32,
    release: f32,
}

fn smoothstep(lo: f32, hi: f32, x: f32) -> f32 {
    let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn hz_to_note(hz: f32) -> f32 {
    69.0 + 12.0 * (hz / 440.0).log2()
}

pub fn note_to_hz(note: f32) -> f32 {
    440.0 * ((note - 69.0) / 12.0).exp2()
}

impl Whistle {
    pub fn new(sample_rate: f32, fft_len: usize, hop: usize) -> Self {
        let bin_hz = sample_rate / fft_len as f32;
        let lo = ((F_LO / bin_hz) as usize).max(2);
        let hi = ((F_HI.min(sample_rate * 0.45) / bin_hz) as usize).min(fft_len / 2 - 2);
        let dt = hop as f32 / sample_rate;
        Self {
            track_lo: (lo / 2).max(2),
            lo,
            hi,
            bin_hz,
            dt,
            floor: vec![0.0; hi + 1],
            clean: vec![0.0; hi + 1],
            rise: 10f32.powf(FLOOR_RISE_DB_PER_SEC * dt / 10.0),
            warm_frames: (WARMUP_SECS / dt) as u32,
            seen: 0,
            on: 0,
            off: 0,
            voiced: false,
            jump: 0,
            onset_note: 0.0,
            note: 0.0,
            vel: 0.0,
            strength: 0.0,
            attack: 1.0 - (-dt / ATTACK_SECS).exp(),
            release: 1.0 - (-dt / RELEASE_SECS).exp(),
        }
    }

    #[allow(clippy::needless_range_loop)]
    pub fn process(&mut self, spec: &[Complex<f32>]) -> WhistleOut {
        let (lo, hi) = (self.lo, self.hi);
        let warming = self.seen < self.warm_frames;
        self.seen = self.seen.saturating_add(1);
        let (mut peak_k, mut peak) = (lo, 0.0f32);
        let mut total = 0.0f32;
        for k in self.track_lo..=hi {
            let p = spec[k].norm_sqr();
            let f = &mut self.floor[k];
            if warming || *f <= 0.0 {
                *f = if *f <= 0.0 { p } else { *f + 0.3 * (p - *f) };
            } else if p < FLOOR_GATE * *f {
                *f += FLOOR_RATE * (p - *f);
            } else {
                *f *= self.rise;
            }
            let c = (p - OVERSUBTRACT * *f).max(0.0);
            self.clean[k] = c;
            if k < lo {
                continue;
            }
            total += c;
            if c > peak {
                peak = c;
                peak_k = k;
            }
        }

        let mut evidence = 0.0;
        let mut hz = 0.0;
        if !warming && peak > MIN_POWER && peak_k > lo && peak_k < hi {
            let a = (peak_k.saturating_sub(2)).max(lo);
            let b = (peak_k + 2).min(hi);
            let local: f32 = self.clean[a..=b].iter().sum();
            let tonal = local / total.max(1e-20);
            let p = spec[peak_k].norm_sqr();
            let snr_db = 10.0 * (p / self.floor[peak_k].max(1e-20)).log10();
            let side: f32 = (peak_k.saturating_sub(8).max(lo)..peak_k.saturating_sub(2))
                .chain(peak_k + 3..=(peak_k + 8).min(hi))
                .map(|k| self.clean[k])
                .sum();
            let purity = local / (local + side).max(1e-20);
            let near = |k: usize| {
                self.clean[k.saturating_sub(1).max(self.track_lo)..=(k + 1).min(hi)].iter().copied().fold(0.0, f32::max)
            };
            let below = if peak_k / 2 >= self.track_lo { near(peak_k / 2) } else { 0.0 };
            let above = if peak_k * 2 < hi { near(peak_k * 2) } else { 0.0 };
            let harmonic = below.max(above) / peak;
            let low = self.clean[self.track_lo..lo].iter().sum::<f32>() / local.max(1e-20);
            evidence = smoothstep(SNR_ON_DB - SNR_SPAN_DB, SNR_ON_DB + SNR_SPAN_DB, snr_db)
                * smoothstep(TONAL_LO, TONAL_HI, tonal)
                * smoothstep(PURITY_LO, PURITY_HI, purity)
                * (1.0 - smoothstep(HARMONIC_LO, HARMONIC_HI, harmonic))
                * (1.0 - smoothstep(LOW_LO, LOW_HI, low));
            let mag = |k: usize| spec[k].norm().max(1e-12).ln();
            let (l, c, r) = (mag(peak_k - 1), mag(peak_k), mag(peak_k + 1));
            let den = l - 2.0 * c + r;
            let delta = if den < 0.0 { (0.5 * (l - r) / den).clamp(-0.5, 0.5) } else { 0.0 };
            hz = (peak_k as f32 + delta) * self.bin_hz;
        }

        if evidence > 0.5 {
            let n = hz_to_note(hz);
            if self.on > 0 && !self.voiced && (n - self.onset_note).abs() > ONSET_STABLE_SEMITONES {
                self.on = 0;
            }
            if self.on == 0 {
                self.onset_note = n;
            }
            self.on += 1;
            self.off = 0;
        } else if evidence < 0.25 {
            self.off += 1;
            self.on = 0;
        }
        let was = self.voiced;
        if !self.voiced && self.on >= ON_FRAMES {
            self.voiced = true;
        } else if self.voiced && self.off >= OFF_FRAMES {
            self.voiced = false;
        }

        if self.voiced && hz > 0.0 && evidence > 0.25 {
            let target = hz_to_note(hz);
            if !was {
                self.note = target;
                self.vel = 0.0;
                self.jump = 0;
            } else if (target - self.note).abs() > JUMP_SEMITONES && self.jump < 2 {
                self.jump += 1;
            } else {
                self.jump = 0;
                // Critically damped spring: x'' = w^2 (target - x) - 2w x'.
                let w = std::f32::consts::TAU * GLIDE_HZ;
                let acc = w * w * (target - self.note) - 2.0 * w * self.vel;
                self.vel += acc * self.dt;
                self.note += self.vel * self.dt;
            }
        }

        let goal = if self.voiced { evidence.max(0.35) } else { 0.0 };
        let k = if goal > self.strength { self.attack } else { self.release };
        self.strength += k * (goal - self.strength);

        WhistleOut {
            hz: if self.strength > 0.01 { note_to_hz(self.note) } else { 0.0 },
            note: self.note,
            strength: self.strength,
            voiced: self.voiced,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use realfft::RealFftPlanner;

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> f32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 >> 40) as f32 / (1u64 << 23) as f32 - 1.0
        }
    }

    fn run(sr: f32, signal: &[f32]) -> Vec<WhistleOut> {
        let n = 1024;
        let hop = 256;
        let fft = RealFftPlanner::<f32>::new().plan_fft_forward(n);
        let win: Vec<f32> = (0..n).map(|i| (std::f32::consts::PI * i as f32 / n as f32).sin().powi(2)).collect();
        let mut inp = fft.make_input_vec();
        let mut out = fft.make_output_vec();
        let mut w = Whistle::new(sr, n, hop);
        let mut res = Vec::new();
        let mut start = 0;
        while start + n <= signal.len() {
            for i in 0..n {
                inp[i] = signal[start + i] * win[i];
            }
            fft.process(&mut inp, &mut out).unwrap();
            res.push(w.process(&out));
            start += hop;
        }
        res
    }

    fn noise(len: usize, amp: f32, seed: u64) -> Vec<f32> {
        let mut r = Rng(seed);
        let mut lp = 0.0f32;
        (0..len)
            .map(|_| {
                lp += 0.15 * (r.next() - lp);
                amp * (lp * 3.0 + 0.3 * r.next())
            })
            .collect()
    }

    #[test]
    fn follows_whistle_in_noise() {
        let sr = 48000.0;
        let len = (sr * 8.0) as usize;
        let mut x = noise(len, 0.05, 7);
        let mut phase = 0.0f32;
        let mut truth = vec![0.0f32; len];
        for i in 0..len {
            let t = i as f32 / sr;
            let on = (1.0..3.5).contains(&t) || (4.5..7.0).contains(&t);
            let glide = 1400.0 + 800.0 * (t * 0.8).sin() + 25.0 * (t * 6.0 * std::f32::consts::TAU).sin();
            phase += std::f32::consts::TAU * glide / sr;
            if on {
                x[i] += 0.08 * phase.sin();
                truth[i] = glide;
            }
        }
        let out = run(sr, &x);
        let (mut voiced_ok, mut voiced_n, mut false_on, mut silent_n, mut err) = (0, 0, 0, 0, Vec::new());
        for (f, o) in out.iter().enumerate() {
            let center = f * 256 + 512;
            let t = center as f32 / sr;
            let edge = [1.0, 3.5, 4.5, 7.0].iter().any(|e| (t - e).abs() < 0.12);
            if edge {
                continue;
            }
            if truth[center] > 0.0 {
                voiced_n += 1;
                if o.strength > 0.3 {
                    voiced_ok += 1;
                    err.push((1200.0 * (o.hz / truth[center]).log2()).abs());
                }
            } else if t > 0.4 {
                silent_n += 1;
                false_on += o.voiced as usize;
            }
        }
        err.sort_by(f32::total_cmp);
        let recall = voiced_ok as f32 / voiced_n as f32;
        let median = err[err.len() / 2];
        let p90 = err[err.len() * 9 / 10];
        assert!(recall > 0.9, "whistle recall {recall:.2}");
        assert!(median < 25.0 && p90 < 60.0, "pitch error median {median:.1} p90 {p90:.1} cents");
        assert!(false_on * 50 < silent_n, "false whistle on {false_on}/{silent_n} noise frames");
    }

    #[test]
    fn ignores_noise_and_clicks() {
        let sr = 48000.0;
        let len = (sr * 6.0) as usize;
        let mut x = noise(len, 0.2, 11);
        let mut r = Rng(3);
        for beat in 0..12 {
            let at = (beat as f32 * 0.5 * sr) as usize;
            for i in 0..2000.min(len - at) {
                x[at + i] += 0.6 * r.next() * (-(i as f32) / 300.0).exp();
            }
        }
        let out = run(sr, &x);
        let on = out.iter().skip(40).filter(|o| o.voiced).count();
        assert!(on * 50 < out.len(), "{on} of {} frames falsely voiced", out.len());
    }
}
