//! G21 — joint minimum description length (research-only; wire v7 untouched).
//!
//! The authorization: three compression mechanisms have each been validated
//! separately — spatial geometry (G18), temporal prediction (the nine
//! production kinds), and the residual backend (G17) — but never jointly.
//! G21 establishes
//!
//!     L*(X) = min_{T,P,B} | serialize(T, P, B, X) |
//!
//! where T is the spatial geometry (C0-C3), P the temporal predictor with
//! its exact production parameter space, and B the residual backend (R0
//! varint, R1 global Rice, R2 block-adaptive Rice) — every component billed
//! through its complete representation.
//!
//! G21-A is the exhaustive reference portfolio: all nine production kinds
//! under each backend, per transformed component stream, per geometry.
//! G21-B measures the model-ranking inversion: does a model that loses
//! under varint win under Rice? Neither outcome is assumed.
//!
//! Billing convention (byte-identical to the G18-C cell receipts, which
//! this module re-asserts as literal regression controls):
//!   varint cell  = the shipped payload itself (no selector beyond v7);
//!   rice cells   = frame(model without its residual varint stream)
//!                  + 1 backend-selector byte + the arm body, where the
//!                  arm body is rice_r1/rice_r2's complete payload (their
//!                  internal selector byte is redundant with the backend
//!                  selector — a uniform documented 1-byte overhead);
//!   portfolio    = frame + 1 + min(varint span, arm1, arm2);
//!   geometry     = + 1 transform-selector byte per transformed stereo
//!                  payload + the packed parity-plane floor (ceil(n/8)) on
//!                  the 17-bit side.
//!
//! RAW bills its shipped submode minimum as the varint cell and recodes the
//! varint stream on the varint-mode frame (header + submode byte) for the
//! Rice arms. The exhaustive optimum is a REFERENCE, not an encoder change.
//!
//! Enumeration cost control: R1/R2 arm costs come from an EXACT closed form
//! (a Rice symbol's bit count is arithmetic: unary q = q+1 bits; an escape
//! is ESC_Q+1 + 8*varint_len(u)), so no bit stream is needed for the sweep.
//! The closed form cannot disagree with the real coder by construction, and
//! every arm whose number enters a printed receipt is re-run through the
//! REAL rice_r1/rice_r2 — which decode-verify their payloads bit-exactly —
//! with an equality assert.

#[cfg(test)]
mod tests {

    /// The exact reversible split S = 2A + B (A = the 16-bit half, B = the
    /// packed parity bit) — duplicated from g18's tests module (the shared
    /// hoist was judged not worth reopening that module's visibility).
    fn side_split(v: i64) -> (i16, u8) {
        let a = floor_div2(v);
        let par = (v & 1) as u8;
        assert!(v == 2 * a + par as i64, "split must reconstruct {v}");
        (
            i16::try_from(a).expect("side half must fit i16 by construction"),
            par,
        )
    }

    /// Sign-preserving floor division by two (g18's tests module holds the
    /// original).
    fn floor_div2(v: i64) -> i64 {
        if v >= 0 {
            v >> 1
        } else {
            -((-v + 1) >> 1)
        }
    }

    // ---------- the exact closed-form Rice arm costs ----------

    /// Closed-form Rice bits for one symbol under parameter k — the exact
    /// inverse of the G17 coder's bit layout (unary q + k remainder bits,
    /// or the escape pattern + a byte-wise varint).
    fn rice_symbol_bits(u: u64, k: u8) -> usize {
        let q = u >> k;
        if q < 12 {
            q as usize + 1 + k as usize
        } else {
            13 + 8 * crate::varint_len(u)
        }
    }

    /// R1 arm body: selector 1 B + k varint + stream, byte-rounded.
    fn fast_r1(symbols: &[u64]) -> (usize, u8, usize) {
        let mut best: Option<(usize, u8, usize)> = None;
        for k in 0..=20u8 {
            let bits: usize = symbols.iter().map(|&u| rice_symbol_bits(u, k)).sum();
            let cost = 1 + crate::varint_len(k as u64) + bits.div_ceil(8);
            if best.is_none_or(|(c, ..)| cost <= c) {
                best = Some((cost, k, bits));
            }
        }
        best.expect("k sweep non-empty")
    }

