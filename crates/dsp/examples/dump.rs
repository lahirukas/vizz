fn main() {
    let path = std::env::args().nth(1).unwrap();
    let fps: f64 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(30.0);
    let b = std::fs::read(&path).unwrap();
    let sr = u32::from_le_bytes(b[24..28].try_into().unwrap());
    let mut i = 12;
    while &b[i..i + 4] != b"data" {
        i += 8 + u32::from_le_bytes(b[i + 4..i + 8].try_into().unwrap()) as usize;
    }
    let s: Vec<f32> = b[i + 8..].chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0).collect();
    let mut a = vizz_dsp::Analyzer::new(sr);
    let chunk = (sr / 100) as usize;
    let mut next = 0.0;
    for c in s.chunks(chunk) {
        a.process(c, &mut |_| {});
        let p = a.snapshot();
        if p.t_audio >= next {
            next += 1.0 / fps;
            let bands: Vec<String> = p.bands.iter().map(|v| format!("{v:.3}")).collect();
            let wave: Vec<String> = p.wave.iter().map(|v| format!("{v:.3}")).collect();
            println!(
                "{:.4} {:.4} {:.4} {} {} {:.2} {:.3} {:.3} {:.3} {:.3} {:.3} {:.1} {:.3} {:.2} {} {} {} {} {} {:.3}",
                p.t_audio,
                p.last_beat,
                p.next_beat,
                p.beat_count,
                p.beat_in_bar,
                p.bpm,
                p.confidence,
                p.level,
                p.kick,
                p.snare,
                p.hat,
                p.pitch_hz,
                p.whistle,
                p.silent_secs,
                bands.join(","),
                wave.join(","),
                p.kick_onsets,
                p.snare_onsets,
                p.hat_onsets,
                p.beat_presence
            );
        }
    }
}
