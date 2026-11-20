//! N26-A — the sidecar security battery. Structured mutations RECOMPUTE the
//! CRC-16 and the declared length so every semantic layer (descriptor
//! validation, coupling integrity, window bounds, render limits) is actually
//! exercised — without that, hostile inputs all die at the checksum boundary
//! and the deeper parser is never tested (the G23 lesson: a mutation found
//! an unchecked side-stack parity plane that the early battery had missed).

#![cfg(test)]

use super::descriptor::StreamDescriptor;
use super::wire::{crc16_ccitt_false, AuxSpec};

/// A canonical one-stream theta-AM gamma sidecar bound to `payload`.
fn canonical() -> (Vec<u8>, Vec<u8>, AuxSpec) {
    // the primary is a REAL wire v8 payload so verify_binding can read its dimensions
    let samples: Vec<i16> = (0..4096i64).map(|i| (i * 7 % 31_000) as i16).collect();
    let payload = crate::encode_v8_mono(
        48000,
        &samples,
        crate::ComponentSpec {
            model: crate::Model {
                kind: crate::MODEL_D1,
                param: 0,
            },
            backend: 1,
        },
    )
    .unwrap();
    let sha = crate::golden::sha256_for_aux(&payload);
    let mut binding = [0u8; 8];
    binding.copy_from_slice(&sha[..8]);
    let spec = AuxSpec {
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
        binding,
    };
    (spec.write(), payload, spec)
}

/// Rewrite the file's epilog (length + crc) after mutating stream bytes.
fn reseal(mut file: Vec<u8>) -> Vec<u8> {
    let n = file.len() - 6;
    let crc = crc16_ccitt_false(&file[..n]);
    let total = file.len() as u32;
    file[n..n + 4].copy_from_slice(&total.to_le_bytes());
    file[n + 4..].copy_from_slice(&crc.to_le_bytes());
    file
}

/// The canonical file WITHOUT the epilog resealing (an epilog-corrupting
/// mutation: byte flips inside streams).
fn broken_epilog(mut file: Vec<u8>) -> Vec<u8> {
    file[20] ^= 0x55; // mutate inside stream 1
    file
}

/// Replace stream `idx`'s fields via a patched descriptor.
fn patched_stream(idx: usize, patch: impl Fn(&mut StreamDescriptor)) -> Vec<u8> {
    let (file, _, spec) = canonical();
    let mut streams = spec.streams.clone();
    patch(&mut streams[idx]);
    let mut out = Vec::new();
    out.extend_from_slice(&file[..16]);
    for s in &streams {
        s.write(&mut out);
    }
    // re-add the epilog
    let crc = crc16_ccitt_false(&out[..16 + streams.len() * 38]);
    out.extend_from_slice(&((out.len() + 6) as u32).to_le_bytes());
    out.extend_from_slice(&crc.to_le_bytes());
    let _ = file;
    out
}

#[test]
fn n26_semantic_mutations_reject() {
    // every mutation RECOMPUTES the crc so it reaches the semantic parser
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("unknown model id 7", patched_stream(0, |s| s.model = 7)),
        ("unknown flag bits", patched_stream(0, |s| s.flags = 0x80)),
        (
            "carrier below 0.5 Hz",
            patched_stream(0, |s| s.carrier_mhz = 499),
        ),
        (
            "carrier above 20 kHz",
            patched_stream(0, |s| s.carrier_mhz = 20_000_001),
        ),
        (
            "model 0 with modulation fields",
            patched_stream(1, |s| {
                s.model = 0;
                s.mod_mhz = 5859;
            }),
        ),
        ("zero AM depth", patched_stream(0, |s| s.mod_depth_q16 = 0)),
        ("routing 0b00", patched_stream(0, |s| s.routing = 0)),
        (
            "routing with unknown bits",
            patched_stream(0, |s| s.routing = 0b111),
        ),
        (
            "mod freq out of bounds",
            patched_stream(0, |s| s.mod_mhz = 12_001),
        ),
        (
            "coupling to a non-existent stream",
            patched_stream(1, |s| s.coupling_ref = 9),
        ),
        (
            "coupling self-reference",
            patched_stream(0, |s| s.coupling_ref = 1),
        ),
    ];
    // the reserved fields live only in the WRITTEN bytes (write() always
    // emits zeros), so that mutation is byte-level: flip stream 1's
    // reserved and re-seal
    {
        let (mut f, _, _) = canonical();
        let pos = 16 + 38 + 36; // stream 1's reserved bytes
        f[pos] = 0xAA;
        let n = f.len() - 6;
        let crc = crc16_ccitt_false(&f[..n]);
        f[n + 4..].copy_from_slice(&crc.to_le_bytes());
        assert!(
            AuxSpec::parse(&f, None).is_err(),
            "nonzero reserved bytes must reject (byte-level, resealed)"
        );
    }
    for (name, f) in &cases {
        let r = AuxSpec::parse(f, None);
        assert!(r.is_err(), "mutation '{name}' must reject");
    }
}