    /// R2 arm body: selector 1 B + nblocks varint + the continuous
    /// per-block stream (each block's k varint rides bit-wise inside the
    /// shared stream), byte-rounded once at the end. The block sweep
    /// MIRRORS the real coder's selection rule exactly: candidates compare
    /// by BYTE-ROUNDED block cost with last-wins ties (ascending k) — an
    /// artifact of rice_r2's own sweep that the reference must reproduce,
    /// not idealize; each block contributes its winner's EXACT bits.
    fn fast_r2(symbols: &[u64]) -> usize {
        let mut total_bits = 0usize;
        for block in symbols.chunks(256) {
            let mut best_rounded: Option<usize> = None;
            let mut winner_bits = 0usize;
            for k in 0..=19u8 {
                let bits = 8 * crate::varint_len(k as u64)
                    + block.iter().map(|&u| rice_symbol_bits(u, k)).sum::<usize>();
                let rounded = bits.div_ceil(8);
                if best_rounded.is_none_or(|b| rounded <= b) {
                    best_rounded = Some(rounded);
                    winner_bits = bits;
                }
            }
            total_bits += winner_bits;
        }
        let nblocks = symbols.len().div_ceil(256);
        1 + crate::varint_len(nblocks as u64) + total_bits.div_ceil(8)
    }

    // ---------- the exhaustive candidate enumeration ----------

    /// One production candidate under all three backends. Costs follow the
    /// module billing: r0_pay = the shipped payload (frame + varint span,
    /// no backend selector); r1/r2 = frame + 1 + arm body; portfolio =
    /// frame + 1 + min(span, arm1, arm2).
    #[derive(Clone)]
    struct Cand {
        kind: u8,
        param: usize,
        frame: usize,
        r0_pay: usize,
        r1: usize,
        r2: usize,
        portfolio: usize,
    }

    impl Cand {
        fn rice_best(&self) -> usize {
            self.r1.min(self.r2)
        }
        fn tag(&self) -> String {
            format!("{}(p{})", name_of(self.kind), self.param)
        }
    }

    fn name_of(kind: u8) -> &'static str {
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

    /// The production candidate space, verbatim from choose_model: RAW 0;
    /// D1-D5 param 0; PERIOD lag 2..=96; OSC variant 0..11; OP arm 0..16.
    /// Every non-RAW candidate asserts the meter==serialize identity
    /// (rice_r0 == the shipped varint span).
    fn stream_enumerate(rate: u32, y: &[i16]) -> Vec<Cand> {
        let mut cands: Vec<Cand> = Vec::new();
        for kind in [
            crate::MODEL_D1,
            crate::MODEL_D2,
            crate::MODEL_D3,
            crate::MODEL_D4,
            crate::MODEL_D5,
        ] {
            push_cand(&mut cands, kind, 0, rate, y);
        }
        for lag in 2..=96usize {
            push_cand(&mut cands, crate::MODEL_PERIOD, lag, rate, y);
        }
        for v in 0..crate::OSC_VARIANT_COUNT {
            push_cand(&mut cands, crate::MODEL_OSC, v, rate, y);
        }
        for op in 0..crate::OP_ARM_COUNT {
            push_cand(&mut cands, crate::MODEL_OP, op, rate, y);
        }
        // RAW: the shipped payload (submode minimum) as the varint cell;
        // Rice arms recode the varint stream on the varint-mode frame
        let payload = crate::payload_len(crate::MODEL_RAW, 0, y).expect("raw metric");
        let residuals = crate::predictor_residuals(y, crate::MODEL_RAW);
        let symbols: Vec<u64> = residuals.iter().map(|&v| crate::zigzag_encode(v)).collect();
        let (f1, _, _) = fast_r1(&symbols);
        let f2 = fast_r2(&symbols);
        let frame = crate::header_len(y.len()) + 1; // header + submode byte
        cands.push(Cand {
            kind: crate::MODEL_RAW,
            param: 0,
            frame,
            r0_pay: payload,
            r1: frame + 1 + f1,
            r2: frame + 1 + f2,
            portfolio: frame + 1 + (span_of(&symbols)).min(f1).min(f2),
        });
        cands
    }

