//! G6 — four-dimensional acoustic dynamics (research-only; wire untouched).
//!
//! Ledger hypothesis: a sound with identifiable excitation events may admit a
//! compact deterministic representation through a small evolving integer state
//! (q, v, e, d), whose event timing explains information global predictors
//! leave in corrections. Acceptance is COMPLETE cost:
//! L_4D = L(model) + L(events) + L(corrections) < L(existing portfolio).
//!
//! Fixtures (the 4D-1..4D-6 contract):
//! 1. 4D-1 struck resonator — damped sine, one strike
//! 2. 4D-2 repeated strikes — three identical strikes (shared dynamics)
//! 3. 4D-3 interacting pair — two recurrences coupled through a cross term
//! 4. 4D-4 real recording — measured from the locked G5-F corpus
//! 5. 4D-5 noise — deterministic white; MDL must REJECT
//! 6. 4D-6 saturated — strike clipped at ±32767; exactness holds
//!
//! Arms (corrections billed identically — shipped zero-run mask + zigzag):
//! - osc2: the shipped 2-state family (its stage-0 corrections + coefficients)
//! - str4: structured 4D, v' = v - qκ - vρ, q' = q + v (Q15 fixed point);
//!   event structure = per-strike state resets (2 B/strike; e,d hold them)
//! - gen4: generic 4×4 linear state (the skeptic arm: 12 free Q15 entries)

/// Complete corrections cost on the shipped codec (mask RLE + zigzag varints).
#[cfg(test)]
fn corrections_cost(r: &[i64]) -> (usize, usize) {
    use crate::masks_rle::mask_rle_wire_len;
    use crate::plan_corrections_masked_op;
    use crate::{varint_len, zigzag_encode};

    let (masks, deltas) = plan_corrections_masked_op(r);
    let bytes = mask_rle_wire_len(&masks)
        + deltas
            .iter()
            .map(|&d| varint_len(zigzag_encode(d)))
            .sum::<usize>();
    (bytes, deltas.len())
}

/// Structured 4D replay: v ← v − qκ − vρ ; q ← q + v (Q15 fixed point).
/// Saturates the observation like the shipped clamped-recording convention.
#[cfg(test)]
fn replay_str4(q0: i64, v0: i64, kap: i64, rho: i64, n: usize) -> Vec<i16> {
    let (mut q, mut v) = (q0, v0);
    (0..n)
        .map(|_| {
            let vn = v - (q * kap).div_euclid(1 << 15) - (v * rho).div_euclid(1 << 15);
            let qn = q + vn;
            v = vn;
            q = qn;
            qn.clamp(-32767, 32767) as i16
        })
        .collect()
}

/// Generic 4×4 linear state (row-major >>15), observation = q component.
#[cfg(test)]
fn replay_gen4(a: &[i64; 16], s0: [i64; 4], n: usize) -> Vec<i16> {
    let mut s = s0;
    (0..n)
        .map(|_| {
            let s2 = [
                (a[0] * s[0] + a[1] * s[1] + a[2] * s[2] + a[3] * s[3]).div_euclid(1 << 15),
                (a[4] * s[0] + a[5] * s[1] + a[6] * s[2] + a[7] * s[3]).div_euclid(1 << 15),
                (a[8] * s[0] + a[9] * s[1] + a[10] * s[2] + a[11] * s[3]).div_euclid(1 << 15),
                (a[12] * s[0] + a[13] * s[1] + a[14] * s[2] + a[15] * s[3]).div_euclid(1 << 15),
            ];
            s = s2;
            s2[0].clamp(-32767, 32767) as i16
        })
        .collect()
}

/// Exact damped sine: x[i] = A·e^(−λt)·cos(ωt+φ) from the strike time,
/// rounded + clamped — the honest physical-event analogue (float-generated
/// THEN quantized, like every earlier fixture class).
#[cfg(test)]
fn struck(f_hz: f64, lam: f64, a0: f64, n: usize, rate: f64, strike_at: usize) -> Vec<i16> {
    let w = 2.0 * std::f64::consts::PI * f_hz / rate;
    (0..n)
        .map(|i| {
            if i < strike_at {
                0
            } else {
                let t = (i - strike_at) as f64 / rate;
                let v = a0 * (-lam * t).exp() * (w * t).cos();
                v.round().clamp(-32767.0, 32767.0) as i16
            }
        })
        .collect()
}

/// 4D-3: two resonators coupled through a weak cross term (exact integer
/// arithmetic on i64 Q15; the observation sums the two partials).
#[cfg(test)]
fn coupled(f1: i64, f2: i64, cross: i64, a0: i64, n: usize) -> Vec<i16> {
    let q = 1i64 << 15;
    let (mut q1, mut v1, mut q2, mut v2) = (a0, 0i64, a0 / 2, 0i64);
    (0..n)
        .map(|_| {
            let vn1 = v1 - (f1 * q1 + cross * q2).div_euclid(q);
            let vn2 = v2 - (f2 * q2 + cross * q1).div_euclid(q);
            let qn1 = q1 + vn1;
            let qn2 = q2 + vn2;
            v1 = vn1;
            q1 = qn1;
            v2 = vn2;
            q2 = qn2;
            (qn1 + qn2).clamp(-32767, 32767) as i16
        })
        .collect()
}

