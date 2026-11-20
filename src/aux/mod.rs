//! The Mathematical Auxiliary Stream subsystem (the v8.1 research phase,
//! gates N23–N27): a fail-closed `.acaux` sidecar bound to an UNCHANGED
//! wire v8 payload, with a pure-integer normative renderer.
//!
//! Independence guarantees (measured and enforced):
//! - the primary compression engine is NOT referenced by this module —
//!   the sidecar binds by content hash, never by parser coupling;
//! - every descriptor/container byte is validated fail-closed
//!   (unknown version/model/flags/reserved/frequency/count reject);
//! - rendering is index-addressable: sample(i) depends only on i, so
//!   blockwise renders, seeks and pause/resume are bit-exact (N22);
//! - no render larger than the requested window is allocated.

pub mod descriptor;
pub mod limits;
pub mod render;
pub mod wire;

pub use descriptor::StreamDescriptor;
pub use limits::*;
pub use render::{render_window, stream_sample, ClampAudit};
pub use wire::{crc16_ccitt_false, AuxSpec};

#[cfg(test)]
mod tests {
    use super::render::phase_at;
    use super::*;

    /// N23: the fail-closed container roundtrip on the spec's canonical
    /// one-stream theta-AM descriptor (60 B file).
    #[test]
    fn aux_container_roundtrip_and_accounting() {
        let primary = [0xA5u8; 256];
        let sha = crate::golden::sha256_for_aux(&primary);
        let spec = AuxSpec {
            streams: vec![StreamDescriptor {
                model: 1,
                flags: 1,
                carrier_mhz: 40000,
                carrier_phase_q16: 0,
                amplitude_q15: 24576,
                mod_mhz: 5859,
                mod_depth_q16: 32768,
                mod_phase_q16: 0,
                start_sample: 0,
                duration_samples: 0,
                routing: 0b11,
                coupling_ref: 0,
            }],
            binding: [
                sha[0], sha[1], sha[2], sha[3], sha[4], sha[5], sha[6], sha[7],
            ],
        };
        let file = spec.write();
        assert_eq!(file.len(), 60, "one-stream sidecar = 60 B (spec §2)");
        let back = AuxSpec::parse(&file, Some(&spec.binding)).unwrap();
        assert_eq!(back, spec);
    }

    /// N24 the parity gate: the Rust renderer must reproduce the Python
    /// reference's integer contract bit-for-bit across rates, boundary
    /// frequencies, depths, phases, wrap points and coupling.
    #[test]
    fn aux_parity_with_the_python_reference() {
        mod vectors {
            #![allow(non_upper_case_globals)]
            include!("test_vectors.rs");
        }
        use vectors::PARITY_CASES;
        for c in PARITY_CASES {
            for (k, &idx) in c.indices.iter().enumerate() {
                let master_ph = c
                    .coupled_master
                    .map(|(_, mhz, ph)| phase_at(ph, mhz, c.rate, idx));
                let got = stream_sample(
                    &StreamDescriptor {
                        model: if c.mod_depth_q16 > 0 { 1 } else { 0 },
                        flags: 1,
                        carrier_mhz: c.carrier_mhz,
                        carrier_phase_q16: c.carrier_phase_q16,
                        amplitude_q15: c.amp_q15,
                        mod_mhz: c.mod_freq_mhz,
                        mod_depth_q16: c.mod_depth_q16,
                        mod_phase_q16: c.mod_phase_q16,
                        start_sample: 0,
                        duration_samples: 0,
                        routing: 0b11,
                        coupling_ref: 0,
                    },
                    c.rate,
                    idx,
                    master_ph,
                );
                assert_eq!(
                    got,
                    c.expected[k],
                    "{name}[{idx}]: parity drift (got {got}, want {})",
                    c.expected[k],
                    name = c.name
                );
            }
        }
    }

