use vizz_render::Params;

const BLEND_SECS: f32 = 2.5;
const FREE_RUN_SWITCH_SECS: f32 = 20.0;

#[derive(Clone, Copy)]
pub struct Preset {
    pub name: &'static str,
    zoom: f32,
    zoom_beat: f32,
    rot: f32,
    rot_beat: f32,
    warp: f32,
    freq: f32,
    speed: f32,
    swirl: f32,
    decay: f32,
    hue_drift: f32,
    blur: f32,
    bright: f32,
    wave: f32,
    scale: f32,
    thick: f32,
    hue: f32,
    sym: f32,
    echo: f32,
    solar: f32,
    orbit: f32,
    dx: f32,
    dy: f32,
}

const BASE: Preset = Preset {
    name: "Nebula Bloom",
    zoom: 1.01,
    zoom_beat: 0.03,
    rot: 0.003,
    rot_beat: 0.01,
    warp: 0.5,
    freq: 3.0,
    speed: 0.7,
    swirl: 0.02,
    decay: 0.96,
    hue_drift: 0.005,
    blur: 0.5,
    bright: 1.0,
    wave: 1.0,
    scale: 0.3,
    thick: 1.0,
    hue: 0.6,
    sym: 0.0,
    echo: 0.0,
    solar: 0.0,
    orbit: 0.05,
    dx: 0.0,
    dy: 0.0,
};

pub const PRESETS: [Preset; 10] = [
    BASE,
    Preset {
        name: "Kaleido Silk",
        zoom: 1.006,
        zoom_beat: 0.02,
        rot: 0.006,
        rot_beat: 0.02,
        warp: 0.35,
        freq: 5.0,
        speed: 0.5,
        swirl: 0.03,
        decay: 0.955,
        hue_drift: 0.01,
        blur: 0.4,
        bright: 1.1,
        wave: 0.0,
        scale: 0.25,
        thick: 0.8,
        hue: 0.1,
        sym: 3.0,
        orbit: 0.1,
        ..BASE
    },
    Preset {
        name: "Hyperdrive",
        zoom: 1.03,
        zoom_beat: 0.06,
        rot: 0.0,
        rot_beat: 0.015,
        warp: 0.2,
        freq: 2.0,
        speed: 1.5,
        swirl: 0.0,
        decay: 0.93,
        hue_drift: 0.006,
        blur: 0.3,
        bright: 1.2,
        wave: 2.0,
        scale: 0.4,
        hue: 0.55,
        echo: 0.3,
        orbit: 0.0,
        ..BASE
    },
    Preset {
        name: "Liquid Mirror",
        zoom: 0.995,
        zoom_beat: 0.02,
        rot: -0.004,
        warp: 1.0,
        freq: 4.0,
        speed: 0.6,
        swirl: 0.05,
        decay: 0.97,
        hue_drift: 0.003,
        blur: 0.8,
        bright: 0.9,
        wave: 3.0,
        thick: 1.2,
        hue: 0.8,
        sym: 1.0,
        orbit: 0.15,
        ..BASE
    },
    Preset {
        name: "Spiral Galaxy",
        zoom: 1.008,
        zoom_beat: 0.025,
        rot: 0.012,
        rot_beat: 0.02,
        warp: 0.3,
        freq: 2.5,
        speed: 0.4,
        swirl: 0.08,
        wave: 4.0,
        hue: 0.7,
        ..BASE
    },
    Preset {
        name: "Prism Mandala",
        zoom: 1.004,
        zoom_beat: 0.03,
        rot: 0.008,
        rot_beat: 0.03,
        freq: 6.0,
        speed: 1.0,
        decay: 0.93,
        hue_drift: 0.02,
        blur: 0.3,
        bright: 0.8,
        scale: 0.25,
        thick: 0.8,
        hue: 0.0,
        sym: 2.0,
        echo: 0.1,
        solar: 0.25,
        ..BASE
    },
    Preset {
        name: "Deep Ocean",
        zoom: 0.99,
        zoom_beat: 0.02,
        rot: 0.003,
        rot_beat: 0.008,
        warp: 0.8,
        speed: 0.3,
        decay: 0.975,
        hue_drift: 0.002,
        blur: 1.0,
        bright: 0.85,
        wave: 0.0,
        thick: 1.4,
        hue: 0.5,
        orbit: 0.2,
        ..BASE
    },
    Preset {
        name: "Fire Dance",
        zoom: 1.015,
        zoom_beat: 0.04,
        rot: -0.006,
        rot_beat: 0.02,
        warp: 1.2,
        freq: 5.0,
        speed: 2.0,
        swirl: -0.03,
        decay: 0.94,
        hue_drift: -0.004,
        bright: 1.15,
        wave: 3.0,
        scale: 0.35,
        hue: 0.02,
        sym: 1.0,
        echo: 0.2,
        ..BASE
    },
    Preset {
        name: "Tunnel Vision",
        zoom: 1.04,
        zoom_beat: 0.05,
        rot: 0.01,
        rot_beat: 0.03,
        warp: 0.1,
        freq: 2.0,
        speed: 0.5,
        swirl: 0.0,
        decay: 0.92,
        hue_drift: 0.008,
        blur: 0.2,
        bright: 1.1,
        scale: 0.25,
        hue: 0.3,
        sym: 3.0,
        echo: 0.4,
        ..BASE
    },
    Preset {
        name: "Aurora",
        zoom: 1.002,
        zoom_beat: 0.015,
        rot: 0.001,
        rot_beat: 0.006,
        warp: 0.9,
        freq: 1.5,
        speed: 0.7,
        swirl: 0.01,
        decay: 0.975,
        hue_drift: 0.006,
        blur: 0.9,
        bright: 0.95,
        wave: 0.0,
        scale: 0.4,
        thick: 1.3,
        hue: 0.35,
        sym: 1.0,
        dy: 1.0,
        ..BASE
    },
];

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn follow(cur: f32, x: f32, up: f32, down: f32, dt: f32) -> f32 {
    let tau = if x > cur { up } else { down };
    cur + (x - cur) * (1.0 - (-dt / tau).exp())
}

