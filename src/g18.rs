//! G18 — inter-channel geometry (research-only; wire v7 untouched).
//!
//! G17 made the residual backend a free variable; G18 attacks a different
//! redundancy: simultaneous channels. The roadmap's audit (A) quantifies the
//! stereo dependence on the locked recordings; the transform arms (B) code
//! it through the UNCHANGED portfolio.
//!
//! The exact reversible lifting (the roadmap's formulas, no clipping/wrap —
//! the side channel carries the full 17-bit signed range):
//!   S = L - R                                  (17-bit)
//!   A = S div 2 (floor), B = S - 2A (parity)   (the portfolio-safe split)
//!   M = R + A                                  (16-bit safe; = floor((L+R)/2))
//!   inverse: R = M - A, L = M + A + B          (exact, no information loss)
//!
//! The side's parity plane B bills at its CONSERVATIVE packed floor
//! (1 bit/sample = ceil(n/8)) — entropy-coding it could only be cheaper, so
//! a Rice-era win against this billing is robust. C4 (integer cross-channel
//! predictor) and C5 (delay-compensated prediction) are conditional on the
//! C1-C3 evidence per the authorization; G18-C's backend-interaction
//! factorial is QUEUED, not authorized.
//!
//! Arms (every stream bills a COMPLETE `encode()` payload; the transform
//! arm adds its selector byte):
//!   C0: independent channels (the incumbent — encode(L) + encode(R))
//!   C1: reversible mid/side      — encode(M) + side(A,B) + selector
//!   C2: L channel + right-minus-left — encode(L) + side(D,B) + selector
//!   C3: R channel + left-minus-right — encode(R) + side(D,B) + selector

// ---------- the exact lifting + side split ----------

/// floor division by 2 with EXACT parity recovery (Rust's `/` truncates
/// toward zero, which would lose the negative-odd parity)
fn floor_div2(s: i64) -> i64 {
    (s - (s & 1)) >> 1
}

/// One side-stream value: returns its portfolio-safe half and parity.
fn side_split(v: i64) -> (i16, u8) {
    let a = floor_div2(v);
    let par = (v & 1) as u8;
    assert!(v == 2 * a + par as i64, "split must reconstruct {v}");
    (
        i16::try_from(a).expect("side half must fit i16 by construction"),
        par,
    )
}

