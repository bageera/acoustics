//! G17 — fractional-bit residual backend (research-only; wire v7 untouched).
//!
//! G15's endgame evidence: the D-ladder's continuation win shrinks ~4× per
//! order, "bounded by the varint coder, not stream structure". G17 asks the
//! roadmap's next question: is that a limitation of the residual BACKEND
//! rather than the mathematics?
//!
//! G17-A re-runs the G11 information audit against the v7-era residuals
//! (D3/D4/D5 selections + the adaptive mask layer moved the distributions):
//! H0(R) of the EXACT symbol sequences the production writer codes vs the
//! actual serialized residual-span bytes. The gap is a diagnostic, not a
//! promise (G8's 0.1% lesson: model overhead can eat a whole gap).
//!
//! G17-B prototypes the smallest viable alternative — Golomb-Rice with a
//! bounded escape (R1) and block-adaptive Rice (R2) — against the shipped
//! varint stream (R0 = the REAL writer's cost, never an estimate). Every
//! predictor, mask section, and field is UNCHANGED; only the delta stream is
//! recoded. The promotion gate is the roadmap inequality:
//! L_new_coder + L_parameters + L_signaling < L_shipping.
//!
//! Rice scheme (this module's own convention, no wire): zigzag value u,
//! quotient q = u >> k coded unary (q ones + one 0), then the remainder's k
//! bits MSB-first; q >= ESC_Q escapes with a fixed unary pattern + a
//! bit-wise varint of the full u. Bit stream is MSB-first into bytes,
//! 0-padded to a byte at the END of the stream; the serialized cost is
//! ceil(bits/8). Encoder and decoder walk identical bits: the metric IS the
//! serialized size by construction.
//!
//! Arm layouts (hypothetical research wire, every byte billed):
//!   R1: [selector 1 B][k varint][Rice stream of every symbol, ceil/8]
//!   R2: [selector 1 B][nblocks varint][per block: k varint written
//!       bit-wise, then the block's Rice symbols; one shared stream,
//!       ceil(total bits/8)]

const G17_ESC_Q: u64 = 12; // unary escape threshold (quotient)
const G17_K_MAX_R1: u8 = 20; // global-k sweep bound (R1)
const G17_K_MAX_R2: u8 = 19; // per-block sweep bound (R2)
const G17_BLOCK: usize = 256; // R2 block size (8 mask words)

// ---------- shared corpus machinery (used by g18/g21 too) ----------

/// The production writer's RESIDUAL SPAN (bytes) + the symbol stream it
/// codes, for the selected kind — the accounting G11-A used, extended to
/// the dense D3/D4/D5 kinds and the v7-era adaptive mask costs.
pub(crate) fn ledger_span(model: crate::Model, rate: u32, y: &[i16]) -> (usize, usize, Vec<u64>) {
    match model.kind {
        crate::MODEL_D1 | crate::MODEL_D2 | crate::MODEL_D3 | crate::MODEL_D4 | crate::MODEL_D5 => {
            let r = crate::predictor_residuals(y, model.kind);
            let actual = r
                .iter()
                .map(|&v| crate::varint_len(crate::zigzag_encode(v)))
                .sum();
            let symbols = r.iter().map(|&v| crate::zigzag_encode(v)).collect();
            (actual, 0, symbols)
        }
        crate::MODEL_PERIOD => {
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
            let mask_bytes =
                crate::mask_adaptive_cost(&crate::period_plan(y, lag).expect("period plan").1);
            (bytes, mask_bytes, symbols)
        }
        crate::MODEL_OSC => {
            let (actual_res, mask_res, symbols) = crate::g9::ledger_bits(model, rate, y);
            (actual_res, mask_res, symbols)
        }
        crate::MODEL_OP => {
            // the delta stream only (the G17 rule: every field, the base
            // y-stream and the mask section ride v7-unchanged; the masked
            // corrections get the backend swap)
            let plan = crate::op_plan(y, model.param).expect("op plan");
            let symbols: Vec<u64> = plan
                .deltas
                .iter()
                .map(|&d| crate::zigzag_encode(d))
                .collect();
            let bytes = symbols.iter().map(|&u| crate::varint_len(u)).sum();
            (bytes, crate::mask_rle_wire_len(&plan.masks), symbols)
        }
        _ => (0, 0, Vec::new()),
    }
}