    fn span_of(symbols: &[u64]) -> usize {
        symbols.iter().map(|&u| crate::varint_len(u)).sum()
    }

    fn push_cand(cands: &mut Vec<Cand>, kind: u8, param: usize, rate: u32, y: &[i16]) {
        // None = the model legitimately rejects this stream (unstable OSC
        // fit, degenerate PERIOD…) — choose_model skips it; so does the
        // exhaustive reference
        let payload = match crate::payload_len(kind, param, y) {
            Some(p) => p,
            None => return,
        };
        let model = crate::Model { kind, param };
        let (res_span, _mask, symbols) = crate::g17::ledger_span(model, rate, y);
        assert_eq!(
            crate::g17::rice_r0(&symbols),
            res_span,
            "R0 identity {}/p{param}",
            name_of(kind)
        );
        let frame = payload - res_span;
        let (f1, _, _) = fast_r1(&symbols);
        let f2 = fast_r2(&symbols);
        cands.push(Cand {
            kind,
            param,
            frame,
            r0_pay: payload,
            r1: frame + 1 + f1,
            r2: frame + 1 + f2,
            portfolio: frame + 1 + res_span.min(f1).min(f2),
        });
    }

    /// The frozen production selection's Cand (it is in the enumerated
    /// space by construction — asserted).
    fn frozen_cand(rate: u32, y: &[i16]) -> (Cand, Vec<Cand>) {
        let frozen = crate::choose_model(y);
        let cands = stream_enumerate(rate, y);
        let hit = cands
            .iter()
            .find(|c| c.kind == frozen.kind && c.param == frozen.param)
            .expect("the frozen selection must be inside the enumerated space")
            .clone();
        (hit, cands)
    }

    /// Every arm cost printed in a receipt is re-run through the REAL
    /// coder with decode verification (the fast form must equal it).
    fn verify_with_real_coder(cand: &Cand, rate: u32, y: &[i16]) {
        let model = crate::Model {
            kind: cand.kind,
            param: cand.param,
        };
        if cand.kind == crate::MODEL_RAW {
            // the raw symbols are the varint-stream view; verify both arms
            let residuals = crate::predictor_residuals(y, crate::MODEL_RAW);
            let symbols: Vec<u64> = residuals.iter().map(|&v| crate::zigzag_encode(v)).collect();
            let (c1, _, d1) = crate::g17::rice_r1(&symbols);
            assert_eq!(d1, symbols, "RAW R1 decode");
            assert_eq!(fast_r1(&symbols).0, c1, "fast == real (RAW R1)");
            let (c2, d2) = crate::g17::rice_r2(&symbols);
            assert_eq!(d2, symbols, "RAW R2 decode");
            assert_eq!(fast_r2(&symbols), c2, "fast == real (RAW R2)");
            return;
        }
        let (_span, _mask, symbols) = crate::g17::ledger_span(model, rate, y);
        assert_eq!(
            crate::g17::rice_r0(&symbols),
            span_of(&symbols),
            "R0 identity"
        );
        let (c1, _, d1) = crate::g17::rice_r1(&symbols);
        assert_eq!(d1, symbols, "R1 decode");
        assert_eq!(fast_r1(&symbols).0, c1, "fast == real (R1)");
        let (c2, d2) = crate::g17::rice_r2(&symbols);
        assert_eq!(d2, symbols, "R2 decode");
        assert_eq!(fast_r2(&symbols), c2, "fast == real (R2)");
    }

