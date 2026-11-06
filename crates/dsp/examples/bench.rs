use std::time::Instant;
use vizz_dsp::{testgen, Analyzer};

fn main() {
    let sr = 48000;
    let sig = testgen::drum_pattern(sr, 128.0, 14.0);
    let mut a = Analyzer::new(sr);
    let chunk = 480;
    let start = Instant::now();
    let mut beats = 0;
    for _ in 0..4 {
        for c in sig.samples.chunks(chunk) {
            a.process(c, &mut |_| beats += 1);
        }
    }
    let audio = 4.0 * sig.duration();
    let cpu = start.elapsed().as_secs_f64();
    println!("audio {audio:.1}s processed in {cpu:.3}s -> {:.2}% of one core, {beats} beats", 100.0 * cpu / audio);
}
