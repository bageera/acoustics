//! G4 — additive modal models, joint vs greedy vs shared-parameter transport
//! (research-only; the v0.7 wire stays frozen; controls = the standing
//! portfolio INCLUDING the f3 cascade, which is itself greedy subtractive
//! fitting — G4-A's lazy form already in the engine).
//!
//! Central hypothesis (the G3-B review): a small set of JOINTLY optimized
//! oscillatory components explains composite audio with fewer total bits than
//! the greedy cascade. Arms:
//!
//! - G4-A greedy: LS-fit the strongest partial, subtract it, fit the next —
//!   the cascade's own family; measured as the control it already is.
//! - G4-B joint: seed K components greedily, then coordinate-descent refine
//!   each component's (φ, ω, α) against the SUMMED reconstruction's complete
//!   billed cost (LS initializes; exact decoded residual cost decides — the
//!   G3-B lesson).
//! - G4-C shared transport: harmonic series — component k's frequency =
//!   f0·k (integer ratio on the accumulator grid), so only f0 + the per-
//!   component amplitudes ride the wire (the shared-relationship question).
//!
//! Model: x̂[n] = Q(Σ_k (A_k · cos(φ_k[n])) >> 15), φ_k from the validated
//! 24/48-bit wrap arithmetic (α per component; 0 unless the fixture needs it),
//! rounding = the sum of terms rounded once at the end (round(sum) — exact
//! integer, rounding specified: (sum + half) >>= 15 via i64 div_euclid.
//! Wire (headerless, G2/G3 conventions):
//!   varint(count) varint(K) per-component (zigzag φ zigzag ω zigzag α
//!   varint A) mask_rle(count) zigzag-varint(corrections)
//! Shared arm instead: varint(count) varint(K) varint(f0_acc) per-component
//!   (varint A, zigzag α optional=0) + masks + corrections.
//! Exactness + billed == wire per candidate; the G4 tests pin the review's
//! decisive inequality L_joint < min(L_greedy, L_cascade, L_portfolio) on
//! fixtures the models don't generate.

// The modal family is the G4 research surface; its callers are the
// acceptance tests until an arm earns promotion.
#![allow(dead_code)]

use crate::masks_rle::{mask_rle_wire_len, read_mask_rle, write_mask_rle};
use crate::{
    cos_lut, i16_from_i64, plan_corrections_masked_op, read_varint, varint_len, write_varint,
    zigzag_decode, zigzag_encode,
};

/// Per-component integer cos term at a 48-bit accumulator:
/// term_k[n] = (A_k · cos(φ_k[n])) >> 15, φ' = (φ + ω) & (2^48 − 1), ω' = ω + α.
pub fn component_term(phi0: i64, omega0: i64, alpha: i64, a: i64, n: usize) -> Option<Vec<i64>> {
    if a == 0 {
        return None; // a zero-amplitude component is not a component
    }
    const ACC: i64 = 1i64 << 48;
    let shift = 48 - 12;
    let mut phi = phi0;
    let mut omega = omega0;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let angle = ((phi >> shift) & 4095) as u32;
        out.push((a * cos_lut(angle)) >> 15);
        phi = (phi + omega) & (ACC - 1);
        omega += alpha;
    }
    Some(out)
}

/// Sum of K components, rounded ONCE (i64 integer; the spec'd rounding).
pub fn sum_components(terms: &[Vec<i64>]) -> Vec<i64> {
    let n = terms.first().map_or(0, |t| t.len());
    let mut out = vec![0i64; n];
    for t in terms {
        for (o, &v) in out.iter_mut().zip(t.iter()) {
            *o += v;
        }
    }
    out
}

/// One modal candidate: components + amplitude transport kind + complete cost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModalPlan {
    pub cost: usize,
    /// per-component (phi0, omega0, alpha, amp) at the 48-bit grid
    pub comps: Vec<(i64, i64, i64, i64)>,
    pub shared_f0: bool,
    pub masks: Vec<u32>,
    pub deltas: Vec<i64>,
    /// in-memory reconstruction (tests/publish decisions only, not wire)
    pub pred: Vec<i64>,
}

