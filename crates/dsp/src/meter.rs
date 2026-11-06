use crate::frontend::CHROMA;

pub const BEATS_PER_BAR: u64 = 4;

pub struct BeatFeatures {
    pub low_accent: f32,
    pub snare_accent: f32,
    pub chroma: [f32; CHROMA],
}

pub struct Meter {
    acc: [f32; BEATS_PER_BAR as usize],
    pub downbeat_pos: u64,
    candidate: u64,
    streak: u32,
    prev_chroma: [f32; CHROMA],
    prev_index: u64,
    mean_change: f32,
    mean_low: f32,
    mean_snare: f32,
    have_prev: bool,
}

impl Default for Meter {
    fn default() -> Self {
        Self::new()
    }
}

fn chroma_distance(a: &[f32; CHROMA], b: &[f32; CHROMA]) -> f32 {
    let (mut dot, mut na, mut nb) = (0.0, 0.0, 0.0);
    for i in 0..CHROMA {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na < 1e-9 || nb < 1e-9 {
        0.0
    } else {
        1.0 - dot / (na * nb).sqrt()
    }
}

impl Meter {
    pub fn new() -> Self {
        Self {
            acc: [0.0; BEATS_PER_BAR as usize],
            downbeat_pos: 0,
            candidate: 0,
            streak: 0,
            prev_chroma: [0.0; CHROMA],
            prev_index: 0,
            mean_change: 1e-3,
            mean_low: 1e-3,
            mean_snare: 1e-3,
            have_prev: false,
        }
    }

    pub fn reset(&mut self) {
        *self = Self { downbeat_pos: self.downbeat_pos, ..Self::new() };
    }

    // Called at beat k with beat k-1's accents and the chroma of segment k-1..k; beat k-1's harmonic change needs both neighbouring segments.
    pub fn on_beat(&mut self, index: u64, f: &BeatFeatures) {
        if self.have_prev && index == self.prev_index + 1 {
            let change = chroma_distance(&self.prev_chroma, &f.chroma);
            self.mean_change += 0.05 * (change - self.mean_change);
            self.mean_low += 0.05 * (f.low_accent - self.mean_low);
            self.mean_snare += 0.05 * (f.snare_accent - self.mean_snare);
            let s = change / (self.mean_change + 1e-4) + 0.6 * f.low_accent / (self.mean_low + 1e-4)
                - 0.4 * f.snare_accent / (self.mean_snare + 1e-4);
            let pos = ((index - 1) % BEATS_PER_BAR) as usize;
            self.acc[pos] += 0.15 * (s - self.acc[pos]);
            self.update_position();
        }
        self.prev_chroma = f.chroma;
        self.prev_index = index;
        self.have_prev = true;
    }

    fn update_position(&mut self) {
        let best = (0..BEATS_PER_BAR as usize).max_by(|&a, &b| self.acc[a].total_cmp(&self.acc[b])).unwrap_or(0) as u64;
        if best == self.downbeat_pos {
            self.streak = 0;
            return;
        }
        if self.acc[best as usize] > self.acc[self.downbeat_pos as usize] + 0.15 {
            if best == self.candidate {
                self.streak += 1;
            } else {
                self.candidate = best;
                self.streak = 1;
            }
            if self.streak >= 2 {
                self.downbeat_pos = best;
                self.streak = 0;
            }
        }
    }

    pub fn beat_in_bar(&self, index: u64) -> u32 {
        ((index + BEATS_PER_BAR - self.downbeat_pos % BEATS_PER_BAR) % BEATS_PER_BAR) as u32 + 1
    }
}
