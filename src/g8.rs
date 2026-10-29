//! G8 — coefficient dynamics (research-only; wire untouched).
//!
//! The G7 review's next hypothesis: the CASCADE's locally fitted coefficients
//! may themselves carry a compact structure (their own dynamics), even though
//! G7-B showed the coefficients do not transfer unchanged.
//!
//! G8-A FIRST (the review's early-closure gate — purely diagnostic): how many
//! bytes do stage coefficients actually cost on the production wire, per
//! window, versus corrections? If the whole budget is small (< ~3-5%), even
//! perfect coefficient prediction cannot matter — close G8 before building
//! anything.
//!
//! Coefficient wire cost in the shipped family-3 cascade: per stage,
//! varint(zigzag(c)) + varint(zigzag(r)) — billed from the plan itself.
//! Also recorded: the (c, r) pairs vs the 2nd-order constraint curve
//! (r2 = Q15 ⇒ pure oscillation; r2 < Q15 = damping), the joint-distribution
//! entropy vs marginals (G8-A's joint-vs-independent question), and the
//! delta-trajectory entropy (G8-B's predictability probe).

/// Wire cost of a stage's coefficient pair (as the f3 cascade serializes it).
#[cfg(test)]
fn coeff_pair_cost(c: i64, r: i64) -> usize {
    use crate::{varint_len, zigzag_encode};
    varint_len(zigzag_encode(c)) + varint_len(zigzag_encode(r))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::g5::analyze_f3;

    fn corpus() -> Vec<(&'static str, u32, Vec<Vec<i16>>)> {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/g5f");
        ["example_word.wav", "can_you_stop.wav", "sample2.wav"]
            .iter()
            .filter_map(|name| {
                let data = std::fs::read(format!("{dir}/{name}")).ok()?;
                let (rate, streams) = crate::parse_wav_multichannel(&data).ok()?;
                Some((*name, rate, streams))
            })
            .collect()
    }

    /// G8-A: the coefficient-budget audit that gates the entire G8 line.
    #[test]
    fn g8a_coefficient_budget_audit() {
        for (name, rate, streams) in corpus() {
            for (ch, y) in streams.iter().enumerate() {
                let wins = y.len() / 4096;
                let (mut payload, mut coeff) = (0usize, 0usize);
                let mut pairs = Vec::new();
                for w in 0..wins {
                    let seg = &y[w * 4096..(w + 1) * 4096];
                    payload += crate::encode(rate, seg).expect("e").len();
                    let stages = analyze_f3(seg, &[1]);
                    if let Some(sa) = stages.first() {
                        coeff += coeff_pair_cost(sa.cwq, sa.r2q);
                        pairs.push((sa.cwq, sa.r2q));
                    }
                }
                let pct = 100.0 * coeff as f64 / payload.max(1) as f64;
                eprintln!(
                    "G8-A {name} ch{ch}: windows={wins} payloads={}B coeff_bytes={}B share={:.2}%",
                    payload, coeff, pct
                );
            }
        }
        // the review's early-closure gate: < 3% ⇒ G8 CLOSED (no adaptive
        // machinery can buy more than the whole coefficient budget).
    }

    /// G8-B: delta trajectories — are successive coefficient CHANGES
    /// predictable? (entropy of deltas vs absolute; small sample honesty.)
    #[test]
    fn g8b_delta_trajectories() {
        for (name, _rate, streams) in corpus() {
            let y = &streams[0];
            let wins = y.len() / 4096;
            let mut abs_pairs = Vec::new();
            let mut delta_pairs = Vec::new();
            for w in 0..wins {
                let seg = &y[w * 4096..(w + 1) * 4096];
                let stages = analyze_f3(seg, &[1]);
                if let Some(sa) = stages.first() {
                    abs_pairs.push((sa.cwq, sa.r2q));
                }
            }
            for w in 1..abs_pairs.len() {
                let (c0, r0) = abs_pairs[w - 1];
                let (c1, r1) = abs_pairs[w];
                delta_pairs.push((c1 - c0, r1 - r0));
            }
            let r2_hist: Vec<i64> = abs_pairs.iter().map(|&(_, r)| r).collect();
            let (h_abs_c, h_abs_r) = {
                // marginals of the ABSOLUTE coefficient streams
                let mut mc = std::collections::HashMap::new();
                for &(c, _) in &abs_pairs {
                    *mc.entry(c).or_insert(0u32) += 1;
                }
                let mut mr = std::collections::HashMap::new();
                for &(_, r) in &abs_pairs {
                    *mr.entry(r).or_insert(0u32) += 1;
                }
                let hm = |m: &std::collections::HashMap<i64, u32>| -> f64 {
                    m.values()
                        .map(|&v| {
                            let p = v as f64 / abs_pairs.len() as f64;
                            -p * p.log2()
                        })
                        .sum::<f64>()
                };
                (hm(&mc), hm(&mr))
            };
            let hd = {
                let mut m = std::collections::HashMap::new();
                for &(dc, dr) in &delta_pairs {
                    *m.entry((dc, dr)).or_insert(0u32) += 1;
                }
                m.values()
                    .map(|&v| {
                        let p = v as f64 / delta_pairs.len().max(1) as f64;
                        -p * p.log2()
                    })
                    .sum::<f64>()
            };
            let near_unity = r2_hist
                .iter()
                .filter(|&&r| (r - crate::OSC_Q15).abs() < 64)
                .count();
            eprintln!(
                "G8-B {name}: n={} H(c)={:.2} H(r)={:.2} H(delta-pairs)={:.2} |r2-Q15|<64: {}/{}",
                abs_pairs.len(),
                h_abs_c,
                h_abs_r,
                hd,
                near_unity,
                abs_pairs.len()
            );
        }
    }
}
