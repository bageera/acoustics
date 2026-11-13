//! G16 — the v7 four-arm gate (research-only; corpus-gated).
//!
//! The combined D5 (kind 8) + adaptive-mask authorization's experiment, with
//! attribution preserved:
//!   A = frozen v6 (fixed 4-B mask words, kinds 0..=7)
//!   B = A + D5 (isolates D5's contribution)
//!   C = v7 mask layer over the frozen-v6 kind set (isolates G10's share)
//!   D = C + D5 = the proposed v7 (both layers)
//! Every arm bills ACTUAL serialized byte counts (`research::arm_costs`,
//! metric == serialize enforced by the invariant tests). OP keeps its
//! original always-RLE mask sections (no selector); dense kinds carry no
//! mask section — their bytes are identical in every arm.
//!
//! Release condition (the authorization): Arm D must beat Arm A on the
//! piano class (held-out, both partitions) and on the PERIOD-2 speech
//! target, with NO unexpected whole-payload regression on the other classes.

#[cfg(test)]
mod tests {
    use crate::arm_costs;

    fn partition_sums(name: &str, y: &[i16]) -> ((usize, usize), (usize, usize)) {
        let wins = (y.len() / 4096).min(200);
        let (mut a_dec, mut d_dec, mut a_ev, mut d_ev) = (0usize, 0usize, 0usize, 0usize);
        for w in 0..wins / 2 {
            let (a, _b, _c, d) = arm_costs(&y[w * 4096..(w + 1) * 4096]);
            a_dec += a;
            d_dec += d;
        }
        for w in wins / 2..wins {
            let (a, _b, _c, d) = arm_costs(&y[w * 4096..(w + 1) * 4096]);
            a_ev += a;
            d_ev += d;
        }
        eprintln!(
            "G16 {name}: decide A={a_dec} D={d_dec} (D-vs-A {:+} B) | eval A={a_ev} D={d_ev} (D-vs-A {:+} B)",
            d_dec as i64 - a_dec as i64,
            d_ev as i64 - a_ev as i64
        );
        ((a_dec, d_dec), (a_ev, d_ev))
    }

    /// The release gate: Arm D beats Arm A on piano (both partitions), the
    /// mask win shows on the PERIOD-2 speech payload, and NO class regresses
    /// beyond the selector's own +1 B per dense mask segment.
    #[test]
    #[ignore] // corpus-gated: scripts/g12_fetch.sh + testdata/g5f
    fn g16_four_arm_v7_gate() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/g12");
        let data = std::fs::read(format!("{dir}/emotional_piano.wav")).expect("g12_fetch");
        let (_rate, streams) = crate::parse_wav_multichannel(&data).expect("parse");
        for (ch, y) in streams.iter().enumerate() {
            let (a, b, c, d) = arm_costs(y);
            eprintln!(
                "G16 piano ch{ch} WHOLE: A={a} B={b} C={c} D={d} | D-vs-A {:+} B | B-vs-A (D5) {:+} | C-vs-A (masks) {:+}",
                d as i64 - a as i64,
                b as i64 - a as i64,
                c as i64 - a as i64
            );
            partition_sums(&format!("piano ch{ch}"), y);
            let ((a_dec, d_dec), (a_ev, d_ev)) = partition_sums(&format!("piano ch{ch} sum"), y);
            assert!(
                d_dec < a_dec && d_ev < a_ev,
                "release condition: Arm D must beat frozen v6 on piano held-out (both partitions)"
            );
        }
        for name in ["park_birds.wav", "park_river.wav"] {
            let data = std::fs::read(format!("{dir}/{name}")).expect("g12_fetch");
            let (_rate, streams) = crate::parse_wav_multichannel(&data).expect("parse");
            for (ch, y) in streams.iter().enumerate() {
                let (a, b, c, d) = arm_costs(y);
                eprintln!(
                    "G16 {name} ch{ch} WHOLE: A={a} B={b} C={c} D={d} | D-vs-A {:+} B",
                    d as i64 - a as i64
                );
                assert!(
                    d <= a + a / 1000,
                    "{name} ch{ch}: no whole-payload regression beyond ~0.1%"
                );
                partition_sums(&format!("{name} ch{ch}"), y);
            }
        }
        let sdir = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/g5f");
        let mut mask_wins = 0usize;
        for name in ["example_word.wav", "can_you_stop.wav", "sample2.wav"] {
            let data = std::fs::read(format!("{sdir}/{name}")).expect("g5f corpus");
            let (rate, streams) = crate::parse_wav_multichannel(&data).expect("parse");
            for (ch, y) in streams.iter().enumerate() {
                let (a, _b, _c, d) = arm_costs(y);
                let model = crate::choose_model(y);
                eprintln!(
                    "G16 speech {name} ch{ch} WHOLE: A={a} D={d} (D-vs-A {:+} B) selected kind={} param={} (rate {rate})",
                    d as i64 - a as i64,
                    model.kind,
                    model.param
                );
                if d < a {
                    mask_wins += 1;
                }
                assert!(
                    d <= a + a / 1000,
                    "{name} ch{ch}: no speech regression beyond ~0.1%"
                );
            }
        }
        assert!(
            mask_wins >= 1,
            "the mask layer (Arm D) must win at least one held speech payload (G10's 2.5% PERIOD-2 anchor)"
        );
        let mut xr = 0xAB1DE_u64;
        let noise: Vec<i16> = (0..4096)
            .map(|_| {
                xr ^= xr << 13;
                xr ^= xr >> 7;
                xr ^= xr << 17;
                (((xr >> 33) % 65536) as i32 - 32768) as i16
            })
            .collect();
        let (a, _b, _c, d) = arm_costs(&noise);
        eprintln!(
            "G16 noise: A={a} D={d} (D-vs-A {:+} B)",
            d as i64 - a as i64
        );
        assert!(d <= a + a / 1000, "noise must not regress");
        let smooth: Vec<i16> = (0..4096usize)
            .map(|i| {
                let t = i as f64 * (0.25 / 4096.0);
                let w = 220.0 * std::f64::consts::TAU * t;
                let body = 3000.0 * w.sin() + 1200.0 * (2.0 * w).sin();
                let ampl = 0.98 + 0.02 * (6.0 * std::f64::consts::TAU * t).sin();
                (body * ampl).round() as i16
            })
            .collect();
        let (a, _b, _c, d) = arm_costs(&smooth);
        eprintln!(
            "G16 smooth synthetic: A={a} D={d} (D-vs-A {:+} B)",
            d as i64 - a as i64
        );
        assert!(d <= a + a / 1000, "synthetic smooth must not regress");
    }
}