/// Deterministic white noise (xorshift64 — reproducible across runs).
#[cfg(test)]
fn noise(seed: u64, n: usize) -> Vec<i16> {
    let mut xr = seed;
    (0..n)
        .map(|_| {
            xr ^= xr << 13;
            xr ^= xr >> 7;
            xr ^= xr << 17;
            (((xr >> 33) % 65536) as i32 - 32768) as i16
        })
        .collect()
}

/// Three identical strikes of the same damped tone (4D-2's target class).
#[cfg(test)]
fn repeated_strikes(n: usize, rate: f64, spacing: usize, tail: usize) -> Vec<i16> {
    let w = 2.0 * std::f64::consts::PI * 917.3 / rate;
    let mut y = vec![0i16; n];
    for k in 0..3 {
        let s = k * spacing;
        for i in 0..tail {
            if s + i < n {
                let t = i as f64 / rate;
                y[s + i] = ((12000.0 * (-0.0022 * t).exp() * (w * t).cos())
                    .round()
                    .clamp(-32767.0, 32767.0)) as i16;
            }
        }
    }
    y
}

/// STR4 sweep with per-strike resets: discovery = smallest COMPLETE cost on
/// the (κ,ρ) grid; billing = params + events + corrections (identical codec).
#[cfg(test)]
fn str4_sweep(
    y: &[i16],
    strikes: &[usize],
    kaps: &[i64],
    rhos: &[i64],
) -> (usize, i64, i64, usize) {
    let mut best = (usize::MAX, 0i64, 0i64, 0usize);
    for &kap in kaps {
        for &rho in rhos {
            let mut full = Vec::with_capacity(y.len());
            for (si, &st) in strikes.iter().enumerate() {
                let len = if si + 1 < strikes.len() {
                    strikes[si + 1] - st
                } else {
                    y.len() - st
                };
                full.extend(replay_str4(1 << 13, 0, kap, rho, len));
            }
            let r: Vec<i64> = y
                .iter()
                .zip(full.iter())
                .map(|(&a, &b)| a as i64 - b as i64)
                .collect();
            let (cbytes, _) = corrections_cost(&r);
            // complete: 2 params varint + q0 + events (2 B/strike) + corrections
            let total = 6 + strikes.len() * 2 + cbytes;
            if total < best.0 {
                best = (total, kap, rho, cbytes);
            }
        }
    }
    best
}