    /// One mono stream's complete G21-B row: the frozen production row per
    /// backend, the exhaustive winners per backend, the ranking verdict
    /// (INVERSION when Rice re-selects the mathematics), and the
    /// joint-vs-sequential saving. The winners' arms already went through
    /// the real coder inside stream_enumerate's callees; verification for
    /// the printed winners happens here (bounded: 4 streams' worth).
    fn stream_report(name: &str, rate: u32, y: &[i16]) {
        let (frozen, cands) = frozen_cand(rate, y);
        // verify the frozen selection's arms (they carry the regression rows)
        verify_with_real_coder(&frozen, rate, y);
        let best_v = cands.iter().min_by_key(|c| c.r0_pay).expect("cands");
        let best_r = cands.iter().min_by_key(|c| c.rice_best()).expect("cands");
        let best_p = cands.iter().min_by_key(|c| c.portfolio).expect("cands");
        verify_with_real_coder(best_v, rate, y);
        verify_with_real_coder(best_r, rate, y);
        verify_with_real_coder(best_p, rate, y);
        let inv = if best_v.kind == best_r.kind && best_v.param == best_r.param {
            "-"
        } else {
            "INVERSION"
        };
        eprintln!(
            "  G21-B {name}: frozen {} varint={} r1={} r2={} portfolio={} | \
             R0-winner {} @ {} | Rice-winner {} @ {} {inv} | portfolio-winner {} @ {} | \
             frozen-vs-joint = {} B",
            frozen.tag(),
            frozen.r0_pay,
            frozen.r1,
            frozen.r2,
            frozen.portfolio,
            best_v.tag(),
            best_v.r0_pay,
            best_r.tag(),
            best_r.rice_best(),
            best_p.tag(),
            best_p.portfolio,
            frozen.portfolio - best_p.portfolio,
        );
    }

    // ---------- the fast forms == the real coder (unit gate) ----------

    #[test]
    fn g21_fast_forms_equal_the_real_coder() {
        // regimes: tiny magnitudes (Rice-optimal), mid, dense escapes,
        // long runs, and n crossing the 256-symbol R2 block edge
        let mut xr: u64 = 0x0BADBEEF5EED;
        let next = |xr: &mut u64| {
            *xr ^= *xr << 13;
            *xr ^= *xr >> 7;
            *xr ^= *xr << 17;
            *xr
        };
        for n in [1usize, 2, 33, 255, 256, 257, 700] {
            let mut syms: Vec<u64> = Vec::with_capacity(n);
            for i in 0..n {
                let raw = next(&mut xr);
                let v = match i % 7 {
                    0..=3 => raw % 9,
                    4..=5 => raw % (1 << 12),
                    _ => (raw & ((1u64 << 61) - 1)) | (1 << 40),
                };
                syms.push(v);
            }
            syms.push(0); // exact zero (the escape-adjacent special case)
            let (c1, _, d1) = crate::g17::rice_r1(&syms);
            assert_eq!(d1, syms, "R1 decode");
            assert_eq!(fast_r1(&syms).0, c1, "fast R1 == real R1 at n={n}");
            let (c2, d2) = crate::g17::rice_r2(&syms);
            assert_eq!(d2, syms, "R2 decode");
            assert_eq!(fast_r2(&syms), c2, "fast R2 == real R2 at n={n}");
        }
    }