#[test]
fn n26_integrity_layer_rejects() {
    // a REAL v8 payload: verify_binding must read the primary's dimensions
    let samples: Vec<i16> = (0..4096i64).map(|i| (i * 7 % 31_000) as i16).collect();
    let payload = crate::encode_v8_mono(
        48000,
        &samples,
        crate::ComponentSpec {
            model: crate::Model {
                kind: crate::MODEL_D1,
                param: 0,
            },
            backend: 1,
        },
    )
    .unwrap();
    // the canonical sidecar binds DIFFERENT bytes; rebind it to this payload
    let digest = crate::golden::sha256_for_aux(&payload);
    let (mut file, _old_payload, spec) = canonical();
    file[6..14].copy_from_slice(&digest[..8]);
    // recompute the crc after the binding patch
    let streams_end = file.len() - 6;
    let crc = crc16_ccitt_false(&file[..streams_end]);
    file[streams_end + 4..].copy_from_slice(&crc.to_le_bytes());

    // 1. crc16 mismatch (raw byte flip, unresealed)
    let broken = broken_epilog(file.clone());
    assert!(
        AuxSpec::parse(&broken, None).is_err(),
        "crc layer must catch"
    );
    // 2. declared-length mismatch
    let mut bad_len = file.clone();
    let len_pos = bad_len.len() - 6;
    bad_len[len_pos..len_pos + 4].copy_from_slice(&999u32.to_le_bytes());
    assert!(
        AuxSpec::parse(&bad_len, None).is_err(),
        "length mismatch rejects"
    );
    // 3. appended bytes (fail-closed termination)
    let mut appended = file.clone();
    appended.extend_from_slice(&[0u8; 16]);
    assert!(
        AuxSpec::parse(&appended, None).is_err(),
        "appended bytes reject"
    );
    // 4. every-truncation sweep must fail
    for cut in 0..file.len() {
        assert!(
            AuxSpec::parse(&file[..cut], None).is_err(),
            "truncation at {cut} must reject"
        );
    }
    // 5. a mismatched binding rejects at verification
    let wrong_payload = vec![0u8; 4096];
    let h = super::parse(&file).unwrap();
    assert!(
        h.verify_binding(&wrong_payload).is_err(),
        "binding mismatch rejects"
    );
    // 6. and the CORRECT payload verifies
    assert!(h.verify_binding(&payload).is_ok());
    let _ = spec;
}

#[test]
fn n26_window_and_work_limits() {
    let (file, payload, _) = canonical();
    let h = super::parse(&file).unwrap();
    let info = h
        .verify_binding(&payload)
        .expect("canonical binds a real v8 payload");
    // oversized caller buffers must REJECT, never slice-panic
    let short = vec![0i16; 8];
    assert!(super::render_window(
        h.spec(),
        info.rate,
        info.sample_count,
        0,
        1000,
        &mut [short.clone(), short],
    )
    .is_err());
    // streams past their start reject at validation...
    let bad = {
        let mut s = h.spec().clone();
        s.streams[0].start_sample = u64::MAX;
        let mut out = Vec::new();
        out.extend_from_slice(b"ACAU");
        out.extend_from_slice(&[1, 0]);
        out.extend_from_slice(&s.binding);
        out.push(s.streams.len() as u8);
        out.push(0);
        for st in &s.streams {
            st.write(&mut out);
        }
        let streams_end = out.len();
        out.extend_from_slice(&((out.len() + 6) as u32).to_le_bytes());
        out.extend_from_slice(&crc16_ccitt_false(&out[..streams_end]).to_le_bytes());
        out
    };
    // the offset bound is a PRIMARY-validation concern (spec §5): parse
    // succeeds, the validation rejects
    let s = AuxSpec::parse(&bad, None)
        .unwrap_or_else(|e| panic!("u64::MAX start parses, then validation rejects: {e}"));
    assert!(
        s.validate_against_primary(48000, 4096).is_err(),
        "u64::MAX start rejects against the primary's count"
    );
    // and a massive render request is bounded by the CALLER's buffer, so
    // the work limit is the window (no unbounded internal allocation)
    let big = vec![0i16; 1024];
    for _ in 0..8 {
        super::render_window(
            h.spec(),
            info.rate,
            info.sample_count,
            0,
            1024,
            &mut [big.clone(), big.clone()],
        )
        .unwrap();
    }
    let _ = reseal; // the helper stays for future mutation rounds
}

#[test]
fn n26_coupling_forward_only() {
    // stream 1 coupled to stream 2 (a BACKWARD reference in stream order =
    // forward-only violated) must reject
    let (_, _, spec) = canonical();
    let mut s = spec.clone();
    s.streams[0].model = 1;
    s.streams[0].coupling_ref = 2; // stream 1 references stream 2: NOT forward-only
    let f = s.write();
    assert!(
        AuxSpec::parse(&f, None).is_err(),
        "non-forward coupling must reject (cycle prevention)"
    );
}