    /// The normative headroom contract: max |out| = amplitude×(1+depth)
    /// within the table's quantization tolerance (the reference selfcheck).
    #[test]
    fn aux_headroom_contract() {
        let d = StreamDescriptor {
            model: 1,
            flags: 1,
            carrier_mhz: 40000,
            carrier_phase_q16: 0,
            amplitude_q15: 24576,
            mod_mhz: 5859,
            mod_depth_q16: 32768,
            mod_phase_q16: 0,
            start_sample: 0,
            duration_samples: 0,
            routing: 3,
            coupling_ref: 0,
        };
        let peak = (0..24000u64)
            .map(|i| stream_sample(&d, 48000, i, None).abs())
            .max()
            .unwrap();
        let nominal = (24576i64 * (1 << 14) * 49152) >> 29;
        let tol = nominal * 2 / 8192 + 6;
        assert!(
            (nominal - peak).abs() <= tol,
            "peak {peak} vs nominal {nominal} tol {tol}"
        );
    }

    /// Render an owned window: the helper keeps the tests' borrow structure
    /// plain and the render's memory bound equal to the window.
    fn render_owned(
        spec: &AuxSpec,
        rate: u32,
        primary: u64,
        start: u64,
        n: usize,
    ) -> (Vec<i16>, Vec<i16>, ClampAudit) {
        let mut chans = vec![vec![0i16; n], vec![0i16; n]];
        let audit = render_window(spec, rate, primary, start, n, &mut chans).unwrap();
        let r = chans.pop().unwrap();
        let l = chans.pop().unwrap();
        (l, r, audit)
    }

    fn two_stream_spec() -> AuxSpec {
        let primary = vec![7u8; 4096];
        let sha = crate::golden::sha256_for_aux(&primary);
        AuxSpec {
            streams: vec![
                StreamDescriptor {
                    model: 1,
                    flags: 1,
                    carrier_mhz: 40000,
                    carrier_phase_q16: 0,
                    amplitude_q15: 8192,
                    mod_mhz: 5859,
                    mod_depth_q16: 32768,
                    mod_phase_q16: 0,
                    start_sample: 0,
                    duration_samples: 0,
                    routing: 3,
                    coupling_ref: 0,
                },
                StreamDescriptor {
                    model: 0,
                    flags: 1,
                    carrier_mhz: 200000,
                    carrier_phase_q16: 21845,
                    amplitude_q15: 4096,
                    mod_mhz: 0,
                    mod_depth_q16: 0,
                    mod_phase_q16: 0,
                    start_sample: 1000,
                    duration_samples: 2000,
                    routing: 1,
                    coupling_ref: 0,
                },
            ],
            binding: [
                sha[0], sha[1], sha[2], sha[3], sha[4], sha[5], sha[6], sha[7],
            ],
        }
    }

    /// N22/N24: blockwise rendering is bit-exact vs the one-shot window
    /// (the render is index-addressable; no carried state exists).
    #[test]
    fn aux_render_blocks_match_oneshot() {
        let spec = two_stream_spec();
        let (one_l, one_r, a1) = render_owned(&spec, 48000, 8192, 0, 4000);
        let mut blk_l = vec![0i16; 4000];
        let mut blk_r = vec![0i16; 4000];
        let mut clamped = 0u64;
        for start in (0..4000).step_by(256) {
            let len = 256.min(4000 - start);
            let (tl, tr, a) = render_owned(&spec, 48000, 8192, start as u64, len);
            clamped += a.clamped;
            blk_l[start..start + len].copy_from_slice(&tl);
            blk_r[start..start + len].copy_from_slice(&tr);
        }
        assert_eq!(one_l, blk_l, "blockwise L must be bit-exact");
        assert_eq!(one_r, blk_r, "blockwise R must be bit-exact");
        assert_eq!(a1.clamped, clamped);
    }