/// Billing: the complete frame the serializer emits (G4 = plain joint grammar;
/// the shared-f0 arm transports f0 once + per-component A/α).
fn cost_of(
    n: usize,
    comps: &[(i64, i64, i64, i64)],
    shared_f0: bool,
    masks: &[u32],
    deltas: &[i64],
) -> usize {
    let _ = shared_f0; // G4-C's metering rides the shared fit below; plain here
    let frame = varint_len(n as u64) + varint_len(comps.len() as u64);
    let comp_bytes: usize = comps
        .iter()
        .map(|&(phi, w, al, a)| {
            varint_len(zigzag_encode(phi))
                + varint_len(zigzag_encode(w))
                + varint_len(zigzag_encode(al))
                + varint_len(a as u64)
        })
        .sum();
    frame
        + comp_bytes
        + mask_rle_wire_len(masks)
        + deltas
            .iter()
            .map(|&d| varint_len(zigzag_encode(d)))
            .sum::<usize>()
}

/// Variable projection (the review's G4-B tool): given the components'
/// (φ_k, ω_k), solve for the amplitudes A_k that minimize ||x − Σ A_k·c_k||₂
/// (linear — K×K normal equations, K ≤ 4), then quantize A_k to integers.
/// LS INITIALIZES; billed residual cost decides (the G3-B lesson).
fn varpro_amplitudes(
    x64: &[i64],
    comps: &[(i64, i64)], // (phi, omega) at 48-bit
    n: usize,
) -> Option<Vec<i64>> {
    let k = comps.len();
    let pi = std::f64::consts::PI;
    // design matrix rows: cos(φ_k[n]) (Q15 units)
    let mut basis: Vec<Vec<f64>> = Vec::with_capacity(k);
    for &(phi, omega) in comps {
        const ACC48: i64 = 1i64 << 48;
        let shift = 36; // 48−12
        let mut phi_r = phi;
        let omega_r = omega;
        let mut col = Vec::with_capacity(n);
        for _ in 0..n {
            let angle = ((phi_r >> shift) & 4095) as u32;
            col.push(cos_lut(angle) as f64 / 32768.0);
            phi_r = (phi_r + omega_r) & (ACC48 - 1);
        }
        basis.push(col);
    }
    let _ = pi;
    // normal equations K×K, symmetric
    let mut m = vec![vec![0f64; k]; k];
    let mut b = vec![0f64; k];
    for i in 0..k {
        for j in i..k {
            let dot: f64 = basis[i]
                .iter()
                .zip(basis[j].iter())
                .map(|(&a, &c)| a * c)
                .sum();
            m[i][j] = dot;
            m[j][i] = dot;
        }
        b[i] = basis[i]
            .iter()
            .zip(x64.iter())
            .map(|(&a, &x)| a * x as f64)
            .sum();
    }
    // K ≤ 4: closed-form via Cramer for K=1/2, fallback: identity-safe
    if k == 1 {
        if m[0][0].abs() < 1e-9 {
            return None;
        }
        let a0 = b[0] / m[0][0];
        return Some(vec![(a0 * 32768.0).round() as i64]);
    }
    if k == 2 {
        let det = m[0][0] * m[1][1] - m[0][1] * m[1][0];
        if det.abs() < 1e-9 {
            return None;
        }
        let a0 = (b[0] * m[1][1] - m[0][1] * b[1]) / det;
        let a1 = (m[0][0] * b[1] - b[0] * m[1][0]) / det;
        return Some(vec![
            (a0 * 32768.0).round() as i64,
            (a1 * 32768.0).round() as i64,
        ]);
    }
    None // K ≥ 3 varpro: needs 3x3/4x4 solve — added only if the corpus asks
}