/// GEN4 skeptic arm: coarse identity-neighborhood grid; complete cost bills
/// 12 matrix entries (Q15 ≈ 3 B each) + corrections.
#[cfg(test)]
fn gen4_best(y: &[i16]) -> (usize, i64, i64, usize) {
    let offs = [1i64 << 12, 1 << 13, 1 << 14];
    let mut best = (usize::MAX, 0i64, 0i64, 0usize);
    for d1 in offs {
        for d2 in offs {
            let mut a = [0i64; 16];
            a[0] = 1 << 15; // q persists
            a[5] = 1 << 15; // v persists
            a[1] = -d1; // v' += −q·d (the osc term)
            a[10] = d2; // e-dynamics placeholder: e' += v·d2
            let seg = replay_gen4(&a, [0, 1 << 10, 0, 0], y.len());
            let full = seg;
            let r: Vec<i64> = y
                .iter()
                .zip(full.iter())
                .map(|(&x, &s)| x as i64 - s as i64)
                .collect();
            let (cbytes, _) = corrections_cost(&r);
            let total = 12 * 3 + cbytes;
            if total < best.0 {
                best = (total, a[1], a[10], cbytes);
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode;
    use crate::g5::anatomy_positions;

    /// The full 4D-1..4D-6 contract: fixtures generated, shipped-vs-stage0
    /// baselines measured, noise rejection and saturation exactness asserted.
    #[test]
    fn g6_four_dim_acoustic_dynamics() {
        let n = 4096;
        let rate = 44_100.0f64;

        // 4D-1: single struck resonator
        let y1 = struck(917.3, 0.0022, 12000.0, n, rate, 0);
        let shipped1 = encode(44100, &y1).expect("e").len();
        let (r1, m1) = anatomy_positions(&y1, 1).expect("stage0");
        let (c1, _) = corrections_cost(&r1);
        eprintln!(
            "4D-1 struck: pcm={}B shipped={}B osc2={}B (nonzero {} mean|state|mag {:.0})",
            2 * n,
            shipped1,
            c1 + 9,
            r1.len(),
            m1.iter().sum::<i64>() as f64 / m1.len() as f64
        );

        // 4D-2: three identical strikes
        let y2 = repeated_strikes(n, rate, 1200, 1100);
        let shipped2 = encode(44100, &y2).expect("e").len();
        let (r2, _) = anatomy_positions(&y2, 1).expect("stage0");
        let (c2, _) = corrections_cost(&r2);
        eprintln!(
            "4D-2 repeated: shipped={}B osc2={}B (nonzero {})",
            shipped2,
            c2 + 9,
            r2.len()
        );

        // 4D-3: coupled pair
        let y3 = coupled(32768, 24576, 256, 12000, n);
        let shipped3 = encode(44100, &y3).expect("e").len();
        let (r3, _) = anatomy_positions(&y3, 1).expect("stage0");
        let (c3, _) = corrections_cost(&r3);
        eprintln!("4D-3 coupled: shipped={}B osc2={}B", shipped3, c3 + 9);

        // 4D-4: real recording (locked corpus)
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/g5f");
        let real = std::fs::read(format!("{dir}/can_you_stop.wav")).expect("g5f_fetch");
        let (rate_r, streams) = crate::parse_wav_multichannel(&real).expect("parse");
        let real_y = &streams[0];
        let shipped_r = encode(rate_r, real_y).expect("e").len();
        let (rr, _) = anatomy_positions(real_y, 1).expect("stage0");
        let (cr, _) = corrections_cost(&rr);
        eprintln!("4D-4 real: shipped={}B stage0={}B", shipped_r, cr + 9);

        // 4D-6: saturated PCM — exactness at overflow
        let y6 = struck(917.3, 0.0022, 32767.0 * 3.0, n, rate, 0);
        let payload6 = encode(44100, &y6).expect("e");
        let (_, back6, _) = crate::decode_payload(&payload6).expect("decode");
        assert_eq!(back6, y6, "saturated PCM must decode exactly");

        eprintln!("G6 baselines measured");
    }

    /// The STR4 replay invariant: the structured 4D form is exact against its
    /// own frozen parameters (pure function of (q0, v0, κ, ρ, n)).
    #[test]
    fn g6_str4_replay_exact() {
        let out = replay_str4(1 << 14, 0, 32768, 256, 32);
        assert_eq!(out.len(), 32);
        let again = replay_str4(1 << 14, 0, 32768, 256, 32);
        assert_eq!(out, again);
    }

    /// GEN4 overflow boundary: the 4×4 >>15 replay holds the clamp at the
    /// boundary (the review's decoder-exactness contract).
    #[test]
    fn g6_gen4_saturation_exact() {
        let a = [
            1i64 << 15,
            0,
            0,
            0,
            0,
            1 << 15,
            0,
            0,
            0,
            0,
            1 << 15,
            0,
            0,
            0,
            0,
            1 << 15,
        ];
        let out = replay_gen4(&a, [32767, 32767, 32767, 32767], 8);
        assert!(out.iter().all(|&v| v == 32767), "saturation must hold");
    }

    /// The G6 acceptance row: STR4's complete cost vs osc2 vs shipped on
    /// 4D-1/4D-2; noise must be REJECTED (4D-5). Discovery = smallest
    /// complete cost on the (κ,ρ) grid; corrections billed identically.
    #[test]
    fn g6_str4_discovery_acceptance() {
        let n = 4096;
        let rate = 44_100.0f64;
        // 4D-1
        let y1 = struck(917.3, 0.0022, 12000.0, n, rate, 0);
        let shipped1 = encode(44100, &y1).expect("e").len();
        let (r1, _) = anatomy_positions(&y1, 1).expect("stage0");
        let (c1, _) = corrections_cost(&r1);
        let (t1, kap1, rho1, _) = str4_sweep(
            &y1,
            &[0],
            &[24576, 32768, 40960, 49152],
            &[0, 128, 256, 512, 1024],
        );
        eprintln!(
            "G6 4D-1: shipped={shipped1} osc2={} str4={t1} (kap={kap1} rho={rho1})",
            c1 + 9
        );
        // 4D-2: three identical strikes (shared dynamics + reset times)
        let y2 = repeated_strikes(n, rate, 1200, 1100);
        let shipped2 = encode(44100, &y2).expect("e").len();
        let (r2, _) = anatomy_positions(&y2, 1).expect("stage0");
        let (c2, _) = corrections_cost(&r2);
        let (t2, kap2, rho2, _) = str4_sweep(
            &y2,
            &[0, 1200, 2400],
            &[24576, 32768, 40960],
            &[0, 128, 256, 512],
        );
        eprintln!(
            "G6 4D-2: shipped={shipped2} osc2={} str4={t2} (kap={kap2} rho={rho2})",
            c2 + 9
        );
        // 4D-5: noise — MDL must reject the structured model
        let y5 = noise(0xABCD, n);
        let shipped5 = encode(44100, &y5).expect("e").len();
        let (t5, _, _, _) = str4_sweep(&y5, &[0], &[32768], &[0]);
        eprintln!(
            "G6 4D-5 noise: shipped={shipped5} str4={t5} (reject={})",
            t5 > shipped5
        );
        assert!(t5 > shipped5, "MDL must reject the 4D model on noise");
        // gen4 priced once on 4D-1 (the skeptic arm: 12 entries' params bill
        // against its corrections)
        let (tg, _, _, _) = gen4_best(&y1);
        eprintln!("G6 4D-1 gen4: {tg}B vs shipped {shipped1}B");
    }
}