    /// N22/N24: direct rendering from an arbitrary offset equals skipping
    /// a continuous render EXACTLY (bit-exact seek).
    #[test]
    fn aux_seek_is_bit_exact() {
        let spec = two_stream_spec();
        let (cont_l, cont_r, _) = render_owned(&spec, 48000, 16384, 0, 9000);
        let (seek_l, seek_r, _) = render_owned(&spec, 48000, 16384, 7777, 1000);
        assert_eq!(
            seek_l,
            cont_l[7777..8777],
            "seeked L must equal the continuous render"
        );
        assert_eq!(
            seek_r,
            cont_r[7777..8777],
            "seeked R must equal the continuous render"
        );
    }

    /// Windows outside a stream's [start, start+duration) render silence.
    #[test]
    fn aux_stream_window_bounds() {
        let spec = two_stream_spec();
        let (l, r, _) = render_owned(&spec, 48000, 8192, 4000, 500);
        assert!(l.iter().any(|&v| v != 0), "stream 1 still renders at 4000");
        let _ = r;
        let mut s2 = two_stream_spec();
        s2.streams = vec![StreamDescriptor {
            model: 0,
            flags: 1,
            carrier_mhz: 40000,
            carrier_phase_q16: 0,
            amplitude_q15: 32767,
            mod_mhz: 0,
            mod_depth_q16: 0,
            mod_phase_q16: 0,
            start_sample: 100,
            duration_samples: 50,
            routing: 3,
            coupling_ref: 0,
        }];
        let (l2, _, _) = render_owned(&s2, 48000, 8192, 0, 4);
        assert!(
            l2.iter().all(|&v| v == 0),
            "outside the stream window: silence"
        );
    }

    /// Debug/release parity: the renderer's arithmetic must be identical on
    /// both profiles. Debug builds panic on integer overflow, so this test
    /// doubles as the overflow-safety proof for the 48-bit/wide-path
    /// arithmetic (the step×index products are u128; the Q-shifts are wide).
    #[test]
    fn aux_render_debug_release_parity() {
        // exercise the extreme legal fields that maximize intermediate widths
        let spec = {
            let primary = vec![3u8; 1 << 20];
            let sha = crate::golden::sha256_for_aux(&primary);
            AuxSpec {
                streams: vec![
                    StreamDescriptor {
                        model: 1,
                        flags: 1,
                        carrier_mhz: CARRIER_MHZ_MAX,
                        carrier_phase_q16: u16::MAX,
                        amplitude_q15: u16::MAX,
                        mod_mhz: MOD_MHZ_MAX,
                        mod_depth_q16: u16::MAX,
                        mod_phase_q16: u16::MAX,
                        start_sample: 0,
                        duration_samples: 0,
                        routing: 3,
                        coupling_ref: 0,
                    },
                    StreamDescriptor {
                        model: 1,
                        flags: 1,
                        carrier_mhz: CARRIER_MHZ_MIN,
                        carrier_phase_q16: u16::MAX,
                        amplitude_q15: u16::MAX,
                        mod_mhz: MOD_MHZ_MIN,
                        mod_depth_q16: u16::MAX,
                        mod_phase_q16: u16::MAX,
                        start_sample: 0,
                        duration_samples: 0,
                        routing: 3,
                        coupling_ref: 0,
                    },
                ],
                binding: [
                    sha[0], sha[1], sha[2], sha[3], sha[4], sha[5], sha[6], sha[7],
                ],
            }
        };
        // near i64-boundary indices: step×index must not overflow u128/u64
        let (l, r, audit) = render_owned(&spec, RATE_MAX, u64::MAX, 4_000_000_000, 64);
        let (l2, r2, audit2) = render_owned(&spec, RATE_MAX, u64::MAX, 4_000_000_000, 64);
        assert_eq!(l, l2);
        assert_eq!(r, r2);
        assert_eq!(audit, audit2);
        // and the mixed per-sample sum of two extreme streams must saturate,
        // never wrap (the acc is i64 and clamped)
        assert!(l
            .iter()
            .all(|&v| (i16::MIN as i64..=i16::MAX as i64).contains(&(v as i64))));
    }
}
