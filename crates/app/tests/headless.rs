use serde_json::Value;
use std::process::Command;

const EXE: &str = env!("CARGO_BIN_EXE_vizz");

fn run(args: &[&str]) -> String {
    let out = Command::new(EXE).args(args).output().expect("run vizz");
    assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn headless_analyze_synthetic_set() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("synth");
    run(&["--gen-test-wavs", dir.to_str().unwrap()]);

    let mut failures = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).collect();
    entries.sort();
    for wav in entries.iter().filter(|p| p.extension().is_some_and(|e| e == "wav")) {
        let truth: Value = serde_json::from_str(&std::fs::read_to_string(wav.with_extension("json")).unwrap()).unwrap();
        let got: Value = serde_json::from_str(&run(&["--headless-analyze", wav.to_str().unwrap()])).unwrap();
        let name = wav.file_stem().unwrap().to_string_lossy().to_string();
        if !["click", "drums", "change", "noise", "speech"].iter().any(|p| name.starts_with(p)) {
            continue;
        }
        let want_bpm = truth["bpm"].as_f64().unwrap();
        let bpm = got["bpm"].as_f64().unwrap();
        let dur = got["duration"].as_f64().unwrap();
        let tb: Vec<f64> = truth["beats"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
        let pb: Vec<f64> = got["beats"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();

        let ok = if tb.is_empty() {
            got["max_confidence"].as_f64().unwrap() < 0.3
        } else {
            let from = truth["lock_from"].as_f64().unwrap();
            let e = vizz_dsp::eval::score(&tb, &pb, from, dur - 0.1);
            (bpm - want_bpm).abs() <= 1.0 && e.missed_20ms == 0 && e.extra_20ms == 0 && e.matched_20ms > 0
        };
        let line = format!("{name}: bpm {bpm} (want {want_bpm}) conf {}", got["confidence"]);
        println!("{} {line}", if ok { "ok  " } else { "FAIL" });
        if !ok {
            failures.push(line);
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
