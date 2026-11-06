#[derive(Debug, Clone, Copy, Default)]
pub struct BeatScores {
    pub f_measure: f64,
    pub cml_t: f64,
    pub aml_t: f64,
    pub matched_20ms: usize,
    pub missed_20ms: usize,
    pub extra_20ms: usize,
    pub max_err: f64,
    pub mean_err: f64,
}

fn nearest(set: &[f64], t: f64) -> Option<(usize, f64)> {
    set.iter().enumerate().map(|(i, b)| (i, b - t)).min_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
}

fn window(v: &[f64], from: f64, to: f64) -> Vec<f64> {
    v.iter().copied().filter(|&t| t >= from && t < to).collect()
}

pub fn f_measure(reference: &[f64], estimated: &[f64], tol: f64) -> f64 {
    if reference.is_empty() || estimated.is_empty() {
        return 0.0;
    }
    let mut used = vec![false; estimated.len()];
    let mut hits = 0;
    for &r in reference {
        let best = estimated
            .iter()
            .enumerate()
            .filter(|(i, &e)| !used[*i] && (e - r).abs() <= tol)
            .min_by(|a, b| (a.1 - r).abs().total_cmp(&(b.1 - r).abs()));
        if let Some((i, _)) = best {
            used[i] = true;
            hits += 1;
        }
    }
    let p = hits as f64 / estimated.len() as f64;
    let r = hits as f64 / reference.len() as f64;
    if p + r == 0.0 {
        0.0
    } else {
        2.0 * p * r / (p + r)
    }
}

// Continuity "t" score (mir_eval style): a reference beat counts if the nearest estimate is within 17.5 % of the local IBI, the previous estimate also matches the previous reference, and the estimated IBI is within 17.5 %.
fn continuity_t(reference: &[f64], estimated: &[f64]) -> f64 {
    const TOL: f64 = 0.175;
    if reference.len() < 2 || estimated.len() < 2 {
        return 0.0;
    }
    let mut correct = 0;
    for m in 1..reference.len() {
        let ibi = reference[m] - reference[m - 1];
        let Some((k, d)) = nearest(estimated, reference[m]) else { continue };
        if k == 0 || d.abs() > TOL * ibi {
            continue;
        }
        let prev_ok = (estimated[k - 1] - reference[m - 1]).abs() <= TOL * ibi;
        let period_ok = ((estimated[k] - estimated[k - 1]) - ibi).abs() <= TOL * ibi;
        if prev_ok && period_ok {
            correct += 1;
        }
    }
    correct as f64 / (reference.len() - 1) as f64
}

fn variations(reference: &[f64]) -> [Vec<f64>; 5] {
    let mids: Vec<f64> = reference.windows(2).map(|w| 0.5 * (w[0] + w[1])).collect();
    let mut double = Vec::with_capacity(reference.len() * 2);
    for (i, &r) in reference.iter().enumerate() {
        double.push(r);
        if let Some(&m) = mids.get(i) {
            double.push(m);
        }
    }
    let odd = reference.iter().copied().step_by(2).collect();
    let even = reference.iter().copied().skip(1).step_by(2).collect();
    [reference.to_vec(), mids, double, odd, even]
}

pub fn score(reference: &[f64], estimated: &[f64], from: f64, to: f64) -> BeatScores {
    let r = window(reference, from, to);
    let e = window(estimated, from, to);
    let mut s = BeatScores { f_measure: f_measure(&r, &e, 0.07), cml_t: continuity_t(&r, &e), ..Default::default() };
    s.aml_t = variations(&r).iter().map(|v| continuity_t(v, &e)).fold(0.0, f64::max);
    let mut sum = 0.0;
    for &t in &r {
        match nearest(estimated, t) {
            Some((_, d)) if d.abs() <= 0.020 => {
                s.matched_20ms += 1;
                s.max_err = s.max_err.max(d.abs());
                sum += d;
            }
            _ => s.missed_20ms += 1,
        }
    }
    s.extra_20ms = e.iter().filter(|&&p| !matches!(nearest(reference, p), Some((_, d)) if d.abs() <= 0.020)).count();
    if s.matched_20ms > 0 {
        s.mean_err = sum / s.matched_20ms as f64;
    }
    s
}
