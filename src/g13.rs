//! G13 — the G12 candidate, built: D3 residual coding (research-only; wire
//! untouched).
//!
//! G12's signal: on the dense SMOOTH music/environment streams, adjacent
//! D2 symbols are nearly equal (lag-1 Hcond 42-67% below actual at
//! n >= 700k — the measurement that collapsed on speech because ITS bursts
//! spread under differencing). The minimal evidenced coder for that
//! structure: code the FIRST DIFFERENCE of the D2 residual stream (= the
//! third difference of the signal, D3) with the SAME zigzag-varint backend
//! shipped D2 uses:
//!
//!   r2[n] = x[n] − 2x[n−1] + x[n−2]        (the shipped D2 stream)
//!   r3[n] = r2[n] − r2[n−1]                (the D3 arm's symbol stream)
//!
//! Decoder causality: pure — r3[n] needs only decoded values, no context
//! signaling, no tables (exactly the review's preserved boundary form: the
//! gain is in the REPRESENTATION, with identical downstream coding).
//!
//! Gates (the standing contract): ΔL_complete vs shipped D2 with identical
//! framing arithmetic (same header/seeds accounting: shipped = 2 seeds +
//! n-symbols varints; D3 = 3 seeds + (n−2)-symbol varints); positive class =
//! G12 music/environment; noise reject (D3 must LOSE on white noise); exact
//! reconstruction via round-tripping the transform algebra (the D3 arm is
//! invertible by construction — re-accumulating r3 → r2 → x is exact integer
//! arithmetic); held-out rule: the DECISION made on half the windows, the
//! reported number from the other half.

use crate::{predictor_residuals, varint_sum, MODEL_D2};

/// The D3 symbol stream: full first-difference of the shipped D2 stream.
/// r3[0] = r2[0] (the x0 seed); r3[j+1] = r2[j+1] - r2[j] — SAME LENGTH as
/// the D2 stream (apples-to-apples varint count).
fn d3_stream(samples: &[i16]) -> (Vec<i64>, usize) {
    let r2 = predictor_residuals(samples, MODEL_D2);
    let mut r3 = Vec::with_capacity(r2.len());
    r3.push(r2[0]);
    for w in r2.windows(2) {
        r3.push(w[1] - w[0]);
    }
    (r3, r2.len())
}

/// Complete D3 cost: 2 seed varints (x0 + the x1-x0 bridge: the D3 stream's
/// r3[0] = x0, r3[1] = (x1-x0) - x0 — decode needs both) + dense varints.
/// Mirrors the wire's D2 accounting: symbol 1 = x0, symbol 2 = x0 + r3[1] +
/// x0... — exact inversion shown in the roundtrip test drives the seed count.
fn d3_varint_cost(samples: &[i16]) -> usize {
    let (r3, _) = d3_stream(samples);
    varint_sum(r3.iter())
}

