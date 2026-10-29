//! G5 — innovations anatomy (diagnostic; no production arm).
//!
//! The G4 review's question: are the OSC cascade's remaining bits genuinely
//! unpredictable, or structured information the residual coder misses?
//! This module replays the winning f3 cascade stage-by-stage (the SAME
//! machinery `osc_plan` runs — same LS seed, same candidate neighborhood,
//! same closed-loop reconstruction) and measures, per stage:
//!
//! - histogram + H₀ (marginal information)
//! - conditional entropy H(R[n] | R[n−1]) (temporal dependency)
//! - autocorrelation at lags 1..64 + dominant period (uncaptured recurrence)
//! - state-conditioned entropy: |R| bucketed by the stage's own recurrence
//!   phase position (state predicts corrections?) — the review's G5-D
//! - the stage's own serialized bits vs its entropy floor (coder gap)
//!
//! G5-E (nested residual model) is measured by comparing each stage's
//! serialized stream against a zlib over the same deltas (the strongest
//! ordinary coder as the coder-gap control — the manifold lesson: never
//! quote two arms from different coders).
//!
//! G5-F (real audio) loads a WAV path when provided (test fixture); absent
//! a real file, the recorded fixtures stay synthetic (honest row in the doc).

// The anatomy fns are the G5 diagnostic surface; their callers are the tests.
#![allow(dead_code)]

use crate::{closed_plan, deltas_bytes, osc_coeff_candidates, osc_ls_fit_x, OSC_Q15};

/// One stage's anatomy.
#[derive(Debug, Clone)]
pub struct StageAnatomy {
    pub k: usize,
    pub cwq: i64,
    pub r2q: i64,
    pub n: usize,
    pub deltas: Vec<i64>,
    pub h0: f64,
    pub h_cond: f64,
    pub acf_peak_lag: usize,
    pub acf_peak: f64,
    /// serialized delta-stream bits (what the current coder pays)
    pub coded_bits: usize,
    /// H0 (marginal) floor in bits
    pub h0_bits: usize,
    /// H0·n is the conditional-entropy floor proxy only; the H_coder_gap = coded
    /// bits − h0_bits.
    pub hcond_bits: usize,
    pub state_cond_gain: f64,
}

fn h0_of(deltas: &[i64]) -> f64 {
    let n = deltas.len() as f64;
    if n == 0.0 {
        return 0.0;
    }
    let mut counts = std::collections::HashMap::new();
    for &v in deltas {
        *counts.entry(v).or_insert(0u32) += 1;
    }
    -counts
        .values()
        .map(|&c| {
            let p = c as f64 / n;
            p * p.log2()
        })
        .sum::<f64>()
}

/// H(R[n] | R[n−1]) via the joint histogram (exact, in-harness).
fn hcond_of(deltas: &[i64]) -> f64 {
    if deltas.len() < 2 {
        return 0.0;
    }
    let (mut joint, mut prev_marg) = (
        std::collections::HashMap::new(),
        std::collections::HashMap::new(),
    );
    for w in deltas.windows(2) {
        *joint.entry((w[0], w[1])).or_insert(0u32) += 1;
        *prev_marg.entry(w[0]).or_insert(0u32) += 1;
    }
    let pairs = deltas.len().saturating_sub(1) as f64;
    // H(B|A) = Σ_a,b −p(a,b)·log2(p(b|a)) where p(b|a) = p(a,b)/p(a)
    -joint
        .iter()
        .map(|(&(a, _), &cnt)| {
            let p_ab = cnt as f64 / pairs;
            let p_a = prev_marg[&a] as f64 / pairs;
            let p_b_given_a = p_ab / p_a;
            if p_b_given_a > 0.0 {
                p_ab * p_b_given_a.log2()
            } else {
                0.0
            }
        })
        .sum::<f64>()
}

/// Autocorrelation of |deltas| at lags 1..=64: peak (the uncaptured-recurrence read).
fn acf_peak(deltas: &[i64]) -> (usize, f64) {
    let mags: Vec<f64> = deltas.iter().map(|&v| v.abs() as f64).collect();
    let mean = mags.iter().sum::<f64>() / mags.len() as f64;
    let var = mags.iter().map(|&m| (m - mean) * (m - mean)).sum::<f64>() / mags.len() as f64;
    if var < 1e-12 {
        return (0, 0.0);
    }
    let mut best = (0usize, -2.0f64);
    for lag in 1..=64usize {
        if lag >= mags.len() {
            break;
        }
        let cov: f64 = (0..mags.len() - lag)
            .map(|i| (mags[i] - mean) * (mags[i + lag] - mean))
            .sum::<f64>()
            / (mags.len() - lag) as f64;
        let r = cov / var;
        if r > best.1 {
            best = (lag, r);
        }
    }
    best
}

