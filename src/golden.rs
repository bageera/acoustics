//! G23-E — permanent golden vectors for wire v8 (+ the historical pin).
//!
//! Every committed fixture carries the exact wire bytes, wire and decoded
//! PCM SHA-256 digests, the expected model kind/param and backend
//! identifiers, and the exact serialized length. The battery decodes each
//! fixture through the public decoder and asserts bit-exact reconstruction;
//! golden correctness therefore must not depend on whether a newer encoder
//! would select the same representation (the vector's spec is explicit; the
//! decoder grammar is the contract).
//!
//! Fixtures are built by pure integer arithmetic (no floats) so the vectors
//! are reproducible on every target architecture.

#![cfg(test)]

// ---------------------------------------------------------------- sha256 --
// Compact std-only SHA-256 (no external digest dependency).
const K256: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut msg = data.to_vec();
    let bitlen = (data.len() as u64) * 8;
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bitlen.to_be_bytes());
    for block in msg.chunks(64) {
        let mut w = [0u32; 64];
        for (i, chunk) in block.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d) = (h[0], h[1], h[2], h[3]);
        let (mut e, mut f, mut g, mut hh) = (h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K256[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    let mut out = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

fn sha256_hex(data: &[u8]) -> String {
    sha256(data).iter().map(|b| format!("{b:02x}")).collect()
}

/// The sha256 exposed for the aux subsystem's binding tests (cfg(test) only).
#[cfg(test)]
pub(crate) fn sha256_for_aux(data: &[u8]) -> [u8; 32] {
    sha256(data)
}

// ------------------------------------------------------- fixture builders --

/// LCG noise (64-bit linear congruential generator, fixed seed).
fn fx_noise(n: usize) -> Vec<i16> {
    let (mut x, mut out) = (1234567u64, Vec::with_capacity(n));
    for _ in 0..n {
        x = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        out.push((x >> 33) as u16 as i16);
    }
    out
}

/// Integer triangle wave.
fn fx_triangle(n: usize) -> Vec<i16> {
    (0..n)
        .map(|i| {
            let t = (i * 5) % 64;
            let v = if t < 32 {
                (t as i64) * 180 - 2_880
            } else {
                (64 - t as i64) * 180 - 2_880
            };
            v as i16
        })
        .collect()
}

/// Integer echo of the triangle (delay 37, gain 55/128).
fn fx_echo(n: usize) -> Vec<i16> {
    let dry = fx_triangle(n);
    (0..n)
        .map(|i| {
            let d = if i >= 37 {
                (dry[i - 37] as i64 * 55) >> 7
            } else {
                0
            };
            (dry[i] as i64 + d) as i16
        })
        .collect()
}

/// Impulse train with wide residuals (Rice escape paths).
fn fx_jumps(n: usize) -> Vec<i16> {
    let mut out = vec![0i16; n];
    let mut i = 0;
    while i < n {
        out[i] = if (i / 97) % 2 == 0 { 31_000 } else { -31_000 };
        i += 97;
        if i < n {
            out[i] = 17_000;
            i += 61;
        }
    }
    out
}

/// Extreme corners.
fn fx_extreme(n: usize) -> Vec<i16> {
    (0..n)
        .map(|i| if i % 2 == 0 { i16::MIN } else { i16::MAX })
        .collect()
}

// --------------------------------------------------------- golden registry --

pub(crate) struct Golden {
    pub name: &'static str,
    pub rate: u32,
    pub channels: u8,
    pub kind: u8,
    pub model_param: usize,
    pub backend: u8,
    pub wire: &'static [u8],
    pub wire_sha256: &'static str,
    pub pcm_sha256: &'static str,
    pub wire_len: usize,
}

// The generated table (#[ignore] fn golden_generate prints the rows; the
// generator decode-verifies every byte BEFORE printing).
include!("golden_table.rs");

/// The battery: every golden decodes bit-exact through the public decoder;
/// lengths and digests reconcile.
#[test]
fn golden_vectors_decode_bit_exact() {
    for g in GOLDEN_VECTORS {
        let (rate, channels) =
            crate::decode_v8(g.wire).unwrap_or_else(|e| panic!("{}: v8 decode: {e}", g.name));
        assert_eq!(rate, g.rate, "{}: rate", g.name);
        assert_eq!(
            channels.len() as u8,
            g.channels,
            "{}: channel count",
            g.name
        );
        assert_eq!(
            g.wire.len(),
            g.wire_len,
            "{}: exact serialized length",
            g.name
        );
        let wire_hex = sha256_hex(g.wire);
        assert_eq!(wire_hex, g.wire_sha256, "{}: wire digest", g.name);
        let mut pcm = Vec::new();
        for ch in &channels {
            pcm.extend_from_slice(pcm_bytes(ch));
        }
        let pcm_hex = sha256_hex(&pcm);
        assert_eq!(pcm_hex, g.pcm_sha256, "{}: decoded PCM digest", g.name);

        // identifier reconciliation: walk the header and the FIRST
        // component stack, asserting the embedded identifiers
        let w = g.wire;
        assert_eq!(&w[0..8], crate::MAGIC, "{}: magic", g.name);
        assert_eq!(w[8], crate::V8_VERSION, "{}: version byte", g.name);
        assert_eq!(w[9], g.channels, "{}: channels byte", g.name);
        assert_eq!(w[10], crate::V8_FLAG_NONE, "{}: flags byte", g.name);
        let mut pos = 15usize;
        let _count = varint_at(w, &mut pos);
        if g.channels == 2 {
            let geo = varint_at(w, &mut pos);
            assert_eq!(geo as usize, g.model_param, "{}: geometry", g.name);
        }
        assert_eq!(w[pos], g.kind, "{}: first component kind", g.name);
        let mut cpos = pos + 1;
        let _res_span = varint_at(w, &mut cpos);
        let _body_len = varint_at(w, &mut cpos);
        let fields_len = varint_at(w, &mut cpos);
        pos = cpos + fields_len as usize;
        assert_eq!(w[pos], g.backend, "{}: first component backend", g.name);
    }
}

fn varint_at(w: &[u8], pos: &mut usize) -> u64 {
    let mut v: u64 = 0;
    let mut shift = 0u32;
    loop {
        let b = w[*pos];
        *pos += 1;
        v |= ((b & 0x7F) as u64) << shift;
        if b & 0x80 == 0 {
            break;
        }
        shift += 7;
    }
    v
}

/// The historical pin: the production v7 reader decodes its own released
/// grammar; version bytes 4/5 reject under the single-version-release
/// advancement documented in docs/wire-v8.md §5.
#[test]
fn golden_historical_dispatch() {
    let samples = fx_triangle(256);
    let payload = crate::serialize(crate::Model { kind: 2, param: 0 }, 8_000, &samples).unwrap();
    assert_eq!(payload[8], crate::VERSION);
    let (rate, back, model) = crate::decode_payload(&payload).unwrap();
    assert_eq!(rate, 8_000);
    assert_eq!(back, samples);
    assert_eq!(model.kind, 2);
    for old in [4u8, 5u8] {
        let mut older = payload.clone();
        older[8] = old;
        assert!(
            crate::decode_payload(&older).is_err(),
            "VERSION {old} rejects"
        );
    }
}

// ------------------------------------------------------------- generator --

/// One-shot fixture generator (`--ignored --nocapture`): prints the literal
/// const decls + registry rows for golden_table.rs. Every candidate is
/// decode-verified BEFORE printing — only proven bytes reach the tree.
#[test]
#[ignore]
fn golden_generate() {
    let rate = 8_000u32;

    struct Row {
        name: String,
        rate: u32,
        channels: u8,
        kind: u8,
        param: usize,
        backend: u8,
        pcm: Vec<i16>,
        wire: Vec<u8>,
    }

    let mut rows: Vec<Row> = Vec::new();

    // ---- mono: all kinds + the D ladder under every applicable backend +
    // Rice/parity/empty-body boundary cases
    let mono: Vec<(&str, u8, usize, Vec<i16>, u8)> = vec![
        ("raw_r0_noise", crate::MODEL_RAW, 0, fx_noise(512), 0),
        ("d1_r1_ramp", crate::MODEL_D1, 0, fx_triangle(1_024), 1),
        ("d2_r1", crate::MODEL_D2, 0, fx_triangle(1_024), 1),
        ("d3_r2", crate::MODEL_D3, 0, fx_triangle(1_024), 2),
        ("d4_r1", crate::MODEL_D4, 0, fx_triangle(1_024), 1),
        ("d5_r2", crate::MODEL_D5, 0, fx_triangle(1_024), 2),
        ("d3_r1_escapes", crate::MODEL_D3, 0, fx_jumps(1_100), 1),
        ("d1_r0_extreme", crate::MODEL_D1, 0, fx_extreme(64), 0),
        ("period_r1", crate::MODEL_PERIOD, 32, fx_triangle(1_024), 1),
        (
            "period_r2_blocky",
            crate::MODEL_PERIOD,
            33,
            fx_echo(3 * 256 + 17),
            2,
        ),
        (
            "period_r1_empty_body",
            crate::MODEL_PERIOD,
            32,
            vec![100i16; 96],
            1,
        ),
        ("op_echo_r1", crate::MODEL_OP, 10, fx_echo(400), 1),
        ("op_loop_r1", crate::MODEL_OP, 0, fx_triangle(402), 1),
    ];
    for (name, kind, param, samples, backend) in mono {
        let spec = crate::ComponentSpec {
            model: crate::Model { kind, param },
            backend,
        };
        let wire = crate::encode_v8_mono(rate, &samples, spec).unwrap();
        crate::decode_v8(&wire).unwrap_or_else(|e| panic!("{name}: {e}"));
        rows.push(Row {
            name: name.to_string(),
            rate,
            channels: 1,
            kind,
            param,
            backend,
            pcm: samples,
            wire,
        });
    }

    // ---- stereo: C0–C3 × backends + the mixed-MODEL stream
    // (name, geo, L kind, L param, L backend, R kind, R param, R backend)
    type StSpec = (&'static str, u64, u8, usize, u8, u8, usize, u8);
    let stereo: Vec<StSpec> = vec![
        (
            "c0_r1r1",
            crate::V8_GEO_C0,
            crate::MODEL_D2,
            0,
            1,
            crate::MODEL_D2,
            0,
            1,
        ),
        (
            "c1_r0r2",
            crate::V8_GEO_C1,
            crate::MODEL_D1,
            0,
            0,
            crate::MODEL_D1,
            0,
            2,
        ),
        (
            "c2_r2r1",
            crate::V8_GEO_C2,
            crate::MODEL_D3,
            0,
            2,
            crate::MODEL_D3,
            0,
            1,
        ),
        (
            "c3_r1r0",
            crate::V8_GEO_C3,
            crate::MODEL_D4,
            0,
            1,
            crate::MODEL_D4,
            0,
            0,
        ),
    ];
    for (name, geo, k_l, p_l, b_l, k_r, p_r, b_r) in stereo {
        let l = fx_triangle(300);
        let r = fx_echo(300);
        let a = crate::ComponentSpec {
            model: crate::Model {
                kind: k_l,
                param: p_l,
            },
            backend: b_l,
        };
        let b = crate::ComponentSpec {
            model: crate::Model {
                kind: k_r,
                param: p_r,
            },
            backend: b_r,
        };
        let wire = crate::encode_v8_stereo(rate, &l, &r, geo, a, b).unwrap();
        crate::decode_v8(&wire).unwrap_or_else(|e| panic!("{name}: {e}"));
        rows.push(Row {
            name: name.to_string(),
            rate,
            channels: 2,
            kind: k_l,
            param: geo as usize,
            backend: b_l,
            pcm: [l, r].concat(),
            wire,
        });
    }

    // mixed-model stereo: D2 left + OP-echo right
    {
        let l = fx_triangle(280);
        let r = fx_echo(280);
        let a = crate::ComponentSpec {
            model: crate::Model {
                kind: crate::MODEL_D2,
                param: 0,
            },
            backend: 1,
        };
        let b = crate::ComponentSpec {
            model: crate::Model {
                kind: crate::MODEL_OP,
                param: 10,
            },
            backend: 2,
        };
        let wire = crate::encode_v8_stereo(rate, &l, &r, crate::V8_GEO_C0, a, b).unwrap();
        crate::decode_v8(&wire).unwrap_or_else(|e| panic!("mixed: {e}"));
        rows.push(Row {
            name: "mixed_model".to_string(),
            rate,
            channels: 2,
            kind: crate::MODEL_D2,
            param: 0,
            backend: 1,
            pcm: [l, r].concat(),
            wire,
        });
    }

    // ---- short + degenerate streams
    let shorts: Vec<(&str, u8, usize, Vec<i16>, u8)> = vec![
        ("short1_raw", crate::MODEL_RAW, 0, vec![7i16], 0),
        ("short4_d1", crate::MODEL_D1, 0, vec![7i16, -9, 41, -3], 0),
        (
            "short_extreme_pair",
            crate::MODEL_D1,
            0,
            vec![i16::MIN, i16::MAX],
            1,
        ),
    ];
    for (name, kind, param, samples, backend) in shorts {
        let spec = crate::ComponentSpec {
            model: crate::Model { kind, param },
            backend,
        };
        let wire = crate::encode_v8_mono(rate, &samples, spec).unwrap();
        crate::decode_v8(&wire).unwrap_or_else(|e| panic!("{name}: {e}"));
        rows.push(Row {
            name: name.to_string(),
            rate,
            channels: 1,
            kind,
            param,
            backend,
            pcm: samples,
            wire,
        });
    }

    // ---- print: const decls then registry rows
    for row in &rows {
        let const_name = row.name.to_uppercase();
        println!("pub(crate) const V_{const_name}: &[u8] = &[");
        for chunk in row.wire.chunks(16) {
            let hex: Vec<String> = chunk.iter().map(|b| format!("0x{b:02x}")).collect();
            println!("    {},", hex.join(", "));
        }
        println!("];");
        println!();
    }
    for row in &rows {
        let const_name = row.name.to_uppercase();
        let pcm = sha256_hex(pcm_bytes(&row.pcm));
        println!(
            r#"    ("{}", {}, {}, {}, {}, {}, V_{}, "{}", "{}", {}),"#,
            row.name,
            row.rate,
            row.channels,
            row.kind,
            row.param,
            row.backend,
            const_name,
            sha256_hex(&row.wire),
            pcm,
            row.wire.len()
        );
    }
}

fn pcm_bytes(s: &[i16]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(s.as_ptr() as *const u8, s.len() * 2) }
}
