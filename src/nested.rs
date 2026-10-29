//! G2 — compositional echo (research-only, no released wire format).
//!
//! The v0.7 flat echo arm bills its inverse intermediate `y` as RAW zigzag
//! varints. G2 asks one question: is a COMPLETE description
//! `count, g, D, inner(y), corrections(r)` smaller than both the flat echo
//! and the best standing portfolio model for the same signal?
//!
//! Architecture notes (per the v0.7 release review):
//! - Single inner model; no recursive model trees.
//! - The inner model is a member of the EXISTING portfolio (RAW incl. its
//!   literal submode, D1/D2, PERIOD lags, LOOP arms, OSC variants) — same
//!   selection machinery, same exact-decode contract, same cost accounting.
//! - The published intermediate `y` is the SAME i16-clamped stream the flat
//!   arm publishes (no silent widening or re-clamping to make nesting easier).
//!   The unclamped inverse exists only inside the recursion (decoder-replayable).
//! - The review's `q` (inner corrections) is structurally zero here: the
//!   portfolio's inner models are lossless by their own tested invariant, so
//!   the container carries `r` (echo-forward corrections on `x`) alone. That
//!   is a conscious simplification of the review's notation — recorded here,
//!   not hidden: it means G2 exactly measures "does the portfolio describe y
//!   more cheaply than flat echo's RAW billing".
//! - The container is NOT a released payload format: no MAGIC, decodable only
//!   by this module, existing encode()/VERSION behavior untouched (v0.7 frozen).

use crate::masks_rle::{mask_rle_wire_len, read_mask_rle, write_mask_rle};
use crate::{
    decode, i16_from_i64, payload_len, plan_corrections_masked_op, read_varint, serialize,
    varint_len, write_varint, zigzag_decode, zigzag_encode, Model, MODEL_D1, MODEL_D2, MODEL_OP,
    MODEL_OSC, MODEL_PERIOD, MODEL_RAW, OP_GAINS, OSC_VARIANT_COUNT,
};

/// One nested candidate: gain id, delay, inner model, inner cost.
/// `published_y` is the i16 stream the inner encodes (in-memory only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NestedEchoPlan {
    pub cost: usize,
    pub gsel: usize,
    pub delay: usize,
    pub inner: Model,
    pub inner_len: usize,
    pub masks: Vec<u32>,
    pub deltas: Vec<i64>,
    pub published_y: Vec<i16>,
}

/// Exact causal echo inverse (the UNclamped recursion; decode-replayable):
/// y[n] = x[n] − ((y[n−D]·g) >> 15), with the head term zero for n < D.
pub(crate) fn echo_inverse(samples: &[i16], g: i64, d_run: usize) -> Option<Vec<i64>> {
    let n = samples.len();
    if d_run == 0 || d_run >= n {
        return None;
    }
    let x: Vec<i64> = samples.iter().map(|&s| s as i64).collect();
    let mut y = vec![0i64; n];
    for i in 0..n {
        let e = if i >= d_run {
            (y[i - d_run] * g) >> 15
        } else {
            0
        };
        y[i] = x[i] - e;
    }
    Some(y)
}

/// Echo-forward replay from a PUBLISHED i16 intermediate — the exact arithmetic
/// decode_op's echo branch uses (i64 multiply, no saturating narrowing).
pub(crate) fn echo_forward(y: &[i16], g: i64, d_run: usize) -> Vec<i64> {
    y.iter()
        .enumerate()
        .map(|(i, &yi)| {
            let e = if i >= d_run {
                (y[i - d_run] as i64 * g) >> 15
            } else {
                0
            };
            yi as i64 + e
        })
        .collect()
}

/// Inner candidate list (cheap first; OSC variants last — the priciest planner).
/// LOOP arms only (0..10): arms 10..15 replay echo/scale transforms — nesting
/// an echo inside a nested echo is the recursive case the review excluded.
fn inner_candidates() -> Vec<Model> {
    let mut v: Vec<Model> = vec![
        Model {
            kind: MODEL_RAW,
            param: 0,
        },
        Model {
            kind: MODEL_D1,
            param: 0,
        },
        Model {
            kind: MODEL_D2,
            param: 0,
        },
    ];
    for lag in 2..=96usize {
        v.push(Model {
            kind: MODEL_PERIOD,
            param: lag,
        });
    }
    for op in 0..10usize {
        v.push(Model {
            kind: MODEL_OP,
            param: op,
        });
    }
    for osc in 0..OSC_VARIANT_COUNT {
        v.push(Model {
            kind: MODEL_OSC,
            param: osc,
        });
    }
    v
}