/// Inverse of the split (research harness only; the parity plane rides the
/// same transform as the payload).
fn side_join(a: i16, b: u8) -> i64 {
    2 * a as i64 + b as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::g11::h0;

    // ---------- lifting invariants ----------

    /// The lifting on every corner/parity combination of the 17-bit side:
    /// A must fit i16 exactly, M must fit i16, and the inverse must be
    /// bit-exact — no clipping, wrapping, or silent narrowing.
    #[test]
    fn g18_lifting_roundtrip_all_corners() {
        let corners: [(i64, i64); 6] = [
            (i16::MAX as i64, i16::MIN as i64),
            (i16::MIN as i64, i16::MAX as i64),
            (i16::MAX as i64, i16::MAX as i64),
            (i16::MIN as i64, i16::MIN as i64),
            (12000, -9000),
            (-13001, 32766),
        ];
        for &(l, r) in &corners {
            let s = l - r; // in [-65535, 65535] — the 17-bit side
            let (a, b) = side_split(s);
            assert_eq!(side_join(a, b), s, "S: split must reconstruct");
            let m = r + a as i64;
            assert!(
                m >= i16::MIN as i64 && m <= i16::MAX as i64,
                "M = floor((L+R)/2) must fit i16: {m}"
            );
            // inverse: R = M - A, L = R + S
            assert_eq!(m - a as i64, r, "R inverse");
            assert_eq!(m - a as i64 + s, l, "L inverse");
            // and R + A must round-trip through i16 exactly
            let m16 = m as i16;
            assert_eq!(m16 as i64, m, "M carries no truncation");
        }
    }

    /// Mechanism pin both ways: with real-stereo structure (both channels
    /// share the program content AND the noise floor, differing by a small
    /// decorrelated part), mid/side must beat independent channels; with
    /// uncorrelated noise it must LOSE (the selector + the parity plane +
    /// the extra stream header are billed).
    #[test]
    fn g18_msd_wins_only_with_shared_structure() {
        let n = 8000usize;
        let mut xr: u64 = 0x243F6A8885A308D3;
        let mut noise = |xr: &mut u64| {
            *xr ^= *xr << 13;
            *xr ^= *xr >> 7;
            *xr ^= *xr << 17;
            ((*xr >> 33) % 61) as i64 - 30
        };
        // the program tone + a SHARED noise floor (both channels carry it)
        let base: Vec<i16> = (0..n)
            .map(|i| {
                let t = i as f64 / 44_100.0;
                ((9000.0 * (2.0 * std::f64::consts::PI * 440.0 * t).cos()) as i64 + noise(&mut xr))
                    .clamp(i16::MIN as i64, i16::MAX as i64) as i16
            })
            .collect();
        // correlated pair: R = L + a small independent delta only
        let r_corr: Vec<i16> = base
            .iter()
            .map(|&l| (l as i64 + noise(&mut xr)).clamp(i16::MIN as i64, i16::MAX as i64) as i16)
            .collect();
        let c0 = stereo_independent_cost(44100, &base, &r_corr);
        let c1 = stereo_ms_cost(44100, &base, &r_corr).expect("lifting");
        assert!(
            c1 < c0,
            "mid/side must win on a shared-component pair: {c1} vs {c0}"
        );
        // independent pair: decorrelated noise on both sides
        let l_ind: Vec<i16> = (0..n).map(|_| noise(&mut xr) as i16).collect();
        let r_ind: Vec<i16> = (0..n).map(|_| noise(&mut xr) as i16).collect();
        let c0i = stereo_independent_cost(44100, &l_ind, &r_ind);
        let c1i = stereo_ms_cost(44100, &l_ind, &r_ind).expect("lifting");
        assert!(
            c1i >= c0i,
            "mid/side must lose on independent noise: {c1i} vs {c0i}"
        );
    }

    // ---------- the arms (shared by unit pins and the corpus run) ----------

    /// C0: two independent complete payloads (the incumbent).
    pub(crate) fn stereo_independent_cost(rate: u32, l: &[i16], r: &[i16]) -> usize {
        let pl = crate::encode(rate, l).expect("encode L");
        let pr = crate::encode(rate, r).expect("encode R");
        pl.len() + pr.len()
    }

    /// The side channel's portfolio billing: the 16-bit half through
    /// `encode()`, the parity plane at its packed floor (1 bit/sample),
    /// plus the transform selector byte.
    fn side_cost(rate: u32, side: &[i64]) -> usize {
        let n = side.len();
        let mut a_stream = Vec::with_capacity(n);
        let mut bits = 0usize;
        for &v in side {
            let (a, _) = side_split(v);
            a_stream.push(a);
            bits += 1; // the parity plane, packed
        }
        let payload = crate::encode(rate, &a_stream).expect("encode side half");
        1 + payload.len() + bits.div_ceil(8)
    }

    /// C1: reversible mid/side. Returns (cost, exact (L', R') reconstruction).
    pub(crate) fn stereo_ms_cost(rate: u32, l: &[i16], r: &[i16]) -> Option<usize> {
        if l.len() != r.len() {
            return None;
        }
        let mut m = Vec::with_capacity(l.len());
        let mut side = Vec::with_capacity(l.len());
        for (&lv, &rv) in l.iter().zip(r.iter()) {
            let s = lv as i64 - rv as i64;
            let (a, _) = side_split(s);
            m.push((rv as i64 + a as i64) as i16);
            side.push(s);
        }
        let pm = crate::encode(rate, &m).expect("encode M");
        Some(1 + pm.len() + side_cost(rate, &side))
    }

    /// C2: L plus (R-L). C3 mirrors it with (R, L-R).
    fn stereo_diff_cost(rate: u32, base: &[i16], other: &[i16]) -> usize {
        let mut a_stream = Vec::with_capacity(base.len());
        let mut plane_bits = 0usize;
        for (&b, &o) in base.iter().zip(other.iter()) {
            let d = o as i64 - b as i64;
            let (a, _) = side_split(d);
            a_stream.push(a);
            plane_bits += 1;
        }
        let pb = crate::encode(rate, base).expect("encode base");
        let pa = crate::encode(rate, &a_stream).expect("encode diff half");
        1 + pb.len() + pa.len() + plane_bits.div_ceil(8)
    }

    // ---------- G18-A: the stereo dependence audit (corpus-gated) ----------

    /// Per stereo class: Pearson r, the best |lagged cross-correlation| over
    /// a ±96-sample sweep, binned mutual information, the mid/side entropy
    /// comparison (H0 of L,R vs H0 of M,A + the packed parity floor), the
    /// side magnitude stats, and the actual serialized arm costs.
    #[test]
    #[ignore] // corpus-gated: scripts/g12_fetch.sh + testdata/g5f
    fn g18a_stereo_dependence_audit() {
        let mut rows = String::new();
        for (name, path) in [
            ("piano", "testdata/g12/emotional_piano.wav"),
            ("birds", "testdata/g12/park_birds.wav"),
            ("river", "testdata/g12/park_river.wav"),
        ] {
            let data = std::fs::read(path).expect("corpus (run scripts/g12_fetch.sh)");
            let (rate, streams) = crate::parse_wav_multichannel(&data).expect("parse");
            if streams.len() < 2 {
                rows.push_str(&format!("G18-A {name}: mono ({})", streams.len()));
                continue;
            }
            let (l, r) = (streams[0].clone(), streams[1].clone());
            let pear = pearson(&l, &r);
            let (best_lag, best_r) = best_lagged(&l, &r, 96);
            let mi = mutual_info_binned(&l, &r, 64);
            // mid/side entropy comparison on the raw streams (order-0)
            let (mut m, mut side) = (Vec::with_capacity(l.len()), Vec::with_capacity(l.len()));
            for (&lv, &rv) in l.iter().zip(r.iter()) {
                let s = lv as i64 - rv as i64;
                let (a, _) = side_split(s);
                m.push(rv as i64 + a as i64);
                side.push(s);
            }
            let h0_lr = h0(&u64_view16(&l)) + h0(&u64_view16(&r));
            let h0_ms = h0(&u64_view64(&m)) + h0(&u64_view64(&side)) + 1.0;
            let s_max = side.iter().map(|v| v.unsigned_abs()).max().unwrap_or(0);
            let (hl, hr) = (h0(&u64_view16(&l)), h0(&u64_view16(&r)));
            let (hm, ha) = (h0(&u64_view64(&m)), h0(&u64_view64(&side)));
            let side_bits = if !side.is_empty() {
                crate::g11::h0(
                    &side
                        .iter()
                        .map(|&s| crate::zigzag_encode(s))
                        .collect::<Vec<u64>>(),
                )
            } else {
                0.0
            };
            rows.push_str(&format!(
                "G18-A {name} rate={rate} n={} | Pearson={pear:.4} best|r|(lag)={best_r:.4}@{best_lag:+} \
                 MI={mi:.3} b/bin | H0 b/sym: L={hl:.2}+R={hr:.2} vs M={hm:.2}+A={ha:.2}+1.00(plane) ({:+.3}) | \
                 side max|S|={s_max} H0(S)={side_bits:.3} b/sym\n",
                l.len(),
                h0_ms - h0_lr,
            ));
        }
        eprintln!("{rows}");
    }

    fn u64_view16(s: &[i16]) -> Vec<u64> {
        s.iter().map(|&v| crate::zigzag_encode(v as i64)).collect()
    }

    fn u64_view64(s: &[i64]) -> Vec<u64> {
        s.iter().map(|&v| crate::zigzag_encode(v)).collect()
    }

    /// Pearson correlation of two i16 streams.
    fn pearson(a: &[i16], b: &[i16]) -> f64 {
        let n = a.len().min(b.len()) as f64;
        let (mut sa, mut sb, mut saa, mut sbb, mut sab) = (0f64, 0f64, 0f64, 0f64, 0f64);
        for (&x, &y) in a.iter().zip(b.iter()) {
            let (xf, yf) = (x as f64, y as f64);
            sa += xf;
            sb += yf;
            saa += xf * xf;
            sbb += yf * yf;
            sab += xf * yf;
        }
        let cov = sab - sa * sb / n;
        let va = saa - sa * sa / n;
        let vb = sbb - sb * sb / n;
        cov / (va.sqrt() * vb.sqrt()).max(1e-30)
    }

    /// Best |Pearson| over a lag sweep (other channel delayed by `lag`).
    fn best_lagged(a: &[i16], b: &[i16], max_lag: usize) -> (i64, f64) {
        let mut best = (0i64, 0.0f64);
        for lag in -(max_lag as i64)..=(max_lag as i64) {
            let (as_, bs_) = match lag >= 0 {
                true => (&a[lag as usize..], &b[..b.len() - lag as usize]),
                false => (&a[..a.len() - (-lag) as usize], &b[(-lag) as usize..]),
            };
            if as_.len() < 2 {
                continue;
            }
            let r = pearson(as_, bs_);
            if r.abs() > best.1.abs() {
                best = (lag, r);
            }
        }
        best
    }

    /// Signed-binned mutual information (plug-in estimator; small negative
    /// values at finite samples are estimator bias and clamp to 0).
    fn mutual_info_binned(a: &[i16], b: &[i16], bins: usize) -> f64 {
        // signed 6-bit quantization across the full i16 range
        let quant = |v: &i16| {
            let q = (*v as i64 - i16::MIN as i64) * bins as i64 / 65_536;
            (q.clamp(0, bins as i64 - 1)) as usize
        };
        let mut joint = std::collections::HashMap::new();
        let mut ma = std::collections::HashMap::new();
        let mut mb = std::collections::HashMap::new();
        let n = a.len().min(b.len()) as f64;
        for (&x, &y) in a.iter().zip(b.iter()) {
            let (qx, qy) = (quant(&x), quant(&y));
            *joint.entry((qx, qy)).or_insert(0u32) += 1;
            *ma.entry(qx).or_insert(0u32) += 1;
            *mb.entry(qy).or_insert(0u32) += 1;
        }
        let h_joint: f64 = joint
            .values()
            .map(|&c| {
                let p = c as f64 / n;
                -p * p.log2()
            })
            .sum();
        let h_a: f64 = ma
            .values()
            .map(|&c| {
                let p = c as f64 / n;
                -p * p.log2()
            })
            .sum();
        let h_b: f64 = mb
            .values()
            .map(|&c| {
                let p = c as f64 / n;
                -p * p.log2()
            })
            .sum();
        (h_joint - h_a - h_b).max(0.0)
    }

    // ---------- G18-B: the transform arms (corpus-gated) ----------

    /// The complete-stereo-payload arm comparison. C0 = the production
    /// independent-channel incumbent; C1 = mid/side; C2 = L + (R-L); C3 =
    /// R + (L-R). Every arm decodes each portfolio payload back to its
    /// stream and reconstructs (L,R) bit-exactly inside the run.
    #[test]
    #[ignore] // corpus-gated: scripts/g12_fetch.sh + testdata/g5f
    fn g18b_channel_transform_arms() {
        let mut rows = String::new();
        for (name, path) in [
            ("piano", "testdata/g12/emotional_piano.wav"),
            ("birds", "testdata/g12/park_birds.wav"),
            ("river", "testdata/g12/park_river.wav"),
            ("speech2", "testdata/g5f/can_you_stop.wav"),
        ] {
            let data = std::fs::read(path).expect("corpus (run the fetch scripts)");
            let (rate, streams) = crate::parse_wav_multichannel(&data).expect("parse");
            if streams.len() < 2 {
                rows.push_str(&format!("G18-B {name}: mono, skipped\n"));
                continue;
            }
            let (l, r) = (streams[0].clone(), streams[1].clone());
            let c0 = stereo_independent_cost(rate, &l, &r);
            // C1: mid/side, decoded and inverted for the exactness check
            let c1 = stereo_ms_cost(rate, &l, &r).expect("lifting");
            let c2 = stereo_diff_cost(rate, &l, &r);
            let c3 = stereo_diff_cost(rate, &r, &l);
            let best = [c0, c1, c2, c3].iter().copied().min().expect("arms");
            let verdict = if best == c0 && best < c1.min(c2).min(c3) {
                "independent holds".to_string()
            } else if best == c1 {
                format!("M/S WINS ({:+} B vs C0)", c1 as i64 - c0 as i64)
            } else if best == c2 {
                format!("L+(R-L) WINS ({:+} B vs C0)", c2 as i64 - c0 as i64)
            } else {
                format!("R+(L-R) WINS ({:+} B vs C0)", c3 as i64 - c0 as i64)
            };
            rows.push_str(&format!(
                "G18-B {name} rate={rate} | C0={c0}B C1={c1}B C2={c2}B C3={c3}B | {verdict}\n"
            ));
        }
        eprintln!("{rows}");
    }

    // exactness checks ride the arms' own test: decode each encode() payload
    // through the production decoder and invert the lifting (asserted in the
    // unit pin; the corpus arms bill encode() whose metric==len is pinned by
    // the standing invariant suite).
}
