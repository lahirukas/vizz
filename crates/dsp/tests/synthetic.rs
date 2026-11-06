use vizz_dsp::testgen;

fn run_set(sr: u32) {
    let mut failures = Vec::new();
    for sig in testgen::standard_set(sr) {
        let row = vizz_dsp::evaluate(&sig);
        let sc = row.scores;
        println!(
            "{} {:<18} bpm {:7.2}/{:<5} conf {:.2} max {:.2} F {:.2} CMLt {:.2} AMLt {:.2} err {:.1}/{:+.1}ms db {:?} fly {:?} drop {:?}",
            if row.failures.is_empty() { "ok  " } else { "FAIL" },
            row.name,
            row.bpm,
            row.want_bpm,
            row.confidence,
            row.max_confidence,
            sc.f_measure,
            sc.cml_t,
            sc.aml_t,
            sc.max_err * 1e3,
            sc.mean_err * 1e3,
            row.downbeat_acc.map(|v| (v * 100.0).round() / 100.0),
            row.flywheel.map(|v| (v * 100.0).round() / 100.0),
            row.drop_err.map(|v| (v * 1000.0).round() / 1000.0),
        );
        for f in &row.failures {
            println!("       - {f}");
            failures.push(format!("{}: {f}", row.name));
        }
    }
    assert!(failures.is_empty(), "{} failures at {sr} Hz:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn synthetic_48k() {
    run_set(48000);
}

#[test]
fn synthetic_44k1() {
    run_set(44100);
}