// ---------- the bit-stream writer ----------

struct RiceBits {
    bytes: Vec<u8>,
    nbits: usize,
}

impl RiceBits {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            nbits: 0,
        }
    }
    fn bit(&mut self, b: u8) {
        if self.nbits.is_multiple_of(8) {
            self.bytes.push(0);
        }
        if b != 0 {
            let last = self.bytes.last_mut().expect("byte seeded");
            *last |= 0x80 >> (self.nbits % 8);
        }
        self.nbits += 1;
    }
    /// unary: `q` ones then a 0 terminator
    fn unary(&mut self, q: u64) {
        for _ in 0..q {
            self.bit(1);
        }
        self.bit(0);
    }
    fn kbits(&mut self, v: u64, k: u8) {
        for i in (0..k).rev() {
            self.bit(((v >> i) & 1) as u8);
        }
    }
    /// varint bytes written bit-wise (1 byte = 8 bits, MSB first)
    fn varint_bits(&mut self, v: u64) {
        let mut out = Vec::new();
        crate::write_varint(&mut out, v);
        for b in out {
            self.kbits(b as u64, 8);
        }
    }
    /// one Rice symbol with bounded escape
    fn symbol(&mut self, u: u64, k: u8) {
        let q = u >> k;
        if q < G17_ESC_Q {
            self.unary(q);
            self.kbits(u & ((1u64 << k) - 1), k);
        } else {
            self.unary(G17_ESC_Q); // the escape pattern
            self.varint_bits(u);
        }
    }
    fn cost(&self) -> usize {
        self.nbits.div_ceil(8)
    }
}

// ---------- the bit-stream reader ----------

struct RiceRead<'a> {
    bytes: &'a [u8],
    nbits: usize,
}

impl<'a> RiceRead<'a> {
    fn bit(&mut self) -> Result<u8, String> {
        let i = self.nbits;
        self.nbits += 1;
        let byte = *self.bytes.get(i / 8).ok_or("truncated Rice stream")?;
        Ok((byte >> (7 - (i % 8))) & 1)
    }
    fn unary(&mut self) -> Result<u64, String> {
        let mut q = 0u64;
        loop {
            if self.bit()? == 1 {
                q += 1;
            } else {
                return Ok(q);
            }
        }
    }
    fn kbits(&mut self, k: u8) -> Result<u64, String> {
        let mut v = 0u64;
        for _ in 0..k {
            v = (v << 1) | self.bit()? as u64;
        }
        Ok(v)
    }
    /// mirror of varint_bits: varint bytes read bit-wise
    fn varint_read(&mut self) -> Result<u64, String> {
        let mut u = 0u64;
        let mut shift = 0u32;
        loop {
            let b = self.kbits(8)?;
            u |= (b & 0x7F) << shift;
            if b & 0x80 == 0 {
                return Ok(u);
            }
            shift += 7;
            if shift >= 64 {
                return Err("varint too long for u64".into());
            }
        }
    }
    fn symbol(&mut self, k: u8) -> Result<u64, String> {
        let q = self.unary()?;
        if q < G17_ESC_Q {
            let rem = if k == 0 { 0 } else { self.kbits(k)? };
            Ok((q << k) | rem)
        } else {
            self.varint_read()
        }
    }
}

// ---------- the arms ----------

/// R0 — the production varint stream: the shipped writer's own cost (the
/// metric==serialize invariant suite pins this identity for every kind).
pub(crate) fn rice_r0(symbols: &[u64]) -> usize {
    symbols.iter().map(|&u| crate::varint_len(u)).sum()
}

fn rice_stream(symbols: &[u64], k: u8) -> RiceBits {
    let mut s = RiceBits::new();
    for &u in symbols {
        s.symbol(u, k);
    }
    s
}