/// Best nested-echo container for `samples`: sweep gains × delays; for each,
/// pick the smallest complete inner payload through the standing portfolio.
pub fn nested_echo_plan(samples: &[i16]) -> Option<NestedEchoPlan> {
    let n = samples.len();
    if n < 4 {
        return None;
    }
    let x: Vec<i64> = samples.iter().map(|&s| s as i64).collect();
    let mut best: Option<NestedEchoPlan> = None;
    let inners = inner_candidates();
    for (gsel, &g) in OP_GAINS.iter().enumerate() {
        for d_run in 1..=64usize {
            if d_run >= n {
                break;
            }
            let y_wide = echo_inverse(samples, g, d_run)?;
            // the PUBLISHED intermediate: same i16 semantics as the flat arm
            let y: Vec<i16> = y_wide
                .iter()
                .map(|&v| v.clamp(i16::MIN as i64, i16::MAX as i64) as i16)
                .collect();
            let mut inner_best: Option<(usize, Model)> = None;
            for cand in &inners {
                let Some(cost) = payload_len(cand.kind, cand.param, &y) else {
                    continue;
                };
                if inner_best.as_ref().is_none_or(|(bc, _)| cost < *bc) {
                    inner_best = Some((cost, *cand));
                }
            }
            let Some((inner_len, inner)) = inner_best else {
                continue;
            };
            let xhat = echo_forward(&y, g, d_run);
            let r: Vec<i64> = (0..n).map(|i| x[i] - xhat[i]).collect();
            let (masks, deltas) = plan_corrections_masked_op(&r);
            // headerless container: frame = varint(count) + gain + delay +
            // varint(inner_len) + inner bytes + rle masks + coded r.
            let container =
                varint_len(n as u64) + 2 * varint_len(0) + varint_len(inner_len.max(1) as u64);
            let cost = container
                + inner_len
                + mask_rle_wire_len(&masks)
                + deltas
                    .iter()
                    .map(|&d| varint_len(zigzag_encode(d)))
                    .sum::<usize>();
            if best.as_ref().is_none_or(|b| cost < b.cost) {
                best = Some(NestedEchoPlan {
                    cost,
                    gsel,
                    delay: d_run,
                    inner,
                    inner_len,
                    masks,
                    deltas,
                    published_y: y,
                });
            }
        }
    }
    best
}

/// Serialize the research container:
/// varint(count) varint(gsel) varint(D) varint(inner_len) inner[..inner_len]
/// mask_rle(count words) zigzag-varint(coded r at mask positions).
pub fn serialize_nested(
    plan: &NestedEchoPlan,
    sample_rate: u32,
    samples: &[i16],
) -> Result<Vec<u8>, String> {
    // exactness guard: the plan must be current for this input
    let recomputed = nested_echo_plan(samples).ok_or("nested plan N/A")?;
    if recomputed.cost != plan.cost
        || recomputed.gsel != plan.gsel
        || recomputed.delay != plan.delay
        || recomputed.inner != plan.inner
    {
        return Err("nested plan not current".into());
    }
    // the inner encodes the PUBLISHED INTERMEDIATE, not the echo-laden signal
    let inner = serialize(plan.inner, sample_rate, &plan.published_y)?;
    if inner.len() != plan.inner_len {
        return Err(format!(
            "nested inner: billed {} but serialized {}",
            plan.inner_len,
            inner.len()
        ));
    }
    let mut out = Vec::with_capacity(plan.cost.max(64));
    write_varint(&mut out, samples.len() as u64);
    write_varint(&mut out, plan.gsel as u64);
    write_varint(&mut out, plan.delay as u64);
    write_varint(&mut out, inner.len() as u64);
    out.extend_from_slice(&inner);
    write_mask_rle(&plan.masks, &mut out);
    for &d in &plan.deltas {
        write_varint(&mut out, zigzag_encode(d));
    }
    Ok(out)
}

