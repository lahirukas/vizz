use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct WidgetCfg {
    pub kind: String,
    pub style: String,
    pub quality: Option<String>,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub palette: Option<[[f32; 3]; 3]>,
    pub opacity: f32,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub pinned: bool,
    pub topmost: bool,
    pub fps: u32,
    pub av_offset_ms: f32,
    pub idle_after_secs: f32,
    pub source: String,
    pub whistle_source: String,
    pub quality: String,
    pub widgets: Vec<WidgetCfg>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            pinned: true,
            topmost: false,
            fps: 60,
            av_offset_ms: 0.0,
            idle_after_secs: 10.0,
            source: "loopback".into(),
            whistle_source: "mic".into(),
            quality: "medium".into(),
            widgets: Vec::new(),
        }
    }
}

pub fn default_path() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| ".".into());
    base.join("Vizz").join("config.json")
}

fn parse_palette(v: Option<&Value>) -> Option<[[f32; 3]; 3]> {
    let a = v?.as_array()?;
    let mut out = [[0.0; 3]; 3];
    for (o, c) in out.iter_mut().zip(a.iter()) {
        let c = c.as_array()?;
        for (oi, ci) in o.iter_mut().zip(c.iter()) {
            *oi = ci.as_f64()?.clamp(0.0, 1.0) as f32;
        }
    }
    (a.len() == 3).then_some(out)
}

fn f(v: &Value, k: &str, d: f64) -> f64 {
    v.get(k).and_then(Value::as_f64).unwrap_or(d)
}

impl Config {
    pub fn load(path: &std::path::Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else { return Self::default() };
        let Ok(v) = serde_json::from_str::<Value>(&text) else {
            eprintln!("config: {} is not valid JSON, using defaults", path.display());
            return Self::default();
        };
        let d = Self::default();
        let widgets = v
            .get("widgets")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|w| WidgetCfg {
                        kind: w.get("kind").and_then(Value::as_str).unwrap_or("ring").to_string(),
                        style: w.get("style").and_then(Value::as_str).unwrap_or("neon").to_string(),
                        quality: w.get("quality").and_then(Value::as_str).map(str::to_string),
                        x: f(w, "x", 80.0) as i32,
                        y: f(w, "y", 80.0) as i32,
                        w: (f(w, "w", 260.0) as i32).clamp(32, 4096),
                        h: (f(w, "h", 260.0) as i32).clamp(32, 4096),
                        palette: parse_palette(w.get("palette")),
                        opacity: f(w, "opacity", 1.0).clamp(0.05, 1.0) as f32,
                    })
                    .collect()
            })
            .unwrap_or(d.widgets);
        Self {
            pinned: v.get("pinned").and_then(Value::as_bool).unwrap_or(d.pinned),
            topmost: v.get("topmost").and_then(Value::as_bool).unwrap_or(false),
            fps: if f(&v, "fps", 60.0) <= 30.0 { 30 } else { 60 },
            av_offset_ms: f(&v, "av_offset_ms", 0.0).clamp(-500.0, 1000.0) as f32,
            idle_after_secs: f(&v, "idle_after_secs", d.idle_after_secs as f64).max(1.0) as f32,
            source: v.get("source").and_then(Value::as_str).unwrap_or("loopback").to_string(),
            whistle_source: v.get("whistle_source").and_then(Value::as_str).unwrap_or("mic").to_string(),
            quality: v.get("quality").and_then(Value::as_str).unwrap_or("medium").to_string(),
            widgets,
        }
    }

    pub fn save(&self, path: &std::path::Path) -> Result<(), String> {
        let widgets: Vec<Value> = self
            .widgets
            .iter()
            .map(|w| json!({"kind": w.kind, "style": w.style, "quality": w.quality, "x": w.x, "y": w.y, "w": w.w, "h": w.h, "palette": w.palette, "opacity": w.opacity}))
            .collect();
        let v = json!({
            "pinned": self.pinned,
            "topmost": self.topmost,
            "fps": self.fps,
            "av_offset_ms": self.av_offset_ms,
            "idle_after_secs": self.idle_after_secs,
            "source": self.source,
            "whistle_source": self.whistle_source,
            "quality": self.quality,
            "widgets": widgets,
        });
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(&v).unwrap()).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, path).map_err(|e| e.to_string())
    }
}
