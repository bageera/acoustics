//! G10 — uniform mask representation (research-only; wire untouched).
//!
//! The G9 review's mask-only ablation. The mask streams measured by G9
//! (flat u32 words on PERIOD/OSC families; already-RLE on OP) are re-billed
//! under three arms with the RESIDUAL STREAM untouched:
//! - M0: the shipped fixed-width words (4 B/word) — frozen control
//! - M1: the verified OP zero-run-first RLE convention (v0.7 masks_rle)
//! - M2: adaptive fixed/RLE selection (min of the two + 1 B selector)
//!
//! Decision quantities: ΔL_mask = L_fixed − L_rle; ΔL_complete =
//! L_shipping(M0) − L_candidate(M2) — residual bytes identical, so
//! ΔL_complete = ΔL_mask − selector overhead exactly.
//!
//! Required cases (the review's list): sparse/dense masks, alternating runs,
//! set-word starts, all-zero/all-one, varint boundaries at 127/128 and
//! 16383/16384, cascade per-stage boundaries preserved, exact mask
//! round-trips, M0-fallback on dense/random masks, locked corpus + fixtures.

use crate::masks_rle::write_mask_rle;

/// Cost on the EXISTING RLE primitives: writer's byte length (authoritative —
/// the writer IS the production encoder for OP masks).
#[cfg(test)]
fn rle_bytes(words: &[u32]) -> usize {
    let mut out = Vec::with_capacity(words.len() * 4 + 8);
    write_mask_rle(words, &mut out);
    out.len()
}

/// M1 measured from the REAL writer, not an estimator (the G7-A/G9 lesson).
#[cfg(test)]
fn rle_cost(words: &[u32]) -> usize {
    rle_bytes(words)
}

/// M2: adaptive — the min + 1 byte of selector signaling.
#[cfg(test)]
fn adaptive_cost(words: &[u32]) -> (usize, bool) {
    let fixed = words.len() * 4;
    let rle = rle_cost(words);
    (fixed.min(rle) + 1, rle < fixed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::masks_rle::read_mask_rle as read_rle;

    /// The review's boundary list — exact decode + arm-sanity on each.
    #[test]
    fn g10_mask_rle_boundary_cases() {
        let cases: Vec<(&str, Vec<u32>)> = vec![
            ("all-zero", vec![0u32; 10]),
            ("all-one", vec![u32::MAX; 10]),
            ("sparse", vec![0, 0, 0, 7, 0, 0, 0, 0, 0, 0]),
            ("dense", vec![u32::MAX, 0xFF00_00FF, u32::MAX]),
            ("set-word start", vec![u32::MAX, 0, 0, 0, 0]),
            ("alternating", vec![0, u32::MAX, 0, u32::MAX, 0]),
            ("one word", vec![5]),
            ("127 words", vec![0u32; 127]),
            ("128 words", vec![0u32; 128]),
            ("16383 words", vec![0u32; 16383]),
            ("16384 words", vec![0u32; 16384]),
        ];
        for (name, words) in &cases {
            let mut out = Vec::new();
            write_mask_rle(words, &mut out);
            let mut pos = 0usize;
            let back = read_rle(&out, &mut pos, words.len())
                .unwrap_or_else(|e| panic!("{name}: decode failed: {e}"));
            assert_eq!(&back, words, "{name}: round-trip must be exact");
            assert_eq!(pos, out.len(), "{name}: reader must consume every byte");
            let (cost, used_rle) = adaptive_cost(words);
            assert!(cost <= words.len() * 4 + 1, "{name}: M2 overruns fixed+1");
            // sparse RLE; dense fallback (M2's selector discipline)
            let sparse = !matches!(*name, "dense" | "all-one" | "one word");
            let (_c, rle_picked) = adaptive_cost(words);
            assert_eq!(
                rle_picked, sparse,
                "{name}: RLE must win sparse and fall back on dense"
            );
            let _ = used_rle;
        }
    }

    /// Dense/random masks fall back to M0 (the review's fallback gate).
    #[test]
    fn g10_dense_fallback() {
        // deterministic pseudo-random dense masks
        let mut xr = 0xC0FFEEu64;
        let randoms: Vec<u32> = (0..4096)
            .map(|_| {
                xr ^= xr << 13;
                xr ^= xr >> 7;
                xr ^= xr << 17;
                (xr >> 19) as u32
            })
            .collect();
        let (c, used_rle) = adaptive_cost(&randoms);
        assert!(!used_rle, "random dense masks must fall back to fixed");
        assert_eq!(c, randoms.len() * 4 + 1);
    }

    /// The corpus measurement: ΔL_mask / ΔL_complete on the exact production
    /// mask streams (G9's walker re-used as the extractor), residual bytes
    /// untouched; per-stage segments preserved for cascades.
    #[test]
    fn g10_corpus_mask_arms() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/g5f");
        let mut rows = String::new();
        for name in ["example_word.wav", "can_you_stop.wav", "sample2.wav"] {
            let data = std::fs::read(format!("{dir}/{name}")).expect("corpus");
            let (rate, streams) = crate::parse_wav_multichannel(&data).expect("parse");
            for (ch, y) in streams.iter().enumerate() {
                let model = crate::choose_model(y);
                let segs = crate::g9::extract_mask_segments(model, rate, y);
                let m0: usize = segs.iter().map(|s| s.len() * 4).sum();
                let m1: usize = segs.iter().map(|s| rle_cost(s)).sum();
                let m2: usize = segs.iter().map(|s| adaptive_cost(s).0).sum();
                rows.push_str(&format!(
                    "G10 {name} ch{ch} (kind={} segs={}): M0={m0} M1={m1} M2={m2} ΔL_mask={:+} ΔL_complete={:+}\n",
                    model.kind,
                    segs.len(),
                    m0 as i64 - m1 as i64,
                    m0 as i64 - m2 as i64,
                ));
            }
        }
        // the two-tone cascade + silence (OP: already RLE — M2 must not
        // double-RLE: segs empty/special-cased)
        let two_tone: Vec<i16> = (0..4096usize)
            .map(|i| {
                let t = i as f64 / 44100.0;
                (8000.0 * (2.0 * std::f64::consts::PI * 917.3 * t).cos()
                    + 5000.0 * (2.0 * std::f64::consts::PI * 1361.0 * t).cos())
                .round()
                .clamp(-32767.0, 32767.0) as i16
            })
            .collect();
        let model = crate::choose_model(&two_tone);
        let segs = crate::g9::extract_mask_segments(model, 44100, &two_tone);
        let m0: usize = segs.iter().map(|s| s.len() * 4).sum();
        let m1: usize = segs.iter().map(|s| rle_cost(s)).sum();
        let m2: usize = segs.iter().map(|s| adaptive_cost(s).0).sum();
        rows.push_str(&format!(
            "G10 two-tone (kind={} segs={}): M0={m0} M1={m1} M2={m2} ΔL_complete={:+}\n",
            model.kind,
            segs.len(),
            m0 as i64 - m2 as i64,
        ));
        eprintln!("{rows}");
    }
}
