# ACOUSTICS

**Mathematical & Physics-Informed Audio Research Engine** — v0.2

A reversible audio-model codec research engine. Every payload decodes to the exact original samples; every experiment pays a measured, explicit information cost.

## Status: v0.2 — first physics-based model (damped oscillator dynamics)

Implemented in Rust with **no external crate dependencies**.

### Models

The encoder serializes each candidate completely *in the payload format itself*, then ships the smallest:

| Model | Representation | Body |
|-------|----------------|------|
| `RAW` (0) | exact reference | one zigzag varint per sample |
| `D1` (1) | first-order differences | residual stream (first sample coded directly) |
| `D2` (2) | second-order differences | second-derivative residual stream |
| `PERIOD` (3) | difference vs. earlier sample at lag L | dense lag sweep 2..=96, per-32-sample selection bitmasks, nonzero residual varint stream |
| `OSC` (4) | damped-oscillator dynamics | initial state (y0,y1) + Q15 dynamics (R, C) + masked corrections |

PERIOD selection bitmask semantics: bit **i** of word **w** says sample `lag + 32w + i` codes
`x[i] - x[i-lag]`; a **clear** bit means that delta is zero and no varint is stored.

### The OSC model — initial state + dynamics + exact corrections

The continuous damped sinusoid `x(t) = A e^(-lambda t) cos(2 pi f t + phi)` sampled at `fs`
satisfies the exact discrete recurrence

```
y[i] = C*y[i-1] - R*y[i-2],   C = 2r*cos(w),  R = r^2,
r = e^(-lambda/fs),           w = 2 pi f / fs
```

v0.2 encodes that recurrence instead of samples: **initial state** `(y0, y1)` as two i16s,
**dynamics** `(R, C)` as Q15 fixed-point i32s — all-integer, so encoder and decoder run
bit-identical recursions (a float fit would make exact reversibility impossible) — plus
**exact corrections** `x[i] - y[i]` under per-32-sample selection masks (same machinery as
PERIOD). Dynamics are fitted by least squares on `s[i] = C*s[i-1] - R*s[i-2]`; five
quantization variants are swept. Unstable/escaping dynamics (envelope > 2^20) are rejected
at decode — malformed OSC payloads can never panic or generate runaway samples.

When the signal truly obeys the dynamics, corrections vanish and the payload is parameters
+ zero masks (the test fixture compresses 10000 PCM bytes to under 1250). Measured results
v0.2: on integer-dynamics-generated signals OSC wins by an order of magnitude; on
float-generated damped sinusoids PERIOD lag 48 still wins (~46% of PCM vs OSC's higher
correction stream from Q15 phase drift) — quantifying that gap is the v0.3 research target.

Binary layout: `ACUSTCS1` magic (8) + version (1) + model kind (1) + sample rate u32 LE (4)
+ sample count varint (+ lag/variant varint for PERIOD/OSC) + model body.

### Guarantees

- **Exact reversibility** — decode reconstructs the original samples bit-for-bit (not
  necessarily the original WAV file's metadata or layout).
- **Measured information cost** — model selection compares real serialized byte counts,
  and a test asserts the selection metric equals actual serialized size for every candidate.
- **Total decode** — malformed, truncated, or hostile payloads are rejected with an error,
  never a panic and never invented samples (incl. varint overrun/u64 overflow, unstable
  dynamics, and `Vec::with_capacity` DoS guards).

Sample-rate metadata rides in the payload header (no companion file).

## Usage

Requires a Rust toolchain (`rustup`).

```
cargo run --release -- benchmark input.wav output.acoust   # encode + print model/size stats
cargo run --release -- decode output.acoust restored.wav   # exact decode to 16-bit WAV
cargo run --release -- info output.acoust                  # describe a payload

cargo test                                                  # 12-test suite
```

Input: mono, 16-bit PCM WAV. Stereo, 8-bit, and exotic formats are rejected with an error.

### Library

```rust
let payload = acoustics::encode(44100, &samples)?;                 // Vec<u8>
let (rate, samples, model) = acoustics::decode_payload(&payload)?; // exact + model used
```

`Model` is `{ kind: u8, param: usize }` — one of `MODEL_RAW`, `MODEL_D1`, `MODEL_D2`,
`MODEL_PERIOD` (`param` = lag in 2..=96), `MODEL_OSC` (`param` = variant index 0..5).

## Automated tests

`cargo test` covers: silence (roundtrip + strong compression), fixed-frequency sine
(structural model selection), i16 extremes + pseudo-random noise (asserts RAW selection),
an integer-dynamics damped sinusoid (asserts OSC selection + >8x compression), 1–2 sample
inputs, malformed-payload and bad-OSC-parameter rejections, WAV parse/write roundtrip,
stereo/8-bit rejection, zigzag+varint roundtrip over boundary values, and the
metric==serialize invariant for every candidate model including all OSC variants.

## Honest limitations (v0.2)

- The osc fit is least-squares over the whole run with one (R, C) pair: no re-fitting,
  no segment switching, float quantization drift accumulates phase error on signals that
  only *approximately* obey the dynamics (their corrections carry that cost).
- PERIOD lag capped at 96 samples; OSC covers one partial per payload, not mixtures.
- Mono 16-bit only.

## Where the research goes next

v0.3 targets: quantify and close the Q15 phase-drift gap (finer coefficient grids,
per-segment refits), then **coupled oscillators** (superpositions and modal mixtures),
nonlinear dynamics, and geometric state-space representations. Reversibility and measured
cost stay nonnegotiable.