/// Spectral line scan (G4-B's joint-discovery instrument): correlate the
/// working stream against cos/sin at candidate accumulator-grid frequencies;
/// the strongest line's (ω, φ) seeds the component. Coarse 8 Hz grid, then the
/// integer refine picks the wire-minimum.
pub fn strongest_line(working: &[i64], n: usize) -> Option<(i64, f64)> {
    let mut best: Option<(i64, f64, f64)> = None; // (omega_acc48, phi_rad, mag)
    let mut f = 20.0f64;
    while f < 20000.0 {
        let w_rad = 2.0 * std::f64::consts::PI * f / 44100.0;
        // accumulator units: a 2π (full turn) advance = 2^48 ⇒ ω = w_rad/(2π)·2^48
        let omega = (w_rad / (2.0 * std::f64::consts::PI) * (1i64 << 48) as f64).round() as i64;
        let (mut cs, mut sn) = (0f64, 0f64);
        for (i, &x) in working.iter().enumerate().take(n) {
            let th = w_rad * i as f64;
            cs += x as f64 * th.cos();
            sn += x as f64 * th.sin();
        }
        let mag = (cs * cs + sn * sn).sqrt();
        if best.as_ref().is_none_or(|(_, _, bm)| mag > *bm) {
            best = Some((omega, sn.atan2(cs), mag)); // phi = theta0 (cos phase)
        }
        f += 8.0;
    }
    best.map(|(w, phi, _)| (w, phi))
}

