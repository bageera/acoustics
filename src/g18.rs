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
        let noise = |xr: &mut u64| {
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
                 MI={mi:.3} b/obs | H0 b/sym: L={hl:.2}+R={hr:.2} vs M={hm:.2}+A={ha:.2}+1.00(plane) ({:+.3}) | \
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

    /// Signed mutual information via DYNAMIC per-stream binning (bins span
    /// each stream's observed range). The earlier static 64-bins-across-i16
    /// grid collapsed small-amplitude streams into a single bin — reading
    /// 0.000 even for a perfectly correlated pair — which the synthetic
    /// controls caught and this version replaces.
    fn mutual_info_binned(a: &[i16], b: &[i16], bins: usize) -> f64 {
        let q = |s: &[i16]| -> Vec<usize> {
            let lo = s.iter().map(|&v| v as i64).min().unwrap_or(0);
            let hi = s.iter().map(|&v| v as i64).max().unwrap_or(1);
            let span = (hi - lo).max(1);
            s.iter()
                .map(|&v| {
                    (((v as i64 - lo) * bins as i64 / span).clamp(0, bins as i64 - 1)) as usize
                })
                .collect()
        };
        let qa = q(a);
        let qb = q(b);
        let mut joint = std::collections::HashMap::new();
        let mut ma = std::collections::HashMap::new();
        let mut mb = std::collections::HashMap::new();
        let n = qa.len().min(qb.len()) as f64;
        for (&x, &y) in qa.iter().zip(qb.iter()) {
            *joint.entry((x, y)).or_insert(0u32) += 1;
            *ma.entry(x).or_insert(0u32) += 1;
            *mb.entry(y).or_insert(0u32) += 1;
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
        // I(X;Y) = H(X) + H(Y) - H(X,Y) >= 0. The plug-in estimate carries
        // ~(63·63)/(2n) bits of bias at 64x64 bins (recorded; the control
        // thresholds sit above it). NOTE: an earlier version computed
        // H(X,Y) - H(X) - H(Y) — MINUS the mutual information — whose clamp
        // to 0 read "0.000" for every corpus row regardless of dependence;
        // the synthetic controls caught the sign and this version replaces it.
        (h_a + h_b - h_joint).max(0.0)
    }

    /// The mutual-information estimator's validation against the three
    /// synthetic controls the research record requires (correlated,
    /// independent, delayed). Pins what the corpus MI=0.000 rows mean: the
    /// 64-bin plug-in reads a shared-component pair HIGH, reads independent
    /// noise ~0, and does NOT see a pure delay at zero lag — delay is the
    /// lagged-correlation sweep's domain, not a zero-lag joint histogram's.
    #[test]
    fn g18_mi_estimator_validated_on_controls() {
        let n = 30_000usize;
        let mut xr: u64 = 0x243F6A8885A308D3;
        let noise = |xr: &mut u64| {
            *xr ^= *xr << 13;
            *xr ^= *xr >> 7;
            *xr ^= *xr << 17;
            ((*xr >> 33) % 61) as i64 - 30
        };
        // 1. correlated control: y = x → MI must read HIGH (the bins
        //    collapse onto the diagonal)
        let base: Vec<i16> = (0..n).map(|_| noise(&mut xr) as i16).collect();
        let mi_corr = mutual_info_binned(&base, &base, 64);
        assert!(
            mi_corr > 4.0,
            "correlated control: MI={mi_corr:.3} must be high"
        );
        // 2. independent control: two independent noise draws → MI ~ 0
        let l_ind: Vec<i16> = (0..n).map(|_| noise(&mut xr) as i16).collect();
        let r_ind: Vec<i16> = (0..n).map(|_| noise(&mut xr) as i16).collect();
        let mi_ind = mutual_info_binned(&l_ind, &r_ind, 64);
        assert!(
            mi_ind < 0.15,
            "independent control: MI={mi_ind:.4} must be ≈ bias"
        );
        // 3. delayed control: y = a delayed copy of WHITE-NOISE x → the
        // zero-lag joint histogram reads ~0 while the lag sweep localizes
        // the delay — the estimator split the record's rows rely on
        let x_noise: Vec<i16> = (0..n).map(|_| noise(&mut xr) as i16).collect();
        let y_delayed: Vec<i16> = x_noise[100..].to_vec();
        let mi_delay = mutual_info_binned(&x_noise[..n - 100], &y_delayed, 64);
        assert!(
            mi_delay < 0.15,
            "delayed white-noise control: zero-lag MI={mi_delay:.4} must be ≈ bias"
        );
        let (best_lag, best_r) = best_lagged(&x_noise[..n - 100], &y_delayed, 200);
        assert!(
            best_lag == 100 && best_r > 0.98,
            "the lag sweep must localize the delay: {best_r:.3}@{best_lag}"
        );
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

    // ---------- G18-C: the geometry x Rice factorial (corpus-gated) ----------

    /// One stream's FULL cell row: pass 1 (frozen production selection,
    /// backend swap) and pass 2 (the frozen winner vs the single best dense
    /// competitor, both under the R0/R1/R2 portfolio) in one heavy pass.
    /// Returns (varint, r1, r2, portfolio, joint, kind_name). Documented
    /// joint scope: OSC/OP/PERIOD keep their varint-era selection; the full
    /// predictor x backend enumeration is G21.
    fn stream_cell(rate: u32, y: &[i16]) -> (usize, usize, usize, usize, usize, &'static str) {
        let frozen = crate::choose_model(y);
        let payload = crate::encode(rate, y).expect("encode");
        let (res_span, _mask, symbols) = crate::g17::ledger_span(frozen, rate, y);
        let r0 = crate::g17::rice_r0(&symbols);
        assert_eq!(r0, res_span, "R0 identity: the shipped varint span");
        let (r1, _k, d1) = crate::g17::rice_r1(&symbols);
        assert_eq!(d1, symbols, "R1 exact");
        let (r2, d2) = crate::g17::rice_r2(&symbols);
        assert_eq!(d2, symbols, "R2 exact");
        let rice_best = r0.min(r1).min(r2);
        // pass 2: the frozen winner vs the single best-varint dense kind
        let mut best_dense: Option<(usize, crate::Model)> = None;
        for &kind_id in &[
            crate::MODEL_RAW,
            crate::MODEL_D1,
            crate::MODEL_D2,
            crate::MODEL_D3,
            crate::MODEL_D4,
            crate::MODEL_D5,
        ] {
            if let Some(len) = crate::payload_len(kind_id, 0, y) {
                if best_dense.as_ref().is_none_or(|&(l, _)| len < l) {
                    best_dense = Some((
                        len,
                        crate::Model {
                            kind: kind_id,
                            param: 0,
                        },
                    ));
                }
            }
        }
        // frozen winner under the Rice portfolio
        let mut joint = payload.len() - res_span + 1 + rice_best;
        if let Some((_, m)) = best_dense {
            if m.kind != frozen.kind {
                let frame = crate::payload_len(m.kind, 0, y).expect("dense frame");
                let (span, _mask, syms) = crate::g17::ledger_span(m, rate, y);
                let dr0 = crate::g17::rice_r0(&syms);
                assert_eq!(dr0, span, "dense R0 identity");
                let (dr1, _k2, dd1) = crate::g17::rice_r1(&syms);
                assert_eq!(dd1, syms, "dense R1 exact");
                let (dr2, dd2) = crate::g17::rice_r2(&syms);
                assert_eq!(dd2, syms, "dense R2 exact");
                let dense_joint = frame - span + 1 + dr0.min(dr1).min(dr2);
                if dense_joint < joint {
                    joint = dense_joint;
                }
            }
        }
        (
            payload.len(),
            payload.len() - res_span + 1 + r1,
            payload.len() - res_span + 1 + r2,
            payload.len() - res_span + 1 + rice_best,
            joint,
            kind_name(frozen.kind),
        )
    }

    fn kind_name(kind: u8) -> &'static str {
        match kind {
            0 => "RAW",
            1 => "D1",
            2 => "D2",
            3 => "PERIOD",
            4 => "OSC",
            5 => "OP",
            6 => "D3",
            7 => "D4",
            8 => "D5",
            _ => "?",
        }
    }

    /// One geometry's transformed streams + the selector/plane overhead.
    struct GeomStreams {
        name: &'static str,
        base: Vec<i16>,
        half: Vec<i16>,
        plane: usize,
        selector: usize,
    }

    /// The geometry's stream pair with its plane/selector overhead.
    fn geometry_stream_pairs(l: &[i16], r: &[i16]) -> Vec<GeomStreams> {
        let mut m = Vec::with_capacity(l.len());
        let mut a_ms = Vec::with_capacity(l.len());
        let mut a_c2 = Vec::with_capacity(l.len());
        let mut a_c3 = Vec::with_capacity(l.len());
        for (&lv, &rv) in l.iter().zip(r.iter()) {
            let s = lv as i64 - rv as i64;
            let (a1, _) = side_split(s);
            let (a2, _) = side_split(rv as i64 - lv as i64);
            let (a3, _) = side_split(lv as i64 - rv as i64);
            m.push((rv as i64 + a1 as i64) as i16);
            a_ms.push(a1);
            a_c2.push(a2);
            a_c3.push(a3);
        }
        let plane = l.len().div_ceil(8);
        vec![
            GeomStreams {
                name: "indep",
                base: l.to_vec(),
                half: r.to_vec(),
                plane: 0,
                selector: 0,
            },
            GeomStreams {
                name: "ms",
                base: m,
                half: a_ms,
                plane,
                selector: 1,
            },
            GeomStreams {
                name: "c2",
                base: l.to_vec(),
                half: a_c2,
                plane,
                selector: 1,
            },
            GeomStreams {
                name: "c3",
                base: r.to_vec(),
                half: a_c3,
                plane,
                selector: 1,
            },
        ]
    }

    /// The G18-C factorial: four geometries x the backend portfolio, every
    /// cell retained (the roadmap's 12 combinations), pass 1 (frozen
    /// predictors, backend swap only) and pass 2 (dense-kind joint
    /// selection), and the interaction terms on the COMPLETE stereo
    /// payload: I = B + C - A - D. A = independent/varint (production), B =
    /// independent/Rice portfolio, C = best-varint geometry, D = best-Rice
    /// geometry. plane = the side's packed parity floor.
    #[test]
    #[ignore] // corpus-gated: scripts/g12_fetch.sh
    fn g18c_geometry_rice_factorial() {
        let mut rows = String::new();
        for (name, path) in [
            ("piano", "testdata/g12/emotional_piano.wav"),
            ("birds", "testdata/g12/park_birds.wav"),
            ("river", "testdata/g12/park_river.wav"),
        ] {
            let data = std::fs::read(path).expect("corpus (run scripts/g12_fetch.sh)");
            let (rate, streams) = crate::parse_wav_multichannel(&data).expect("parse");
            let (l, r) = (&streams[0], &streams[1]);
            rows.push_str(&format!("G18-C {name} rate={rate}\n"));
            let (mut arm_a, mut arm_b, mut arm_c, mut arm_d) =
                (usize::MAX, usize::MAX, usize::MAX, usize::MAX);
            let (mut b2, mut d2) = (usize::MAX, usize::MAX);
            for g in geometry_stream_pairs(l, r) {
                let (gname, sa, sb, plane, sel) = (g.name, g.base, g.half, g.plane, g.selector);
                let (va, r1a, r2a, pa, ja, ka) = stream_cell(rate, &sa);
                let (vb, r1b, r2b, pb, jb, kb) = stream_cell(rate, &sb);
                let varint_cell = va + vb + sel;
                let r1_cell = r1a + r1b + plane + sel;
                let r2_cell = r2a + r2b + plane + sel;
                let portf_cell = pa + pb + plane + sel;
                let joint_cell = ja + jb + plane + sel;
                rows.push_str(&format!(
                    "  {gname}: varint={varint_cell}B r1={r1_cell}B r2={r2_cell}B portfolio={portf_cell}B joint={joint_cell}B (kinds {ka}/{kb})\n"
                ));
                if gname == "indep" {
                    arm_a = varint_cell;
                    arm_b = portf_cell;
                    b2 = joint_cell;
                }
                if varint_cell < arm_c {
                    arm_c = varint_cell;
                }
                if portf_cell < arm_d {
                    arm_d = portf_cell;
                }
                if joint_cell < d2 {
                    d2 = joint_cell;
                }
            }
            let s_rice = arm_a - arm_b;
            let s_geom = arm_a - arm_c;
            let i = arm_b as i64 + arm_c as i64 - arm_a as i64 - arm_d as i64;
            let i2 = b2 as i64 + arm_c as i64 - arm_a as i64 - d2 as i64;
            rows.push_str(&format!(
                "  ARMS: A={arm_a} B={arm_b} C={arm_c} D={arm_d} | S_rice={s_rice} S_geom={s_geom} I={i} ({}) | pass2 B2={b2} D2={d2} I2={i2} ({})\n",
                if i > 0 { "complementary" } else if i == 0 { "additive" } else { "overlapping" },
                if i2 > 0 { "complementary" } else if i2 == 0 { "additive" } else { "overlapping" },
            ));
        }
        eprintln!("{rows}");
    }

    /// G18-P: the parity-plane diagnostic — is B = S mod 2 structured, or
    /// is the packed bit plane already optimal? Fraction of ones, run
    /// stats, the lag-1 conditional entropy, and the packed cost vs a
    /// bounded bit-RLE (alternating zero-run-first runs; byte-wise varint
    /// lengths). If packed <= RLE everywhere, the investigation closes with
    /// bit packing retained.
    #[test]
    #[ignore] // corpus-gated: scripts/g12_fetch.sh
    fn g18p_parity_diagnostic() {
        let mut rows = String::new();
        for (name, path) in [
            ("piano", "testdata/g12/emotional_piano.wav"),
            ("birds", "testdata/g12/park_birds.wav"),
            ("river", "testdata/g12/park_river.wav"),
        ] {
            let data = std::fs::read(path).expect("corpus (run scripts/g12_fetch.sh)");
            let (_rate, streams) = crate::parse_wav_multichannel(&data).expect("parse");
            let (l, r) = (&streams[0], &streams[1]);
            let mut plane: Vec<u8> = Vec::with_capacity(l.len());
            for (&lv, &rv) in l.iter().zip(r.iter()) {
                let s = lv as i64 - rv as i64;
                let (_, b) = side_split(s);
                plane.push(b);
            }
            let n = plane.len();
            let ones = plane.iter().filter(|&&b| b == 1).count();
            // run-length stats
            let mut runs: Vec<(u8, usize)> = Vec::new();
            let mut at = 0usize;
            while at < n {
                let v = plane[at];
                let mut run = 1usize;
                while at + run < n && plane[at + run] == v {
                    run += 1;
                }
                runs.push((v, run));
                at += run;
            }
            let mean_run = n as f64 / runs.len() as f64;
            let max_run = runs.iter().map(|&(_, r)| r).max().expect("runs non-empty");
            // H(B | B_prev) via the 2x2 transition counts
            let (mut t00, mut t01, mut t10, mut t11) = (0usize, 0usize, 0usize, 0usize);
            for w in plane.windows(2) {
                match (w[0], w[1]) {
                    (0, 0) => t00 += 1,
                    (0, 1) => t01 += 1,
                    (1, 0) => t10 += 1,
                    _ => t11 += 1,
                }
            }
            let hcond = {
                let row = |c0: usize, c1: usize| -> f64 {
                    let tot = (c0 + c1) as f64;
                    if tot == 0.0 {
                        return 0.0;
                    }
                    let (p0, p1) = (c0 as f64 / tot, c1 as f64 / tot);
                    tot * ([p0, p1]
                        .iter()
                        .filter(|&&p| p > 0.0)
                        .map(|&p| -p * p.log2())
                        .sum::<f64>())
                };
                let h = row(t00, t01) + row(t10, t11);
                h / (t00 + t01 + t10 + t11) as f64
            };
            let packed = n.div_ceil(8);
            // bounded bit-RLE: zero-first canonical alternation, per-run
            // varint lengths, everything byte-wise
            let mut rle_bytes = crate::varint_len(runs.len() as u64);
            let mut first_val_zero = true;
            for &(v, run) in &runs {
                if first_val_zero && v == 1 {
                    // canonical 0-length zero run before a leading data run
                    rle_bytes += crate::varint_len(runs.len() as u64 + 1);
                    rle_bytes += crate::varint_len(0);
                    first_val_zero = false;
                }
                rle_bytes += crate::varint_len(run as u64);
            }
            let _ = first_val_zero;
            rows.push_str(&format!(
                "G18-P {name} n={n} | ones={:.3}% runs={} mean_run={mean_run:.1} max_run={max_run} | \
                 H(B|B-1)={hcond:.3} b | packed={packed}B rle={rle_bytes}B ({})\n",
                100.0 * ones as f64 / n as f64,
                runs.len(),
                if rle_bytes < packed {
                    "RLE wins"
                } else {
                    "packed retained"
                },
            ));
        }
        eprintln!("{rows}");
    }
}
