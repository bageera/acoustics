# Changelog

All notable changes to ACOUSTICS are documented here.
Format based on Keep a Changelog; versioning is SemVer.

## [0.2.0] — 2026-10-27

### Added

- First physics-based model, `OSC` (kind 4): the damped-oscillator relation
  `x(t) = A e^(-lambda t) cos(2 pi f t + phi)` encoded as **initial state + dynamics +
  exact corrections** through its exact discrete recurrence
  `y[i] = C*y[i-1] - R*y[i-2]` (C = 2r·cos w, R = r², w = 2πf/fs, r = e^(-lambda/fs)).
  - All-integer Q15 fixed-point dynamics: encoder and decoder run bit-identical
    recursions — float fits are used only at encode time for parameter estimation.
  - Payload: initial state (y0, y1) as i16 pair; dynamics (R, C) as Q15 i32 pair;
    corrections `x[i] - y[i]` under per-32-sample selection masks (PERIOD machinery).
  - Least-squares fit of the recurrence over the run, five quantization variants
    swept by the normal smallest-payload selection.
  - Unstable/escaping dynamics (|y| > 2^20) rejected at decode; malformed OSC
    parameters can never panic or produce runaway samples.
- Model struct field renamed `lag` → `param` (PERIOD lag or OSC variant index);
  payload version bumped to 2.
- Tests: OSC selection + >8x compression on an integer-dynamics damped sinusoid;
  OSC bad-parameter/unstable-dynamics rejection; metric==serialize extended to every
  OSC variant. Suite now 12 tests.

### Measured (research notes)

- Integer-dynamics-generated signals: OSC wins by an order of magnitude (exact fit ⇒
  corrections vanish; 10000 PCM bytes → <1250).
- Float-generated damped sinusoid (fs/48, lambda = fs/8000): PERIOD lag 48 still wins
  (~45.9% of PCM) over OSC — Q15 coefficient quantization accumulates phase drift
  across the run and the correction stream carries it. Closing this gap is the v0.3
  target: finer coefficient grids, per-segment re-fits, then coupled oscillators.

### Deferred

- Coupled oscillators / modal mixtures; nonlinear dynamics; geometric state spaces.
- Stereo/multi-channel and higher bit depths; PERIOD lags > 96.

## [0.1.0] — 2026-10-27

### Added

- Reversible encoding/decoding framework, stdlib-only Rust:
  - `RAW` — exact per-sample reference representation.
  - `D1` — first-order difference (temporal slope) model.
  - `D2` — second-order difference (sample trajectory) model.
  - `PERIOD` — differences against earlier samples at candidate delays; dense lag
    sweep 2..=96, per-32-sample selection bitmasks, varint residual stream
    (zero deltas omitted via clear mask bit).
- Model evaluation: each candidate serialized completely and the smallest payload
  selected; selection metric tested equal to true serialized size for every candidate.
- Exact decode with full malformed-input rejection (bad magic/version/kind, truncation,
  trailing bytes, zero rate, varint overrun/overflow, PERIOD lag bounds).
- WAV I/O: mono 16-bit PCM parse (chunk-walking, metadata ignored) and canonical
  44-byte writer; stereo/8-bit rejected. Sample rate stored in the payload header.
- CLI: `benchmark` (encode + stats), `decode` (exact restore to WAV), `info`
  (payload description).
- Automated test suite (10 tests): silence, fixed-frequency sine, i16 extremes +
  pseudo-random noise, 1–2 sample inputs, nine malformed-payload rejections,
  WAV roundtrip, format rejection, zigzag/varint boundary values, metric==serialize.

### Fixed

- `write_varint` emitted non-canonical multi-byte encodings for values ≥ 256 (loop
  terminated on `v < 0x80` after a premature shift); standard LEB128 restored, caught
  by the roundtrip + metric==serialize tests.

### Deferred

- Physics-based representations (damped oscillators: initial state + dynamics + exact
  corrections) — next milestone.
- Stereo/multi-channel and higher bit depths; PERIOD lags > 96.