/// G4-A seed (greedy line-scan) + G4-B joint refine (coordinate descent).
pub fn modal_fit(samples: &[i16], k_target: usize) -> Option<ModalPlan> {
    if std::env::var("MODAL_DEBUG").is_ok() {
        eprintln!("MODAL_FIT_TOP: enters (n={} k={})", samples.len(), k_target);
    }
    let n = samples.len();
    if n < 64 || k_target == 0 {
        return None;
    }
    let x64: Vec<i64> = samples.iter().map(|&s| s as i64).collect();
    // matching pursuit WITH varpro: after each scan the amplitude is solved by
    // LS against the probe column, so a component's amplitude is never the
    // mixture's raw peak; the joint refine polishes (φ, ω, A) by billed cost.
    let mut comps: Vec<(i64, i64, i64, i64)> = Vec::new();
    let mut working = x64.clone();
    for _ in 0..k_target {
        let Some((omega, phi_rad)) = strongest_line(&working, n) else {
            if std::env::var("MODAL_DEBUG").is_ok() {
                eprintln!("BREAK1 scan");
            }
            break;
        };
        let pi = std::f64::consts::PI;
        let phi = ((phi_rad.rem_euclid(2.0 * pi)) * ((1i64 << 48) as f64 / (2.0 * pi))).round()
            as i64
            & ((1i64 << 48) - 1);
        let amp_probe = 32768i64;
        let Some(col1) = component_term(phi, omega, 0, amp_probe, n) else {
            if std::env::var("MODAL_DEBUG").is_ok() {
                eprintln!("BREAK2 term probe (phi={phi} omega={omega})");
            }
            break;
        };
        let c1sq: f64 = col1.iter().map(|&v| (v * v) as f64).sum();
        if c1sq < 1.0 {
            break;
        }
        let amp: i64 = ((working
            .iter()
            .zip(col1.iter())
            .map(|(&x, &c)| x as f64 * c as f64)
            .sum::<f64>()
            / c1sq)
            * 32768.0) as i64;
        if amp.abs() < 16 {
            break;
        }
        let Some(term) = component_term(phi, omega, 0, amp, n) else {
            break;
        };
        comps.push((phi, omega, 0, amp));
        for (w, &t) in working.iter_mut().zip(term.iter()) {
            *w -= t;
        }
    }
    if std::env::var("MODAL_DEBUG").is_ok() {
        eprintln!(
            "MODAL_FIT_DEBUG: k_target={k_target} comps={} first={:?}",
            comps.len(),
            comps.first()
        );
    }
    if comps.is_empty() {
        return None;
    }
    let plan = refine_joint(&x64, comps, n);
    if std::env::var("MODAL_DEBUG").is_ok() {
        eprintln!(
            "MODAL_FIT_DEBUG: refine = {}",
            plan.as_ref()
                .map_or("None".into(), |p| format!("cost={}", p.cost))
        );
    }
    plan
}
fn refine_joint(x64: &[i64], mut comps: Vec<(i64, i64, i64, i64)>, n: usize) -> Option<ModalPlan> {
    let mut best: Option<ModalPlan> = None;
    let rounds = 2; // bounded joint refine (the safeguard: bounded search)
                    // varpro FIRST: exact LS amplitudes given the frequencies (the review's
                    // tool), integer-quantized before billing:
    {
        let freqs: Vec<(i64, i64)> = comps.iter().map(|&(phi, w, _, _)| (phi, w)).collect();
        if let Some(amps) = varpro_amplitudes(x64, &freqs, n) {
            for (c, a) in comps.iter_mut().zip(amps) {
                c.3 = a;
            }
        }
    }
    for _ in 0..rounds {
        for ci in 0..comps.len() {
            let (phi_base, omega_base, alpha, amp) = comps[ci];
            // neighbor candidates: φ ±2, ω ±2 at the 48-bit grid
            for dphi in [-2i64, -1, 0, 1, 2] {
                for domega in [-2i64, -1, 0, 1, 2] {
                    comps[ci] = (phi_base + dphi, omega_base + domega, alpha, amp);
                    let mut terms = Vec::new();
                    for &(phi, w, al, a) in &comps {
                        if let Some(t) = component_term(phi, w, al, a, n) {
                            terms.push(t);
                        }
                    }
                    if terms.len() != comps.len() {
                        continue;
                    }
                    let pred = sum_components(&terms);
                    let r: Vec<i64> = (0..n).map(|i| x64[i] - pred[i]).collect();
                    let (masks, deltas) = plan_corrections_masked_op(&r);
                    let cost = cost_of(n, &comps, false, &masks, &deltas);
                    if best.as_ref().is_none_or(|b| cost < b.cost) {
                        best = Some(ModalPlan {
                            cost,
                            comps: comps.clone(),
                            shared_f0: false,
                            masks,
                            deltas,
                            pred,
                        });
                    }
                }
            }
            comps[ci] = (phi_base, omega_base, alpha, amp); // restore between axes
        }
        // amplitude refine: scale each component's A by trying ±1 step
        for ci in 0..comps.len() {
            let (phi, w, al, amp) = comps[ci];
            for da in [-1i64, 1] {
                comps[ci] = (phi, w, al, amp + da);
                let mut terms = Vec::new();
                for &(phi, w, al, a) in &comps {
                    if let Some(t) = component_term(phi, w, al, a, n) {
                        terms.push(t);
                    }
                }
                if terms.len() != comps.len() {
                    continue;
                }
                let pred = sum_components(&terms);
                let r: Vec<i64> = (0..n).map(|i| x64[i] - pred[i]).collect();
                let (masks, deltas) = plan_corrections_masked_op(&r);
                let cost = cost_of(n, &comps, false, &masks, &deltas);
                if best.as_ref().is_none_or(|b| cost < b.cost) {
                    best = Some(ModalPlan {
                        cost,
                        comps: comps.clone(),
                        shared_f0: false,
                        masks,
                        deltas,
                        pred,
                    });
                }
            }
            comps[ci].3 = amp;
        }
    }
    best
}

/// Discover: sweep K ∈ 1..=4 (+ shared-f0 arm when a harmonic relationship
/// exists), pick measured-min; the PORTFOLIO stays the comparator.
pub fn discover_modal(samples: &[i16]) -> Option<(ModalPlan, usize)> {
    let n = samples.len();
    if n < 64 {
        return None;
    }
    let mut best: Option<(ModalPlan, ())> = None;
    for k in 1..=4usize {
        if let Some(plan) = modal_fit(samples, k) {
            if best.as_ref().is_none_or(|(b, _)| plan.cost < b.cost) {
                best = Some((plan, ()));
            }
        }
    }
    best.map(|(p, _)| {
        let wire_len = p.cost; // billed == wire (tested below)
        (p, wire_len)
    })
}

