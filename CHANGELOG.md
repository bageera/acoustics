# Changelog

All notable changes to ACOUSTICS are documented here.
Format based on Keep a Changelog; versioning is SemVer.

## [0.6.0] — 2026-10-27

### Added

- v0.6 rounding-cell geometry machinery in `feas` (wire format untouched):
  - `cell_geometry_search`: dynamics-lattice sweep with per-dynamics tier-1
    strip-polygon diagnostics (vertices/area) and decoder-verified violation
    counts.
  - `latent_seed_search`: the decisive free-seed arm — for the latent family
    the strip polygon IS the feasible seed region; integer grid points ring
    outward from the polygon centroid, each verified by the f64 latent
    decoder.
- F1d acceptance test reproducing the v0.5 Python experiment's exact fixture
  (917.3 Hz, A=12000, damping 1.84e-5/sample, phi0 = 0.37, float64-generating
  equation, n = 44,100). Suite: 26 tests.

### Measured (research notes) — the v0.6 question answered for this family

- Pinned seeds (closed loop): violation floor 44,086/44,100 = the recurrence
  LOCKS to the rounded seed states, seed error persists under |r|≈1 dynamics —
  unavoidable, measured.
- Free seeds (latent, AR(2)-Q15): at n = 44,100 the strip polygon COLLAPSES —
  no (z0, z1) seed pair lies inside every rounding cell for any lattice
  (C, R) — so zero correction is unavoidable within this family too, and the
  geometry names the reason: the true generator's (r, cos w) are transcendental
  and only approximable on the Q15 lattice; feasibility decays with N exactly
  as the F1c collapse test predicted.
- The two experiments reconcile: the v0.5 Python experiment's 1,528 corrections
  (3.46%) belong to its Q40-phase + Q20-interpolated-LUT + Q28-envelope decoder
  — a different, finer lattice structure whose remaining violations are
  governed by ITS quantization geometry (phase drift floor ~1.7e-3 rad over
  the run contributes < 0.02 sample units — the trig/table lattice, not the
  phase accumulator, is its binding constraint). The strip-geometry
  non-emptiness test is the decision rule for any finer lattice: zero
  correction is achievable iff the polygon stays nonempty at the target N.
- "Unavoidable vs undiscovered" is therefore now a decidable measurement per
  model family, not a search-quality question.

### Deferred

- Higher-precision decoder families (Q40 phase accumulator, interpolated
  trig tables) as Rust variants with the polygon test at their own lattices;
  segment-wise feasibility (reset windows where the polygon re-opens);
  real-audio corpora.

## [0.5.0] — 2026-10-27

### Added

- `feas` research module (wire format untouched): the work order's three
  foundations implemented as measurement machinery.
  - F1 rounding-cell feasibility: `fundamental_solutions` (u/v bases of the
    recurrence), `real_feasible_region` (tier 1 — per-sample strips
    x-1/2 <= a·u+b·v < x+1/2 intersected as half-planes), and
    `decoder_feasible_search` (tier 2 — the only tier that matters: lattice
    dynamics verified against the ACTUAL integer decoder arithmetic, ring
    search with L0/L1 candidate generation). Three-outcome taxonomy
    (real-valued / decoder-representable / infeasible) is now executable.
  - F2 minimum-information instruments: `residual_stats` (empirical H0,
    order-1 conditional entropy, binned-Laplace cross-entropy with
    probabilities integrated over integer bins — never density-as-length),
    and `j_cost` = L(M) + L(theta) + L_ideal(R). L0/L1 retained strictly as
    candidate generators.
  - F3 `latent_state_run` vs `closed_loop_residual` head-to-head under
    identical cost accounting.
- Tests F1a-F3 (+F2b): 6 research acceptance tests; suite 25.

### Measured (research notes)

- F1 tier separation is REAL: on the truncated-integer fixture
  (C=64210, R=32000, div_euclid) ideal-f64 strip feasibility is EMPTY by
  n≈10 (truncation drift breaks the linearity s=a·u+b·v), yet the shipped
  closed-loop remains exact by construction. The work order's warning —
  feasibility must be checked against the actual decoder, not the ideal
  oscillator — is measured, not assumed.
