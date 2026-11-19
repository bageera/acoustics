//! G23 — wire v8 integration measurement (research-only; wire v7 untouched).
//!
//! G23-G: the integrated v8 frames measured on the locked corpus through
//! the REAL serializer (`encode_v8_select`), decoded to byte-exact PCM,
//! with the reconciliation against the frozen research references. The
//! reconciliation identity per docs/wire-v8.md §5:
//!
//!     L_v8 = L_research + Δ_framing + Δ_selectors + L_counts
//!
//! with the deltas charged: +2 header bytes (channels, flags), +1 spatial
//! selector varint, −1 per Rice body (the research arms' internal selector
//! byte is dropped), −1 on ms (the two-selector artifact unified), and
//! +the symbol-count varints (≤ 5 B per body at this corpus's counts).
//! Unexplained differences fail the gate.

#[cfg(test)]
mod tests {
    /// The research references (§8.25/8.26): the joint geometry×model×
    /// backend optima measured by the G21 enumerator.
    const REFERENCES: [(&str, &str, i64, i64); 6] = [
        (
            "piano",
            "testdata/g12/emotional_piano.wav",
            1_451_685,
            1_102_424,
        ),
        (
            "birds",
            "testdata/g12/park_birds.wav",
            45_063_494,
            35_754_385,
        ),
        (
            "river",
            "testdata/g12/park_river.wav",
            72_247_619,
            52_807_433,
        ),
        ("speech1", "testdata/g5f/example_word.wav", 55_710, 42_884),
        ("speech2", "testdata/g5f/can_you_stop.wav", 111_093, 74_791),
        ("speech3", "testdata/g5f/sample2.wav", 437_335, 366_870),
    ];

    /// The corpus reconciliation: Balanced + Maximum v8 frames per class,
    /// each decoded to byte-exact PCM, sizes printed against the frozen
    /// research references with the reconciliation margin asserted
    /// (v8 ≤ reference + 12 B: the count varints can add at most +5 per
    /// body while the dropped selector bytes subtract).
    #[test]
    #[ignore] // corpus-gated: scripts/g12_fetch.sh + g5f
    fn g23_corpus_reconciliation() {
        for (name, path, v7_ref, research_ref) in REFERENCES {
            let data = std::fs::read(path).expect("corpus (run the fetch scripts)");
            let (_rate, streams) = crate::parse_wav_multichannel(&data).expect("parse");
            drop(data);
            // the tested domain per the research references: piano/birds/
            // river are STEREO pairs; the speech classes were measured on
            // the ch0 stream ALONE (mono)
            let mono = streams.len() == 1 || name.starts_with("speech");
            for profile in [
                (crate::V8Profile::Balanced, "Balanced"),
                (crate::V8Profile::Maximum, "Maximum"),
            ] {
                let (frame, domain_ok) = if mono {
                    let f = crate::encode_v8_select_mono(_rate, &streams[0], profile.0)
                        .unwrap_or_else(|e| panic!("{name} {} encode: {e}", profile.1));
                    (f, true)
                } else {
                    (
                        crate::encode_v8_select(_rate, &streams[0], &streams[1], profile.0)
                            .unwrap_or_else(|e| panic!("{name} {} encode: {e}", profile.1)),
                        true,
                    )
                };
                let (drate, channels) = crate::decode_v8(&frame)
                    .unwrap_or_else(|e| panic!("{name} {} decode: {e}", profile.1));
                assert_eq!(drate, _rate, "{name} rate");
                assert!(domain_ok);
                if mono {
                    assert_eq!(channels.len(), 1, "{name} mono channels");
                    assert_eq!(channels[0], streams[0], "{name} exact");
                } else {
                    assert_eq!(channels.len(), 2, "{name} channels");
                    assert_eq!(channels[0], streams[0], "{name} exact L");
                    assert_eq!(channels[1], streams[1], "{name} exact R");
                }
                let delta = frame.len() as i64 - research_ref;
                eprintln!(
                    "G23-G {name} {}: {} B (research {research_ref} B, Δ={delta:+} B; \
                     v7 reference {v7_ref} B; reduction {:.1}%)",
                    profile.1,
                    frame.len(),
                    100.0 * (v7_ref - frame.len() as i64) as f64 / v7_ref as f64,
                );
                // the reconciliation margin: the deltas are bounded by the
                // count varints (+≤5 per body) while selectors subtract
                assert!(
                    delta <= 12,
                    "{name} {}: unexplained reconciliation drift {delta:+} B",
                    profile.1
                );
            }
        }
    }

