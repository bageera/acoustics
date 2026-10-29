# Changelog

All notable changes to ACOUSTICS are documented here.
Format based on Keep a Changelog; versioning is SemVer.

### Measured (research only)

- **G14 D4 follow-up** (src/g13.rs::g14_*): the external synthetic harness
  D4 denial (parity/worse on 8 classes; zlib-era -19.6% = the compression
  layer) does NOT hold on real audio - the in-repo definitive test (held-out,
  complete-v5-portfolio comparison) shows D4 beats the ENTIRE portfolio on
  piano by +3.1-4.0% beyond D3 (both partitions confirm); birds/river/speech
  confirm each class' own step. Class law: differencing order rises with
  smoothness (speech D1 / river D2 / birds D3 / piano D4), each step
  selectable. D4 promotion (kind 7, v6 gate) = RECORDED, user's call.

## [Unreleased]

### Measured (research only)


- **G4 additive modal models — DENIED, cascade mechanism named**
  (`src/modal.rs`, research-only): greedy line-scan seeding (G4-A) + varpro
  LS amplitudes + joint coordinate-descent refine (G4-B) with billed==wire
  and exact decode on every row. Modal loses 2-4× on every fixture (single
  tone 11,036 vs 2,839; two tones 8,974 vs 6,052; damped pair 12,818 vs
  2,670; noise rejection pinned 12,320 ≥ 8,209). Mechanism (measured): the
  per-sample LUT-quantization floor; the f3 cascade dodges it by integer
  recurrence decomposition with cell-level residual. Two unit-conversion
  bugs caught by the billing instruments en route (missing /(2π) in the
  scan's accumulator conversion; a spurious +π/2 on the scanned phase).
  science.md §8.6.
### Measured (research only)

- **G5 innovations anatomy — the cascade's residual carries a 65.7% coder gap**
  (`src/g5.rs`, diagnostic): stage-0 H₀ 7.809 bits vs conditional 3.716
  (H(R|prev) — half the entropy is temporal structure), ACF lag-1 = 0.505,
  state-conditioned spread 2.18× (the recurrence's magnitude predicts the
  correction's size). G5-E: a D1-context on the deltas (same zigzag/RLE
  machinery, billed both sides) = 4,489 B vs 5,518 B (−18.6%) on the winning
  model's own residual. Decision: a context coder on the correction stream is
  JUSTIFIED as the next wire milestone (payload VERSION 5); no new generative
  family first. science.md §8.7.
## [0.10.0] — 2026-10-29

### Added

- **MODEL_D4 = 7 — payload VERSION 6** (G14, `src/g13.rs` + wire): fourth-order
  differences, promoted ALONE (no ladder abstraction) per the authorization.
  The real-recording gate: D4 beats the ENTIRE v5 portfolio on the real piano
  (+3.1-4.0% beyond D3; whole-file wire rows 753,157 → 726,229 B = +26,928 B /
  +3.6% ch0, 758,067 → 735,190 = +3.1% ch1, exact); birds/river reject (their
  own steps hold); broadband noise falls back to D2; speech Δ=0 (D1 stays).
  Decoder boundaries hand-traced + pinned: the k=3 row inherits the D3 stream's
  degenerate k=2 case (r4[3] = x3 − 4x2 + 6x1 − **3x0**, the −3 NOT the
  textbook −1 — the 4-sample roundtrip pins it); k ≥ 4 uniform 4th-order.
  v6 decoders reject VERSION ≠ 6 (4/5 pinned); all v5 kinds unchanged.

## [0.9.0] — 2026-10-29

> Published to crates.io; verified by registry read-back + a fresh
> path-free consumer built from the registry: encode/decode exact, the new
> `MODEL_D3 = 6` const resolves, the full research surface callable.


### Added

- **MODEL_D3 (kind 6) — payload VERSION 5** (G12→G13, `src/g13.rs`): third-order
  differences; the first new production model since v0.2 and the first earned by
  the complete-portfolio gate. Held-out vs the ENTIRE v4 portfolio: piano
  +17.8%/+14.2% (both channels), birds +1.8%/+0.2%, river −7.1/−11.2% — D3 is
  SELECTABLE (the portfolio picks it only when it wins; broadband rejects).
  Real-piano wire rows: v5 selects D3 and reconstructs exactly at 53.4%/53.7%
  of PCM (was 64.7%/62.2% = −17.5..−18.5%). Invariants cover the
  smooth-decaying fixture class; the third-difference accumulation is
  hand-traced + round-trip pinned; v6+ rejected.

### Measured (research only)

- G13 instruments (`src/g13.rs`): held-out D3-vs-D2 benchmark, the
  complete-portfolio promotion gate, and the wire promotion test
  (corpus-gated, `scripts/g12_fetch.sh`).
- G12 acoustic-diversity corpus (`src/g12.rs` + `scripts/g12_fetch.sh`): piano
  = D2 170/172 windows (62-65% of PCM); field recordings = D2/D1 sweeps;
  speech = the only mixed class; lag-1 Hcond 42-67% below actual at n ≥ 700k.

## [0.8.2] — 2026-10-28

### Fixed

- `g3::{discover_delta_l, delta_l, echo_transform, identity_transform,
  lift_transform, best_portfolio, TransformOutcome}` were still pub(crate)
  (missed in 0.8.1's sweep) — publicized; caught by the same consumer gate.

## [0.8.1] — 2026-10-28

### Fixed

- The crate's research surface was unreachable from outside: `g5::analyze_f3`,
  `StageAnatomy` (fields), `phase::{phase_forward, crossings_ls,
  discovered_wire, decode_phase, PHASE_*}` and
  `modal::{modal_fit, discover_modal, serialize_modal, decode_modal,
  ModalPlan, component_term, sum_components, strongest_line}` were
  pub(crate) — 0.8.0 shipped them unreachable despite the changelog
  advertising the research modules. Publicized; caught by the
  fresh-registry consumer gate.

## [0.8.0] — 2026-10-28

### Added

- The research container modules ship inside the crate (research-only APIs;
  the v0.7 wire format is unchanged): `nested` (compositional echo),
  `phase` (phase-trajectory discovery), `modal` (additive modal models),
  `g3` (charged ΔL representation-space measurement), `g5` (innovations
  anatomy — the winning cascade's residual carries a 65.7% coder gap;
  D1-context −18.6% measured).
- Payload VERSION 4 (RAW literal submode, mask RLE, OP battery) — first
  published version carrying the v0.7-era wire.

### Measured (research only)

- **G5-F real-recording validation** (`src/g5f.rs` + `testdata/g5f/` +
  `scripts/g5f_fetch.sh`, hash-locked CC0 corpus): complete portfolio on real
  speech = 36-72% of PCM with exact reconstruction (strongest real-audio
  positive). The D1-context residual coder — G5-E's synthetic −18.6% — LOSES
  on real audio (+3.6..+10.8%, billed both sides): large isolated transition
  deltas spread under differencing; the redundancy is conditional but NOT
  lag-1-adjacent (Hcond/H0 = 0.21-0.55 measured). v5's design brief changes
  to STATE-conditioned residual modeling, gated on measured ΔL vs the shipped
  coder through the release gate. `parse_wav_multichannel` added (per-channel;
  no resampling/mixdown). science.md §8.8.
### Measured (research only)

- **v5-S state-conditioned residual coding DENIED in histogram form** (`src/context.rs`):
  residual values unchanged, probabilities bucketed on the decoder's causal state
  (synthetic-frozen thresholds; S6 randomized control behaving). More buckets are
  monotonically worse on the synthetic fixture (estimation tax); on the locked real
  corpus the shipped coder beats every histogram form (53,182 vs 61,543 B; 95,573 vs
  166,597 B; 430,712 vs 490,074 B) — the mask ALREADY carries zero/nonzero for free.
  Unexplored remainder recorded: an adaptive range coder (no per-bucket table
  transport). v5 still NOT justified; wire untouched. science.md 8.9.

### Measured (research only)

- **G5-G adaptive probability learning CLOSED — the v5 residual-context investigation
  ends** (`context.rs::g5g`): sequential Laplace-EMA log-loss on the masked corrections
  (zero/nonzero not re-coded). Conditioning adds nothing beyond global adaptation
  (A2 = A1 within noise on all streams); every adaptive arm loses to the shipped
  zigzag-RLE by 6.7-7.1x (sparse transient corrections break online scale models).
  The review's boundary rule fires: close v5 residual-context; redirect to new
  acoustic structure / broader corpus. science.md 8.10.

### Measured (research only)

- **G6 four-dimensional acoustic dynamics DENIED** (`src/g6.rs`): mono PCM as
  observations of a (q, v, e, d) integer state-space. The structured 4D arm
  loses 18-21x to the shipped portfolio on its own target classes (struck,
  repeated-strike); the event-compaction hypothesis is ALREADY carried by the
  standing periodic families (1 strike 541 B vs 3 identical strikes 571 B);
  the generic 4x4 skeptic arm confirms the review's prediction. Noise rejected,
  saturation exact. Spacetime/Clifford branch reserved for spatial audio.

### Measured (research only)

- **G7 acoustic structure attribution** (`src/g7.rs`): the G6-4D-4 1.42% "stage0
  win" AUDITED and repudiated (accounting artifact; exact payload_len billing
  shows the shipped portfolio beats the best single OSC variant on every real
  stream by 1.3-24%). Causal perturbation: both recurrence coefficients
  load-bearing (+34-53% corrections per ±2 Q15 step). Event transfer: parameters
  do NOT transfer even across halves of one recording (refit wins by 8-16%);
  per-window coefficients 99/100 distinct — the portfolio's speech win is the
  cascade machinery (many short-lived local stages), not a persistent physical
  law. science.md 8.12.

### Measured (research only)

- **G8 coefficient dynamics CLOSED at G8-A/B** (`src/g8.rs`): the cascade's
  stage-coefficient budget measured at 0.09-0.10% of the payload on every locked
  real stream (54-404 B) — even perfect coefficient prediction cannot matter.
  Delta trajectories carry no additional structure (H(delta) = H(abs) within
  noise); all 125 measured stages are damped poles (r2 < Q15 by >64 units).
  The dynamics-of-equations hypothesis is priced below every gain in the
  ledger. science.md 8.13.

### Measured (research only)

- **G9 bitstream anatomy COMPLETE** (`src/g9.rs`, test-gated): the production
  writer's byte layout walked and categorized on the locked corpus — residuals
  are 91-100% of every real-speech payload; masks 9.1% of the PERIOD-2 payload
  (flat u32 table; mask RLE currently wired only for OP), 8.5% of the tonal
  cascade payload; metadata/coefficient/seed/other ≤0.02% combined (G8's bound
  exact at the wire). Mask-RLE-outside-OP recorded as a possible future WIRE
  change (version bump + full release gate required). science.md 8.14.

### Measured (research only)

- **G10 uniform mask representation measured** (`src/g10.rs` +
  `g9::extract_mask_segments`): mask-only ablation with residuals untouched.
  M2 (adaptive fixed/RLE, 1-B selector) saves 2,853 B (2.5%) on the PERIOD-2
  real-speech payload, LOSES 1 B on the tonal cascade's dense mask words
  (adaptive selection required, not optional), saves 0 on D1 (no mask words
  on the wire). All boundary cases verified: sparse/dense, alternating runs,
  set-word starts, varint boundaries 127/128 and 16383/16384, exact
  round-trips, dense/random fallback asserted. Corpus-wide ceiling ~0.5%.
  Wire-bump decision OPEN (user's call); science.md 8.15.

### Measured (research only)

- **G11-A residual information audit** (`src/g11.rs` + `g9::ledger_bits`,
  measurement-only): H0/conditional/actual measured on the exact production
  residual streams. Stable gap = the integral varint floor: 16-16.5% of
  long-stream payloads (sample2-class) above the H0 code; a fractional-bit
  backend is the only mechanism, gated on beating shipped >0.5% net of its
  own tables. Conditional-entropy promises on short streams (−38%..−58%)
  collapse under the coder evidence (G5-F adjacency tested and lost; G5-G
  closed adaptive context) + small-n pair-counting bias (the review's caveat,
  measured). No substantial recoverable headroom on tested speech; corpus
  expansion = the recommended next ground. science.md 8.16.

### Measured (research only)

- **G12 acoustic-diversity corpus** (`src/g12.rs` + `scripts/g12_fetch.sh`;
  `testdata/g12/` gitignored with hash-locked acquisition): piano/music = D2
  (170/172 windows, 62-65% of PCM, exact whole-file reconstruction), field
  recordings = D2/D1 sweeps (51-73% of PCM), speech = the only class needing
  the mixed portfolio. Residual diagnostics at n >= 700k symbols: music/env
  classes show lag-1 conditional structure 42-67% below actual cost (the
  measurement that collapsed on speech at small n) — recorded as the G12-
  derived candidate (class-conditional D1 coding), gated, NOT built.
  park_wind.wav = 24-bit, preserved unmeasured (engine is i16-only); freesound
  piano note blocked on API token (recorded, not faked). science.md 8.17.

### Measured (research only)

- **G13 D3 residual coding — the G12 candidate BUILT** (`src/g13.rs`, research-only):
  the first difference of the shipped D2 stream with identical varint coding.
  Held-out numbers on the G12 corpus: piano +17.6%/+14.0% (decision-half
  confirms: +78,628/+56,981 B), birds +1.5%/−0.2%, river **−7.4/−8.1%** —
  the class law confirmed: D3 wins on smooth decaying streams, dies on
  stochastic ones (noise +239 B, speech Δ=0: the same-conditions gate honest).
  Candidate = D3 as a 7th kind under the portfolio's existing adaptive
  selection; WIRE DECISION OPEN (user's call; version bump + release gate
  required). science.md 8.18.

## [Unreleased]

### Measured (research only)

- **G3-B phase trajectories: model DISCOVERS the dynamics, representation
  LOSES the description test** (`src/phase.rs`, research-only): zero-crossing
  quadratic LS recovers (φ0, ω0, α) from the signal (≥ 8 crossings required —
  documented); 24-bit exact wrap arithmetic; B2's independent sweep fixture:
  phase model 8,483 B vs the OSC f3 cascade's 4,520 B (1.88× — loss pinned).
  B1 constant tone: no regression (engine keeps OSC); B5 two-tone miss and
  B6 random-FM rejection pinned; B3/B4 recorded.
- **G3-B.1 quantization ablation** (the review's focused experiment): LUT
  interpolation ≈ NO effect (pinned); the 24-bit ACCUMULATOR WIDTH was the
  dominant floor (48-bit + interpolation cuts max|r| 12,103 → 1,076, ~11×,
  pinned); correcting it STILL doesn't save the model (complete cost 8,546 >
  4,520; ideal H₀-coded residual ≈ 5.4 kB). Verdict: insufficient MDL
  advantage on B2-class fixtures — the binding constraint is residual
  SPARSITY, not precision. Phase family CLOSED. science.md §8.5.
## [Unreleased]

### Measured (research only)

- **paste-G3-A: reversible integer lifting — DENIED in every placement**
  (`src/g3.rs::lift_transform`): direct lift +35 B vs best(x); after echo
  (swept winner g3/D=64) +16 B; after echo (G2-C pair) +25 B. Mechanism:
  stream-splitting costs exceed the structure exposed; the portfolio's diff
  family already covers the transform's redundancy. Noise case rejects
  honestly (i16 spill, no silent narrowing). science.md §8.4.
## [Unreleased]

### Added

- **crates.io release**: the engine ships as the `manifold-acoustics` crate
  (version 0.7.0; the lib crate name for imports remains `acoustics`).
  MIT LICENSE file added; Cargo.toml carries repository/keywords/categories
  metadata and the README. Verified live via the crates.io registry API and
  a fresh consumer project (`manifold-acoustics = "0.7.0"`) whose
  encode → decode → exact-equality smoke test passes.
  Packaging commit `a172709`; keyword-length fix follow-up `0147d33`.
## [Unreleased]

### Added

- **G3 — representation-space discovery** (`src/g3.rs`; research-only,
  v0.7/G2 untouched): the charged ΔL measurement
  `ΔL_T = L_best(x) − [L(T) + L_best(T⁻¹(x))]` over fully specified
  reversible transforms — echo (G2's transform, gain × delay swept 1..64),
  identity (the exact-zero control), and diff (the structurally-zero
  negative control, measured and kept honest).
  - Identity control measures exactly 0 (accounting verified end-to-end).
  - On the INDEPENDENT float-generated damped tone, the swept echo space
    reaches ΔL = +389 B (gain full, D=64) — cross-validates G2-C's 385-B
    portfolio gap as genuine transformation benefit, not selection luck.
  - On noise every transform is ΔL < 0 (−1,491…−5,326 B): the representation
    space is MDL-rejected wholesale, matching G2-E.
  - Discovery fn (swept max ΔL) added for the G3-C transfer question.
  - Findings in docs/science.md §8.3.

## [Unreleased]

### Added

- **G2 — compositional echo**, the review's controlled single-inner-model
  experiment (`src/nested.rs`; research-only container, no wire-format
  exposure, v0.7 behavior unchanged):
  `count, gain, delay, inner(y), corrections` where `inner` is selected by
  the standing portfolio (RAW/D1/D2/PERIOD/LOOP/OSC) over the published
  i16-clamped intermediate.
  - G2 acceptance tests A–F: exact reconstruction everywhere; billed ==
    emitted size (all-gains × all-delays sweep with a fixed inner); savings
    over BOTH controls on the float-generated fixture (G2-C: 195 B vs 4,129
    flat / 580 portfolio); nesting rejected on white noise (9,573 vs
    portfolio 8,209); saturated wide-intermediate exact at 47 B (G2-F).
  - Findings + the honest G2-D near-parity loss recorded in
    docs/science.md §8.2.
  - One accounting bug found en route: the container initially billed a
    14-byte header it does not write (headerless frame); fixed and pinned by
    the invariant test.

## [0.7.0] — 2026-10-28

### Added

- **The pre-dimensionality battery (MODEL_OP, kind 5)** — the user's rule made
  operational: before any dimensionality increase, the encoder sweeps simple
  discrete operations (loop/repeat at ten periods up to 128+; echo at four
  integer gains × delays 1..64; integer scale with exact inverse when
  invertible; 16 arms total) under the same serialize-and-count discipline.
  Each arm decodes by its explicit inverse; select = smallest complete payload.
- **Corrections mask RLE** (`src/masks_rle.rs`): zero-run-first run-length
  coding over the per-word mask tables. Kills the 4 B-per-32-samples floor
  v0.4 P5 measured dominating exact-fit payloads (all-zero 5,152-sample table:
  644 → ~4 B).
- **RAW literal submode** (the Audio-G1 report's request): kind 0 bills the
  smaller of zigzag-varint stream vs literal PCM bytes; white noise now caps
  at 100.21% instead of expanding to 137%.
- **Release gates made explicit in tests**: `metric == serialize` and
  `decode(encode(x)) == x` now run over a four-shape fixture set
  (quasiperiodic, echo-of-recurrence, extreme noise, speech-like) for RAW,
  D1, D2, every PERIOD lag, every OP arm.
- `examples/g1battery.rs`: the per-arm + portfolio harness over the Audio-G1
  fixture classes (8 fixtures, reproducible; numbers recorded in
  docs/science.md §8.1).
- Payload VERSION 3 → 4: RAW payloads gain a submode byte; OP payloads (RLE
  masks, header arm id) never shipped in an older version.

### Fixed

- Mask RLE wire/accounting agreement: leading zero-run emitted when a table
  starts with set words (encoder/decoder convention mismatch desynced decode);
  run-count varints billed at their true lengths (counts ≥ 128 are 2 bytes).
- Echo decode: `y[i − D]` underflow for i < D guarded; corrections measured
  against the i16-clamped y the decoder replays (exact decode restored when
  the inverse recursion spills past ±32767).
- Cost accounting: the OP arm-id varint and the echo arm's ylen varint are now
  billed — model selection sees the true serialized cost.
- Removed the structurally lossy interleave arm (it billed only odd
  positions while decode zeroed the evens).

### Measured

- Battery portfolio on the Audio-G1 classes: LOOP wins the exact lattice
  fixtures (square 213 B, triangle 304 B — first OP-portfolio wins);
  ECHO ties the portfolio on speech-like (99.22%) with zero corrections on
  its generator class but does not win against structure-aware competitors
  (billed inverse stream; nesting deferred to G2); noise capped at 100.21%.

## [0.6.1] — 2026-10-27

### Added

- `docs/` directory: `docs/README.md` (index, conventions, reproduction rules)
  and `docs/science.md` (the foundations document). science.md states the
  founding invariants (exact reversibility, J(θ) description-length ranking,
  the three-tier feasibility taxonomy), the model families, and the complete
  measured record per milestone — v0.2 oscillator baselines, the P1–P4
  phase-coherence decomposition (closed-loop > precision; 31,283 vs 40,526 B
  vs PERIOD), the v0.4 integer-oscillator stability theorem (periods 3/4/6
  only; damping structurally impossible), the honest P6 cascade tie, the F1–F3
  feasibility findings (tier separation measured at n≈10; shuffle probe
  cond₁ 2.22→3.39 bits; latent anchor-free win vs closed-loop truncation win),
  and the v0.6 decidability result (strip-polygon emptiness proves zero
  correction unavoidable for AR(2)-Q15 at n = 44,100 on the v0.5 fixture; the
  Python experiment's 1,528 corrections attributed to its trig-table lattice).
- Every number in science.md cites its committed test; all ten receipt tests
  verified present in the tree (26 total). Reproduction is `cargo test
  -- --nocapture <filter>`, never session memory.

### Changed

- No code or wire-format changes — docs-only commit.

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