- F1 tier 1 works exactly where linearity holds: on the exact-evolving
  undamped fixture (C=1, R=1) the feasible polygon is nonempty at n=600
  with centroid at the true seed (4096, 8192), and the tier-2 ring search
  recovers the generator exactly.
- F2: H0 >= conditional entropy holds (instrument sanity); shuffling a
  residual stream leaves H0 bit-identical while conditional entropy rises
  2.22 -> 3.39 bits (temporal structure measured separately, as the work
  order requires).
- F3 head-to-head: on the exact-evolving fixture BOTH models hit zero
  corrections — but the latent model needs NO bootstrap anchors (closed
  loop always codes 2; latent 0): a measured anchor-free win for latent
  state when the dynamics are exactly representable. On the truncated
  fixture the latent ideal-state model drifts (L0=898, J=1706 bits) while
  closed-loop holds at its 2 anchors (J=28): feedback wins exactly where
  the arithmetic truncates. Neither direction is universal — matching the
  work order's prediction.

### Deferred

- Multi-segment feasibility, real-audio corpora (speech/instruments),
  nonlinear dynamics, spatial acoustic-field models.

## [0.4.0] — 2026-10-27

### Added

- Exact discrete dynamics (P5, the v0.3 memo's second modeling family): grid 2 —
  coefficient lattice q = 1, so the recurrence `y[i] = C*y[i-1] - R*y[i-2]` runs with
  zero per-step truncation error. Stability analysis closes the space: R = r^2 must be
  a non-negative integer with r <= 1 forces R = 1, and |C| <= 2r = 2 forces
  C in {-1, 0, 1} — exactly three undamped integer oscillators, periods 3, 4, 6.
  New variants 6 (f0 open-loop) and 7 (f1 closed-loop) sweep that space; coefficient
  fields are zigzag varints ((1,1) costs 2 bytes vs the fixed 8-byte i32 pair).
- Oscillator mixtures (P6/E1): variant family f3 (variants 8/9/10 = residual cascade
  on Q15/Q30/integer grids). Stage s is closed-plan-fit on the residual seen so far
  (greedy coefficient pick by residual energy, up to 3 stages); each stage's deltas
  are coded; rec_s reconstructs level_s exactly, the level ladder is decoder-
  computable, and the sum of the reconstructed streams is exactly x.
- Tests P5 (integer exactness: periods 3/4/6 encode header+masks only, ~15x
  compression), P6 (two-part mixture: every cascade variant exactly
  round-trips; cascade >= single), P6b (stage-count cost assertion).
  Suite: 19 tests.

### Measured (research notes)

- P5: exactly-integer undamped oscillators are provably limited to periods 3, 4, 6 —
  damping on the integer lattice is structurally impossible (real roots cannot both
  have modulus < 1 with R an integer >= 1). This is why the continuous family needs
  fixed-point grids at all: the exact-discrete family and the approximated-continuous
  family are complementary, not redundant. Exact-fit integer payloads are header +
  mask-table only (~6.7% of PCM at n=3000); the mask table, not the delta stream, is
  the structural floor.
- P6 (honest negative): the greedy residual cascade TIES closed-loop-single on the
  two-part fixture (4,512 B at k=1 vs 4,512 B single) rather than beating it. Decom-
  position: (a) the first stage's greedy fit captures the whole signal's joint AR(2)
  and leaves little for stage 2; (b) prediction-only intermediate stages — the design
  that would avoid paying stage-level delta streams — compound drift like open-loop
  f0. Named v0.5 requirement: jointly optimized or periodically re-anchored
  intermediate stages (which connects directly to the per-segment re-fit target).

### Deferred

- Joint stage optimization / re-anchored cascade stages (v0.5).
- Nonlinear dynamics; geometric state spaces.
- Stereo/multi-channel and higher bit depths; PERIOD lags > 96.

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