    #[test]
    fn g21_production_space_is_fully_enumerated_on_a_tiny_fixture() {
        // a synthetic periodic-with-noise stream: every production kind is
        // applicable, the enumeration covers 9 kinds' spaces, the frozen
        // selection is found among the candidates, and the billing
        // identities hold (r0_pay == frame + span for non-RAW kinds)
        let n = 4_096usize;
        let y: Vec<i16> = (0..n)
            .map(|i| {
                let w = (i % 32) as i64 * 120 - 1_900;
                (w + if i % 512 == 0 { 3_000 } else { 0 }) as i16
            })
            .collect();
        let rate = 8_000u32;
        let (_, cands) = frozen_cand(rate, &y);
        // 128 = 5 dense + 95 PERIOD lags + 11 OSC variants + 16 OP arms;
        // up to a few may legitimately reject on a given stream (unstable
        // OSC fits, degenerate OP plans) — corpus runs are the authoritative
        // full scans, the fixture only proves coverage of the space
        assert!(
            cands.len() >= 124,
            "the production space must be enumerated: {} candidates",
            cands.len()
        );
        for c in &cands {
            if c.kind != crate::MODEL_RAW {
                let (_s, _m, syms) = crate::g17::ledger_span(
                    crate::Model {
                        kind: c.kind,
                        param: c.param,
                    },
                    rate,
                    &y,
                );
                assert_eq!(c.r0_pay, c.frame + span_of(&syms), "r0 == frame+span");
            }
        }
        // and the winners are verified by the real coder
        let best_r = cands.iter().min_by_key(|c| c.rice_best()).expect("cands");
        verify_with_real_coder(best_r, rate, &y);
        stream_report("fixture", rate, &y);
    }

    // ---------- G21-A/B corpus: stereo geometry x model x backend ----------
    //
    // REGRESSION-CONTROL BILLING NOTE: the transformed varint cells bill
    // the side stream's packed parity plane (G18-B's complete whole-stereo
    // convention, §8.23). The §8.24 G18-C matrix's varint column originally
    // omitted it — an incomplete-billing defect that this module's frozen
    // cells re-assert against the COMPLETE numbers: the corrected C arms
    // are independent on piano/birds (the §8.23 result) and c2 on river
    // (−8.6%, unchanged). The Rice (r1/r2) columns were complete throughout
    // and pin to §8.24 byte-for-byte.

    /// One geometry's transformed streams + overhead. The recorded G18
    /// tables bill the ms selector differently — §8.23's varint arms carry
    /// TWO selector bytes (geometry + side bundle), §8.24's rice columns
    /// carry ONE — so the frozen regression cells reproduce each table
    /// under its own convention; every NEW (exhaustive) measurement bills
    /// one selector byte per transformed geometry.
    struct Geom {
        name: &'static str,
        base: Vec<i16>,
        half: Vec<i16>,
        plane: usize,
        selector_v: usize, // varint-column selector bytes (§8.23 convention)
        selector_r: usize, // rice-column selector bytes (§8.24 convention)
    }

    fn geometry_list(l: &[i16], r: &[i16]) -> Vec<Geom> {
        let n = l.len();
        let mut m = Vec::with_capacity(n);
        let mut a_ms = Vec::with_capacity(n);
        let mut a_c2 = Vec::with_capacity(n);
        let mut a_c3 = Vec::with_capacity(n);
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
        let plane = n.div_ceil(8);
        // selector-byte convention inherited byte-for-byte from the G18-B
        // billing (§8.23): the ms arm bills TWO (the geometry selector plus
        // the side's half+parity bundle selector); c2/c3 bill ONE; indep
        // bills none. More billed than needed never overstates a win.
        vec![
            Geom {
                name: "indep",
                base: l.to_vec(),
                half: r.to_vec(),
                plane: 0,
                selector_v: 0,
                selector_r: 0,
            },
            Geom {
                name: "ms",
                base: m,
                half: a_ms,
                plane,
                selector_v: 2,
                selector_r: 1,
            },
            Geom {
                name: "c2",
                base: l.to_vec(),
                half: a_c2,
                plane,
                selector_v: 1,
                selector_r: 1,
            },
            Geom {
                name: "c3",
                base: r.to_vec(),
                half: a_c3,
                plane,
                selector_v: 1,
                selector_r: 1,
            },
        ]
    }

