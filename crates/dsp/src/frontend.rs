use crate::snapshot::NUM_BANDS;
use realfft::num_complex::Complex;
use realfft::{RealFftPlanner, RealToComplex};
use std::sync::Arc;

pub const TIMBRE: usize = 8;
pub const CHROMA: usize = 12;
const LOG_GAMMA: f32 = 1000.0;
const MEDIAN_LEN: usize = 31;
const WEIGHT_MIN: f32 = 0.5;
const WEIGHT_MAX: f32 = 2.0;
const ONSET_FLOOR: f32 = 0.3;
const PEAK_AVG_RATE: f32 = 0.05;

#[derive(Clone, Copy, Default)]
pub struct FrameOut {
    pub odf: f32,
    pub odf_low: f32,
    pub odf_snare: f32,
    pub odf_high: f32,
    pub kick_onset: bool,
    pub snare_onset: bool,
    pub hat_onset: bool,
    pub rms: f32,
    pub low: f32,
    pub mid: f32,
    pub high: f32,
    pub centroid: f32,
    pub chroma_ready: bool,
}

struct Picker {
    ring: [f32; MEDIAN_LEN],
    pos: usize,
    prev: f32,
    prev2: f32,
    scale: f32,
    hold: u32,
    refractory: u32,
    peak_avg: f32,
}

impl Picker {
    fn new(refractory: u32) -> Self {
        Self { ring: [0.0; MEDIAN_LEN], pos: 0, prev: 0.0, prev2: 0.0, scale: 1e-3, hold: 0, refractory, peak_avg: 0.0 }
    }