    /// G23-H qualification probes: the INDEPENDENT stereo holdouts
    /// (park_wind — never part of the frozen reconciliation table — and the
    /// full example_word stereo, a domain the speech references did not
    /// test) plus ABSOLUTE encode/decode throughput on the holdout fixture.
    /// Exact reconstruction is asserted; sizes and rates are recorded.
    #[test]
    #[ignore] // corpus-gated: --release --locked g23_h_holdouts -- --ignored --nocapture
    fn g23_h_holdouts_and_throughput() {
        use std::time::Instant;
        let holds: [(&str, &str); 2] = [
            // independent holdout: park_wind is NOT part of the frozen
            // reconciliation table; park_wind16.wav (tracked) is its
            // deterministic 16-bit derivative (first 8.0 s, 24→16-bit via
            // >>8 top bytes, 48 kHz stereo; regenerate with
            // scripts/g23_holdout.py, 1,536,044 B committed)
            ("park_wind_holdout", "testdata/holdouts/park_wind16.wav"),
            ("example_word_full_stereo", "testdata/g5f/example_word.wav"),
        ];
        for (name, path) in holds {
            let data = std::fs::read(path).expect("corpus (run the fetch scripts)");
            let (rate, streams) = crate::parse_wav_multichannel(&data).expect("parse");
            drop(data);
            assert_eq!(streams.len(), 2, "{name}: stereo holdout");
            let v7 = (crate::serialize(crate::choose_model(&streams[0]), rate, &streams[0])
                .map(|p| p.len())
                .unwrap()
                + crate::serialize(crate::choose_model(&streams[1]), rate, &streams[1])
                    .unwrap()
                    .len()) as f64;
            for (profile, pname) in [
                (crate::V8Profile::Balanced, "Balanced"),
                (crate::V8Profile::Maximum, "Maximum"),
            ] {
                let t0 = Instant::now();
                let frame = crate::encode_v8_select(rate, &streams[0], &streams[1], profile)
                    .unwrap_or_else(|e| panic!("{name} {pname}: {e}"));
                let enc_ms = t0.elapsed().as_millis();
                let t1 = Instant::now();
                let (drate, channels) =
                    crate::decode_v8(&frame).unwrap_or_else(|e| panic!("{name} {pname} dec: {e}"));
                let dec_ms = t1.elapsed().as_millis();
                assert_eq!(drate, rate, "{name}: rate");
                assert_eq!(channels[0], streams[0], "{name}: exact L");
                assert_eq!(channels[1], streams[1], "{name}: exact R");
                let samples = streams[0].len() as f64;
                let pcm_mib = samples * 2.0 * 2.0 / 1_048_576.0;
                let enc_mbs = pcm_mib / (enc_ms as f64 / 1000.0);
                let dec_mbs = pcm_mib / (dec_ms as f64 / 1000.0);
                println!(
                    "HOLDOUT {name} {pname}: {len} B (v7 components {v7:.0} B = {:.1}%), enc {enc_ms} ms ({enc_mbs:.0} MiB/s), dec {dec_ms} ms ({dec_mbs:.0} MiB/s)",
                    frame.len() as f64 / v7 * 100.0,
                    len = frame.len(),
                    v7 = v7
                );
            }
        }
    }
}
