//! G5-F — the real-recording harness (CC0 corpus, hash-locked; see
//! scripts/g5f_fetch.sh + testdata/g5f/). Diagnostic only.
//!
//! Per recording (whole-file AND fixed 4096-sample windows) x per channel
//! (no resampling, no mixdown — stereo stays separate streams):
//! 1. the complete portfolio's cost (encode()) — the standing winner
//! 2. the winning OSC model's stage-0 residual anatomy (G5's instruments)
//! 3. the residual-context readout: direct zigzag/RLE vs the D1-context
//!    coder on the same deltas (billed both sides — the G5-E finding)
//!
//! Outcomes compared: (a) residual predictability (Hcond/H0), (b) residual
//! coding-cost reduction, (c) improvement over the complete portfolio.

// The G5-F fixture window constant; used by the corpus readout test.
#![allow(dead_code)]

pub(crate) const G5F_WINDOW: usize = 4096;

#[cfg(test)]
mod tests {
    use crate::masks_rle::mask_rle_wire_len as rle_len;
    use crate::parse_wav_multichannel as parse;
    use crate::plan_corrections_masked_op as plan_ctx;
    use crate::varint_len;
    use crate::zigzag_encode;

    use crate::g5::analyze_f3;
    use crate::g5f::G5F_WINDOW as WINDOW;
    const G5F_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/g5f");

    fn channels(path: &str) -> (u32, Vec<Vec<i16>>) {
        let data = std::fs::read(path).expect("corpus file (run scripts/g5f_fetch.sh)");
        parse(&data).expect("parse")
    }

    /// The three-outcome G5-F readout over the locked corpus: whole-file +
    /// fixed windows, per channel, context-vs-direct billed both sides.
    #[test]
    fn g5f_real_corpus_readout() {
        let files = [
            ("example_word.wav", "stereo 44.1k, 0.9 s"),
            ("can_you_stop.wav", "mono 24k, 3.5 s"),
            ("sample2.wav", "stereo 44.1k, 9.3 s"),
        ];
        for (name, desc) in files {
            let path = format!("{G5F_DIR}/{name}");
            let (rate, streams_all) = channels(&path);
            for (ch, stream) in streams_all.iter().enumerate() {
                let wins = stream.len() / WINDOW;
                let mut port = 0usize;
                let mut coded = 0usize;
                let mut ctx_bytes = 0usize;
                let (mut h0s, mut hcs) = (0.0, 0.0);
                for w in 0..wins {
                    let seg = &stream[w * WINDOW..(w + 1) * WINDOW];
                    port += crate::encode(rate, seg).expect("e").len();
                    let stages = analyze_f3(seg, &[1]);
                    let Some(st) = stages.first() else { continue };
                    h0s += st.h0;
                    hcs += st.h_cond;
                    coded += st.coded_bits / 8;
                    let ctx: Vec<i64> = st.deltas.windows(2).map(|w| w[1] - w[0]).collect();
                    let (masks, deltas) = plan_ctx(&ctx);
                    ctx_bytes += rle_len(&masks)
                        + deltas
                            .iter()
                            .map(|&d| varint_len(zigzag_encode(d)))
                            .sum::<usize>();
                }
                let whole = crate::encode(rate, stream).expect("e").len();
                eprintln!(
                    "G5-F {name} ch{ch} ({desc}): whole={whole}B windows={wins} portfolio(windows)={port}B stage0 coded={coded}B context={ctx_bytes}B H0={:.2} Hcond={:.2} ratio={:.2}",
                    if wins > 0 { h0s / wins as f64 } else { 0.0 },
                    if wins > 0 { hcs / wins as f64 } else { 0.0 },
                    if wins > 0 { hcs / h0s } else { 0.0 },
                );
            }
        }
    }
}