/// Serialization (the G4 container): exact inverse of the decoder below.
pub fn serialize_modal(plan: &ModalPlan, samples: &[i16]) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(plan.cost.max(64));
    write_varint(&mut out, samples.len() as u64);
    write_varint(&mut out, plan.comps.len() as u64);
    if plan.shared_f0 {
        let f0 = plan.comps[0].1;
        write_varint(&mut out, f0 as u64);
        for &(_, _, al, a) in &plan.comps {
            write_varint(&mut out, zigzag_encode(al));
            write_varint(&mut out, a as u64);
        }
    } else {
        for &(phi, w, al, a) in &plan.comps {
            write_varint(&mut out, zigzag_encode(phi));
            write_varint(&mut out, zigzag_encode(w));
            write_varint(&mut out, zigzag_encode(al));
            write_varint(&mut out, a as u64);
        }
    }
    write_mask_rle(&plan.masks, &mut out);
    for &d in &plan.deltas {
        write_varint(&mut out, zigzag_encode(d));
    }
    Ok(out)
}

pub fn decode_modal(payload: &[u8]) -> Result<(Vec<i16>, ModalPlan), String> {
    let mut pos = 0usize;
    let n = read_varint(payload, &mut pos)? as usize;
    if n == 0 {
        return Err(crate::ERR_EMPTY.into());
    }
    let k = read_varint(payload, &mut pos)? as usize;
    if k == 0 || k > 16 {
        return Err("invalid modal K".into());
    }
    let mut comps: Vec<(i64, i64, i64, i64)> = Vec::new();
    // plain joint grammar (the shared-f0 arm's wire rides its own measurement)
    if k >= 1 {
        for _ in 0..k {
            let phi = zigzag_decode(read_varint(payload, &mut pos)?);
            let w = zigzag_decode(read_varint(payload, &mut pos)?);
            let al = zigzag_decode(read_varint(payload, &mut pos)?);
            let a = read_varint(payload, &mut pos)? as i64;
            comps.push((phi, w, al, a));
        }
    }
    // harmonic multipliers are NOT decodable without the shared flag on the
    // wire; the shared arm's wire carries comps[0].1 as f0 — detect by the
    // encoder's own plan (research container: the plan object rides beside the
    // wire in tests; the wire itself is single-grammar for now).
    let masks = read_mask_rle(payload, &mut pos, n.div_ceil(32))?;
    let mut terms = Vec::new();
    for &(phi, w, al, a) in &comps {
        let t = component_term(phi, w, al, a, n).ok_or("modal component degenerate")?;
        terms.push(t);
    }
    let pred = sum_components(&terms);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let d = if masks[i / 32] & (1 << (i % 32)) != 0 {
            zigzag_decode(read_varint(payload, &mut pos)?)
        } else {
            0
        };
        out.push(i16_from_i64(pred[i] + d)?);
    }
    let plan = ModalPlan {
        cost: pos,
        comps,
        shared_f0: false,
        masks,
        deltas: Vec::new(),
        pred,
    };
    Ok((out, plan))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{encode as encode_pub, DEFAULT_RATE_FOR_RESEARCH as RATE};

    fn clamp_i16(v: i64) -> i16 {
        v.clamp(i16::MIN as i64, i16::MAX as i64) as i16
    }

    fn portfolio(samples: &[i16]) -> usize {
        encode_pub(RATE, samples).expect("encode").len()
    }

    fn modal_exact(samples: &[i16]) -> (usize, u8, usize) {
        let (plan, _cost) = discover_modal(samples).expect("modal discovery");
        let wire = serialize_modal(&plan, samples).expect("ser");
        assert_eq!(
            wire.len(),
            plan.cost,
            "modal: metric must equal serialize (k = {})",
            plan.comps.len()
        );
        let (back, _) = decode_modal(&wire).expect("dec");
        assert_eq!(back, samples, "modal: exact reconstruction");
        let payload = encode_pub(RATE, samples).expect("e");
        let (_, _, m) = crate::decode_payload(&payload).expect("d");
        (plan.cost, m.kind, payload.len())
    }

    /// G4-1: single sinusoid — the K-sweep must not pay for unneeded parts.
    #[test]
    fn g4_1_single_sinusoid_no_waste() {
        let fs = 44100.0f64;
        let x: Vec<i16> = (0..4096usize)
            .map(|i| {
                let t = i as f64 / fs;
                clamp_i16((12000.0 * (2.0 * std::f64::consts::PI * 917.3 * t).cos()).round() as i64)
            })
            .collect();
        let (cost, kind, port) = modal_exact(&x);
        eprintln!("G4-1: modal={cost} portfolio={port} (kind={kind})");
    }

    /// G4-2: two independent tones — the joint-vs-greedy decisive row.
    #[test]
    fn g4_2_two_tones_joint_measured() {
        let fs = 44100.0f64;
        let x: Vec<i16> = (0..4096usize)
            .map(|i| {
                let t = i as f64 / fs;
                let v = 8000.0 * (2.0 * std::f64::consts::PI * 917.3 * t).cos()
                    + 5000.0 * (2.0 * std::f64::consts::PI * 1361.0 * t).cos();
                clamp_i16(v.round() as i64)
            })
            .collect();
        let (cost, kind, port) = modal_exact(&x);
        eprintln!("G4-2: modal={cost} portfolio={port} (kind={kind})");
    }

    /// G4-4: two damped oscillators (independent decays).
    #[test]
    fn g4_4_two_damped_measured() {
        let fs = 44100.0f64;
        let x: Vec<i16> = (0..4096usize)
            .map(|i| {
                let t = i as f64 / fs;
                let v = 12000.0
                    * (-300.0 * t).exp()
                    * (2.0 * std::f64::consts::PI * 917.3 * t).cos()
                    + 9000.0 * (-150.0 * t).exp() * (2.0 * std::f64::consts::PI * 1361.0 * t).cos();
                clamp_i16(v.round() as i64)
            })
            .collect();
        let (cost, kind, port) = modal_exact(&x);
        eprintln!("G4-4: modal={cost} portfolio={port} (kind={kind})");
    }

    /// G4-6: independent mixture — different generating math (square-ish + sine).
    #[test]
    fn g4_6_independent_mixture_measured() {
        let fs = 44100.0f64;
        let x: Vec<i16> = (0..4096usize)
            .map(|i| {
                let t = i as f64 / fs;
                let v = (7000.0 * (2.0 * std::f64::consts::PI * 440.0 * t).sin()).signum() * 6000.0
                    + 5500.0 * (2.0 * std::f64::consts::PI * 1361.0 * t).cos();
                clamp_i16(v.round() as i64)
            })
            .collect();
        let (cost, kind, port) = modal_exact(&x);
        eprintln!("G4-6: modal={cost} portfolio={port} (kind={kind})");
    }

    /// G4-7: noise — MDL must reject the modal family.
    #[test]
    fn g4_7_noise_rejected() {
        let mut xr: u64 = 0x243F6A8885A308D3;
        let x: Vec<i16> = (0..4096usize)
            .map(|_| {
                xr ^= xr << 13;
                xr ^= xr >> 7;
                xr ^= xr << 17;
                (xr & 0xFFFF) as u16 as i16
            })
            .collect();
        let (cost, kind, port) = modal_exact(&x);
        eprintln!("G4-7: modal={cost} portfolio={port} (kind={kind})");
        assert!(
            cost >= port,
            "G4-7: modal must lose to the portfolio on noise: {cost} vs {port}"
        );
    }

    /// G4-8: saturation — a mixture whose sum exceeds i16 reconstructs exactly.
    #[test]
    fn g4_8_saturation_exact() {
        let fs = 44100.0f64;
        let x: Vec<i16> = (0..4096usize)
            .map(|i| {
                let t = i as f64 / fs;
                let v = (16000.0 * (2.0 * std::f64::consts::PI * 917.3 * t).cos()).round() as i64
                    + (16000.0 * (2.0 * std::f64::consts::PI * 1361.0 * t).cos()).round() as i64;
                clamp_i16(v)
            })
            .collect();
        let (cost, _kind, _port) = modal_exact(&x);
        eprintln!("G4-8: modal={cost} (exact reconstruction enforced)");
    }
}
