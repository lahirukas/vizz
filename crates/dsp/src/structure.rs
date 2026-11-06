use crate::frontend::{CHROMA, TIMBRE};
use crate::meter::BEATS_PER_BAR;

const BAR_RING: usize = 64;
const NOVELTY_SPAN: usize = 4;
const BEAT_RING: usize = 16;
const FEAT: usize = TIMBRE + CHROMA + 1;

#[derive(Clone, Copy, Default)]
pub struct BeatSummary {
    pub low: f32,
    pub high: f32,
    pub centroid: f32,
    pub energy: f32,
    pub snare_onsets: u32,
    pub kick_onsets: u32,
    pub timbre: [f32; TIMBRE],
    pub chroma: [f32; CHROMA],
}

#[derive(Clone, Copy)]
struct Bar {
    feat: [f32; FEAT],
}

pub struct Structure {
    bars: [Bar; BAR_RING],
    bar_count: u64,
    bar_acc: [f32; FEAT],
    bar_beats: u32,
    novelty32: [f32; 32],
    pub phrase_len: u32,
    pub phrase_offset: u64,
    beats: [BeatSummary; BEAT_RING],
    beat_count: u64,
    low_long: f32,
    snare_rate_long: f32,
    pub drop_probability: f32,
    build_peak: f32,
    build_low_min: f32,
    since_build: u32,
    pub drops: u32,
    build_anchor: Option<u64>,
    bass_drop_bar: Option<u64>,
}

impl Default for Structure {
    fn default() -> Self {
        Self::new()
    }
}

impl Structure {
    pub fn new() -> Self {
        Self {
            bars: [Bar { feat: [0.0; FEAT] }; BAR_RING],
            bar_count: 0,
            bar_acc: [0.0; FEAT],
            bar_beats: 0,
            novelty32: [0.0; 32],
            phrase_len: 8,
            phrase_offset: 0,
            beats: [BeatSummary::default(); BEAT_RING],
            beat_count: 0,
            low_long: 0.0,
            snare_rate_long: 1.0,
            drop_probability: 0.0,
            build_peak: 0.0,
            build_low_min: f32::MAX,
            since_build: u32::MAX,
            drops: 0,
            build_anchor: None,
            bass_drop_bar: None,
        }
    }

    // Bars are closed one beat late, so on a downbeat the bar in progress is one past bar_count.
    fn current_bar(&self, beat_in_bar: u32) -> u64 {
        self.bar_count + (beat_in_bar == 1 && self.bar_beats > 0) as u64
    }

    pub fn phrase_pos(&self, beat_in_bar: u32, bar_phase: f32) -> f32 {
        let len = self.phrase_len as u64;
        let bar_in = (self.current_bar(beat_in_bar) + len * 64 - self.phrase_offset % len) % len;
        ((bar_in as f32 + bar_phase) / len as f32).clamp(0.0, 1.0)
    }

    pub fn drop_eta_beats(&self, beat_in_bar: u32, beat_phase: f32) -> f32 {
        let cb = self.current_bar(beat_in_bar);
        let target = match self.build_anchor {
            Some(a) => {
                let mut t = a + 4;
                while t <= cb {
                    t += 4;
                }
                t
            }
            None => {
                let len = self.phrase_len as u64;
                cb + len - (cb + len * 64 - self.phrase_offset % len) % len
            }
        };
        ((target - cb) * BEATS_PER_BAR) as f32 - (beat_in_bar as f32 - 1.0) - beat_phase
    }

    pub fn on_beat(&mut self, s: &BeatSummary, beat_in_bar: u32) -> bool {
        if beat_in_bar == 1 && self.bar_beats > 0 {
            self.close_bar();
        }
        for (i, v) in s.timbre.iter().enumerate() {
            self.bar_acc[i] += v;
        }
        for (i, v) in s.chroma.iter().enumerate() {
            self.bar_acc[TIMBRE + i] += v;
        }
        self.bar_acc[FEAT - 1] += s.energy;
        self.bar_beats += 1;

        self.beats[(self.beat_count % BEAT_RING as u64) as usize] = *s;
        self.beat_count += 1;
        let before = self.drop_probability;
        let dropped = self.update_drop(s);
        if dropped || self.drop_probability < 0.15 {
            self.build_anchor = None;
        } else if before < 0.3 && self.drop_probability >= 0.3 && self.build_anchor.is_none() {
            self.build_anchor = Some(self.bass_drop_bar.unwrap_or(self.bar_count + (beat_in_bar >= 4) as u64));
        }
        dropped
    }

    fn close_bar(&mut self) {
        let mut feat = [0.0; FEAT];
        for (f, a) in feat.iter_mut().zip(self.bar_acc.iter()) {
            *f = a / self.bar_beats as f32;
        }
        self.bars[(self.bar_count % BAR_RING as u64) as usize] = Bar { feat };
        self.bar_count += 1;
        self.bar_acc = [0.0; FEAT];
        self.bar_beats = 0;
        if self.bar_count >= 2 * NOVELTY_SPAN as u64 {
            let boundary = self.bar_count - NOVELTY_SPAN as u64;
            let nov = self.novelty(boundary);
            let slot = (boundary % 32) as usize;
            self.novelty32[slot] = 0.7 * self.novelty32[slot] + nov;
            self.update_phrase_grid();
        }
    }

