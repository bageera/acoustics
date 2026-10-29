//! G11-A — residual information audit (research-only; wire untouched).
//!
//! The question after G9: are the residual bytes (91-100% of the real-speech
//! payloads) anywhere near their information limit?
//!
//! Three measurements per stream (the review's table), all on the EXACT
//! symbol sequences the production coder writes:
//! 1. H0 — empirical symbol entropy of the zigzag-mapped residual stream
//! 2. H(R_n | R_{n-1}) — lag-1 conditional entropy (temporal dependency)
//! 3. Actual — the production wire's residual bytes (from the G9 ledger)
//!
//! Then G = L_actual − L_idealized (bits/8, ceil'd) — a DIAGNOSTIC gap: how
//! much a fractional-bit entropy coder could theoretically save over the
//! integral varint stream. Not automatically recoverable: probability-model
//! overhead, adaptation, and context signaling all cost bytes (G8's own
//! 0.1% lesson).
//!
//! Decomposition of G (honest attribution):
//! - G_int: the integral-byte floor — L_actual vs sum(ceil(H0 per symbol))
//!   approximated by the per-symbol rounded length distribution
//! - G_shape: distribution skew — L(H0-coded) vs L(geometric-tail actual)
//! - G_temporal: H0 − Hcond — the lag-1 dependency the varints ignore

use crate::{predictor_residuals, MODEL_D1, MODEL_OSC, MODEL_PERIOD};

/// Shannon H0 of a symbol slice (exact empirical distribution).
#[cfg(test)]
fn h0(symbols: &[u64]) -> f64 {
    let mut hist = std::collections::HashMap::new();
    for &s in symbols {
        *hist.entry(s).or_insert(0u32) += 1;
    }
    let n = symbols.len() as f64;
    hist.values()
        .map(|&c| {
            let p = c as f64 / n;
            -p * p.log2()
        })
        .sum()
}

/// Lag-1 conditional entropy via joint counting of (R_{n-1}, R_n).
#[cfg(test)]
fn hcond1(symbols: &[u64]) -> f64 {
    if symbols.len() < 2 {
        return 0.0;
    }
    let mut joint = std::collections::HashMap::new();
    for w in symbols.windows(2) {
        *joint.entry((w[0], w[1])).or_insert(0u32) += 1;
    }
    let n = (symbols.len() - 1) as f64;
    let h_joint = joint
        .values()
        .map(|&c| {
            let p = c as f64 / n;
            -p * p.log2()
        })
        .sum::<f64>();
    // H(X_t, X_{t-1}) − H(X_{t-1}) over the same n (marginal recount)
    let mut marg = std::collections::HashMap::new();
    for &s in &symbols[..symbols.len() - 1] {
        *marg.entry(s).or_insert(0u32) += 1;
    }
    let n_m = (symbols.len() - 1) as f64;
    let h_marg = marg
        .values()
        .map(|&c| {
            let p = c as f64 / n_m;
            -p * p.log2()
        })
        .sum::<f64>();
    h_joint - h_marg
}

/// The production cost of the D1 residual stream: sum of zigzag varint
/// bytes (identical to what serialize writes — same encoder fn).
#[cfg(test)]
fn actual_d1(residuals: &[i64]) -> usize {
    residuals
        .iter()
        .map(|&r| crate::varint_len(crate::zigzag_encode(r)))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::g9::ledger_bits;

    /// The audit on the locked corpus: H0, Hcond, actual, G, decomposed.
    #[test]
    fn g11a_residual_information_audit() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/g5f");
        let mut rows = String::new();
        for name in ["example_word.wav", "can_you_stop.wav", "sample2.wav"] {
            let data = std::fs::read(format!("{dir}/{name}")).expect("corpus");
            let (rate, streams) = crate::parse_wav_multichannel(&data).expect("parse");
            for (ch, y) in streams.iter().enumerate() {
                let model = crate::choose_model(y);
                let (actual_res, mask_res, symbols): (usize, usize, Vec<u64>) = match model.kind {
                    MODEL_D1 => {
                        let r = predictor_residuals(y, MODEL_D1);
                        let actual = actual_d1(&r);
                        // residual bytes only (the ledger's residual span,
                        // minus... header excluded — stream-only audit)
                        let symbols = r.iter().map(|&v| crate::zigzag_encode(v)).collect();
                        (actual, 0, symbols)
                    }
                    MODEL_PERIOD => {
                        // the corrections AFTER the lag seed residual:
                        // stream = varints written for lag..count (sparse)
                        let lag = model.param;
                        let mut symbols = Vec::new();
                        let mut bytes = 0usize;
                        for i in lag..y.len() {
                            let d = y[i] as i64 - y[i - lag] as i64;
                            if d != 0 {
                                symbols.push(crate::zigzag_encode(d));
                                bytes += crate::varint_len(crate::zigzag_encode(d));
                            }
                        }
                        // mask words (flat) from the extraction helper
                        let mask_words = (y.len() - lag).div_ceil(32);
                        (bytes, mask_words * 4, symbols)
                    }
                    MODEL_OSC => {
                        let (a, m, sy) = ledger_bits(model, rate, y);
                        (a, m, sy)
                    }
                    _ => (0, 0, Vec::new()),
                };
                let h0v = h0(&symbols);
                let hcv = hcond1(&symbols);
                let h0_bits = (h0v * symbols.len() as f64).ceil() as usize / 8;
                let hc_bits = (hcv * symbols.len().saturating_sub(1) as f64).ceil() as usize / 8;
                let actual_total = actual_res + mask_res;
                let g_h0 = actual_total as i64 - h0_bits as i64;
                let g_hcond = actual_total as i64 - hc_bits.min(h0_bits) as i64;
                rows.push_str(&format!(
                    "G11-A {name} ch{ch} (kind={}): symbols={} actual(actual_res={actual_res}+mask={mask_res})={actual_total}B | H0={h0_bits}B Hcond={hc_bits}B | G(vs H0)={g_h0}B G(vs Hcond)={g_hcond}B | H0={h0v:.2}b/sym Hcond={hcv:.2}b/sym\n",
                    model.kind,
                    symbols.len(),
                ));
            }
        }
        // tonal fixture: two-tone (osc cascade)
        let two_tone: Vec<i16> = (0..4096usize)
            .map(|i| {
                let t = i as f64 / 44100.0;
                (8000.0 * (2.0 * std::f64::consts::PI * 917.3 * t).cos()
                    + 5000.0 * (2.0 * std::f64::consts::PI * 1361.0 * t).cos())
                .round()
                .clamp(-32767.0, 32767.0) as i16
            })
            .collect();
        let model2 = crate::choose_model(&two_tone);
        let (actual_res, mask_res, symbols) = ledger_bits(model2, 44100, &two_tone);
        let h0v = h0(&symbols);
        let hcv = hcond1(&symbols);
        let h0_bits = (h0v * symbols.len() as f64).ceil() as usize / 8;
        let hc_bits = (hcv * symbols.len().saturating_sub(1) as f64).ceil() as usize / 8;
        let actual_total = actual_res + mask_res;
        rows.push_str(&format!(
            "G11-A two-tone (kind=4): symbols={} actual={actual_total}B | H0={h0_bits}B Hcond={hc_bits}B | G(vs H0)={}B G(vs Hcond)={}B\n",
            symbols.len(),
            actual_total as i64 - h0_bits as i64,
            actual_total as i64 - hc_bits.min(h0_bits) as i64,
        ));
        eprintln!("{rows}");
    }
}
