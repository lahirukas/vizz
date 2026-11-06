fn main() {
    let path = std::env::args().nth(1).unwrap();
    let b = std::fs::read(&path).unwrap();
    let sr = u32::from_le_bytes(b[24..28].try_into().unwrap());
    let mut i = 12;
    while &b[i..i + 4] != b"data" {
        i += 8 + u32::from_le_bytes(b[i + 4..i + 8].try_into().unwrap()) as usize;
    }
    let s: Vec<f32> = b[i + 8..].chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0).collect();
    let mut a = vizz_dsp::Analyzer::new(sr);
    for (k, c) in s.chunks((sr / 100) as usize).enumerate() {
        a.process(c, &mut |_| {});
        let snap = a.snapshot();
        if k % 2 == 0 {
            println!("{:.2} {:.1} {:.3}", snap.t_audio, snap.pitch_hz, snap.whistle);
        }
    }
}
