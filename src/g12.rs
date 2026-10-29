//! G12 — acoustic diversity corpus (research-only; wire untouched).
//!
//! The review's three questions per acoustic class:
//! 1. Which model wins? (per-model selection frequencies over the class)
//! 2. Does the residual-information distribution change with class?
//!    (G9 anatomy + G11 entropy diagnostics re-run per class)
//! 3. Does a genuine opportunity emerge (a class-specific gap big enough
//!    to justify a NEW targeted experiment)?
//!
//! Corpus (hash-locked via scripts/g12_fetch.sh; testdata/g12 gitignored —
//! 262 MB — the script + recorded hashes are the reproducible recipe):
//! - emotional_piano.wav: 16-bit stereo 44.1k, ~15 s (overlapping musical
//!   events; sustained tones with attack transients)
//! - park_birds.wav / park_river.wav: 16-bit stereo 48k, ~2.5-3 min each
//!   (stochastic environments / recurring natural sounds)
//! - park_wind.wav: 24-bit — ORIGINAL PRESERVED, engine measurement DEFERRED
//!   (the review's rule: never silently convert)
//! Speech controls: the locked G5-F corpus.
//!
//! Memory discipline: whole-file encode only where it fits (piano); the long
//! field recordings measured in fixed 4096 windows (as G5-F) + per-channel.

use crate::g11::{h0, hcond1};
use crate::{choose_model, encode, predictor_residuals, MODEL_D1, MODEL_D2, MODEL_PERIOD};

const WINDOW: usize = 4096;

#[cfg(test)]
mod tests {
    use super::*;

    fn g12_dir() -> String {
        concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/g12").to_string()
    }

    /// The review's Q1+Q2: per-model selection frequencies, exact-PCM
    /// benchmark table, and residual entropy diagnostics on each class.
    #[test]
    #[ignore] // runs only with the hash-locked corpus: scripts/g12_fetch.sh
    fn g12_acoustic_diversity_benchmark() {
        let dir = g12_dir();
        let files = [
            ("emotional_piano.wav", "instrumental/music", true),
            ("park_birds.wav", "environmental/stochastic", false),
            ("park_river.wav", "environmental/stochastic", false),
        ];
        for (name, class, whole_ok) in files {
            let data =
                std::fs::read(format!("{dir}/{name}")).expect("run scripts/g12_fetch.sh first");
            let (rate, streams) = crate::parse_wav_multichannel(&data).expect("parse");
            for (ch, y) in streams.iter().enumerate() {
                // bounded representative sample: first 200 windows/channel
                let wins = (y.len() / WINDOW).min(200);
                let (mut pcm, mut port) = (0usize, 0usize);
                let mut kinds: std::collections::HashMap<u8, usize> =
                    std::collections::HashMap::new();
                let (mut h0s, mut hcs, mut actuals) = (0f64, 0f64, 0usize);
                for w in 0..wins {
                    let seg = &y[w * WINDOW..(w + 1) * WINDOW];
                    pcm += seg.len() * 2;
                    let model = choose_model(seg);
                    *kinds.entry(model.kind).or_insert(0) += 1;
                    let payload = encode(rate, seg).expect("e");
                    port += payload.len();
                    // residual diagnostics on the WINNING stream
                    if model.kind == MODEL_D1
                        || model.kind == MODEL_D2
                        || model.kind == MODEL_PERIOD
                    {
                        let r = predictor_residuals(seg, model.kind);
                        let sy: Vec<u64> = r.iter().map(|&v| crate::zigzag_encode(v)).collect();
                        h0s += h0(&sy) * sy.len() as f64;
                        hcs += hcond1(&sy) * sy.len().saturating_sub(1) as f64;
                        actuals += payload.len() - 17;
                    }
                }
                let h0b = (h0s / 8.0).ceil() as usize;
                let hcb = (hcs / 8.0).ceil() as usize;
                eprintln!(
                    "G12 {name} ch{ch} ({class}): windows={wins} pcm={pcm}B portfolio={port}B ({:.1}% of PCM) | kinds={kinds:?} | H0={h0b}B Hcond={hcb}B actual-res~{actuals}B",
                    100.0 * port as f64 / pcm.max(1) as f64
                );
                // whole-file form where memory allows
                if whole_ok {
                    let model = choose_model(y);
                    let payload = encode(rate, y).expect("whole");
                    let pcm_w = y.len() * 2;
                    eprintln!(
                        "G12 {name} ch{ch} WHOLE (kind={}): pcm={pcm_w}B payload={}B ({:.1}%)",
                        model.kind,
                        payload.len(),
                        100.0 * payload.len() as f64 / pcm_w as f64
                    );
                }
            }
        }
        // speech control rows (locked G5-F corpus, same instruments)
        let d0 = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/g5f");
        for name in ["can_you_stop.wav", "sample2.wav"] {
            let data = std::fs::read(format!("{d0}/{name}")).expect("g5f corpus");
            let (rate, streams) = crate::parse_wav_multichannel(&data).expect("parse");
            for (ch, y) in streams.iter().enumerate() {
                let wins = (y.len() / WINDOW).min(20);
                let (mut kinds, mut port, mut pcm) =
                    (std::collections::HashMap::new(), 0usize, 0usize);
                for w in 0..wins {
                    let seg = &y[w * WINDOW..(w + 1) * WINDOW];
                    pcm += seg.len() * 2;
                    let model = choose_model(seg);
                    *kinds.entry(model.kind).or_insert(0) += 1;
                    port += encode(rate, seg).expect("e").len();
                }
                eprintln!(
                    "G12 (speech control) {name} ch{ch}: windows={wins} pcm={pcm}B portfolio={port}B ({:.1}%) kinds={kinds:?}",
                    100.0 * port as f64 / pcm.max(1) as f64
                );
            }
        }
    }

    /// The exact reconstruction gate on the whole-file piano stream.
    #[test]
    #[ignore]
    fn g12_exact_reconstruction_piano() {
        let dir = g12_dir();
        let data = std::fs::read(format!("{dir}/emotional_piano.wav")).expect("g12 corpus");
        let (rate, streams) = crate::parse_wav_multichannel(&data).expect("parse");
        let payload = encode(rate, &streams[0]).expect("encode");
        let (_, back, _) = crate::decode_payload(&payload).expect("decode");
        assert_eq!(back, streams[0], "music class must reconstruct exactly");
    }
}
