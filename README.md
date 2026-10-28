# ACOUSTICS

**Mathematical & Physics-Informed Audio Research Engine** — v0.3

A reversible audio-model codec research engine. Every payload decodes to the exact original samples; every experiment pays a measured, explicit information cost.

## Status: v0.3 — phase-coherent dynamics (P1–P4 investigations)

Implemented in Rust with **no external crate dependencies**.

### Models

The encoder serializes each candidate completely *in the payload format itself*, then ships the smallest:

| Model | Representation | Body |
|-------|----------------|------|
| `RAW` (0) | exact reference | one zigzag varint per sample |
| `D1` (1) | first-order differences | residual stream (first sample coded directly) |
| `D2` (2) | second-order differences | second-derivative residual stream |
| `PERIOD` (3) | difference vs. earlier sample at lag L | dense lag sweep 2..=96, per-32-sample selection bitmasks, nonzero residual varint stream |
| `OSC` (4) | damped-oscillator family (3 families × 2 grids) | family fields + masked corrections |

PERIOD selection bitmask semantics: bit **i** of word **w** says sample `lag + 32w + i` codes
`x[i] - x[i-lag]`; a **clear** bit means that delta is zero and no varint is stored.

### The OSC family — initial state + dynamics + exact corrections

The continuous damped sinusoid `x(t) = A e^(-lambda t) cos(2 pi f t + phi)` sampled at `fs`
satisfies the exact discrete recurrence

```
y[i] = C*y[i-1] - R*y[i-2],   C = 2r*cos(w),  R = r^2,
r = e^(-lambda/fs),           w = 2 pi f / fs
```

All synthesis/decoding arithmetic is pure integer — encoder and decoder evaluate
bit-identically, so exact reversibility never depends on float behavior. Floats appear
only in the encode-side least-squares *fit*.

Variant id = `family*2 + grid` (grid bit: 0 = Q15, 1 = Q30 — the P1 experiment):

| Variant | Family | Description |
|---------|--------|-------------|
| 0 | f0 | open-loop recurrence (v0.2 control: drift compounds through generated values) |
| 1 | f0 | open-loop recurrence, Q30 coefficients (P1) |
| 2 | f1 | closed-loop recurrence — predictions regenerate from **reconstructed** samples, so correction error cannot compound (per-position truncation only) |
| 3 | f1 | closed-loop recurrence, Q30 (P1) |
| 4 | f2 | phase-lattice synthesis, coarse sweep (P2/P3) |
| 5 | f2 | phase-lattice synthesis, refined sweep (P2/P3) |

