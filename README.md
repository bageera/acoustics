# ACOUSTICS

**Mathematical & Physics-Informed Audio Research Engine** — v0.1

A reversible audio-model codec research engine. Every payload decodes to the exact original samples; every experiment pays a measured, explicit information cost.

## Status: v0.1 — reversible encoding baseline

Implemented in Rust with **no external crate dependencies**.

### Models

The encoder serializes each candidate completely *in the payload format itself*, then ships the smallest:

| Model | Representation | Body |
|-------|----------------|------|
| `RAW` (0) | exact reference | one zigzag varint per sample |
| `D1` (1) | first-order differences | residual stream (first sample coded directly) |
| `D2` (2) | second-order differences | second-derivative residual stream |
| `PERIOD` (3) | difference vs. earlier sample at lag L | dense lag sweep 2..=96, per-32-sample selection bitmasks, nonzero residual varint stream |

PERIOD selection bitmask semantics: bit **i** of word **w** says sample `lag + 32w + i` codes
`x[i] - x[i-lag]`; a **clear** bit means that delta is zero and no varint is stored (encoder
never profits coding zero, since a direct zero residual also costs 1 byte).

Binary layout: `ACUSTCS1` magic (8) + version (1) + model kind (1) + sample rate u32 LE (4)
+ sample count varint (+ lag varint for PERIOD) + model body.

### Guarantees

- **Exact reversibility** — decode reconstructs the original samples bit-for-bit (not
  necessarily the original WAV file's metadata or layout).
- **Measured information cost** — model selection compares real serialized byte counts,
  and a test asserts the selection metric equals actual serialized size for every candidate.
- **Total decode** — malformed, truncated, or hostile payloads are rejected with an error,
  never a panic and never invented samples (incl. varint overrun/u64 overflow and
  `Vec::with_capacity` DoS guards).

Sample-rate metadata rides in the payload header (no companion file).

## Usage

Requires a Rust toolchain (`rustup`).

```
cargo run --release -- benchmark input.wav output.acoust   # encode + print model/size stats
cargo run --release -- decode output.acoust restored.wav   # exact decode to 16-bit WAV
cargo run --release -- info output.acoust                  # describe a payload

cargo test                                                  # 10-test suite
```

Input: mono, 16-bit PCM WAV. Stereo, 8-bit, and exotic formats are rejected with an error.

### Library

```rust
let payload = acoustics::encode(44100, &samples)?;                 // Vec<u8>
let (rate, samples, model) = acoustics::decode_payload(&payload)?; // exact + model used
```

`Model` is `{ kind: u8, lag: usize }` — one of `MODEL_RAW`, `MODEL_D1`, `MODEL_D2`,
`MODEL_PERIOD` (with `lag` in `2..=96`).

## Automated tests

`cargo test` covers: silence (roundtrip + strong compression), fixed-frequency sine
(asserts PERIOD selection), i16 extremes + pseudo-random noise (asserts RAW selection),
1–2 sample inputs, nine malformed-payload rejections, WAV parse/write roundtrip,
stereo/8-bit rejection, zigzag+varint roundtrip over boundary values, and the
metric==serialize invariant for every candidate model.

## Honest limitations (v0.1)

- Not yet a physics-based codec — no damped-oscillator or coupled-system models yet.
- PERIOD lag capped at 96 samples; lower frequencies (e.g. 150 Hz @ 44.1 kHz) still fall
  back to RAW/D1/D2.
- Mono 16-bit only.

## Where the actual scientific research begins

The next milestone is the first **physics-based representation experiment**: model an
acoustic signal as **initial state + dynamics + exact corrections** using damped
oscillatory systems — mathematically tractable, exactly invertible, with measured
information cost. Beyond that: coupled oscillators, nonlinear dynamics, and geometric
state-space representations. Reversibility and measured cost stay nonnegotiable.