/// R1 — single best global k.
/// Returns (cost, k, decoded). cost = selector 1 B + k varint + ceil(bits/8).
pub(crate) fn rice_r1(symbols: &[u64]) -> (usize, u8, Vec<u64>) {
    let mut best: Option<(usize, u8, Vec<u8>, usize)> = None;
    for k in 0..=G17_K_MAX_R1 {
        let stream = rice_stream(symbols, k);
        let cost = 1 + crate::varint_len(k as u64) + stream.cost();
        if best.as_ref().is_none_or(|&(c, ..)| cost <= c) {
            // the real payload: selector byte, k varint, then the stream
            let mut bytes = Vec::with_capacity(cost);
            bytes.push(1u8);
            crate::write_varint(&mut bytes, k as u64);
            bytes.extend_from_slice(&stream.bytes);
            best = Some((cost, k, bytes, stream.nbits));
        }
    }
    let (cost, k, bytes, bits) = best.expect("k sweep non-empty");
    // decode from the payload: skip the selector byte and the k varint
    let k_len = crate::varint_len(k as u64);
    let mut rd = RiceRead {
        bytes: &bytes[1 + k_len..],
        nbits: 0,
    };
    let mut decoded = Vec::with_capacity(symbols.len());
    for _ in 0..symbols.len() {
        decoded.push(rd.symbol(k).expect("R1 decode"));
    }
    assert_eq!(rd.nbits, bits, "R1: exact bit consumption");
    assert_eq!(bytes.len(), cost, "R1: metric must equal payload length");
    (cost, k, decoded)
}

/// R2 — per-block adaptive k; each block bills its own k varint bit-wise
/// inside a single continuous stream. Returns (cost, decoded).
pub(crate) fn rice_r2(symbols: &[u64]) -> (usize, Vec<u64>) {
    let nblocks = symbols.len().div_ceil(G17_BLOCK);
    let stream = {
        let mut s = RiceBits::new();
        let mut at = 0usize;
        for _ in 0..nblocks {
            let end = (at + G17_BLOCK).min(symbols.len());
            let slice = &symbols[at..end];
            let mut best: Option<(usize, u8)> = None;
            for k in 0..=G17_K_MAX_R2 {
                let mut cand = RiceBits::new();
                cand.varint_bits(k as u64);
                for &u in slice {
                    cand.symbol(u, k);
                }
                if best.as_ref().is_none_or(|&(c, _)| cand.cost() <= c) {
                    best = Some((cand.cost(), k));
                }
            }
            let k = best.expect("k sweep non-empty").1;
            s.varint_bits(k as u64);
            for &u in slice {
                s.symbol(u, k);
            }
            at = end;
        }
        s
    };
    // wire: selector byte + nblocks varint + the continuous block stream
    let mut out = Vec::new();
    out.push(2u8);
    crate::write_varint(&mut out, nblocks as u64);
    out.extend_from_slice(&stream.bytes);
    // decode
    let mut rd = RiceRead {
        bytes: &out[1..],
        nbits: 0,
    };
    let nb = rd.varint_read().expect("R2 nblocks") as usize;
    let mut decoded = Vec::with_capacity(symbols.len());
    let mut at = 0usize;
    for _ in 0..nb {
        let end = (at + G17_BLOCK).min(symbols.len());
        let k = rd.varint_read().expect("R2 block k") as u8;
        for _ in at..end {
            decoded.push(rd.symbol(k).expect("R2 decode"));
        }
        at = end;
    }
    assert_eq!(
        rd.nbits,
        crate::varint_len(nblocks as u64) * 8 + stream.nbits,
        "R2: exact bit consumption (nblocks varint + block stream)"
    );
    assert_eq!(out.len(), cost_of_r2(&stream, nblocks), "R2: metric==len");
    (out.len(), decoded)
}

