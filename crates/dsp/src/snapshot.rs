use std::sync::atomic::{fence, AtomicU32, Ordering};

pub const NUM_BANDS: usize = 32;
pub const WAVE_LEN: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnalysisSnapshot {
    pub t_audio: f64,
    pub last_beat: f64,
    pub next_beat: f64,
    pub beat_count: u32,
    pub beat_in_bar: u32,
    pub kick_onsets: u32,
    pub snare_onsets: u32,
    pub hat_onsets: u32,
    pub drops: u32,
    pub bpm: f32,
    pub confidence: f32,
    pub beat_phase: f32,
    pub bar_phase: f32,
    pub phrase_pos: f32,
    pub phrase_bars: f32,
    pub next_beat_eta_ms: f32,
    pub drop_probability: f32,
    pub drop_eta_beats: f32,
    pub level: f32,
    pub kick: f32,
    pub snare: f32,
    pub hat: f32,
    pub silent_secs: f32,
    pub pitch_hz: f32,
    pub pitch_note: f32,
    pub whistle: f32,
    pub beat_presence: f32,
    pub bands: [f32; NUM_BANDS],
    pub wave: [f32; WAVE_LEN],
}

impl AnalysisSnapshot {
    pub const EMPTY: Self = Self {
        t_audio: 0.0,
        last_beat: 0.0,
        next_beat: 0.0,
        beat_count: 0,
        beat_in_bar: 1,
        kick_onsets: 0,
        snare_onsets: 0,
        hat_onsets: 0,
        drops: 0,
        bpm: 0.0,
        confidence: 0.0,
        beat_phase: 0.0,
        bar_phase: 0.0,
        phrase_pos: 0.0,
        phrase_bars: 8.0,
        next_beat_eta_ms: 0.0,
        drop_probability: 0.0,
        drop_eta_beats: -1.0,
        level: 0.0,
        kick: 0.0,
        snare: 0.0,
        hat: 0.0,
        silent_secs: 0.0,
        pitch_hz: 0.0,
        pitch_note: 0.0,
        whistle: 0.0,
        beat_presence: 0.0,
        bands: [0.0; NUM_BANDS],
        wave: [0.0; WAVE_LEN],
    };

    fn f64s(&mut self) -> [&mut f64; 3] {
        [&mut self.t_audio, &mut self.last_beat, &mut self.next_beat]
    }

    fn u32s(&mut self) -> [&mut u32; 6] {
        [
            &mut self.beat_count,
            &mut self.beat_in_bar,
            &mut self.kick_onsets,
            &mut self.snare_onsets,
            &mut self.hat_onsets,
            &mut self.drops,
        ]
    }

    fn f32s(&mut self) -> [&mut f32; 18] {
        [
            &mut self.bpm,
            &mut self.confidence,
            &mut self.beat_phase,
            &mut self.bar_phase,
            &mut self.phrase_pos,
            &mut self.phrase_bars,
            &mut self.next_beat_eta_ms,
            &mut self.drop_probability,
            &mut self.drop_eta_beats,
            &mut self.level,
            &mut self.kick,
            &mut self.snare,
            &mut self.hat,
            &mut self.silent_secs,
            &mut self.pitch_hz,
            &mut self.pitch_note,
            &mut self.whistle,
            &mut self.beat_presence,
        ]
    }
}

impl Default for AnalysisSnapshot {
    fn default() -> Self {
        Self::EMPTY
    }
}

const WORDS: usize = 3 * 2 + 6 + 18 + NUM_BANDS + WAVE_LEN;

pub struct SnapshotBus {
    seq: AtomicU32,
    words: [AtomicU32; WORDS],
}

impl Default for SnapshotBus {
    fn default() -> Self {
        Self::new()
    }
}

impl SnapshotBus {
    pub fn new() -> Self {
        Self { seq: AtomicU32::new(0), words: std::array::from_fn(|_| AtomicU32::new(0)) }
    }

    pub fn publish(&self, snapshot: &AnalysisSnapshot) {
        let mut s = *snapshot;
        let seq = self.seq.load(Ordering::Relaxed);
        self.seq.store(seq.wrapping_add(1), Ordering::Relaxed);
        fence(Ordering::Release);
        let mut i = 0;
        let mut put = |v: u32| {
            self.words[i].store(v, Ordering::Relaxed);
            i += 1;
        };
        for f in s.f64s() {
            let b = f.to_bits();
            put(b as u32);
            put((b >> 32) as u32);
        }
        for u in s.u32s() {
            put(*u);
        }
        for f in s.f32s() {
            put(f.to_bits());
        }
        for f in s.bands.iter().chain(s.wave.iter()) {
            put(f.to_bits());
        }
        self.seq.store(seq.wrapping_add(2), Ordering::Release);
    }

    pub fn read(&self, out: &mut AnalysisSnapshot) {
        loop {
            let s1 = self.seq.load(Ordering::Acquire);
            if s1 & 1 == 1 {
                std::hint::spin_loop();
                continue;
            }
            self.read_words(out);
            fence(Ordering::Acquire);
            if self.seq.load(Ordering::Relaxed) == s1 {
                return;
            }
        }
    }

    pub fn version(&self) -> u32 {
        self.seq.load(Ordering::Acquire)
    }

    fn read_words(&self, s: &mut AnalysisSnapshot) {
        let mut i = 0;
        let mut get = || {
            let v = self.words[i].load(Ordering::Relaxed);
            i += 1;
            v
        };
        for f in s.f64s() {
            let lo = get() as u64;
            let hi = get() as u64;
            *f = f64::from_bits(lo | (hi << 32));
        }
        for u in s.u32s() {
            *u = get();
        }
        for f in s.f32s() {
            *f = f32::from_bits(get());
        }
        for f in s.bands.iter_mut().chain(s.wave.iter_mut()) {
            *f = f32::from_bits(get());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let bus = SnapshotBus::new();
        let mut s = AnalysisSnapshot::EMPTY;
        s.t_audio = 12.345678901;
        s.next_beat = 13.0;
        s.beat_count = 42;
        s.drops = 3;
        s.bpm = 128.0;
        s.drop_eta_beats = 7.5;
        s.pitch_hz = 1234.5;
        s.whistle = 0.75;
        s.bands[5] = 0.5;
        s.wave[127] = -0.25;
        bus.publish(&s);
        let mut r = AnalysisSnapshot::EMPTY;
        bus.read(&mut r);
        assert_eq!(s, r);
    }
}
