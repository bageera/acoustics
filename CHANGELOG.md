# Changelog

All notable changes to ACOUSTICS are documented here.
Format based on Keep a Changelog; versioning is SemVer.

## [0.3.0] — 2026-10-27

### Added

- Phase-coherence investigations P1–P4 (v0.3 memo), all as OSC variants under the
  existing smallest-complete-payload selection with PERIOD as control:
  - P1 (precision): Q30 coefficient grid alongside Q15 — variant id is now
    `family*2 + grid` (six variants; f0 open-loop, f1 closed-loop, f2 lattice).
  - Closed-loop family f1: predictions regenerate from the *reconstructed* run, so
    correction error cannot compound (limiting case of P3 re-anchoring: reset every
    sample). Payload: (C, R) i32 pair; all n positions masked (bits 0–1 always coded).
  - P2 (rational frequency): f2 phase-lattice synthesis — exact rational frequency
    p/q on a lattice with q | 4096 (integer LUT angle, zero frequency-representation
    error for lattice-native periods), baked Q15 quarter-wave cosine table
    (`src/cos_q13.rs`, generated — no runtime float trig), Q15 envelope pow.
  - P3 (segmentation): per-segment reset anchors every 2^(sel+5) samples; anchor
    positions always coded; anchor-stride tradeoff curve exposed via
    `research::f2_stride_curve`.
  - P4 (description-length fitting): not a family — the plan search itself. LS seeds
    a ±2-lattice-step neighborhood; the encoder evaluates the complete serialized
    payload per candidate and keeps the smallest. The MSE-rounding candidate is always
    in the sweep, so DL ≤ LS structurally; `research::p4_ls_vs_dl` measures the gap.
- `research` module (public measurement surface): `osc_variant_costs`,
  `period_control_cost`, `f2_stride_curve`, `p4_ls_vs_dl`, `synth_phase_lattice`.
- Four research acceptance tests (P1 drift over 44k samples, P2 rational exact fit,
  P3 anchor tradeoff, P4 LS-vs-DL) with per-variant exact-roundtrip assertions and
  printed measurements. Suite: 16 tests.
- Payload version bumped to 3 (v0.2 variant semantics changed); old-version payloads
  decode-error cleanly.

### Measured (research notes)

- Float damped sinusoid, 44,100 samples (the fixture where v0.2 lost to PERIOD):
  PERIOD control 40,526 B (45.9%); f0 Q15 85,700 B (97.2%); f0 Q30 75,918 B (86.1%);
  **f1 closed-loop Q15 31,283 B (35.5%) — OSC now beats PERIOD**; f1 Q30 31,320 B.
  Decomposition: finer grid (P1) bought ~10% on open-loop; closing the feedback loop
  bought >3x (removes drift accumulation, not its increment). Q30 adds nothing once
  closed-loop (residual gap ≈ 0), matching the drift model.
- P2: f2-lattice-native tone (p=1/512, 6000 samples) → 783 B (6.5% of PCM),
  corrections vanish on exact rational fits.
- P3: anchor stride 64 beats no-anchor on the drift fixture (28,098 vs 28,531 B
  refined); benefit decays with longer strides — full curve in the test output.
- P4: DL ties LS on open-loop fixtures, saves 5 B on f1/Q15; DL ≤ LS holds everywhere.

### Deferred

- Coupled oscillators / mixtures (one partial per payload today); per-segment local
  re-fits; exact discrete dynamics (reversible integer lattice transformations) as a
  second modeling family; nonlinear dynamics; geometric state spaces.
- Stereo/multi-channel and higher bit depths; PERIOD lags > 96.

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