fn cost_of_r2(stream: &RiceBits, nblocks: usize) -> usize {
    1 + crate::varint_len(nblocks as u64) + stream.cost()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------- roundtrip / metric invariants ----------

    /// Rice roundtrip + metric==payload across k, covering the escape
    /// boundary (quotients ESC_Q-1 and ESC_Q) and full-width u64 values.
    #[test]
    fn g17_rice_roundtrip_and_metric() {
        let mut edge: Vec<u64> = vec![0, 1, 2, 5, 63, 64, 4095, 4096];
        for &k in &[0u8, 5, 12] {
            edge.push((G17_ESC_Q - 1) << k);
            edge.push(G17_ESC_Q << k);
            edge.push(((G17_ESC_Q << k) | ((1u64 << k) / 2)).max(G17_ESC_Q << k));
        }
        edge.push(u64::MAX / 2);
        edge.push(u64::MAX);
        for k in 0..=G17_K_MAX_R1 {
            let stream = rice_stream(&edge, k);
            let mut rd = RiceRead {
                bytes: &stream.bytes,
                nbits: 0,
            };
            let mut back = Vec::with_capacity(edge.len());
            for _ in 0..edge.len() {
                back.push(rd.symbol(k).expect("roundtrip"));
            }
            assert_eq!(back, edge, "k={k}: exact Rice roundtrip");
            assert_eq!(rd.nbits, stream.nbits, "k={k}: exact bit consumption");
            assert_eq!(stream.bytes.len(), stream.cost(), "k={k}: metric==len");
        }
        // arm roundtrips on the same edges
        let (_, _, back1) = rice_r1(&edge);
        assert_eq!(back1, edge, "R1 arm roundtrip");
        let (_, back2) = rice_r2(&edge);
        assert_eq!(back2, edge, "R2 arm roundtrip");
    }

    /// The mechanism pin (NOT a wire claim): on a heavy-tail-small residual
    /// population the Rice backend must beat the shipped varint stream,
    /// with the selector + k parameters fully billed.
    #[test]
    fn g17_rice_wins_on_small_magnitude_distribution() {
        let mut pop: Vec<u64> = Vec::new();
        let mut xr: u64 = 0x243F6A8885A308D3;
        for _ in 0..4096 {
            xr ^= xr << 13;
            xr ^= xr >> 7;
            xr ^= xr << 17;
            let u = match (xr >> 40) % 64 {
                0..=37 => 0,
                38..=49 => (xr >> 44) % 2,
                50..=58 => 2 + (xr >> 44) % 2,
                59..=63 => 4 + (xr >> 44) % 4,
                _ => 8 + (xr >> 44) % 8,
            };
            pop.push(u);
        }
        let r0 = rice_r0(&pop);
        let (r1, _, back1) = rice_r1(&pop);
        assert_eq!(back1, pop, "R1 exact");
        assert!(
            r1 < r0,
            "Rice must beat varint on small-magnitude residuals: R1={r1} vs R0={r0}"
        );
        // an order-0 code cannot undercut the H0 prefix floor (± rounding)
        let h0_bits = crate::g11::h0(&pop) * pop.len() as f64;
        assert!(
            (r1 as f64) * 8.0 >= h0_bits - 64.0,
            "R1 must stay at/above the H0 floor: {r1} B vs {h0_bits:.0} b"
        );
    }

    // ---------- shared corpus machinery ----------

    fn corpus_classes() -> Vec<(&'static str, String)> {
        vec![
            ("piano-ch0", "testdata/g12/emotional_piano.wav:0"),
            ("piano-ch1", "testdata/g12/emotional_piano.wav:1"),
            ("birds-ch0", "testdata/g12/park_birds.wav:0"),
            ("birds-ch1", "testdata/g12/park_birds.wav:1"),
            ("river-ch0", "testdata/g12/park_river.wav:0"),
            ("river-ch1", "testdata/g12/park_river.wav:1"),
            ("speech1-ch0", "testdata/g5f/example_word.wav:0"),
            ("speech2-ch0", "testdata/g5f/can_you_stop.wav:0"),
            ("speech3-ch0", "testdata/g5f/sample2.wav:0"),
        ]
        .into_iter()
        .map(|(name, spec)| {
            let (path, ch) = spec.split_once(':').expect("spec");
            (name, format!("{path}\n{ch}"))
        })
        .collect()
    }

    /// G17-A: the v7-era wire audit. Per class: the selected kind, the
    /// residual symbol count, the REAL serialized residual span (plus the
    /// mask span where the kind carries one), H0 and lag-1 Hcond in
    /// bits/symbol, and the gap in bytes. The integral stream cannot
    /// undercut the H0 prefix floor on long runs — pinned with generous
    /// sampling slack.
    #[test]
    #[ignore] // corpus-gated: scripts/g12_fetch.sh + testdata/g5f
    fn g17a_v7_residual_entropy_audit() {
        let mut rows = String::new();
        for (name, spec) in corpus_classes() {
            let (path, ch) = spec.split_once('\n').expect("spec");
            let data = std::fs::read(path).expect("corpus (run the fetch scripts)");
            let (rate, streams) = crate::parse_wav_multichannel(&data).expect("parse");
            let y = &streams[ch.parse::<usize>().expect("ch")];
            let model = crate::choose_model(y);
            let (actual_res, mask_res, symbols) = ledger_span(model, rate, y);
            let h0v = crate::g11::h0(&symbols);
            let hcv = crate::g11::hcond1(&symbols);
            let h0_bits = (h0v * symbols.len() as f64).ceil() as usize / 8;
            let hc_bits = (hcv * symbols.len().saturating_sub(1) as f64).ceil() as usize / 8;
            let gap_h0 = actual_res as i64 - h0_bits as i64;
            rows.push_str(&format!(
                "G17-A {name} kind={} n={} | res_span={actual_res}B mask={mask_res}B | \
                 H0={h0_bits}B ({h0v:.3} b/sym) Hcond={hc_bits}B ({hcv:.3} b/sym) | \
                 gap(H0)={gap_h0}B gap(Hcond)={}B\n",
                model.kind,
                symbols.len(),
                actual_res as i64 - hc_bits.min(h0_bits) as i64,
            ));
            if symbols.len() >= 100_000 {
                assert!(
                    (actual_res as f64) * 8.0 + 512.0 >= h0v * symbols.len() as f64,
                    "{name}: integral stream undercut the H0 floor — investigate"
                );
            }
        }
        eprintln!("{rows}");
    }

    /// G17-B: the backend arms on the same classes — R0 (the real writer),
    /// R1 (global-k Rice), R2 (block-adaptive Rice); every arm's decoded
    /// stream must equal the input EXACTLY, and the honest inequality rows
    /// are printed (a loss is a result, not a failure). Selector bytes and
    /// all parameter costs are billed.
    #[test]
    #[ignore] // corpus-gated: scripts/g12_fetch.sh + testdata/g5f
    fn g17b_rice_backend_arms() {
        let mut rows = String::new();
        for (name, spec) in corpus_classes() {
            let (path, ch) = spec.split_once('\n').expect("spec");
            let data = std::fs::read(path).expect("corpus (run the fetch scripts)");
            let (rate, streams) = crate::parse_wav_multichannel(&data).expect("parse");
            let y = &streams[ch.parse::<usize>().expect("ch")];
            let model = crate::choose_model(y);
            let (r0, mask_res, symbols) = ledger_span(model, rate, y);
            let (r1, k1, back1) = rice_r1(&symbols);
            assert_eq!(back1, symbols, "{name}: R1 exact");
            let (r2, back2) = rice_r2(&symbols);
            assert_eq!(back2, symbols, "{name}: R2 exact");
            // the best arm, honestly chosen
            let (best_cost, best_arm) = if r2 < r1 { (r2, "R2") } else { (r1, "R1") };
            // whole-frame translation: header + fields + masks are IDENTICAL
            // across arms, so new_frame = old_frame - r0 + best_cost — the
            // number the wire decision reads
            let frame = r0 + mask_res;
            let new_frame = frame as i64 - r0 as i64 + best_cost as i64;
            let verdict = if best_cost < r0 {
                format!(
                    "RICE WINS ({best_arm} {:+} B vs R0; frame {frame}→{new_frame} B)",
                    best_cost as i64 - r0 as i64
                )
            } else {
                format!(
                    "varint holds (R1 {:+}, R2 {:+})",
                    r1 as i64 - r0 as i64,
                    r2 as i64 - r0 as i64
                )
            };
            rows.push_str(&format!(
                "G17-B {name} kind={} n={} | R0={r0}B R1={r1}B (k={k1}) R2={r2}B | {verdict}\n",
                model.kind,
                symbols.len(),
            ));
        }
        eprintln!("{rows}");
    }
}
