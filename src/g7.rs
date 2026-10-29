//! G7 — acoustic structure attribution (research-only; wire untouched).
//!
//! The G6 review's milestone: determine WHICH relationships make the standing
//! portfolio win, before any new grammar.
//!
//! 7-A (recurrence anatomy — includes the G6-4D-4 AUDIT the review ordered:
//! stage0-vs-shipped re-billed with the EXACT production framing via
//! payload_len, killing the 1.42% artifact question):
//! 1. shipped whole-file payload (the production comparator)
//! 2. every OSC variant's payload_len (the exact kind-4 framing)
//! 3. the best OP arm
//! 4. the stage0 (r, |y|) correction anatomy (G5's instruments)
//! 7-B (event transfer): parameters discovered from sound A reconstruct sound
//! B — same generator class without refitting (decode-time transfer).
//! 7-C (causal perturbation): perturb one coefficient at a time (c, r, q0, y1)
//! around a discovered stage and record the corrections cost delta — which
//! parameters the compactness DEPENDS on.
//! 7-D (physical generalization): parameters across OTHER real recordings.

/// Shipped-codec corrections cost (identical billing across G7 arms).
#[cfg(test)]
fn corrections_cost(r: &[i64]) -> (usize, usize) {
    use crate::masks_rle::mask_rle_wire_len as _rle;
    use crate::plan_corrections_masked_op;
    use crate::{varint_len, zigzag_encode};
    let (masks, deltas) = plan_corrections_masked_op(r);
    let bytes = _rle(&masks)
        + deltas
            .iter()
            .map(|&d| varint_len(zigzag_encode(d)))
            .sum::<usize>();
    (bytes, deltas.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode;
    use crate::g5::analyze_f3;
    use crate::g5::anatomy_positions as positions;
    use crate::payload_len;

    fn corpus() -> std::collections::HashMap<&'static str, (u32, Vec<Vec<i16>>)> {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/g5f");
        let mut m = std::collections::HashMap::new();
        for name in ["example_word.wav", "can_you_stop.wav", "sample2.wav"] {
            let data = std::fs::read(format!("{dir}/{name}")).expect("g5f corpus");
            m.insert(name, crate::parse_wav_multichannel(&data).expect("parse"));
        }
        m
    }

    /// G7-A: the G6-4D-4 audit + the recurrence anatomy on real speech.
    /// The 1.42% "win" must survive EXACT kind framing to stand.
    #[test]
    fn g7a_recurrence_anatomy_and_4d4_audit() {
        let corpus = corpus();
        for (name, (rate, streams)) in &corpus {
            let y = &streams[0];
            let shipped = encode(*rate, y).expect("e").len();
            let mut best_osc: Option<(u8, usize)> = None;
            for variant in 0..9u8 {
                if let Some(n) = payload_len(4, variant as usize, y) {
                    if best_osc.map(|(_, m)| n < m).unwrap_or(true) {
                        best_osc = Some((variant, n));
                    }
                }
            }
            let _ = best_osc;
            if let Some((variant, n)) = best_osc {
                eprintln!(
                    "G7-A {name}: shipped={shipped} best_osc(kind4 var{variant})={n} delta={}",
                    shipped as i64 - n as i64
                );
            }
            // stage0 anatomy for the record (dense-corrections baseline)
            let (r, _) = positions(y, 1).expect("stage0");
            let (cb, nnz) = corrections_cost(&r);
            eprintln!("G7-A {name}: stage0 corrections-only={cb}B (nnz {nnz}; audit: NOT comparable to shipped — kind framing/coeffs/seeds billed only in kind rows)");
        }
    }

    /// G7-B: event transfer. Parameters discovered from REAL sound A must
    /// explain sound B (two disjoint segments of the SAME recording = same
    /// physical system; the transfer = reconstruct B with A's coefficients,
    /// corrections-only billing, vs B's own fitted stage).
    #[test]
    fn g7b_event_transfer() {
        let corpus = corpus();
        for (name, (_rate, streams)) in &corpus {
            let y = &streams[0];
            if y.len() < 8192 {
                continue;
            }
            let (a, b) = y.split_at(y.len() / 2);
            let fit_a = crate::g5::analyze_f3(a, &[1]);
            let Some(sa) = fit_a.first() else { continue };
            // transfer: replay B under A's coefficients (decoder has (c, r)) —
            // corrections billed
            let x_b: Vec<i64> = b.iter().map(|&s| s as i64).collect();
            // replay_b = shipped closed loop with sa.cwq, sa.r2q over B's seed
            let mut p1 = x_b[0];
            let mut p2 = x_b[1];
            let mut deltas = Vec::new();
            for &act in &x_b[2..] {
                let pred = (p1 * sa.cwq - p2 * sa.r2q).div_euclid(crate::OSC_Q15);
                p2 = p1;
                p1 = act;
                deltas.push(act - pred);
            }
            let (cb_t, _) = corrections_cost(&deltas);
            // control: B's OWN stage (fresh fit on B)
            let fit_b = crate::g5::analyze_f3(b, &[1]);
            let Some(sb) = fit_b.first() else { continue };
            eprintln!(
                "G7-B {name}: transfer(A's coeffs on B)={}B vs own-fit(B's stage)={}B",
                cb_t,
                sb.coded_bits / 8
            );
        }
    }

    /// G7-C: causal perturbation — which coefficients the compactness depends
    /// on (Q15 grid steps around the discovered stage; corrections billed).
    #[test]
    fn g7c_causal_perturbation() {
        let corpus = corpus();
        let (_rate, streams) = corpus.get("can_you_stop.wav").expect("corpus key");
        let y = &streams[0];
        let win = &y[..4096.min(y.len())];
        let stages = analyze_f3(win, &[1]);
        let Some(sa) = stages.first() else { return };
        let base = sa.coded_bits;
        // perturbations: cwq ±1..3 Q15 units, r2q ±1..3 (the stage's own grid)
        let mut rows = Vec::new();
        for (label, dc, dr) in [("c−2", -2, 0), ("c+2", 2, 0), ("r−2", 0, -2), ("r+2", 0, 2)] {
            // re-run the closed-loop replay with the perturbed coefficients:
            let x: Vec<i64> = win.iter().map(|&s| s as i64).collect();
            let mut p1 = x[0];
            let mut p2 = x[1];
            let mut deltas = Vec::new();
            for &act in &x[2..] {
                let pred = (p1 * (sa.cwq + dc) - p2 * (sa.r2q + dr)).div_euclid(crate::OSC_Q15);
                p2 = p1;
                p1 = act;
                deltas.push(act - pred);
            }
            let (cb, _) = corrections_cost(&deltas);
            rows.push((label, cb * 8));
        }
        for (label, bits) in rows {
            eprintln!(
                "G7-C can_you_stop w0: base={}bits perturbed({label})={}bits delta={:+}",
                base,
                bits,
                bits as f64 - base as f64
            );
        }
    }

    /// G7-D: physical generalization — stage coefficients across ALL corpus
    /// streams (persist across recordings? record the coefficient set).
    #[test]
    fn g7d_coefficient_persistence() {
        let corpus = corpus();
        for (name, (_rate, streams)) in &corpus {
            for (ch, y) in streams.iter().enumerate() {
                let wins = y.len() / 4096;
                let mut rows = Vec::new();
                for w in 0..wins {
                    let stages = analyze_f3(&y[w * 4096..(w + 1) * 4096], &[1]);
                    if let Some(sa) = stages.first() {
                        rows.push((sa.cwq, sa.r2q));
                    }
                }
                // distinct coefficients across windows (persistence metric)
                let distinct: std::collections::HashSet<(i64, i64)> =
                    rows.iter().copied().collect();
                eprintln!(
                    "G7-D {name} ch{ch}: {wins} windows, {} distinct stage-0 (c,r) pairs — first 3: {:?}",
                    distinct.len(),
                    &rows[..3.min(rows.len())]
                );
            }
        }
    }
}
