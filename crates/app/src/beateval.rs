use std::process::ExitCode;
use vizz_dsp::testgen;

fn opt(v: Option<f64>) -> String {
    v.map_or("–".into(), |x| format!("{x:.2}"))
}

pub fn synthetic_report() -> ExitCode {
    let mut failed = 0;
    for sr in [44100, 48000] {
        println!("\n### {sr} Hz\n");
        println!(
            "| signal | BPM (want) | conf | F | CMLt | AMLt | max err ms | downbeat | flywheel | drop err s | result |"
        );
        println!("|---|---|---|---|---|---|---|---|---|---|---|");
        for sig in testgen::standard_set(sr) {
            let r = vizz_dsp::evaluate(&sig);
            let s = r.scores;
            let want = if sig.rhythmic() { format!("{:.0}", r.want_bpm) } else { "–".into() };
            let rhythmic = sig.rhythmic();
            let cell = |v: f64| if rhythmic { format!("{v:.2}") } else { "–".into() };
            let result =
                if r.failures.is_empty() { "pass".to_string() } else { format!("FAIL: {}", r.failures.join("; ")) };
            failed += !r.failures.is_empty() as usize;
            println!(
                "| {} | {:.2} ({want}) | {:.2} | {} | {} | {} | {} | {} | {} | {} | {result} |",
                r.name,
                r.bpm,
                if rhythmic { r.confidence } else { r.max_confidence },
                cell(s.f_measure),
                cell(s.cml_t),
                cell(s.aml_t),
                if rhythmic { format!("{:.1}", s.max_err * 1e3) } else { "–".into() },
                opt(r.downbeat_acc),
                opt(r.flywheel),
                opt(r.drop_err),
            );
        }
    }
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

pub fn eval_dir(dir: &str) -> ExitCode {
    let Ok(entries) = std::fs::read_dir(dir) else {
        eprintln!("cannot read {dir}");
        return ExitCode::FAILURE;
    };
    let mut files: Vec<_> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("wav")))
        .collect();
    files.sort();
    println!("| file | BPM | conf | BPM spread (last 2/3) | locked % | drops |");
    println!("|---|---|---|---|---|---|");
    for path in files {
        let Ok((sr, mono)) = crate::wav::read_mono(&path.to_string_lossy()) else { continue };
        let r = vizz_dsp::analyze_offline(sr, &mono);
        let tail = &r.trace[r.trace.len() / 3..];
        let bpms: Vec<f32> = tail.iter().map(|s| s.bpm).filter(|&b| b > 0.0).collect();
        let (lo, hi) = bpms.iter().fold((f32::MAX, 0.0f32), |(l, h), &b| (l.min(b), h.max(b)));
        let locked = tail.iter().filter(|s| s.confidence >= 0.5).count() as f32 / tail.len().max(1) as f32;
        println!(
            "| {} | {:.1} | {:.2} | {} | {:.0} | {} |",
            path.file_name().unwrap_or_default().to_string_lossy(),
            r.final_snapshot.bpm,
            r.final_snapshot.confidence,
            if bpms.is_empty() { "–".into() } else { format!("{:.1}", hi - lo) },
            locked * 100.0,
            r.drop_times.len()
        );
    }
    ExitCode::SUCCESS
}