    fn mean_feat(&self, from: u64, to: u64) -> [f32; FEAT] {
        let mut m = [0.0; FEAT];
        for b in from..to {
            for (mi, v) in m.iter_mut().zip(self.bars[(b % BAR_RING as u64) as usize].feat.iter()) {
                *mi += v;
            }
        }
        let n = (to - from).max(1) as f32;
        m.iter_mut().for_each(|v| *v /= n);
        m
    }

    fn novelty(&self, boundary: u64) -> f32 {
        let span = NOVELTY_SPAN as u64;
        let a = self.mean_feat(boundary - span, boundary);
        let b = self.mean_feat(boundary, boundary + span);
        let mut timbre = 0.0;
        for i in 0..TIMBRE {
            timbre += (a[i] - b[i]).abs();
        }
        let mut chroma = 0.0;
        for i in TIMBRE..TIMBRE + CHROMA {
            chroma += (a[i] - b[i]).abs();
        }
        let energy = ((a[FEAT - 1] + 1e-6) / (b[FEAT - 1] + 1e-6)).ln().abs();
        timbre / TIMBRE as f32 + chroma + energy
    }

    fn update_phrase_grid(&mut self) {
        let n = &self.novelty32;
        let s8 = |o: usize| (0..4).map(|m| n[o + 8 * m]).sum::<f32>() / 4.0;
        let o8 = (0..8).max_by(|&a, &b| s8(a).total_cmp(&s8(b))).unwrap_or(0);
        let s16 = |o: usize| (n[o] + n[o + 16]) / 2.0;
        let (a16, b16) = (s16(o8), s16(o8 + 8));
        let (o16, other16) = if a16 >= b16 { (o8, b16) } else { (o8 + 8, a16) };
        let mut len = 8;
        let mut off = o8;
        if s16(o16) > 1.6 * other16 + 1e-3 {
            len = 16;
            off = o16;
            let (a32, b32) = (n[o16], n[o16 + 16]);
            if a32.max(b32) > 1.6 * a32.min(b32) + 1e-3 {
                len = 32;
                off = if a32 >= b32 { o16 } else { o16 + 16 };
            }
        }
        if s8(o8) > 1e-3 {
            self.phrase_len = len;
            self.phrase_offset = off as u64;
        }
    }

    fn recent(&self, back: u64) -> &BeatSummary {
        &self.beats[((self.beat_count - 1 - back) % BEAT_RING as u64) as usize]
    }

    fn slope(&self, n: u64, f: impl Fn(&BeatSummary) -> f32) -> f32 {
        let n = n.min(self.beat_count);
        if n < 3 {
            return 0.0;
        }
        let (mut sx, mut sy, mut sxx, mut sxy) = (0.0, 0.0, 0.0, 0.0);
        for i in 0..n {
            let x = i as f32;
            let y = f(self.recent(n - 1 - i));
            sx += x;
            sy += y;
            sxx += x * x;
            sxy += x * y;
        }
        let nf = n as f32;
        (nf * sxy - sx * sy) / (nf * sxx - sx * sx).max(1e-6)
    }

    fn update_drop(&mut self, s: &BeatSummary) -> bool {
        let recent_low = (s.low + self.recent(1.min(self.beat_count - 1)).low) * 0.5;
        if self.low_long == 0.0 {
            self.low_long = s.low;
        }
        let bass_drop = (1.0 - recent_low / (self.low_long + 1e-6)).clamp(0.0, 1.0);
        if bass_drop > 0.5 && self.bass_drop_bar.is_none() {
            self.bass_drop_bar = Some(self.bar_count);
        } else if bass_drop < 0.3 {
            self.bass_drop_bar = None;
        }
        let riser = ((self.slope(8, |b| b.centroid) * 40.0).clamp(0.0, 1.0)
            * (self.slope(8, |b| (b.high + 1e-6).ln()) * 2.0).clamp(0.0, 1.0))
        .sqrt();
        let roll_rate = (s.snare_onsets + self.recent(1.min(self.beat_count - 1)).snare_onsets) as f32 * 0.5;
        let roll = ((roll_rate - self.snare_rate_long - 0.5) / 2.0).clamp(0.0, 1.0);
        let x = 1.2 * bass_drop + 0.9 * riser + 0.9 * roll - 1.3;
        let build = 1.0 / (1.0 + (-4.0 * x).exp());

        let mut dropped = false;
        if self.drop_probability > 0.5 {
            self.build_peak = self.build_peak.max(self.drop_probability);
            self.build_low_min = self.build_low_min.min(s.low);
            self.since_build = 0;
        } else {
            self.since_build = self.since_build.saturating_add(1);
        }
        if self.since_build <= 4
            && self.build_peak > 0.5
            && s.kick_onsets > 0
            && s.low > 2.0 * self.build_low_min.max(1e-6)
            && s.low > 0.6 * self.low_long
        {
            dropped = true;
            self.drops += 1;
            self.drop_probability = 0.0;
            self.build_peak = 0.0;
            self.build_low_min = f32::MAX;
            self.since_build = u32::MAX;
        } else {
            self.drop_probability += 0.35 * (build - self.drop_probability);
        }
        if build < 0.3 {
            let k = if self.beat_count < 16 { 0.25 } else { 0.05 };
            self.snare_rate_long += k * (s.snare_onsets as f32 - self.snare_rate_long);
        }
        let a = if bass_drop > 0.5 { 0.01 } else { 0.08 };
        self.low_long += a * (s.low - self.low_long);
        dropped
    }
}
