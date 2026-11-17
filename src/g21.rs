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

    /// Millisecond wall-clock helper (all timings are real elapsed).
    struct PhasesMs;
    impl PhasesMs {
        fn ms(d: std::time::Duration) -> f64 {
            d.as_secs_f64() * 1_000.0
        }
    }

    /// One stream's arm row: (arm tag with stream name, billed cost, ms).
    type ArmRow = (String, usize, f64);
    /// One geometry's full arm column plus its tag.
    type GeomRows = (String, Vec<ArmRow>);

    // ---------- G21-C: exact reuse (preflight) + the timed selection arms ----------
    //
    // Directive order: measure the enumeration's phase costs and eliminate
    // redundant computation (zero selection regret) BEFORE approximate
    // candidate screening. All timings below are REAL strategy executions;
    // nothing is derived from a time model.

    /// The reuse-path plan pass: ONE plan/residual scan feeding both the
    /// billing mirror and the symbol stream. Returns
    /// (meter_payload, res_span, symbols). OSC is the measured exception
    /// (certified meter + ledger pass both fit internally — the preflight
    /// records that duplication explicitly).
    fn plan_pass(kind: u8, param: usize, y: &[i16]) -> Option<(usize, usize, Vec<u64>)> {
        let (payload, res_span, symbols) = match kind {
            crate::MODEL_D1
            | crate::MODEL_D2
            | crate::MODEL_D3
            | crate::MODEL_D4
            | crate::MODEL_D5 => {
                let residuals = crate::predictor_residuals(y, kind);
                let symbols: Vec<u64> =
                    residuals.iter().map(|&v| crate::zigzag_encode(v)).collect();
                let span = span_of(&symbols);
                let len = crate::dense_len_from_residuals(kind, y.len(), &residuals);
                (len, span, symbols)
            }
            crate::MODEL_PERIOD => {
                let (residuals, masks) = crate::period_plan(y, param)?;
                let symbols: Vec<u64> = residuals[param..]
                    .iter()
                    .filter(|&&r| r != 0)
                    .map(|&r| crate::zigzag_encode(r))
                    .collect();
                let span = span_of(&symbols);
                let len = crate::period_len_from_plan(y, param, &(residuals, masks));
                (len, span, symbols)
            }
            crate::MODEL_OP => {
                let plan = crate::op_plan(y, param)?;
                let symbols: Vec<u64> = plan
                    .deltas
                    .iter()
                    .map(|&d| crate::zigzag_encode(d))
                    .collect();
                let span = span_of(&symbols);
                let len = crate::op_len_from_plan(param, &plan);
                (len, span, symbols)
            }
            crate::MODEL_OSC => {
                // single fit: the plan carries the meter cost AND the
                // emission deltas (the certified meter is osc_plan.cost)
                let plan = crate::osc_plan(y, param)?;
                let symbols: Vec<u64> = plan
                    .deltas
                    .iter()
                    .map(|&d| crate::zigzag_encode(d))
                    .collect();
                let span = span_of(&symbols);
                (plan.cost, span, symbols)
            }
            crate::MODEL_RAW => {
                let payload = crate::payload_len(kind, 0, y)?;
                let residuals = crate::predictor_residuals(y, kind);
                let symbols: Vec<u64> =
                    residuals.iter().map(|&v| crate::zigzag_encode(v)).collect();
                let span = span_of(&symbols);
                (payload, span, symbols)
            }
            _ => return None,
        };
        Some((payload, res_span, symbols))
    }

    /// The reuse-pass billing mirroring push_cand's meter semantics
    /// (r0_pay = the payload; r1/r2/portfolio per the module convention).
    fn cand_from_plan(kind: u8, param: usize, y: &[i16]) -> Option<(Cand, Vec<u64>)> {
        let (payload, res_span, symbols) = plan_pass(kind, param, y)?;
        let (f1, _, _) = fast_r1(&symbols);
        let f2 = fast_r2(&symbols);
        let frame = payload - res_span;
        Some((
            Cand {
                kind,
                param,
                frame,
                r0_pay: payload,
                r1: frame + 1 + f1,
                r2: frame + 1 + f2,
                portfolio: frame + 1 + res_span.min(f1).min(f2),
            },
            symbols,
        ))
    }

    #[test]
    fn g21c_reuse_mirrors_match_the_certified_meters() {
        // the mirrors are line-for-line copies; this is the per-class
        // equality check on a representative (kind, param) set, catching
        // any arithmetic drift both directions (meter or mirror)
        let n = 4_096usize;
        let y: Vec<i16> = (0..n)
            .map(|i| {
                let w = (i % 32) as i64 * 120 - 1_900;
                (w + if i % 512 == 0 { 3_000 } else { 0 }) as i16
            })
            .collect();
        let mut checked = 0usize;
        for (kind, params) in [
            (crate::MODEL_D1, vec![0usize]),
            (crate::MODEL_D2, vec![0]),
            (crate::MODEL_D3, vec![0]),
            (crate::MODEL_D4, vec![0]),
            (crate::MODEL_D5, vec![0]),
            (crate::MODEL_PERIOD, vec![2usize, 32, 96]),
            (crate::MODEL_OP, vec![0usize, 15]),
            (crate::MODEL_OSC, vec![0usize, 6]),
            (crate::MODEL_RAW, vec![0usize]),
        ] {
            for param in params {
                let meter = crate::payload_len(kind, param, &y);
                let Some(meter) = meter else {
                    // the model legitimately rejects this stream — the
                    // production search skips it; the mirror must agree
                    let rejected = plan_pass(kind, param, &y).is_none();
                    assert!(
                        rejected,
                        "meter rejected {kind}/{param} but the mirror accepted it"
                    );
                    continue;
                };
                let (payload, span, symbols) = plan_pass(kind, param, &y)
                    .unwrap_or_else(|| panic!("plan pass failed {kind}/{param}"));
                assert_eq!(payload, meter, "mirror == meter {kind}/{param}");
                let model = crate::Model { kind, param };
                let (ls_span, _m, ls_symbols) = crate::g17::ledger_span(model, 8_000, &y);
                if kind == crate::MODEL_RAW {
                    // RAW's ledger span is the varint stream even when the
                    // submode minimum billed literal: compare the span only
                    // when the stream submode won
                    if payload == crate::header_len(n) + 1 + ls_span {
                        assert_eq!(span, ls_span, "RAW span {kind}");
                        checked += 1;
                    } else {
                        checked += 1; // literal mode: span comparison skipped
                    }
                } else {
                    assert_eq!(span, ls_span, "span == ledger span {kind}/{param}");
                    assert_eq!(
                        symbols, ls_symbols,
                        "symbols == ledger symbols {kind}/{param}"
                    );
                    checked += 1;
                }
            }
        }
        assert!(checked >= 12, "mirror checks ran: {checked}");
    }

    /// The timed J arms for ONE stream (all real executions; the rice
    /// sweeps, plans and verifications that each strategy runs are the
    /// strategy's own elapsed wall clock). Returns rows of
    /// (arm, cost, ms). J3 is the reference.
    fn timed_j_arms(name: &str, y: &[i16], rows: &mut Vec<ArmRow>) {
        // J0 — frozen production (choose_model IS the production search)
        let a = std::time::Instant::now();
        let frozen = crate::choose_model(y);
        let j0_cost = crate::payload_len(frozen.kind, frozen.param, y).expect("frozen meter");
        let t_j0 = PhasesMs::ms(a.elapsed());

        // J1 — frozen predictors, then backend selection on that candidate
        let b = std::time::Instant::now();
        let (fc, _fsyms) = cand_from_plan(frozen.kind, frozen.param, y).expect("frozen cand");
        verify_with_real_coder(&fc, 0, y);
        let j1_cost = fc.portfolio;
        let t_j1 = PhasesMs::ms(b.elapsed());

        // J3 — the exhaustive joint enumeration (plan pass + rice per
        // candidate), winner verified through the real coder
        let c = std::time::Instant::now();
        let cands = enumerate_reuse(y);
        let j3_cost = cands.iter().map(|c2| c2.portfolio).min().expect("cands");
        let j3_win = cands
            .iter()
            .min_by_key(|c2| c2.portfolio)
            .expect("cands")
            .clone();
        verify_with_real_coder(&j3_win, 0, y);
        let t_j3 = PhasesMs::ms(c.elapsed());

        // J4-K — phase 1: the billing-only screen (plan pass, rice withheld,
        // ranked by the meter payload); phase 2: the shortlist (top-K plus
        // the mandated baseline) re-planned, exactly rice-billed, verified.
        // Screening by the varint metric is backend-blind by design — the
        // measured regret quantifies exactly that blindness (G21-B's
        // inversion lives here).
        let j4 = |k: usize| -> (usize, f64) {
            let s = std::time::Instant::now();
            let mut screen: Vec<(usize, u8, usize)> = Vec::new();
            for kind in [
                crate::MODEL_D1,
                crate::MODEL_D2,
                crate::MODEL_D3,
                crate::MODEL_D4,
                crate::MODEL_D5,
            ] {
                if let Some((payload, _span, _)) = plan_pass(kind, 0, y) {
                    screen.push((payload, kind, 0));
                }
            }
            for lag in 2..=96usize {
                if let Some((payload, _span, _)) = plan_pass(crate::MODEL_PERIOD, lag, y) {
                    screen.push((payload, crate::MODEL_PERIOD, lag));
                }
            }
            for v in 0..crate::OSC_VARIANT_COUNT {
                if let Some((payload, _span, _)) = plan_pass(crate::MODEL_OSC, v, y) {
                    screen.push((payload, crate::MODEL_OSC, v));
                }
            }
            for op in 0..crate::OP_ARM_COUNT {
                if let Some((payload, _span, _)) = plan_pass(crate::MODEL_OP, op, y) {
                    screen.push((payload, crate::MODEL_OP, op));
                }
            }
            if let Some((payload, _span, _)) = plan_pass(crate::MODEL_RAW, 0, y) {
                screen.push((payload, crate::MODEL_RAW, 0));
            }
            screen.sort_by_key(|&(pay, _, _)| pay);
            let mut best = usize::MAX;
            let mut bill = |cand: &Cand| {
                if cand.portfolio < best {
                    best = cand.portfolio;
                }
            };
            let mut baseline_billed = false;
            for &(pay, kind, param) in screen.iter().take(k) {
                let _ = pay;
                if kind == frozen.kind && param == frozen.param {
                    baseline_billed = true;
                }
                if let Some((cand, _)) = cand_from_plan(kind, param, y) {
                    bill(&cand);
                }
            }
            if !baseline_billed {
                // the mandated fully-billed baseline candidate
                bill(&fc);
            }
            (best, PhasesMs::ms(s.elapsed()))
        };
        let (j4a, t4a) = j4(4);
        let (j4b, t4b) = j4(8);
        let (j4c, t4c) = j4(16);
        let (j4d, t4d) = j4(32);

        for (arm, cost, ms) in [
            ("J0", j0_cost, t_j0),
            ("J1", j1_cost, t_j1),
            ("J3", j3_cost, t_j3),
            ("J4-A", j4a, t4a),
            ("J4-B", j4b, t4b),
            ("J4-C", j4c, t4c),
            ("J4-D", j4d, t4d),
        ] {
            rows.push((format!("{name} [{arm}]"), cost, ms));
        }
        // zero baseline regressions: every shortlist arm retains the frozen
        // baseline, so no arm may bill worse than J1
        for (cost, _) in [(j4a, t4a), (j4b, t4b), (j4c, t4c), (j4d, t4d)] {
            assert!(
                cost <= j1_cost,
                "baseline regression: J4 billed {cost} > J1 {j1_cost}"
            );
        }
    }

    /// The reuse-path enumeration (plan pass + rice per candidate) for J3.
    fn enumerate_reuse(y: &[i16]) -> Vec<Cand> {
        let mut cands = Vec::new();
        for kind in [
            crate::MODEL_D1,
            crate::MODEL_D2,
            crate::MODEL_D3,
            crate::MODEL_D4,
            crate::MODEL_D5,
        ] {
            if let Some((cand, _)) = cand_from_plan(kind, 0, y) {
                cands.push(cand);
            }
        }
        for lag in 2..=96usize {
            if let Some((cand, _)) = cand_from_plan(crate::MODEL_PERIOD, lag, y) {
                cands.push(cand);
            }
        }
        for v in 0..crate::OSC_VARIANT_COUNT {
            if let Some((cand, _)) = cand_from_plan(crate::MODEL_OSC, v, y) {
                cands.push(cand);
            }
        }
        for op in 0..crate::OP_ARM_COUNT {
            if let Some((cand, _)) = cand_from_plan(crate::MODEL_OP, op, y) {
                cands.push(cand);
            }
        }
        if let Some((cand, _)) = cand_from_plan(crate::MODEL_RAW, 0, y) {
            cands.push(cand);
        }
        cands
    }

    /// The class-level G21-C harness: per geometry the stream-level arms
    /// (summed over the two transformed streams) plus overhead costs; the
    /// stereo-level best per arm; the J2 geometry-first strategy; and the
    /// measured regret + speedup tables against J3.
    #[ignore] // corpus-gated: scripts/g12_fetch.sh + g5f
    #[test]
    fn g21c_selection_arms() {
        let _out_mark = 0usize;
        let classes = [
            ("piano", "testdata/g12/emotional_piano.wav", true),
            ("birds", "testdata/g12/park_birds.wav", true),
            ("river", "testdata/g12/park_river.wav", true),
            ("speech1", "testdata/g5f/example_word.wav", false),
            ("speech2", "testdata/g5f/can_you_stop.wav", false),
            ("speech3", "testdata/g5f/sample2.wav", false),
        ];
        for (name, path, stereo) in classes {
            // (geometry tag, base, half, plane, selector) — the reuse Geom
            type PairT = (String, Vec<i16>, Vec<i16>, usize, usize);
            let data = std::fs::read(path).expect("corpus (run the fetch scripts)");
            let (rate, streams) = crate::parse_wav_multichannel(&data).expect("parse");
            eprintln!("G21-C {name} rate={rate}");
            let pairs: Vec<PairT> = if stereo {
                let (l, r) = (&streams[0], &streams[1]);
                let geoms = geometry_list(l, r);
                geoms
                    .iter()
                    .map(|g| {
                        (
                            g.name.to_string(),
                            g.base.clone(),
                            g.half.clone(),
                            g.plane,
                            g.selector_r,
                        )
                    })
                    .collect()
            } else {
                vec![(
                    format!("{name}-mono"),
                    streams[0].clone(),
                    streams[0].clone(),
                    0,
                    0,
                )]
            };
            // per-geometry arm sums (cost and time), then stereo-level arms
            let mut g_rows: Vec<GeomRows> = Vec::new();
            #[allow(clippy::type_complexity)]
            let g_rows_ref = &mut g_rows;
            let _ = g_rows_ref;
            for (gname, base, half, plane, sel) in &pairs {
                let overhead = sel + plane;
                let mut rows: Vec<(String, usize, f64)> = Vec::new();
                let mut half_rows: Vec<(String, usize, f64)> = Vec::new();
                timed_j_arms(&format!("{gname}:base"), base, &mut rows);
                if stereo {
                    timed_j_arms(&format!("{gname}:half"), half, &mut half_rows);
                    for ((_, ca, ma), (_, cb, mb)) in rows.iter_mut().zip(half_rows.iter()) {
                        *ca += cb + overhead;
                        *ma += mb;
                    }
                } else {
                    let _ = half_rows; // mono: the pair IS one stream
                    let _ = overhead;
                }
                g_rows.push((gname.clone(), rows));
            }
            // J2 (geometry-first): stage-1 selects the geometry by the
            // frozen varint cell (the production metric); stage-2 runs the
            // joint selection (J3-equivalent) inside that geometry.
            let mut j2_geom = 0usize;
            {
                let mut best = usize::MAX;
                for (gi, (gname, base, half, _plane, sel)) in pairs.iter().enumerate() {
                    let fb = crate::choose_model(base);
                    let fh = crate::choose_model(half);
                    let pb = crate::payload_len(fb.kind, fb.param, base).expect("meter");
                    let ph2 = crate::payload_len(fh.kind, fh.param, half).expect("meter");
                    let cell = pb + ph2 + sel;
                    if cell < best {
                        best = cell;
                        j2_geom = gi;
                        let _ = gname;
                    }
                }
            }
            let mut best_rows = g_rows[0].1.clone();
            let arm_names = ["J0", "J1", "J3", "J4-A", "J4-B", "J4-C", "J4-D"];
            let mut winner_geoms = vec![g_rows[0].0.clone(); best_rows.len()];
            for (gi, (_, rows)) in g_rows.iter().enumerate().skip(1) {
                for (i, (_, cost, ms)) in rows.iter().enumerate() {
                    if *cost < best_rows[i].1 {
                        best_rows[i].1 = *cost;
                        best_rows[i].2 = *ms;
                        winner_geoms[i] = g_rows[gi].0.clone();
                    }
                }
            }
            let j3_row = best_rows
                .iter()
                .find(|r| r.0.ends_with("[J3]"))
                .expect("J3")
                .clone();
            let j2_row = g_rows[j2_geom]
                .1
                .iter()
                .find(|r| r.0.ends_with("[J3]"))
                .expect("J2")
                .clone();
            for (i, (arm, cost, ms)) in best_rows.iter().enumerate() {
                let kind = arm
                    .split(" [")
                    .nth(1)
                    .map(|s| s.trim_end_matches(']'))
                    .unwrap_or("?");
                let _ = kind;
                let regret = (*cost as i64) - (j3_row.1 as i64);
                let pct = 100.0 * regret as f64 / j3_row.1 as f64;
                let speed = j3_row.2 / ms;
                eprintln!(
                    "  JARM {}: [{}] cost={cost}B ms={ms:.0} regret={regret}B ({pct:.3}%) S={speed:.2}x",
                    arm_names.get(i).copied().unwrap_or("?"),
                    winner_geoms[i],
                );
            }
            eprintln!(
                "  J2(geometry-first): cost={}B ms={:.0} regret={}B | geometry={}",
                j2_row.1,
                j2_row.2,
                j2_row.1 as i64 - j3_row.1 as i64,
                pairs[j2_geom].0,
            );
        }
    }
}