    /// The four backend cells for a stereo geometry. `frozen` = pass-1
    /// (the production selections ride the backend swap); `exhaustive` =
    /// each stream independently re-selects its own (kind, param) per
    /// backend — the G21 joint optimum within this geometry.
    struct StereoCells {
        frozen: [usize; 4], // varint, r1, r2, portfolio
        exh: [usize; 4],
        winner_tags: String, // the exhaustive per-backend selections
    }

    fn stereo_cells(rate: u32, g: &Geom) -> StereoCells {
        let overhead_v = g.selector_v + g.plane;
        let overhead_r = g.selector_r + g.plane;
        let (fb, base_cands) = frozen_cand(rate, &g.base);
        let (fh, half_cands) = frozen_cand(rate, &g.half);
        verify_with_real_coder(&fb, rate, &g.base);
        verify_with_real_coder(&fh, rate, &g.half);
        let frozen = [
            fb.r0_pay + fh.r0_pay + overhead_v,
            fb.r1 + fh.r1 + overhead_r,
            fb.r2 + fh.r2 + overhead_r,
            fb.portfolio + fh.portfolio + overhead_r,
        ];
        let take = |key: fn(&Cand) -> usize| -> (Cand, Cand) {
            let b = base_cands
                .iter()
                .min_by_key(|c| key(c))
                .expect("cands")
                .clone();
            let h = half_cands
                .iter()
                .min_by_key(|c| key(c))
                .expect("cands")
                .clone();
            (b, h)
        };
        let (vb, vh) = take(|c| c.r0_pay);
        let (rb1, rh1) = take(|c| c.r1);
        let (rb2, rh2) = take(|c| c.r2);
        let (pb, ph) = take(|c| c.portfolio);
        verify_with_real_coder(&rb1, rate, &g.base);
        verify_with_real_coder(&rh1, rate, &g.half);
        verify_with_real_coder(&pb, rate, &g.base);
        verify_with_real_coder(&ph, rate, &g.half);
        let exh = [
            vb.r0_pay + vh.r0_pay + overhead_r,
            rb1.r1 + rh1.r1 + overhead_r,
            rb2.r2 + rh2.r2 + overhead_r,
            pb.portfolio + ph.portfolio + overhead_r,
        ];
        let winner_tags = format!(
            "R0 {}|{}, R1 {}|{}, R2 {}|{}, portfolio {}|{}",
            vb.tag(),
            vh.tag(),
            rb1.tag(),
            rh1.tag(),
            rb2.tag(),
            rh2.tag(),
            pb.tag(),
            ph.tag(),
        );
        StereoCells {
            frozen,
            exh,
            winner_tags,
        }
    }

    /// The exhaustive reference for ONE stereo recording: the frozen G18-C
    /// regression rows (complete billing), the exhaustive joint matrix,
    /// L*(X), and the mono per-stream G21-B inversion reports.
    fn stereo_reference(
        class: &str,
        path: &str,
        frozen_varint: [usize; 4],
        rice_cols: [[usize; 4]; 2],
    ) {
        let data = std::fs::read(path).expect("corpus (run scripts/g12_fetch.sh)");
        let (rate, streams) = crate::parse_wav_multichannel(&data).expect("parse");
        let (l, r) = (&streams[0], &streams[1]);
        let geoms = geometry_list(l, r);
        eprintln!("G21-A {class} rate={rate}");
        let mut best_exh = (usize::MAX, usize::MAX, "");
        for g in &geoms {
            let sc = stereo_cells(rate, g);
            let gi = match g.name {
                "indep" => 0usize,
                "ms" => 1,
                "c2" => 2,
                _ => 3,
            };
            // pass-1 frozen regression controls: the varint column pins the
            // COMPLETE-billing G18-B arms; the rice columns pin §8.24
            assert_eq!(sc.frozen[0], frozen_varint[gi], "{class}/{}/varint", g.name);
            assert_eq!(sc.frozen[1], rice_cols[0][gi], "{class}/{}/r1", g.name);
            assert_eq!(sc.frozen[2], rice_cols[1][gi], "{class}/{}/r2", g.name);
            // the portfolio column pins min(r0, r1, r2) per stream, plane
            // included — the G18-C B/D convention
            eprintln!(
                "  {gname}: FROZEN v={v} r1={r1} r2={r2} p={p} | EXHAUSTIVE v={ev} r1={er1} r2={er2} p={ep} | {tags}",
                gname = g.name,
                v = sc.frozen[0],
                r1 = sc.frozen[1],
                r2 = sc.frozen[2],
                p = sc.frozen[3],
                ev = sc.exh[0],
                er1 = sc.exh[1],
                er2 = sc.exh[2],
                ep = sc.exh[3],
                tags = sc.winner_tags,
            );
            if sc.exh[3] < best_exh.0 {
                best_exh = (sc.exh[3], frozen_varint[gi], g.name);
            }
        }
        // the frozen production whole-stereo payload = the indep varint cell
        let production = frozen_varint[0];
        eprintln!(
            "  L*({class}) = {} B under {} (production {} B; joint saving {} B, {:.1}%)",
            best_exh.0,
            best_exh.2,
            production,
            production - best_exh.0,
            100.0 * (production - best_exh.0) as f64 / production as f64,
        );
        // per-stream mono reports on the independent channels
        stream_report(&format!("{class}-ch0"), rate, l);
        stream_report(&format!("{class}-ch1"), rate, r);
    }

