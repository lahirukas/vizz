use std::time::Instant;
use vizz_dsp::pitch::hz_to_note;
use vizz_dsp::AnalysisSnapshot;

const PULSE_SECS: f32 = 0.12;
const STEER_RATE: f64 = 6.0;
const SNAP_BEATS: f64 = 0.4;
const DRIVE_RATE: f32 = 1.5;
const DRIVE_RELEASE: f32 = 4.0;
const DRIVE_SILENT_RELEASE: f32 = 14.0;
const SILENT_AFTER_SECS: f32 = 0.12;
const NO_ONSET_BEATS: f32 = 1.5;
const PRESENCE_GONE: f32 = 0.25;
const TRAIL_STEP_SECS: f32 = 0.03;
const TRAIL_LEN: usize = 16;
const PITCH_LO_HZ: f32 = 500.0;
const PITCH_HI_HZ: f32 = 2500.0;
const PEAK_HOLD_SECS: f32 = 0.35;
const JUMP_SECS: f32 = 0.6;
const FREE_SCENE_SECS: f32 = 30.0;
const SCENE_BLEND_SECS: f32 = 1.2;
const PEAK_FALL_PER_SEC: f32 = 1.2;

fn smoothstep(lo: f32, hi: f32, x: f32) -> f32 {
    let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn wind(phase: f32) -> f32 {
    ((phase - 0.6) / 0.4).clamp(0.0, 1.0).powi(2)
}

fn pitch_norm(hz: f32) -> f32 {
    let (lo, hi) = (hz_to_note(PITCH_LO_HZ), hz_to_note(PITCH_HI_HZ));
    ((hz_to_note(hz.max(1.0)) - lo) / (hi - lo)).clamp(0.0, 1.0)
}

pub fn at_phase(live: &vizz_render::Params, phase: f32, bpm: f32, beat_in_bar: f32) -> vizz_render::Params {
    let mut p = *live;
    let since = phase * 60.0 / bpm;
    p.drive = p.drive.max(0.9);
    p.phase = phase;
    p.beat_t = since;
    p.pulse = (-since / PULSE_SECS).exp();
    p.wind = wind(phase);
    p.beat_in_bar = beat_in_bar;
    p.bar_phase = (beat_in_bar - 1.0 + phase) / 4.0;
    p
}

pub struct BeatClock {
    last_t_audio: f64,
    anchor: Instant,
    av_offset: f64,
    start: Instant,
    last_frame: Option<Instant>,
    pos: Option<f64>,
    beat_at: Option<Instant>,
    drive: f32,
    pitch: f32,
    trail: [(f32, f32); TRAIL_LEN],
    trail_clock: f32,
    peaks: [f32; 32],
    hold: [f32; 32],
    travel: f32,
    scene: f32,
    scene_target: f32,
    last_phrase: f32,
    drops_seen: Option<u32>,
    jump: f32,
    free_scene: f32,
    onsets_seen: u32,
    since_onset: f32,
}

impl BeatClock {
    pub fn new(av_offset_ms: f32) -> Self {
        let now = Instant::now();
        Self {
            last_t_audio: -1.0,
            anchor: now,
            av_offset: av_offset_ms as f64 / 1000.0,
            start: now,
            last_frame: None,
            pos: None,
            beat_at: None,
            drive: 0.0,
            pitch: 0.5,
            trail: [(0.5, 0.0); TRAIL_LEN],
            trail_clock: 0.0,
            peaks: [0.0; 32],
            hold: [0.0; 32],
            travel: 0.0,
            scene: 0.0,
            scene_target: 0.0,
            last_phrase: 0.0,
            drops_seen: None,
            jump: 0.0,
            free_scene: 0.0,
            onsets_seen: 0,
            since_onset: 0.0,
        }
    }

    pub fn stream_now(&mut self, s: &AnalysisSnapshot, now: Instant) -> f64 {
        if s.t_audio != self.last_t_audio {
            self.last_t_audio = s.t_audio;
            self.anchor = now;
        }
        let ahead = now.duration_since(self.anchor).as_secs_f64().min(0.1);
        s.t_audio + ahead - self.av_offset
    }

    fn target_pos(s: &AnalysisSnapshot, t: f64) -> f64 {
        let period = 60.0 / s.bpm as f64;
        let count = s.beat_count as f64;
        if t >= s.next_beat {
            count + 1.0 + (t - s.next_beat) / period
        } else if t >= s.last_beat {
            count + (t - s.last_beat) / (s.next_beat - s.last_beat).max(0.25 * period)
        } else {
            count - (s.last_beat - t) / period
        }
    }

    fn structure(&mut self, s: &AnalysisSnapshot, p: &mut vizz_render::Params, dt: f32, locked: bool) {
        if self.drops_seen.is_some_and(|d| d != s.drops) {
            self.jump = 1.0;
        }
        self.drops_seen = Some(s.drops);
        self.jump *= (-dt / JUMP_SECS).exp();
        if locked && p.drive > 0.15 {
            if s.phrase_pos + 0.5 < self.last_phrase {
                self.scene_target += 1.0;
            }
            self.free_scene = 0.0;
        } else {
            self.free_scene += dt;
            if self.free_scene > FREE_SCENE_SECS {
                self.free_scene = 0.0;
                self.scene_target += 1.0;
            }
        }
        self.last_phrase = s.phrase_pos;
        self.scene += (self.scene_target - self.scene) * (1.0 - (-dt / SCENE_BLEND_SECS).exp());
        let tempo = if s.bpm > 0.0 { s.bpm / 120.0 } else { 0.5 };
        let speed = (0.2 + 0.25 * tempo) * (0.45 + 0.55 * p.drive)
            + 0.35 * p.level * p.drive
            + 1.4 * p.pulse * p.drive
            + 5.0 * self.jump;
        self.travel += dt * speed;
        p.aux2 = speed;
        p.travel = self.travel;
        p.scene = self.scene;
        p.jump = self.jump;
        p.phrase = s.phrase_pos;
    }

    pub fn set_av_offset(&mut self, ms: f32) {
        self.av_offset = ms as f64 / 1000.0;
    }

    pub fn beat_pos(&self) -> f32 {
        self.pos.unwrap_or(0.0) as f32
    }

    pub fn params(&mut self, s: &AnalysisSnapshot, now: Instant, base: vizz_render::Params) -> vizz_render::Params {
        let t = self.stream_now(s, now);
        let dt = self.last_frame.map_or(0.0, |f| now.duration_since(f).as_secs_f64()).min(0.1);
        self.last_frame = Some(now);
        let mut p = base;
        p.time = now.duration_since(self.start).as_secs_f32();
        p.conf = s.confidence;
        p.bpm = s.bpm;
        p.level = s.level;
        p.kick = s.kick;
        p.snare = s.snare;
        p.hat = s.hat;
        p.drop = s.drop_probability;
        p.drop_eta = if s.drop_eta_beats >= 0.0 { s.drop_eta_beats } else { -1.0 };
        p.dt = (dt as f32).clamp(1.0 / 240.0, 0.1);

        let locked = s.bpm > 0.0 && s.beat_count > 0;
        let silent = s.silent_secs > SILENT_AFTER_SECS;
        let onsets = s.kick_onsets.wrapping_add(s.snare_onsets).wrapping_add(s.hat_onsets);
        if onsets != self.onsets_seen {
            self.onsets_seen = onsets;
            self.since_onset = 0.0;
        } else {
            self.since_onset += dt as f32;
        }
        let beat_len = if s.bpm > 0.0 { 60.0 / s.bpm } else { 0.5 };
        let beats_gone = self.since_onset > (beat_len * NO_ONSET_BEATS).max(0.6) || s.beat_presence < PRESENCE_GONE;
        let goal = if locked && !silent && !beats_gone { smoothstep(0.02, 0.3, s.confidence) } else { 0.0 };
        let rate = if goal >= self.drive {
            DRIVE_RATE
        } else if silent || beats_gone {
            DRIVE_SILENT_RELEASE
        } else {
            DRIVE_RELEASE
        };
        self.drive += (goal - self.drive) * (1.0 - (-rate * dt as f32).exp());
        p.drive = self.drive;

        if locked {
            let period = 60.0 / s.bpm as f64;
            let target = Self::target_pos(s, t);
            let pos = match self.pos {
                Some(pos) if (target - pos).abs() < SNAP_BEATS => {
                    let steered = pos + dt / period + (target - pos) * (1.0 - (-STEER_RATE * dt).exp());
                    steered.max(pos)
                }
                _ => target,
            };
            if self.pos.is_some_and(|old| pos.floor() > old.floor()) {
                self.beat_at = Some(now);
            }
            self.pos = Some(pos);
            let phase = pos.rem_euclid(1.0) as f32;
            let ahead = pos.floor() as i64 - s.beat_count as i64;
            let bib = (s.beat_in_bar as i64 - 1 + ahead).rem_euclid(4) + 1;
            p.phase = phase;
            p.wind = wind(phase);
            p.beat_in_bar = bib as f32;
            p.bar_phase = ((bib - 1) as f32 + phase) / 4.0;
        } else {
            self.pos = None;
        }
        let since = self.beat_at.map_or(10.0, |b| now.duration_since(b).as_secs_f32());
        p.beat_t = since;
        p.pulse = (-since / PULSE_SECS).exp();

        if s.whistle > 0.01 {
            let target = pitch_norm(s.pitch_hz);
            self.pitch += (target - self.pitch) * (1.0 - (-25.0 * dt as f32).exp());
        }
        p.whistle = s.whistle;
        p.pitch = self.pitch;
        self.trail_clock += dt as f32;
        if self.trail_clock >= TRAIL_STEP_SECS {
            self.trail_clock = 0.0;
            self.trail.rotate_right(1);
        }
        self.trail[0] = (self.pitch, s.whistle);
        for (k, &(pos, st)) in self.trail.iter().enumerate() {
            p.trail[k * 2] = pos;
            p.trail[k * 2 + 1] = st;
        }

        for (i, b) in p.bands.iter_mut().enumerate() {
            *b = s.bands[i];
            if *b >= self.peaks[i] {
                self.peaks[i] = *b;
                self.hold[i] = PEAK_HOLD_SECS;
            } else if self.hold[i] > 0.0 {
                self.hold[i] -= dt as f32;
            } else {
                self.peaks[i] = (self.peaks[i] - PEAK_FALL_PER_SEC * dt as f32).max(*b);
            }
        }
        p.peaks = self.peaks;
        self.structure(s, &mut p, dt as f32, locked);
        p.wave = s.wave;
        p
    }
}