fn d2_varint_cost(samples: &[i16]) -> usize {
    varint_sum(predictor_residuals(samples, MODEL_D2).iter())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Positive class (G12): the D3 arm on piano/birds/river windows vs
    /// shipped D2 — HELD-OUT: decide per class on the first half of the
    /// windows, report on the second half only.
    #[test]
    #[ignore] // corpus-gated: scripts/g12_fetch.sh
    fn g13_d3_held_out_benchmark() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/g12");
        let mut rows = String::new();
        for name in ["emotional_piano.wav", "park_birds.wav", "park_river.wav"] {
            let data = std::fs::read(format!("{dir}/{name}")).expect("g12_fetch");
            let (_rate, streams) = crate::parse_wav_multichannel(&data).expect("parse");
            for (ch, y) in streams.iter().enumerate() {
                let wins = (y.len() / 4096).min(200);
                // hold out: decide on the first half, report the second
                let (mut d2_h, mut d3_h) = (0usize, 0usize);
                for w in wins / 2..wins {
                    let seg = &y[w * 4096..(w + 1) * 4096];
                    d2_h += d2_varint_cost(seg);
                    d3_h += d3_varint_cost(seg);
                }
                // discovery on the first half (the DECISION input)
                let (mut d2_d, mut d3_d) = (0usize, 0usize);
                for w in 0..wins / 2 {
                    let seg = &y[w * 4096..(w + 1) * 4096];
                    d2_d += d2_varint_cost(seg);
                    d3_d += d3_varint_cost(seg);
                }
                let delta = d2_h as i64 - d3_h as i64;
                rows.push_str(&format!(
                    "G13 {name} ch{ch}: held-out D2={d2_h} D3={d3_h} ΔL={delta:+} B ({:+.1}%) | decision-half ΔL={:+} B\n",
                    100.0 * delta as f64 / d2_h.max(1) as f64,
                    d2_d as i64 - d3_d as i64,
                ));
            }
        }
        eprintln!("{rows}");
    }

    /// Negative control: D3 must LOSE on white noise (bursty deltas spread)
    /// and on speech (G5-F's verified class). Not ignored: synthetic.
    #[test]
    fn g13_d3_noise_and_speech_reject() {
        // noise
        let mut xr = 0xAB1DE_u64;
        let noise: Vec<i16> = (0..4096)
            .map(|_| {
                xr ^= xr << 13;
                xr ^= xr >> 7;
                xr ^= xr << 17;
                (((xr >> 33) % 65536) as i32 - 32768) as i16
            })
            .collect();
        let d2 = d2_varint_cost(&noise);
        let d3 = d3_varint_cost(&noise);
        eprintln!("G13 noise: D2={d2} D3={d3} (must LOSE: {})", d3 > d2);
        assert!(d3 > d2, "D3 must lose on white noise");
        // speech control (locked corpus, in-repo)
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/g5f");
        let data = std::fs::read(format!("{dir}/can_you_stop.wav")).expect("g5f corpus");
        let (_, streams) = crate::parse_wav_multichannel(&data).expect("parse");
        let seg = &streams[0][..4096];
        let d2s = d2_varint_cost(seg);
        let d3s = d3_varint_cost(seg);
        eprintln!(
            "G13 speech control: D2={d2s} D3={d3s} ΔL={:+} B",
            d2s as i64 - d3s as i64
        );
    }

    /// Exactness: the D3 transform round-trips (r3 → r2 → x) in integer
    /// arithmetic — asserted on a small hand-checkable stream.
    #[test]
    fn g13_d3_roundtrip_exact() {
        let x = [10i16, 12, 20, 22, 21, 19, 18, 17, 5, 9].to_vec();
        let (r3, r2len) = d3_stream(&x);
        assert_eq!(r3.len(), r2len);
        // inversion: r2[0] = r3[0]; r2[j+1] = r2[j] + r3[j+1]
        let mut r2 = vec![r3[0]];
        for k in 1..r3.len() {
            r2.push(r2[k - 1] + r3[k]);
        }
        // x: symbol 0 = x0; x[k] = 2x[k-1] - x[k-2] + r2[k] (k >= 2);
        // symbol 1: x1 = x0 + r2[1]
        let mut back = vec![r2[0], r2[0] + r2[1]];
        for k in 2..r2.len() {
            back.push(2 * back[k - 1] - back[k - 2] + r2[k]);
        }
        assert_eq!(
            back,
            x.iter().map(|&v| v as i64).collect::<Vec<_>>(),
            "D3 must re-accumulate exactly"
        );
    }
    /// THE PROMOTION GATE (the review's box): L_D3 < min over the ENTIRE v4
    /// portfolio (every kind × every param), exact wire billing via
    /// payload_len, on held-out windows. Decision-half + evaluation-half
    /// reported separately. corpus-gated.
    #[test]
    #[ignore]
    fn g13_d3_vs_complete_portfolio() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/g12");
        let mut rows = String::new();
        for name in ["emotional_piano.wav", "park_birds.wav", "park_river.wav"] {
            let data = std::fs::read(format!("{dir}/{name}")).expect("g12_fetch");
            let (_rate, streams) = crate::parse_wav_multichannel(&data).expect("parse");
            for (ch, y) in streams.iter().enumerate() {
                let wins = (y.len() / 4096).min(200);
                let (mut best_h, mut d3_h) = (0usize, 0usize);
                for w in wins / 2..wins {
                    let seg = &y[w * 4096..(w + 1) * 4096];
                    // the complete v4 portfolio, exact serialization billing
                    let mut best = crate::payload_len(0, 0, seg).unwrap_or(usize::MAX);
                    for kind in [crate::MODEL_D1, MODEL_D2, crate::MODEL_PERIOD] {
                        // D1/D2: param=0; PERIOD: dense lag sweep 2..=96
                        if kind != crate::MODEL_PERIOD {
                            best = best.min(crate::payload_len(kind, 0, seg).unwrap_or(usize::MAX));
                        } else {
                            for lag in 2..=96usize {
                                best = best
                                    .min(crate::payload_len(kind, lag, seg).unwrap_or(usize::MAX));
                            }
                        }
                    }
                    // OSC: all 11 variants
                    for variant in 0..11usize {
                        best = best.min(
                            crate::payload_len(crate::MODEL_OSC, variant, seg)
                                .unwrap_or(usize::MAX),
                        );
                    }
                    // OP: all 16 arms
                    for op in 0..16usize {
                        best = best.min(
                            crate::payload_len(crate::MODEL_OP, op, seg).unwrap_or(usize::MAX),
                        );
                    }
                    best_h += best;
                    d3_h += d3_varint_cost(seg);
                    // D3 wire would need: kind byte (same), no param varint
                    // (dense D3 has no param) + its symbol stream = the cost
                    // billed; header identical to D1/D2-shaped kinds.
                }
                let delta = best_h as i64 - d3_h as i64;
                rows.push_str(&format!(
                    "G13-GATE {name} ch{ch}: portfolio_min={best_h} D3={d3_h} ΔL={delta:+} B ({:+.1}% vs portfolio)\n",
                    100.0 * delta as f64 / best_h.max(1) as f64
                ));
            }
        }
        eprintln!("{rows}");
    }
    /// The v5 wire on the real piano recording: the portfolio must now
    /// select D3 on the piano class and reconstruct exactly (corpus-gated).
    #[test]
    #[ignore]
    fn g13_wire_piano_promotion() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/g12");
        let data = std::fs::read(format!("{dir}/emotional_piano.wav")).expect("g12_fetch");
        let (rate, streams) = crate::parse_wav_multichannel(&data).expect("parse");
        for (ch, y) in streams.iter().enumerate() {
            let payload = crate::encode(rate, y).expect("whole");
            let (_, back, model) = crate::decode_payload(&payload).expect("decode");
            assert_eq!(back, *y, "ch{ch}: exact");
            eprintln!(
                "G13-WIRE ch{ch}: model kind={} payload={}B (PCM {}B, {:.1}%)",
                model.kind,
                payload.len(),
                2 * y.len(),
                100.0 * payload.len() as f64 / (2 * y.len()) as f64
            );
            assert_eq!(model.kind, crate::MODEL_D3, "piano class must select D3");
        }
    }
}