pub struct Flux {
    from: usize,
    to: usize,
    mix: f32,
    scene: i64,
    free: f32,
    bass: f32,
    mid: f32,
    treb: f32,
    pub pinned: Option<usize>,
}

impl Flux {
    pub fn new() -> Self {
        Self { from: 0, to: 0, mix: 1.0, scene: -1, free: 0.0, bass: 0.0, mid: 0.0, treb: 0.0, pinned: None }
    }

    pub fn current_name(&self) -> &'static str {
        PRESETS[self.to].name
    }

    pub fn next(&mut self) {
        self.switch_to((self.to + 1) % PRESETS.len());
    }

    fn switch_to(&mut self, next: usize) {
        if next == self.to {
            return;
        }
        self.from = if self.mix >= 1.0 { self.to } else { self.from };
        self.to = next;
        self.mix = 0.0;
    }

    pub fn step(&mut self, p: &mut Params, dt: f32) {
        let scene = p.scene.round() as i64;
        let locked = p.bpm > 0.0 && p.drive > 0.15;
        self.free += dt;
        if let Some(i) = self.pinned {
            self.switch_to(i.min(PRESETS.len() - 1));
        } else if (locked && scene != self.scene) || (!locked && self.free > FREE_RUN_SWITCH_SECS) {
            self.scene = scene;
            self.free = 0.0;
            let next = (self.to + 1 + (scene.unsigned_abs() as usize * 3)) % PRESETS.len();
            self.switch_to(next);
        }
        self.mix = (self.mix + dt / BLEND_SECS).min(1.0);
        let m = self.mix * self.mix * (3.0 - 2.0 * self.mix);
        let (a, b) = (PRESETS[self.from], PRESETS[self.to]);
        let l = |x: f32, y: f32| lerp(x, y, m);
        self.bass = follow(self.bass, p.kick, 0.02, 0.25, dt);
        self.mid = follow(self.mid, p.snare, 0.02, 0.2, dt);
        self.treb = follow(self.treb, p.hat, 0.01, 0.15, dt);
        let orbit = l(a.orbit, b.orbit);
        p.flux = [
            l(a.zoom, b.zoom),
            l(a.zoom_beat, b.zoom_beat),
            l(a.rot, b.rot),
            l(a.rot_beat, b.rot_beat),
            l(a.warp, b.warp),
            l(a.freq, b.freq),
            l(a.speed, b.speed),
            l(a.swirl, b.swirl),
            orbit * (p.time * 0.31).sin(),
            orbit * (p.time * 0.23).cos(),
            l(a.dx, b.dx),
            l(a.dy, b.dy),
            l(a.decay, b.decay),
            l(a.hue_drift, b.hue_drift),
            l(a.blur, b.blur),
            l(a.bright, b.bright),
            a.wave,
            b.wave,
            m,
            l(a.scale, b.scale),
            l(a.thick, b.thick),
            l(a.hue, b.hue),
            a.sym,
            b.sym,
            m,
            l(a.echo, b.echo),
            l(a.solar, b.solar),
            0.0,
            self.bass,
            self.mid,
            self.treb,
            p.pulse * p.drive,
        ];
    }
}
