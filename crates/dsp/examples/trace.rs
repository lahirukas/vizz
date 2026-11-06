use vizz_dsp::testgen;

fn main() {
    let name = std::env::args().nth(1).unwrap_or_else(|| "breakdown_128".into());
    let sr = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(48000);
    let sig = testgen::standard_set(sr).into_iter().find(|s| s.name == name).expect("unknown signal");
    let r = vizz_dsp::analyze_offline(sr, &sig.samples);
    for s in r.trace.iter().step_by(5) {
        println!(
            "t {:5.2} bpm {:6.2} conf {:.2} bib {} ph {:.2} drop {:.2} eta {:5.1} phrase {:.2}",
            s.t_audio,
            s.bpm,
            s.confidence,
            s.beat_in_bar,
            s.beat_phase,
            s.drop_probability,
            s.drop_eta_beats,
            s.phrase_pos
        );
    }
    for &b in &r.beats {
        let d = sig.beats.iter().map(|t| b - t).min_by(|a, c| a.abs().total_cmp(&c.abs())).unwrap_or(0.0);
        print!("{b:.3}({:+.0}) ", d * 1000.0);
    }
    println!("\ndrops at {:?}, truth drop {:?} breakdown {:?}", r.drop_times, sig.drop_at, sig.breakdown);
}
