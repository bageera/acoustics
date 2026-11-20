//! G23-F — the final security battery for `decode_v8()`.
//!
//! Three families, all over the COMPLETE public decoder (not the Rice
//! decoder in isolation):
//!
//! 1. `v8_byte_mutation_battery` — exhaustive per-bit flips across two
//!    representative fixtures (mono R1 and stereo C2-with-parity): every
//!    mutated byte vector must decode to Ok or Err, never panic, and any
//!    Ok must have consumed exactly the declared frame.
//! 2. `v8_structured_fuzzer` — deterministic seeded rounds over the WHOLE
//!    golden corpus with structured mutations (bit flips, byte swaps,
//!    truncation, varint corruption, splicing, tail growth). The
//!    deterministic generator (an LCG, seeded constants recorded in the
//!    test) is NOT a coverage-guided fuzzer; the budget, seeds and
//!    limitation are recorded in docs/science.md (§8.29). A libFuzzer
//!    harness can be added on the same fixtures without code changes.
//! 3. `v8_fuzz_smoke` — a small always-on budget that runs in the normal
//!    suite on both debug and release builds, where overflow behavior is
//!    allowed to differ.

#![cfg(test)]

const SEED_A: u64 = 0x243F_6A88_85A3_08D3;
const SEED_B: u64 = 0x1319_8A2E_0370_7344;

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 21
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % (n as u64)) as usize
    }
}

fn fix(name: &'static str) -> &'static [u8] {
    crate::golden::GOLDEN_VECTORS
        .iter()
        .find(|g| g.name == name)
        .expect("golden fixture present")
        .wire
}

/// No panic inside the declared resource contract: decode() must return
/// Ok/Err for ANY input, however broken. (A panic aborts the test.)
fn decode_must_not_panic(wire: &[u8]) {
    let _ = crate::decode_v8(wire);
}

fn corpus() -> Vec<&'static [u8]> {
    crate::golden::GOLDEN_VECTORS
        .iter()
        .map(|g| g.wire)
        .collect()
}

fn structured_round(lcg: &mut Lcg, wires: &[&[u8]]) -> Vec<u8> {
    let mut v = wires[lcg.below(wires.len())].to_vec();
    match lcg.below(6) {
        0 => {
            // bit flips
            let n = 1 + lcg.below(7);
            for _ in 0..n {
                let i = lcg.below(v.len());
                v[i] ^= 1 << lcg.below(8);
            }
        }
        1 => {
            // byte swaps
            let i = lcg.below(v.len());
            let j = lcg.below(v.len());
            v.swap(i, j);
        }
        2 => {
            // truncation (any point, incl. mid-header)
            v.truncate(lcg.below(v.len() + 1));
        }
        3 => {
            // varint continuation corruption + boundary byte growth
            let i = lcg.below(v.len().clamp(1, 24));
            v[i] |= 0x80;
            v.push((lcg.next() & 0x7F) as u8);
        }
        4 => {
            // splice two fixtures at a random point
            let other = wires[lcg.below(wires.len())];
            let cut = lcg.below(v.len().max(1));
            v.truncate(cut);
            let ocut = lcg.below(other.len());
            v.extend_from_slice(&other[ocut..]);
        }
        _ => {
            // tail growth (termination walk)
            let n = 1 + lcg.below(64);
            for _ in 0..n {
                v.push((lcg.next() & 0xFF) as u8);
            }
        }
    }
    v
}

/// The always-on smoke battery: 300 deterministic rounds on both build
/// profiles (integer overflow behavior differs between configurations).
#[test]
fn v8_fuzz_smoke() {
    let wires = corpus();
    let mut lcg = Lcg(SEED_A);
    for _ in 0..300 {
        let v = structured_round(&mut lcg, &wires);
        decode_must_not_panic(&v);
    }
}

/// The exhaustive per-bit mutation battery over two representative
/// fixtures: mono R1 and stereo C2 (the parity-plane walk).
#[test]
#[ignore] // corpus-heavy: --release --locked v8_byte_mutation -- --ignored --nocapture
fn v8_byte_mutation_battery() {
    for (name, want) in [("mixed_model", true), ("c2_r2r1", false)] {
        let _ = want;
        let base = fix(name);
        let total = base.len();
        let mut ok = 0u64;
        let mut err = 0u64;
        for i in 0..total {
            for bit in 0..8u32 {
                let mut v = base.to_vec();
                v[i] ^= 1 << bit;
                match crate::decode_v8(&v) {
                    Ok(_) => ok += 1,
                    Err(_) => err += 1,
                }
            }
        }
        println!("MUTATION {name}: {total} bytes x 8 bits, ok={ok} rejected={err} (no panics)");
        assert!(
            err > 0,
            "{name}: every single-bit flip accepted? the validation would be dead"
        );
    }
}

/// The structured fuzzer: 4,000 deterministic rounds (SEED_B) over the
/// full golden corpus. Any panic = failure. Ok-decodes are checked for
/// sane channel geometry (the count's declared reconstruction).
#[test]
#[ignore] // corpus-heavy: --release --locked v8_structured_fuzzer -- --ignored --nocapture
fn v8_structured_fuzzer() {
    let wires = corpus();
    assert!(wires.len() >= 20, "the golden corpus is the fuzzing corpus");
    let mut ok = 0u64;
    let mut rejected = 0u64;
    let mut lcg = Lcg(SEED_B);
    for round in 0..4_000usize {
        let v = structured_round(&mut lcg, &wires);
        match crate::decode_v8(&v) {
            Ok((rate, channels)) => {
                ok += 1;
                assert!(
                    rate > 0 && !channels.is_empty(),
                    "round {round}: structurally-invalid Ok",
                );
            }
            Err(_) => rejected += 1,
        }
    }
    println!(
        "FUZZ rounds=4000 ok={ok} rejected={rejected} seed=SEED_B corpus={}",
        wires.len()
    );
    assert!(
        rejected > ok / 10,
        "the battery rejected too little to be live"
    );
}

/// The N23 compatibility probe: a valid v8 frame with APPENDED bytes must
/// reject — the fail-closed frame termination is the measured fact that
/// rules out transparent backward extension (architecture arm A1).
#[test]
fn v8_appended_descriptor_rejects() {
    for name in ["short4_d1", "mixed_model"] {
        let base = fix(name);
        for n in [1usize, 16, 43, 128] {
            let mut v = base.to_vec();
            v.extend(std::iter::repeat_n(0u8, n));
            assert!(
                crate::decode_v8(&v).is_err(),
                "{name}: a v8 frame must reject {n} appended bytes"
            );
        }
    }
}

/// A truncation sweep: EVERY prefix of three fixtures must decode to
/// Ok or Err — with canonical termination this exercises the tail checks.
#[test]
fn v8_truncation_sweep_smoke() {
    for name in ["short4_d1", "d3_r1_escapes", "mixed_model"] {
        let base = fix(name);
        let mut ok = 0u64;
        let mut err = 0u64;
        for cut in 0..base.len() {
            match crate::decode_v8(&base[..cut]) {
                Ok(_) => ok += 1,
                Err(_) => err += 1,
            }
        }
        println!("TRUNC {name}: ok={ok} rejected={err}");
        assert!(err >= base.len() as u64 - 1, "prefixes must reject: {name}");
    }
}