    fn push(&mut self, v: f32) -> bool {
        let mut sorted = self.ring;
        sorted.sort_unstable_by(f32::total_cmp);
        let median = sorted[MEDIAN_LEN / 2];
        self.scale = (self.scale * 0.999).max(v);
        let thr = (1.5 * median + 0.2 * self.scale).max(ONSET_FLOOR * self.peak_avg);
        self.hold = self.hold.saturating_sub(1);
        let onset = self.hold == 0 && self.prev > self.prev2 && self.prev >= v && self.prev > thr;
        if onset {
            self.hold = self.refractory;
            self.peak_avg += PEAK_AVG_RATE * (self.prev - self.peak_avg);
        }
        self.ring[self.pos] = v;
        self.pos = (self.pos + 1) % MEDIAN_LEN;
        self.prev2 = self.prev;
        self.prev = v;
        onset
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Region {
    Kick,
    SnareBody,
    Mid,
    SnareNoise,
    Air,
}

pub struct Frontend {
    pub sr: f32,
    pub n: usize,
    pub hop: usize,
    pub fr: f32,
    fft: Arc<dyn RealToComplex<f32>>,
    window: Vec<f32>,
    win_norm: f32,
    ring: Vec<f32>,
    pos: usize,
    fft_in: Vec<f32>,
    fft_out: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,
    cfft: Arc<dyn RealToComplex<f32>>,
    cwindow: Vec<f32>,
    cin: Vec<f32>,
    cout: Vec<Complex<f32>>,
    cscratch: Vec<Complex<f32>>,
    cbins: Vec<(u32, u8)>,
    chroma_every: usize,
    chroma_count: usize,
    pub chroma: [f32; CHROMA],
    band_edges: [usize; NUM_BANDS + 1],
    region: [Region; NUM_BANDS],
    prior: [f32; NUM_BANDS],
    pub weights: [f32; NUM_BANDS],
    mean_flux: [f32; NUM_BANDS],
    beat_flux: [f32; NUM_BANDS],
    weight_tick: u32,
    prev_log: [f32; NUM_BANDS],
    flux: [f32; NUM_BANDS],
    pub band_db: [f32; NUM_BANDS],
    pub timbre: [f32; TIMBRE],
    pick: [Picker; 3],
}

impl Frontend {
    pub fn new(sample_rate: u32) -> Self {
        let sr = sample_rate as f32;
        let n = ((sr * 0.0213) as usize).next_power_of_two().max(256);
        let hop = n / 4;
        let nc = n * 4;
        let mut planner = RealFftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(n);
        let cfft = planner.plan_fft_forward(nc);
        let hann = |len: usize| -> Vec<f32> {
            (0..len).map(|i| (std::f32::consts::PI * i as f32 / len as f32).sin().powi(2)).collect()
        };
        let window = hann(n);
        let cwindow = hann(nc);
        let win_norm = 2.0 / window.iter().sum::<f32>();

        let bin_hz = sr / n as f32;
        let nyq = n / 2;
        let mut band_edges = [0usize; NUM_BANDS + 1];
        let f_hi = 16000.0f32.min(sr * 0.45);
        for (i, e) in band_edges.iter_mut().enumerate() {
            let f = 30.0 * (f_hi / 30.0).powf(i as f32 / NUM_BANDS as f32);
            *e = ((f / bin_hz).round() as usize).clamp(1, nyq);
        }
        for i in 1..=NUM_BANDS {
            if band_edges[i] <= band_edges[i - 1] {
                band_edges[i] = (band_edges[i - 1] + 1).min(nyq + 1);
            }
        }
        let mut region = [Region::Mid; NUM_BANDS];
        let mut prior = [1.0; NUM_BANDS];
        for b in 0..NUM_BANDS {
            let fc = 0.5 * (band_edges[b] + band_edges[b + 1]) as f32 * bin_hz;
            let (r, w) = match fc {
                f if f < 150.0 => (Region::Kick, 2.0),
                f if f < 320.0 => (Region::SnareBody, 1.3),
                f if f < 2000.0 => (Region::Mid, 0.8),
                f if f < 6000.0 => (Region::SnareNoise, 1.2),
                _ => (Region::Air, 0.7),
            };
            region[b] = r;
            prior[b] = w;
        }

        let cbin_hz = sr / nc as f32;
        let cbins = (1..nc / 2)
            .filter_map(|k| {
                let f = k as f32 * cbin_hz;
                (100.0..2000.0).contains(&f).then(|| {
                    let midi = 69.0 + 12.0 * (f / 440.0).log2();
                    (k as u32, (midi.round() as i32).rem_euclid(12) as u8)
                })
            })
            .collect();

        Self {
            sr,
            n,
            hop,
            fr: sr / hop as f32,
            fft_in: fft.make_input_vec(),
            fft_out: fft.make_output_vec(),
            scratch: fft.make_scratch_vec(),
            fft,
            window,
            win_norm,
            ring: vec![0.0; nc],
            pos: 0,
            cin: cfft.make_input_vec(),
            cout: cfft.make_output_vec(),
            cscratch: cfft.make_scratch_vec(),
            cfft,
            cwindow,
            cbins,
            chroma_every: 4,
            chroma_count: 0,
            chroma: [0.0; CHROMA],
            band_edges,
            region,
            prior,
            weights: prior,
            mean_flux: [1e-3; NUM_BANDS],
            beat_flux: [1e-3; NUM_BANDS],
            weight_tick: 0,
            prev_log: [0.0; NUM_BANDS],
            flux: [0.0; NUM_BANDS],
            band_db: [-120.0; NUM_BANDS],
            timbre: [0.0; TIMBRE],
            pick: std::array::from_fn(|_| Picker::new((0.06 * sr / hop as f32) as u32)),
        }
    }

    pub fn spectrum(&self) -> &[Complex<f32>] {
        &self.fft_out
    }

    pub fn push_sample(&mut self, x: f32) {
        self.ring[self.pos] = x;
        self.pos = (self.pos + 1) % self.ring.len();
    }

    fn recent(&self, len: usize, i: usize) -> f32 {
        let cap = self.ring.len();
        self.ring[(self.pos + cap - len + i) % cap]
    }

    pub fn frame(&mut self, near_beat: bool) -> FrameOut {
        let mut sq = 0.0;
        for i in 0..self.n {
            let s = self.recent(self.n, i);
            sq += s * s;
            self.fft_in[i] = s * self.window[i];
        }
        let _ = self.fft.process_with_scratch(&mut self.fft_in, &mut self.fft_out, &mut self.scratch);

        let norm2 = self.win_norm * self.win_norm;
        let mut out = FrameOut { rms: (sq / self.n as f32).sqrt(), ..Default::default() };
        let (mut cen_num, mut cen_den) = (0.0f32, 0.0f32);
        for b in 0..NUM_BANDS {
            let (a, e) = (self.band_edges[b], self.band_edges[b + 1]);
            let p: f32 = self.fft_out[a..e].iter().map(|c| c.norm_sqr()).sum();
            let mag = (p * norm2 / (e - a) as f32).sqrt();
            let l = (1.0 + LOG_GAMMA * mag).ln();
            let d = (l - self.prev_log[b]).max(0.0);
            self.prev_log[b] = l;
            self.flux[b] = d;
            self.band_db[b] = 20.0 * (mag + 1e-7).log10();
            let wd = self.weights[b] * d;
            out.odf += wd;
            match self.region[b] {
                Region::Kick => {
                    out.odf_low += d;
                    out.low += mag;
                }
                Region::SnareBody | Region::SnareNoise => {
                    out.odf_snare += d;
                    out.mid += mag;
                }
                Region::Mid => out.mid += mag,
                Region::Air => {
                    out.odf_high += d;
                    out.high += mag;
                }
            }
            cen_num += b as f32 * l;
            cen_den += l;
        }
        for (g, t) in self.timbre.iter_mut().enumerate() {
            let k = NUM_BANDS / TIMBRE;
            *t = self.prev_log[g * k..(g + 1) * k].iter().sum::<f32>() / k as f32;
        }
        out.centroid = if cen_den > 1e-6 { cen_num / cen_den / (NUM_BANDS - 1) as f32 } else { 0.0 };
        out.kick_onset = self.pick[0].push(out.odf_low);
        out.snare_onset = self.pick[1].push(out.odf_snare);
        out.hat_onset = self.pick[2].push(out.odf_high);
        self.learn_weights(near_beat);

        self.chroma_count += 1;
        if self.chroma_count >= self.chroma_every {
            self.chroma_count = 0;
            self.compute_chroma();
            out.chroma_ready = true;
        }
        out
    }

    fn learn_weights(&mut self, near_beat: bool) {
        for b in 0..NUM_BANDS {
            self.mean_flux[b] += 0.002 * (self.flux[b] - self.mean_flux[b]);
            if near_beat {
                self.beat_flux[b] += 0.02 * (self.flux[b] - self.beat_flux[b]);
            }
        }
        self.weight_tick += 1;
        if self.weight_tick < 64 {
            return;
        }
        self.weight_tick = 0;
        let ratio: [f32; NUM_BANDS] = std::array::from_fn(|b| self.beat_flux[b] / (self.mean_flux[b] + 1e-4));
        let mean = (ratio.iter().sum::<f32>() / NUM_BANDS as f32).max(1e-6);
        for ((w, p), r) in self.weights.iter_mut().zip(self.prior).zip(ratio) {
            *w = p * (r / mean).clamp(WEIGHT_MIN, WEIGHT_MAX);
        }
    }

    fn compute_chroma(&mut self) {
        let nc = self.cin.len();
        for i in 0..nc {
            self.cin[i] = self.recent(nc, i) * self.cwindow[i];
        }
        let _ = self.cfft.process_with_scratch(&mut self.cin, &mut self.cout, &mut self.cscratch);
        let mut c = [0.0f32; CHROMA];
        for &(k, pc) in &self.cbins {
            c[pc as usize] += self.cout[k as usize].norm();
        }
        let sum: f32 = c.iter().sum();
        if sum > 1e-6 {
            for v in &mut c {
                *v /= sum;
            }
        }
        self.chroma = c;
    }
}