/// Decode the research container; returns (rate, samples).
pub fn decode_nested(payload: &[u8]) -> Result<(u32, Vec<i16>, NestedEchoPlan), String> {
    let mut pos = 0usize;
    let count = read_varint(payload, &mut pos)? as usize;
    if count == 0 {
        return Err(crate::ERR_EMPTY.into());
    }
    let gsel = read_varint(payload, &mut pos)? as usize;
    if gsel >= OP_GAINS.len() {
        return Err("invalid nested echo gain".into());
    }
    let d_run = read_varint(payload, &mut pos)? as usize;
    let inner_len = read_varint(payload, &mut pos)? as usize;
    let inner_end = pos.checked_add(inner_len).ok_or(crate::ERR_TRUNCATED)?;
    let inner = payload.get(pos..inner_end).ok_or(crate::ERR_TRUNCATED)?;
    pos = inner_end;
    let (rate, z, inner_model) = decode(inner).map_err(|e| format!("nested inner: {e}"))?;
    if z.len() != count {
        return Err("nested inner sample count mismatch".into());
    }
    // the portfolio's contract: inner decode is exact, so z IS the published y.
    let g = OP_GAINS[gsel];
    let xhat = echo_forward(&z, g, d_run);
    let masks = read_mask_rle(payload, &mut pos, count.div_ceil(32))?;
    let mut samples = Vec::with_capacity(count);
    for i in 0..count {
        let d = if masks[i / 32] & (1 << (i % 32)) != 0 {
            zigzag_decode(read_varint(payload, &mut pos)?)
        } else {
            0
        };
        samples.push(i16_from_i64(xhat[i] + d)?);
    }
    let plan = NestedEchoPlan {
        cost: pos, // bytes consumed == honest container length
        gsel,
        delay: d_run,
        inner: inner_model,
        inner_len,
        masks,
        deltas: Vec::new(), // deltas live in the wire, not reconstructed here
        published_y: z,
    };
    Ok((rate, samples, plan))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OSC_Q15;

    const RATE: u32 = 44100;

    fn clamp_i16(v: i64) -> i16 {
        v.clamp(i16::MIN as i64, i16::MAX as i64) as i16
    }

    /// The three-candidate comparison the G2 gate demands:
    /// (flat echo, nested, portfolio best).
    fn controls(samples: &[i16]) -> (usize, Option<usize>, usize) {
        let flat = (10..=13)
            .filter_map(|op| payload_len(MODEL_OP, op, samples))
            .min()
            .expect("flat echo applicable");
        let nested = nested_echo_plan(samples).map(|p| p.cost);
        let portfolio = crate::encode(RATE, samples).expect("encode").len();
        (flat, nested, portfolio)
    }

    /// Exactness + accounting gate shared by every acceptance test: serialize
    /// the plan, check billed size == wire size, decode, check equality.
    fn roundtrip_exact(samples: &[i16]) -> (usize, usize, String) {
        let plan = nested_echo_plan(samples).expect("plan");
        let wire = serialize_nested(&plan, RATE, samples).expect("ser");
        assert_eq!(
            wire.len(),
            plan.cost,
            "container: metric must equal serialize"
        );
        let (_, back, _) = decode_nested(&wire).expect("dec");
        assert_eq!(back, samples, "container: exact reconstruction");
        (
            plan.cost,
            plan.delay,
            format!("kind{}/param{}", plan.inner.kind, plan.inner.param),
        )
    }

    /// G2-A: echo of an exact integer recurrence — does the inner portfolio
    /// describe y more cheaply than flat echo's RAW varints?
    #[test]
    fn g2_a_d2_inner_on_recurrence_echo() {
        let mut b = vec![3000i64, -1000i64];
        for i in 2..1200 {
            let (p, q) = (b[i - 1], b[i - 2]);
            b.push((p * 56100 - q * 29000).div_euclid(OSC_Q15));
        }
        let g = OP_GAINS[1];
        let x: Vec<i16> = (0..1200usize)
            .map(|i| {
                let e = if i >= 16 { (b[i - 16] * g) >> 15 } else { 0 };
                clamp_i16(b[i] + e)
            })
            .collect();
        let (flat, nested, portfolio) = controls(&x);
        let (cost, d, inner) = roundtrip_exact(&x);
        eprintln!("G2-A flat={flat} nested={nested:?} wire={cost} portfolio={portfolio} D={d} inner={inner}");
        assert!(
            cost < flat.min(portfolio),
            "G2-A: nested must beat both controls: nested {cost} vs flat {flat} portfolio {portfolio}"
        );
    }

    /// G2-B: echo of an exact periodic lattice — LOOP inner.
    #[test]
    fn g2_b_loop_inner_on_lattice_echo() {
        let base: Vec<i16> = (0..64usize)
            .map(|i| if (i / 32) % 2 == 0 { 10000 } else { -10000 })
            .collect();
        let g = OP_GAINS[2];
        let x: Vec<i16> = (0..1200usize)
            .map(|i| {
                let b = base[i % 64] as i64;
                let e = if i >= 24 { (b * g) >> 15 } else { 0 };
                clamp_i16(b + e)
            })
            .collect();
        let (flat, nested, portfolio) = controls(&x);
        let (cost, d, inner) = roundtrip_exact(&x);
        eprintln!("G2-B flat={flat} nested={nested:?} wire={cost} portfolio={portfolio} D={d} inner={inner}");
        assert!(
            cost < flat.min(portfolio),
            "G2-B: nested must beat both controls: nested {cost} vs flat {flat} portfolio {portfolio}"
        );
    }

    /// G2-C: echo of an INDEPENDENT float-generated damped tone — OSC inner.
    #[test]
    fn g2_c_osc_inner_on_float_damped_echo() {
        let base_f: Vec<f64> = (0..4096usize)
            .map(|i| {
                let t = i as f64 / 44100.0;
                12000.0
                    * (-18400.0 * t).exp()
                    * (2.0 * std::f64::consts::PI * 917.3 * t + 0.37).cos()
            })
            .collect();
        let g = OP_GAINS[1];
        let x: Vec<i16> = (0..4096usize)
            .map(|i| {
                let e = if i >= 32 {
                    (base_f[i - 32] * g as f64 / OSC_Q15 as f64).round() as i64
                } else {
                    0
                };
                clamp_i16(base_f[i].round() as i64 + e)
            })
            .collect();
        let (flat, nested, portfolio) = controls(&x);
        let (cost, d, inner) = roundtrip_exact(&x);
        eprintln!("G2-C flat={flat} nested={nested:?} wire={cost} portfolio={portfolio} D={d} inner={inner}");
        assert!(
            cost < flat.min(portfolio),
            "G2-C: nested must beat both controls: nested {cost} vs flat {flat} portfolio {portfolio}"
        );
    }

    /// G2-D: echo of speech-like audio — exploratory (no win gate); record
    /// the comparison, require exactness + honest accounting.
    #[test]
    fn g2_d_speech_independent_base() {
        let base: Vec<i16> = (0..3000usize)
            .map(|i| {
                let t = i as f64 / 44100.0;
                let env = (6.0 * t).sin().abs().max(0.1);
                let v = env
                    * (3000.0 * (2.0 * std::f64::consts::PI * 120.0 * t).sin()
                        + 2000.0 * (2.0 * std::f64::consts::PI * 700.0 * t).sin()
                        + 1500.0 * (2.0 * std::f64::consts::PI * 1220.0 * t).sin());
                (v + (i * 2654435761 % 97) as f64 / 8.0) as i16
            })
            .collect();
        let g = OP_GAINS[1];
        let x: Vec<i16> = (0..3000usize)
            .map(|i| {
                let e = if i >= 40 {
                    (base[i - 40] as i64 * g) >> 15
                } else {
                    0
                };
                clamp_i16(base[i] as i64 + e)
            })
            .collect();
        let (flat, nested, portfolio) = controls(&x);
        let (cost, d, inner) = roundtrip_exact(&x);
        eprintln!("G2-D flat={flat} nested={nested:?} wire={cost} portfolio={portfolio} D={d} inner={inner}");
    }

    /// G2-E: echo of random noise — MDL must REJECT unhelpful nesting: the
    /// complete nested candidate may not beat the portfolio (RAW literal) or
    /// flat echo.
    #[test]
    fn g2_e_rejects_nesting_on_noise() {
        let mut xr: u64 = 0x243F6A8885A308D3;
        let x: Vec<i16> = (0..4096usize)
            .map(|_| {
                xr ^= xr << 13;
                xr ^= xr >> 7;
                xr ^= xr << 17;
                (xr & 0xFFFF) as u16 as i16
            })
            .collect();
        let (flat, nested, portfolio) = controls(&x);
        eprintln!("G2-E flat={flat} nested={nested:?} portfolio={portfolio}");
        let n = nested.expect("nested plan exists on noise");
        assert!(
            n >= portfolio.min(flat),
            "G2-E: nested must LOSE on plain noise: nested {n} vs flat {flat} portfolio {portfolio}"
        );
    }

    /// G2-F: echo with i16 saturation — the inverse recursion spills past the
    /// published i16 range mid-stream; nesting must publish the clamped y and
    /// still decode exactly.
    #[test]
    fn g2_f_wide_intermediate_saturation_exact() {
        let mut y_real = vec![0i64; 1200];
        for i in 1..1200 {
            y_real[i] = (y_real[i - 1] * 28672) >> 15;
        }
        y_real[10] = 40000;
        y_real[600] = -40000;
        let g = OP_GAINS[3];
        let x: Vec<i16> = (0..1200usize)
            .map(|i| {
                let e = if i >= 8 { (y_real[i - 8] * g) >> 15 } else { 0 };
                clamp_i16(y_real[i] + e)
            })
            .collect();
        let (cost, d, inner) = roundtrip_exact(&x);
        let (flat, nested, portfolio) = controls(&x);
        eprintln!("G2-F flat={flat} nested={nested:?} wire={cost} portfolio={portfolio} D={d} inner={inner}");
    }

    /// Container invariant across ALL (gsel, D) with a FIXED inner (RAW):
    /// billed size == wire size, exact decode — the per-variant coverage
    /// discipline that caught v0.7's accounting drift.
    #[test]
    fn g2_container_invariant_all_gains_delays() {
        let samples: Vec<i16> = (0..240usize)
            .map(|i| {
                if i % 61 < 56 {
                    ((i % 13) * 900) as i16
                } else {
                    let x = (i * 811) % 65536;
                    (x as i32 - 32768) as i16
                }
            })
            .collect();
        let x64: Vec<i64> = samples.iter().map(|&s| s as i64).collect();
        for (gsel, &g) in OP_GAINS.iter().enumerate() {
            for d_run in 1..=64usize {
                if d_run >= 240 {
                    break;
                }
                let y_wide = echo_inverse(&samples, g, d_run).expect("inv");
                let y: Vec<i16> = y_wide
                    .iter()
                    .map(|&v| v.clamp(i16::MIN as i64, i16::MAX as i64) as i16)
                    .collect();
                let inner_model = Model {
                    kind: MODEL_RAW,
                    param: 0,
                };
                let inner_len = payload_len(MODEL_RAW, 0, &y).expect("raw");
                let xhat = echo_forward(&y, g, d_run);
                let r: Vec<i64> = (0..240).map(|i| x64[i] - xhat[i]).collect();
                let (masks, deltas) = plan_corrections_masked_op(&r);
                let container =
                    varint_len(240u64) + 2 * varint_len(0) + varint_len(inner_len.max(1) as u64);
                let cost = container
                    + inner_len
                    + mask_rle_wire_len(&masks)
                    + deltas
                        .iter()
                        .map(|&d| varint_len(zigzag_encode(d)))
                        .sum::<usize>();
                let inner_ser = serialize(inner_model, RATE, &y).expect("iser");
                assert_eq!(
                    inner_ser.len(),
                    inner_len,
                    "g{gsel} d{d_run}: inner billed == wire"
                );
                let mut out = Vec::new();
                write_varint(&mut out, 240);
                write_varint(&mut out, gsel as u64);
                write_varint(&mut out, d_run as u64);
                write_varint(&mut out, inner_len as u64);
                out.extend_from_slice(&inner_ser);
                write_mask_rle(&masks, &mut out);
                for &d in &deltas {
                    write_varint(&mut out, zigzag_encode(d));
                }
                assert_eq!(out.len(), cost, "g{gsel} d{d_run}: metric == serialize");
                let (_, back, _) = decode_nested(&out).expect("dec");
                assert_eq!(back, samples, "g{gsel} d{d_run}: exact reconstruction");
            }
        }
    }
}