**f2 (P2 rational frequency + P3 re-anchoring):** exact rational frequency `p/q` on an
integer phase lattice with `q in {512, 1024, 2048, 4096}` — the LUT angle
`position * 4096 / q` is an exact integer, so lattice-native periods carry zero
frequency-representation error. Quarter-wave cosine LUT (Q15, baked constants); Q15
envelope pow with per-segment reset **anchors** every `2^(stride_sel+5)` samples
(P3's segment-vs-correction tradeoff); anchor positions are always coded.

**P4 (description-length fitting) is the plan search itself:** least squares seeds a
coefficient neighborhood (±2 lattice steps around the MSE-rounding point); the encoder
evaluates the complete serialized payload — fields, masks, every delta — for each
candidate and the smallest wins. The MSE candidate is always inside the sweep set, so
DL selection ≤ LS selection structurally.

Binary layout: `ACUSTCS1` magic (8) + version (1) + model kind (1) + sample rate u32 LE (4)
+ sample count varint (+ lag/variant varint for PERIOD/OSC) + model body.

### Measured v0.3 results (the float-drift benchmark closed)

Float-generated damped sinusoid, 44,100 samples (`lambda = fs/8000`, period fs/48),
complete payloads incl. all format bytes:

```
PERIOD control (lag 48):          40,526 bytes  (45.9% of PCM)   <- beat all of v0.2
OSC f0 open-loop  Q15 (v0.2):     85,700 bytes  (97.2% of PCM)
OSC f0 open-loop  Q30 (P1):       75,918 bytes  (86.1% of PCM)  <- finer grid helps f0...
OSC f1 closed-loop Q15:           31,283 bytes  (35.5% of PCM)  <- ...but closing the loop helps far more
OSC f1 closed-loop Q30 (P1):      31,320 bytes  (35.5%)
```

**OSC now beats PERIOD on the exact fixture where v0.2 lost** — and the decomposition is
the scientific finding: Q30 (P1) bought ~10% on open-loop, while feeding corrections back
into the prediction (closed loop, P3's limiting case) bought >3x, because it removes
drift *accumulation* rather than shrinking its increment. The residual Q30-vs-Q15 gap at
closed loop is ~zero, as predicted: with no compounding there is nothing left for extra
precision to fix.

P2 (exact rational fit): a tone synthesized by the f2 lattice itself (p=1/512) encodes in
**783 bytes for 6000 samples (6.5% of PCM)** — parameters + zero corrections.

P3 (anchor stride): re-anchoring beats no-anchors on the drift fixture at stride 64
(28,098 vs 28,531 bytes refined); longer strides decay back toward no-anchor cost.
Curve printed by the `p3_segment_size_tradeoff` test.

P4 (LS vs DL fitting): DL ties LS on open-loop (f0) fixtures here and saves bytes on
closed-loop f1/Q15 (1,459 → 1,454). The guarantee `DL ≤ LS` holds on every variant by
construction; where it pays is signal-dependent — exactly why it is measured, not assumed.

Three criteria stay separate throughout (physical/numerical fidelity vs information
efficiency); no single score is computed anywhere.

### Guarantees

- **Exact reversibility** — decode reconstructs the original samples bit-for-bit (not
  necessarily the original WAV file's metadata or layout).
- **Measured information cost** — model selection compares real serialized byte counts;
  a test asserts the selection metric equals actual serialized size for every candidate
  family, grid, and PERIOD lag.
- **Total decode** — malformed, truncated, or hostile payloads are rejected with an
  error, never a panic and never invented samples (varint overrun/u64 overflow, unstable
  or envelope-escaping dynamics, invalid lattice geometry, `Vec::with_capacity` DoS guards).

Sample-rate metadata rides in the payload header (no companion file).

## Usage

Requires a Rust toolchain (`rustup`).

```
cargo run --release -- benchmark input.wav output.acoust   # encode + print model/size stats
cargo run --release -- decode output.acoust restored.wav   # exact decode to 16-bit WAV
cargo run --release -- info output.acoust                  # describe a payload

cargo test                                                  # 16-test suite
cargo test -- --nocapture p1_phase                          # print research measurements
```

Input: mono, 16-bit PCM WAV. Stereo, 8-bit, and exotic formats are rejected with an error.

### Library

```rust
let payload = acoustics::encode(44100, &samples)?;                 // Vec<u8>
let (rate, samples, model) = acoustics::decode_payload(&payload)?; // exact + model used
```

`Model` is `{ kind: u8, param: usize }` — one of `MODEL_RAW`, `MODEL_D1`, `MODEL_D2`,
`MODEL_PERIOD` (`param` = lag in 2..=96), `MODEL_OSC` (`param` = variant id 0..6).

The `acoustics::research` module exposes the measurement surface:
`osc_variant_costs`, `period_control_cost`, `f2_stride_curve`, `p4_ls_vs_dl`,
`synth_phase_lattice` (exact integer f2 pipeline for fixtures).

## Automated tests

`cargo test` covers: silence, fixed-frequency sine, i16 extremes + pseudo-random noise,
an integer-dynamics damped sinusoid (OSC selection + >8x compression), 1–2 sample inputs,
malformed-payload and bad-OSC-parameter rejections, WAV parse/write roundtrip,
stereo/8-bit rejection, zigzag+varint boundary values, metric==serialize for every
candidate — plus the four research acceptance tests P1–P4 (drift comparison over 44k
samples, rational exact fit, anchor-stride tradeoff curve, LS-vs-DL fitting).

## Honest limitations (v0.3)

- One oscillator per payload: no mixtures/superpositions (coupled oscillators are v0.4).
- f0/f1 fit one (R, C) pair for the whole run; f2 anchors amplitude but phase restarts
  from fitted `phi0` per segment rather than re-fitting locally.
- PERIOD lag capped at 96 samples; f2 lattices have `q | 4096` (period ≥ 1 sample
  resolution is coarse at high q); envelope is Q15 pow evaluated per segment only.
- Mono 16-bit only. Encode is a 6-variant × 25-candidate sweep: cost, not speed, is the
  current objective.

## Where the research goes next

v0.4 targets: **coupled oscillators** (superpositions and modal mixtures), per-segment
local re-fits, nonlinear dynamics, geometric state-space representations, and — per the
v0.3 memo — a second modeling family of **exact discrete dynamical systems** (reversible
integer lattice transformations) to stand beside the continuous-dynamics family.
Reversibility and measured cost stay nonnegotiable.