    // ---------- corpus-gated G21-A/B entry points ----------

    /// The G21-A exhaustive reference + G21-B inversion rows for the piano
    /// class. Regression controls (asserted, not printed): the frozen
    /// varint cells re-pin the COMPLETE-billing G18-B whole-stereo arms
    /// (§8.23) and the frozen rice cells re-pin the G18-C rice matrix
    /// (§8.24) byte-for-byte.
    #[test]
    #[ignore] // corpus-gated: scripts/g12_fetch.sh
    fn g21a_exhaustive_reference_piano() {
        stereo_reference(
            "piano",
            "testdata/g12/emotional_piano.wav",
            [1_451_685, 1_516_941, 1_519_629, 1_528_832],
            [
                [1_168_606, 1_168_405, 1_190_275, 1_200_819],
                [1_126_048, 1_141_848, 1_162_722, 1_170_836],
            ],
        );
    }

    #[test]
    #[ignore] // corpus-gated: scripts/g12_fetch.sh
    fn g21a_exhaustive_reference_birds() {
        stereo_reference(
            "birds",
            "testdata/g12/park_birds.wav",
            [45_063_494, 46_610_404, 47_011_370, 46_846_787],
            [
                [36_922_321, 37_236_892, 37_967_345, 37_431_467],
                [35_966_797, 36_510_771, 37_478_174, 36_577_107],
            ],
        );
    }

    #[test]
    #[ignore] // corpus-gated: scripts/g12_fetch.sh
    fn g21a_exhaustive_reference_river() {
        stereo_reference(
            "river",
            "testdata/g12/park_river.wav",
            [72_247_619, 66_196_668, 66_003_381, 67_527_705],
            [
                [53_527_671, 53_028_479, 52_945_597, 53_450_834],
                [53_528_357, 52_885_636, 52_807_433, 53_412_144],
            ],
        );
    }

    /// The G21-B model-ranking inversion rows for the three mono speech
    /// holdouts (geometry is identity for mono; the enumeration is the
    /// full production candidate space under each backend).
    #[test]
    #[ignore] // corpus-gated: scripts/g5f_fetch.sh
    fn g21b_ranking_inversion_speech() {
        for (name, path) in [
            ("speech1", "testdata/g5f/example_word.wav"),
            ("speech2", "testdata/g5f/can_you_stop.wav"),
            ("speech3", "testdata/g5f/sample2.wav"),
        ] {
            let data = std::fs::read(path).expect("g5f corpus (run scripts/g5f_fetch.sh)");
            let (rate, streams) = crate::parse_wav_multichannel(&data).expect("parse");
            stream_report(name, rate, &streams[0]);
        }
    }
}