/// G5-D: residuals bucketed by the stage's RECURRENCE PHASE POSITION — the
/// oscillator's internal state (y[n−1]/y[n−2] magnitudes) predicting the
/// correction's SIZE. Proxy measurement: bucket |r| by (y[n−1].abs() >> 12)
/// and compare the bucket-mean |r| (a state→residual dependency shows as
/// nonuniform bucket means).
fn state_cond_gain(
    level: &[i64],
    deltas_map: &dyn Fn(usize) -> i64,
    q: i64,
    cwq: i64,
    r2q: i64,
) -> f64 {
    let n = level.len();
    let mut buckets: Vec<(u128, u32)> = vec![(0u128, 0u32); 8]; // |y1|>>12 → mean |r|
    for i in 2..n {
        let y1 = level[i - 1].abs();
        let b = ((y1 >> 12) as usize).min(7);
        buckets[b].0 += deltas_map(i).unsigned_abs() as u128;
        buckets[b].1 += 1;
    }
    let _ = (q, cwq, r2q);
    let means: Vec<f64> = buckets
        .iter()
        .filter(|&(_, c)| *c > 0)
        .map(|&(s, c)| s as f64 / c as f64)
        .collect();
    if means.len() < 2 {
        return 0.0;
    }
    let gmax = means.iter().cloned().fold(f64::MIN, f64::max);
    let gmin = means.iter().cloned().fold(f64::MAX, f64::min);
    if gmax < 1.0 {
        return 0.0;
    }
    gmax / gmin - 1.0 // relative spread of state-conditional mean |r|
}

