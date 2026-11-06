use std::f32::consts::PI;
use vizz_render::Params;

const WHISTLE_ON: f32 = 0.3;
const FADE_DRAWING: f32 = 1.3;
const FADE_IDLE: f32 = 3.5;
const BASE_SPEED: f32 = 0.5;
const MAX_SPEED: f32 = 1.5;
const RAMP_SECS: f32 = 2.5;
// Pitch is normalised so one semitone is about 0.03; this gives roughly 2.4 rad/s of turn per semitone.
const TURN_GAIN: f32 = 80.0;
const MAX_TURN: f32 = 5.0;
const DEAD_ZONE: f32 = 0.006;
const REF_TAU: f32 = 1.1;
const TAP_MAX_SECS: f32 = 0.35;
const DOUBLE_GAP_SECS: f32 = 0.45;
const BURST_SECS: f32 = 1.1;
const EDGE: f32 = 0.93;

pub struct Sketch {
    pos: [f32; 2],
    prev: [f32; 2],
    heading: f32,
    speed: f32,
    hue: f32,
    pitch_ref: f32,
    clock: f32,
    on_since: f32,
    last_tap_end: Option<f32>,
    active: bool,
    burst: f32,
    burst_at: [f32; 2],
    seed: u32,
}

pub struct SketchOut {
    pos: [f32; 4],
    info: [f32; 4],
    burst: f32,
    burst_at: [f32; 2],
    whistle: f32,
}

impl SketchOut {
    pub fn apply(&self, p: &mut Params) {
        p.sketch_pos = self.pos;
        p.sketch_info = self.info;
        p.aux0 = self.burst;
        p.aux1 = self.whistle;
        // trail[7].xy in HLSL.
        p.trail[28] = self.burst_at[0];
        p.trail[29] = self.burst_at[1];
    }
}

impl Sketch {
    pub fn new() -> Self {
        Self {
            pos: [0.0; 2],
            prev: [0.0; 2],
            heading: 0.3,
            speed: BASE_SPEED,
            hue: 0.55,
            pitch_ref: 0.5,
            clock: 0.0,
            on_since: 0.0,
            last_tap_end: None,
            active: false,
            burst: 0.0,
            burst_at: [0.0; 2],
            seed: 7,
        }
    }

    fn random(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        (self.seed >> 8) as f32 / (1u32 << 24) as f32
    }

    fn explode(&mut self) {
        self.burst = 1.0;
        self.burst_at = self.pos;
        self.heading += PI * (0.5 + self.random());
        self.speed = MAX_SPEED * 1.4;
    }

    fn bounce(&mut self, aspect: f32) {
        let ax = EDGE * aspect.max(1.0);
        let ay = EDGE / aspect.min(1.0);
        if self.pos[0].abs() > ax {
            self.pos[0] = self.pos[0].clamp(-ax, ax);
            self.heading = PI - self.heading;
        }
        if self.pos[1].abs() > ay {
            self.pos[1] = self.pos[1].clamp(-ay, ay);
            self.heading = -self.heading;
        }
    }

    pub fn step(&mut self, p: &Params, dt: f32, aspect: f32) -> SketchOut {
        self.clock += dt;
        self.burst = (self.burst - dt / BURST_SECS).max(0.0);
        let on = p.whistle > WHISTLE_ON;
        if on && !self.active {
            self.pitch_ref = p.pitch;
            self.on_since = self.clock;
            if self.last_tap_end.is_some_and(|t| self.clock - t < DOUBLE_GAP_SECS) {
                self.last_tap_end = None;
                self.explode();
            }
        }
        if !on && self.active {
            let held = self.clock - self.on_since;
            self.last_tap_end = (held < TAP_MAX_SECS).then_some(self.clock);
        }
        let connect = on && self.active;
        self.active = on;
        self.prev = self.pos;
        if on {
            let held = self.clock - self.on_since;
            let dp = p.pitch - self.pitch_ref;
            let bend = if dp.abs() < DEAD_ZONE { 0.0 } else { dp - DEAD_ZONE * dp.signum() };
            self.heading += (bend * TURN_GAIN).clamp(-MAX_TURN, MAX_TURN) * dt;
            self.pitch_ref += (p.pitch - self.pitch_ref) * (1.0 - (-dt / REF_TAU).exp());
            let ramp = (held / RAMP_SECS).clamp(0.0, 1.0);
            let target = BASE_SPEED + (MAX_SPEED - BASE_SPEED) * ramp * ramp * (3.0 - 2.0 * ramp);
            self.speed += (target * (0.7 + 0.6 * p.whistle) - self.speed) * (1.0 - (-dt * 4.0).exp());
            self.hue = (0.95 - p.pitch * 0.85).rem_euclid(1.0);
        } else {
            self.speed *= (-dt * 2.5).exp();
        }
        self.pos[0] += self.heading.cos() * self.speed * dt;
        self.pos[1] += self.heading.sin() * self.speed * dt;
        self.bounce(aspect);
        let fade = if on { FADE_DRAWING } else { FADE_IDLE };
        SketchOut {
            pos: [self.pos[0], self.pos[1], self.prev[0], self.prev[1]],
            info: [self.heading, self.hue, fade, connect as u32 as f32],
            burst: self.burst,
            burst_at: self.burst_at,
            whistle: p.whistle,
        }
    }
}