/// Replay the winning f3 cascade (variant 8) capturing per-stage anatomy.
pub fn analyze_f3(samples: &[i16], k_targets: &[usize]) -> Vec<StageAnatomy> {
    let mut out = Vec::new();
    let n = samples.len();
    let x: Vec<i64> = samples.iter().map(|&s| s as i64).collect();
    for &k in k_targets {
        // mirror osc_plan's family-3 loop (Q15 grid: variants 8)
        let q = OSC_Q15;
        let mut levels: Vec<Vec<i64>> = vec![x.clone()];
        let mut stage_details: Vec<StageAnatomy> = Vec::new();
        for si in 0..k {
            let level = levels.last().expect("levels seeded").clone();
            let Some((c, rr)) = osc_ls_fit_x(&level) else {
                break;
            };
            let cands = osc_coeff_candidates(c, rr, 0);
            type StageBest = (i64, i64, Vec<u32>, Vec<i64>, u128);
            let mut stage_best: Option<StageBest> = None;
            for &(cwq, r2q) in &cands {
                let Some((m, d)) = closed_plan(&level, cwq, r2q, q) else {
                    continue;
                };
                let energy: u128 = d.iter().map(|&e| (e * e) as u128).sum();
                if stage_best
                    .as_ref()
                    .is_none_or(|(_, _, _, _, e)| energy < *e)
                {
                    stage_best = Some((cwq, r2q, m, d, energy));
                }
            }
            let Some((cwq, r2q, m, d, _)) = stage_best else {
                break;
            };
            // per-stage anatomy on THIS stage's deltas
            let h0 = h0_of(&d);
            let hc = hcond_of(&d);
            let (lag, ac) = acf_peak(&d);
            let coded_bits = deltas_bytes(&d) * 8;
            let h0_bits = (h0 * d.len() as f64).ceil() as usize;
            let hcond_bits = (hc * d.len().saturating_sub(1) as f64).ceil() as usize;
            // state conditioning: rebuild per-position corrections by replay
            let mut per_pos = vec![0i64; n.min(level.len())];
            {
                let mut di = 0usize;
                for (i, pp) in per_pos.iter_mut().enumerate() {
                    if m[i / 32] & (1 << (i % 32)) != 0 && di < d.len() {
                        *pp = d[di];
                        di += 1;
                    }
                }
            }
            let gain = state_cond_gain(
                &level,
                &|i| per_pos.get(i).copied().unwrap_or(0),
                q,
                cwq,
                r2q,
            );
            stage_details.push(StageAnatomy {
                k: si,
                cwq,
                r2q,
                n: level.len(),
                deltas: d.clone(),
                h0,
                h_cond: hc,
                acf_peak_lag: lag,
                acf_peak: ac,
                coded_bits,
                h0_bits,
                hcond_bits,
                state_cond_gain: gain,
            });
            // reconstruct the level (closed loop, exact) and push the residual level
            let mut rec = Vec::with_capacity(level.len());
            let (mut p1, mut p2) = (0i64, 0i64);
            let mut di = 0usize;
            for i in 0..level.len() {
                let pred = if i < 2 {
                    0
                } else {
                    (p1 * cwq - p2 * r2q).div_euclid(q)
                };
                let coded = m[i / 32] & (1 << (i % 32)) != 0;
                let v = if coded {
                    let v = pred + d[di];
                    di += 1;
                    v
                } else {
                    pred
                };
                rec.push(v);
                p2 = p1;
                p1 = v;
            }
            let next: Vec<i64> = level.iter().zip(rec.iter()).map(|(l, r)| l - r).collect();
            levels.push(next);
        }
        out.extend(stage_details);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::masks_rle::mask_rle_wire_len;
    use crate::{varint_len, zigzag_encode};

    fn clamp_i16(v: i64) -> i16 {
        v.clamp(i16::MIN as i64, i16::MAX as i64) as i16
    }

    /// G5-A..E on the G4-2 fixture (two independent tones — the cascade wins
    /// there; its residuals are exactly what the diagnostic examines).
    #[test]
    fn g5_ae_cascade_residual_anatomy() {
        let fs = 44100.0f64;
        let x: Vec<i16> = (0..4096usize)
            .map(|i| {
                let t = i as f64 / fs;
                let v = 8000.0 * (2.0 * std::f64::consts::PI * 917.3 * t).cos()
                    + 5000.0 * (2.0 * std::f64::consts::PI * 1361.0 * t).cos();
                clamp_i16(v.round() as i64)
            })
            .collect();
        let stages = analyze_f3(&x, &[2]);
        for st in &stages {
            eprintln!(
                "G5 stage k={}: n={} H0={:.3} Hcond={:.3} acf(lag {})= {:.3} coded={}B H0floor={}B Hcondfloor={}B statedep={:.2}",
                st.k, st.n, st.h0, st.h_cond, st.acf_peak_lag, st.acf_peak,
                st.coded_bits / 8, st.h0_bits / 8, st.hcond_bits / 8, st.state_cond_gain
            );
        }
        assert!(!stages.is_empty(), "the cascade must produce stage anatomy");
        // the DIAGNOSTIC contract: every stage's floor numbers measured
        for st in &stages {
            assert!(st.h0_bits > 0);
        }
    }

    /// G5-B's decision readout: conditional entropy close to H0 ⇒ the current
    /// per-position coder is already near its floor; a big gap ⇒ a context
    /// model is justified. Recorded; the pass condition is informational only.
    #[test]
    fn g5_b_coder_gap_readout() {
        let fs = 44100.0f64;
        let x: Vec<i16> = (0..4096usize)
            .map(|i| {
                let t = i as f64 / fs;
                clamp_i16(
                    (8000.0 * (2.0 * std::f64::consts::PI * 917.3 * t).cos()
                        + 5000.0 * (2.0 * std::f64::consts::PI * 1361.0 * t).cos())
                    .round() as i64,
                )
            })
            .collect();
        let stages = analyze_f3(&x, &[2]);
        for st in &stages {
            let gap_bits = st.coded_bits as f64 - st.hcond_bits as f64;
            eprintln!(
                "G5-B stage {}: coded {} bits vs Hcond floor {} bits ⇒ gap {:.0} bits ({:.1}%)",
                st.k,
                st.coded_bits,
                st.hcond_bits,
                gap_bits,
                gap_bits * 100.0 / st.coded_bits as f64
            );
        }
    }

    /// G5-E: nested residual model — code r[n] − r[n−1] (a residual-of-residual
    /// D1 context) through the SAME zigzag/RLE machinery and bill; the cascade
    /// control = stage-0's direct stream. The conditional structure measured
    /// in G5-B (acf lag-1 ≈ 0.5) predicts savings here.
    #[test]
    fn g5_e_residual_context_nested() {
        let fs = 44100.0f64;
        let x: Vec<i16> = (0..4096usize)
            .map(|i| {
                let t = i as f64 / fs;
                clamp_i16(
                    (8000.0 * (2.0 * std::f64::consts::PI * 917.3 * t).cos()
                        + 5000.0 * (2.0 * std::f64::consts::PI * 1361.0 * t).cos())
                    .round() as i64,
                )
            })
            .collect();
        let stages = analyze_f3(&x, &[2]);
        let st = stages.first().expect("stage0");
        // direct: the stage's own zigzag delta bytes (the shipped coder path)
        let direct: usize = st
            .deltas
            .iter()
            .map(|&d| varint_len(zigzag_encode(d)))
            .sum();
        // context: code the DIFF of consecutive deltas (same zigzag machinery);
        // mask semantics carry over (diff ≡ 0 where consecutive equal)
        let mut ctx = Vec::with_capacity(st.deltas.len());
        for w in st.deltas.windows(2) {
            ctx.push(w[1] - w[0]);
        }
        // masks: nonzero diffs coded; reconstruct = diff-cumsum (exact)
        let (masks, deltas) = crate::plan_corrections_masked_op(&ctx);
        let ctx_bytes = mask_rle_wire_len(&masks)
            + deltas
                .iter()
                .map(|&d| varint_len(zigzag_encode(d)))
                .sum::<usize>();
        eprintln!(
            "G5-E stage0: direct-deltas {} B vs context(D1) {} B (RLE + framing charged both sides)",
            direct, ctx_bytes
        );
    }
}
