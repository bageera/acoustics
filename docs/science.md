# The Science of ACOUSTICS — measured foundations (informative)

Status: **informative**. ACOUSTICS is a reversible audio-model codec research
engine; this document states the mathematical foundations the project is built
on and the measured results that back them. Every number cites a committed
test; run `cargo test -- --nocapture <filter>` to reproduce any table. The
companion index is [README.md](README.md).

## 0. One-line map

| Milestone | Question | Result | Receipt |
|---|---|---|---|
| v0.2 (oscillator) | Can a physics model displace per-sample coding? | Yes on its own fixture, no on float-generated | `damped_oscillator_selects_osc_and_compresses` |
| v0.3 (P1–P4) | What controls phase-drift cost? | Feedback > precision; OSC beat PERIOD | `p1_phase_drift_comparison_over_44k_samples` |
| v0.4 (P5/P6) | Can integer dynamics be exact? Can mixtures cascade? | Periods 3/4/6 exactly; cascade ties (negative) | `p5_integer_dynamics_exact_and_tiny`, `p6_mixture_cascade_beats_single` |
| v0.5 (F1–F3) | Is fit quality the right objective? | No — feasibility + J(θ) are; tier separation measured | `f1a`–`f2b`, `f3_latent_vs_closed_loop` |
| v0.6 (F1d) | Are the remaining corrections unavoidable? | Decidable per family via strip geometry | `f1d_v05_fixture_violation_floor` |

## 1. The founding invariants

**Exact reversibility.** Every payload decodes to the original samples
bit-for-bit. This is a hard contract, not a target: selection compares the
*actual serialized bytes* of every candidate (a test asserts the selection
metric equals the true serialized size for every family, grid, and lag), and
the decoder is total — hostile payloads (varint overflow, unstable dynamics,
envelope escape, lattice impossibility) are rejected as errors, never panics
and never invented samples.

**Measured description length.** The ranking objective is

> J(θ) = L(M) + L(θ) + L_ideal(R | M, θ)

complete mathematical description: model identification, parameter bits, and
the idealized residual cost. H₀ (distribution), conditional entropy (temporal
structure), and binned cross-entropy (probabilities integrated over integer
bins — never density values as code lengths) are separate instruments.
L₀ = Σ 1[r≠0] and L₁ = Σ|r| are candidate *generators*, not objectives: a
thousand identical nonzero corrections can cost less than a hundred
unpredictable ones, so counting violations or magnitudes never ranks the
final choice.

**Three-tier feasibility taxonomy** (measured in v0.5, tests `f1a`–`f1c`):

| Tier | Definition | Verified by |
|---|---|---|
| Real-valued feasible | a mathematical (a, b) exists for the strips | half-plane intersection |
| **Decoder-representable** | a *serialized* solution exists under exact decoder arithmetic | lattice run against the real decoder |
| Infeasible | corrections or a different model required | polygon empty / search exhausted |

Tier 2 is the only one that matters for a reversible engine — and the tiers
genuinely differ (§7, P-F1).

## 2. Model families

The encoder serializes each candidate completely and ships the smallest
(`MODEL_*` constants; wire layout in the README):

- **RAW / D1 / D2** — per-sample reference, first- and second-order differences
  (v0.1 baselines; mandatory controls).
- **PERIOD** — x̂[i] = x[i−L], L swept 2..=96: the control that v0.2 lost to
  and v0.3 beat.
- **OSC families** — the damped-oscillator recurrence y[i] = C·y[i−1] −
  R·y[i−2] with C = 2r·cos ω, R = r², expressed as *initial state + dynamics +
  exact corrections*:
  - f0 open-loop (state evolves from coded seed; drift compounds),
  - f1 closed-loop (predictions regenerate from *reconstructed* samples;
    correction error cannot compound — P3's limiting case),
  - f2 phase-lattice synthesis (exact rational frequency p/q, q | 4096),
  - f3 residual cascade (mixtures; stage s fit on the residual seen so far),
  - grid bit: Q15 (2⁻¹⁵ coefficient lattice) vs Q30, and — the second modeling
    family — **grid 2: integer coefficients (q = 1), zero per-step truncation**.

All synthesis/decode arithmetic is pure integer (encoder-side LS fitting may
use float; the shipped decoder never sees one). Determinism across processes
is a reversibility requirement, not a preference.

## 3. P1–P4 — phase coherence (v0.3)

Fixture: float64-generated damped sinusoid, 44,100 samples (λ = fs/8000,
period fs/48). Complete payload costs, including every format byte:

| Arm | Bytes | % of PCM |
|---|---|---|
| PERIOD control (lag 48) | 40,526 | 45.9% |
| f0 open-loop Q15 | 85,700 | 97.2% |
| f0 open-loop Q30 | 75,918 | 86.1% |
| **f1 closed-loop Q15** | **31,283** | **35.5%** |
| f1 closed-loop Q30 | 31,320 | 35.5% |

Receipt: `cargo test p1_phase_drift_comparison_over_44k_samples -- --nocapture`.

**The decomposition is the finding**: higher phase precision (P1) bought ~10%
on open-loop; closing the feedback loop bought >3×, because it removes drift
*accumulation* rather than shrinking its increment. Once closed, Q30 adds
nothing (gap ≈ 0) — with no compounding there is nothing left for extra
precision to fix. On exact rational fits (P2), corrections vanish entirely:
a lattice-native tone (p=1/512) encodes at 783 B for 6,000 samples (6.5%).
Anchor strides (P3) beat no-anchor at stride 64 and decay with length.
Description-length fitting (P4) ties or beats MLSE by construction: the MSE
candidate is always inside the sweep set, so DL ≤ LS is structural, and
*where* it pays is signal-dependent.

## 4. P5 — exact discrete dynamics (v0.4)

Stability closes the integer-oscillator space completely: R = r² must be a
non-negative integer with r ≤ 1 forces R = 1, and |C| ≤ 2r then forces
C ∈ {−1, 0, 1}. Exactly three undamped integer oscillators exist, with
periods 3, 4, and 6; **damping on the integer lattice is structurally
impossible** (real roots cannot both have modulus < 1 with R an integer ≥ 1).

Receipt: `cargo test p5_integer_dynamics_exact_and_tiny -- --nocapture`.
Measured: the three systems encode as header + mask table only — deltas vanish
entirely; ~6.7% of PCM at n = 3000 (~15×), the mask table (4 B per 32 samples)
being the structural floor. The theorem also explains *why* the continuous
family needs fixed-point grids at all: the exact-discrete and approximated-
continuous families are complementary, not redundant.

## 5. E1/P6 — mixtures (v0.4, honest negative)

The residual cascade (f3) reconstructs exactly (Σ rec_s = x by construction),
but the greedy two-stage cascade **ties** closed-loop-single on the two-part
fixture (4,512 bytes both at n = 4,000). Decomposition: the first stage's fit
captures the joint signal with little left for stage 2, and prediction-only
intermediate stages (the design that would avoid paying stage deltas) compound
drift exactly like open-loop f0. Retained as a measured negative; joint or
periodically re-anchored stage optimization is the named follow-up. Receipt:
`cargo test p6_mixture_cascade_beats_single p6b_stage_count_cost_curve
-- --nocapture`.

## 6. F1–F3 — feasibility geometry and description length (v0.5)

**F1 (tiers are real).** On the truncated-integer fixture (C = 64210,
R = 32000, floor division), the ideal real-valued strip region
x − ½ ≤ a·u + b·v < x + ½ is **empty by n ≈ 10** — integer truncation breaks
the linear superposition s = a·u + b·v that the strip math assumes — while the
shipped closed loop remains exact on the same samples by construction. On the
exact-evolving undamped fixture (C = 1, R = 1) the polygon is non-empty at
n = 600 with its centroid at the true seed, and the tier-2 lattice search
recovers the generator bit-exactly. The work order's warning — feasibility has
to be checked against the *actual deterministic decoder*, not the ideal
oscillator — is therefore a measured effect, not a caution.

**F2 (instruments).** H₀ ≥ conditional entropy holds on every fixture
(instrument sanity), and the shuffle probe pins the two measurements apart:
permuting a residual stream leaves H₀ bit-identical while conditional entropy
rises 2.22 → 3.39 bits — temporal structure is a separate, measurable quantity.
Binned cross-entropy fits a Laplace model *at integer bins*; density values are
never used as code lengths.

**F3 (latent vs closed loop).** Identical cost accounting, two dynamical
regimes:

| Fixture | Closed loop | Latent (round at output only) |
|---|---|---|
| Exactly-evolving (C = 1, R = 1) | exact + 2 structural bootstrap anchors | exact + **0 anchors** (anchor-free win) |
| Truncated (C = 64210, R = 32000) | exact + 2 anchors, **J = 28 bits** | drifts, L₀ = 898/900, **J = 1,706 bits** |

Neither direction is universal — precisely as the v0.5 work order predicted
(the "fivefold penalty" figure is assumption-dependent). Latent evolution wins
where the arithmetic is exactly representable; feedback wins wherever
truncation would otherwise compound. Both ship.

## 7. Rounding-cell geometry — the v0.6 decidability result

The v0.5 companion experiment (independent Python implementation) searched 75
parameter candidates for a Q40-phase/Q20-LUT/Q28-envelope latent decoder
against this fixture: 917.3 Hz, A = 12,000, damping 1.84e−5 per sample,
φ₀ = 0.37, generated by an independent float64 equation, n = 44,100. Best
candidate: **1,528 mismatches (3.46%), all corrections ±1, residual H₀ =
0.252 bits/sample**, idealized J ≈ 11,246 bits (≈ 1,406 bytes). That
experiment is a *local-search* result — it cannot say whether zero correction
disexists or merely hides.

v0.6 answers the question with geometry instead of search (`f1d`
+ `cell_geometry_search` + `latent_seed_search`):

- **Pinned seeds** (closed loop: the state is the reconstructed, hence rounded,
  run): violation floor **44,086 / 44,100**. The recurrence locks to rounded
  seed states; with |r| ≈ 1 the seed error persists forever. Unavoidable —
  measured, first violation at index 2, max deviation 3,033 units.
- **Free seeds** (latent family): the strip polygon **collapses at
  n = 44,100** — no (z₀, z₁) pair lies inside every rounding cell for any
  (C, R) on the Q15 lattice. The generator's r = e^−λ and cos(2πf/fs) are
  transcendental; the lattice can only approximate them, and feasibility decays
  with N exactly as the F1c collapse test predicted.

**Verdict: within the AR(2)-Q15 family, zero correction at this fixture is
unavoidable — proven by polygon emptiness, not bounded by search.** The two
experiments reconcile: the Python decoder's finer lattice structure (Q40 phase
increment, interpolated trig) shrinks the cell violations but cannot eliminate
them; its phase-drift floor (~1.7e−3 rad over the whole run) contributes
< 0.02 sample units, so the **trig/table quantization is the binding
constraint**, not phase accumulation.

**The decision rule this gives the project**: for any decoder family and any
target record length N, zero correction is achievable **iff the strip polygon
stays non-empty at N**. "Unavoidable vs undiscovered" is a decidable per-family
measurement — the geometry, not the search budget, owns the answer.

## 8. The pre-dimensionality rule

> **Before increasing a model's dimensionality, determine whether the data was
> produced by a simple discrete operation we have not identified.**

A residual stream that resists single-oscillator fits does not automatically
argue for more dimensions (more stages, coupled systems, higher-order
recurrences). The cheap hypothesis family — the simple discrete operations
with explicit, decode-exact inverses — comes first, because each one turns an
apparently many-dimensional signal into a few bytes of parameters:

- **superposition** x = a + b (sum of two exact generators — the P6 fixture's
  own constructor),
- **echo** x[n] = y[n] + g·y[n−D],
- **loop/repeat** x[n] = y[n mod L],
- **patch/copy** x[n] = y[n−D] on a span,
- **scale** x = g·y (integer gain, Q lattice),
- **downsample/interleave/cumsum** (D-class already covers diff-invert).

Measured support in this repo: the P6 tie is this principle in miniature — a
fixture *built by the simplest superposition* resisted a richer continuous
model (cascade) that never hypothesized the constructor. The manifold
project's fold probe states the same conclusion from the image side
("the redundancy is the mechanism, the re-layout is incidental"): the
repeat-rule beat every reordering arm it was compared to.

Operational form: before any new model family is built, the encoder runs the
discrete-hypothesis battery (each hypothesis = a plan in the same
serialize-and-count discipline, decoded by its explicit inverse) and only adds
dimensionality when every simple operation loses on measured J. The battery is
cheap (each arm is decode-cheap and few-parameter); the dimensionality is not.

### 8.1 The battery, measured (Audio-G1 classes, v0.7)

The 16-arm battery ran against regenerated 4,096-sample versions of the
Audio-G1 fixture classes (`examples/g1battery.rs`). Two measurements per the
review protocol: every arm individually (the arm list prints costs in bytes),
then the portfolio's measured-smallest choice. Portfolio results (PCM = 8,192 B):

| Fixture     | Winning model            | Encoded | % of PCM |
| ----------- | ------------------------ | ------- | -------- |
| ramp        | D2 recurrence            | 539     | 6.58%    |
| square      | OP loop L=64 (arm 7)     | 213     | 2.60%    |
| triangle    | OP loop L=128 (arm 9)    | 304     | 3.71%    |
| damped tone | OSC f3 cascade (var 8)   | 549     | 6.70%    |
| mixed tones | OSC f1 closed-loop       | 7,381   | 90.10%   |
| piecewise   | OSC f3 cascade           | 4,746   | 57.93%   |
| speech-like | OP echo arm              | 8,128   | 99.22%   |
| white noise | RAW literal (submode 1)  | 8,209   | 100.21%  |

Honest readings:

- **LOOP is the battery's first real win**: on exact lattice fixtures it ties
  or beats the discrete-phase OSC variants (square 213 B, triangle 304 B).
  The ramp still belongs to D2: the loop's base stream costs more than the
  second-order recurrence's parameters — a loop arm is a cost only when the
  base is small.
- **ECHO on speech-like content**: the arm nearly ties the portfolio (8,128 vs
  8,128 at 99.22%) but does not win; on its own generator class the exact
  fixture decodes with zero corrections. The measured ceiling stands: the arm
  bills its inverse y-stream as RAW varints (single-pass battery), so nested
  coding of y would be needed to beat structure-aware competitors. That
  nesting is deliberately deferred (G2 research line, see below).
- **RAW literal submode** (the G1-report request): selection now bills the
  smaller of zigzag-varint stream vs literal PCM bytes. White noise went from
  137.39% → 100.21% — expansion capped at 17 bytes of header.
- The battery caught its own falsifier: the `metric == serialize` invariant,
  extended to every arm on four fixture shapes (quasiperiodic, echo-of-
  recurrence, extreme noise, speech-like), exposed a run-count varint that the
  RLE accounting billed as 1 byte but the wire wrote as 2 (counts ≥ 128).
  Accounting exactness is now measured, not assumed — with the review's
  qualification that tests cover the tested shapes only, not every input.

Nesting (x = E_θ(F_φ(z)+q)+r) is the named G2 boundary: a controlled
two-stage experiment (echo, then one model for y) before any general
composition.

### 8.2 G2 — compositional echo, first experiment (v0.8-dev)

The v0.7 boundary question — does a compact sequence of compatible
transformations beat one flat model? — got its first controlled test
(`src/nested.rs`, research-only container; no released wire format; nothing in
`encode()`/the v0.7 wire changed). Nesting restricted to ONE inner model from
the standing portfolio; the published intermediate is the same i16-clamped
stream the flat echo arm publishes (no silent widening). The review's inner
corrections `q` are structurally zero: the portfolio's inner models are
lossless by their own invariant, so the container carries only echo-forward
corrections on `x` — which makes G2 exactly measure "does the portfolio
describe y more cheaply than flat echo's RAW billing".

Complete three-candidate results (flat ECHO = best arm 10..13; portfolio =
encode()'s measured-smallest; nested = the container):

| Test | Base (generated independently of the container) | flat ECHO | nested | portfolio | inner winner |
| ---- | ------------------------------------------------ | --------- | ------ | --------- | ------------ |
| G2-A | integer recurrence echo (gain Q15/2, D=16)       | 1,290     | 188    | 297       | OSC f3 Q15   |
| G2-B | square-lattice echo (gain 3·Q15/4, D=24)         | 2,422     | 294    | 401       | PERIOD lag 2 |
| G2-C | float-damped-tone echo (gain Q15/2, D=32)        | 4,129     | 195    | 580       | OSC f3 Q30   |
| G2-D | speech-like echo (gain Q15/2, D=40)              | 5,581     | 3,024  | 3,016     | OSC f1 Q15   |
| G2-E | white noise                                      | 12,572    | 9,573  | 8,209     | (rejected)   |
| G2-F | saturated wide intermediate (gain Q15, D=8)      | 1,226     | 47     | 50        | RAW varint   |

Acceptance gate results:

1. Exact PCM reconstruction for every nested candidate — **pass** (per-test
   roundtrip assertions plus a per-variant sweep over ALL 4 gains × 63 delays
   with a fixed inner on a mixed fixture).
2. Billed size == emitted size — **pass** (caught during development: the
   container initially billed a 14-byte header it does not write — headerless
   frame; the invariant test now enforces the headerless accounting).
3. Savings over BOTH controls on at least one independent fixture — **pass**
   (G2-C: float-generated base, 195 B vs 4,129 flat and 580 portfolio).
4. Rejection of unhelpful nesting — **pass**: on white noise the nested
   container (9,573 B) loses to the plain portfolio (8,209 B, RAW literal) —
   MDL keeps the flat winner; nesting buys nothing there.
5. No underflow/overflow/saturation mismatch — **pass**: G2-F drives the
   inverse recursion past ±32767 mid-stream (unsaturated intermediate
   published i16-clamped, exactly as the flat arm publishes) and still
   reconstructs exactly at 47 B vs 1,226 flat.
6. Reproducible per-arm results recorded — **this section**; the tests print
   all three candidates per fixture (`--nocapture`).

Honest reading. The nested intermediate was described by PORTFOLIO models
(OSC f3 cascade, PERIOD lag-2, f1 closed-loop) — meaning the composition's
value came from applying an existing exact model to the transform's inverse,
not from new math. G2-D (speech-like) shows composition near-parity: nested
3,024 vs portfolio 3,016 — the 8-byte edge goes to the flat portfolio, and
the record keeps that loss beside the wins. Noise rejects nesting. The
hypothesis survives its first experiment: composition shortened the complete
description on every structured fixture, largest exactly where the inner y
obeys a rule the flat arm cannot see.

### 8.3 G3 — charged ΔL over the representation space (v0.8-dev)

G2's question "does composition help?" is only half answered: a win can come
from the transformation or from a cheaper sweep. G3 measures the separation
with the charged metric — every transform parameter, correction, mask and
framing byte counted, both sides through the same measured cost machinery:

\[ \Delta L_T = L_{\mathrm{best}}(x) - \left[ L(T) + L_{\mathrm{best}}(T^{-1}(x)) \right] \]

Implementation (`src/g3.rs`, research-only): T ∈ {identity, diff, echo};
echo = the G2 transform exactly (gain id + swept delay 1..64, causal
inverse, forward corrections charged like the v0.7 flat arm — RLE masks +
zigzag deltas). "L_best" = the standing portfolio's measured-smallest
complete payload (`encode()`), so a transform can only win by exposing
structure no flat model sees. Results:

| Measurement                     | ΔL (B)      | Meaning |
| ------------------------------- | ----------- | ------- |
| identity control                | **0** exactly | accounting verified end-to-end |
| diff (D1) on two-part tones     | structurally ≤ 0 | the portfolio already contains this transform — a "won" ΔL here would have been a selection artifact |
| echo, independent damped tone (swept) | **+389** (full gain, D=64) | genuine transformation benefit, larger than any fixed-pair estimate |
| echo on noise (8 pairs swept)   | −1,491 … −5,326 | representation space rejected wholesale on structureless input |

Cross-validation: G3-A's swept +389 B vs G2-C's independently measured
385-B portfolio gap (195 vs 580) — two different measurements, one effect.
The transformation benefit on that fixture is real, quantified, and bigger
than the G2 report's fixed-pair reading (10 B) suggested; parameter sweep
is part of the value, and it is charged in ΔL.

Standing honest negatives: noise rejects every transform; the identity
control's exact 0 guards the accounting (any future transform whose ΔL
computes nonzero under identity has a billing bug, and the invariant will
name it).

### 8.4 paste-G3-A — integer lifting before/after the portfolio (v0.8-dev)

The work-order paste ranked reversible integer lifting as the easiest new
family ("apply lifting before the OP portfolio"). Measured on the
independent float-generated damped fixture and noise:

| Placement | Composition | Complete cost (B) | vs the control |
| --------- | ----------- | ----------------- | -------------- |
| direct | lift(x), two streams | 615 vs best(x) **580** | **+35 loss** |
| after echo (swept winner g3/D=64) | L(T_echo) + lift(y) = 3 + 0 + 102 + 100 = **205** vs echo-only 3 + 186 = **189** | | **+16 loss** |
| after echo (G2-C pair g1/D=32) | 3 + 290 + 300 = **593** vs 3 + 565 = **568** | | **+25 loss** |

Verdict: **lifting DENIED in every placement** on this fixture. Mechanism:
Haar lifting is stream-SPLITTING — it cuts one well-modeled stream into two
half-modeled streams whose serialized sum exceeds the unsplit cost, and the
portfolio already contains the diff family (D1/D2), so the transform finds
redundancy the portfolio's predictors already reach without paying the
split. On noise the lift honestly rejects (odd-stream values spill i16 —
no silent narrowing, the review's width rule held). This is the same law as
the G6 quaternary probe (manifold, same day): re-layout is not a mechanism.

Recorded for the record's completeness, NOT retained. The paste's remaining
rows (phase-trajectory discovery, coupled oscillators, source–filter) stay
queued in priority order.

### 8.5 G3-B — phase trajectories: the model discovers, the representation loses (v0.8-dev)

The paste's model (φ[n+1] = φ[n] + ω[n] mod 2^w, ω[n+1] = ω[n] + α; cos
synthesis from the baked Q15 table; A from max|x|; corrections on x − x̂):

| Contract row | Fixture (independent) | Phase model | Portfolio | Verdict |
| ------------ | --------------------- | ----------- | --------- | ------- |
| B2 linear sweep | float chirp 917→+2.7 kHz | 8,483 B | **4,520 B** (OSC f3 cascade, kind 4 var 8) | loss, pinned |
| B1 constant tone | float tone | (α=0 = control) | OSC family keeps the win | no regression |
| B3 vibrato | float vib 5 Hz/60 Hz | measured | | miss expected (sinusoidal ω ≠ α-chirp) |
| B4 damped sweep | float decay+sweep | measured | | recorded |
| B5 two tones | float mixture | | | miss PINNED (phase > portfolio) |
| B6 random FM | piecewise random f | | | rejection PINNED (phase > portfolio) |
| noise | iid | honest reject (spill) | | |

**Discovery worked. Representation lost.** The zero-crossing quadratic LS
recovers the trajectory from the signal (measured: ω = 917 Hz exactly, α
within ~10% of the generator's truth; fixture needs ≥ 8 crossings — the
documented dependence), yet the complete description pays 1.88× the
cascade's. The seven phase tests pass with B2's negative pinned.

**G3-B.1 — quantization ablation** (hold the phase law + coder; vary
only the waveform reconstruction; three reconstruction arms at the four
phase-reference offsets):

| Arm | Reconstruction | max \|r\| | H₀ (bits) | billed ≈ |
| --- | -------------- | --------- | --------- | -------- |
| Q0 | 24-bit acc, 12-bit LUT | 12,103 | 11.7 | 8,971 |
| Q1 | + fractional interpolation | 12,097 | 11.7 | 8,969 |
| Q3 | 48-bit acc + interpolation | **1,076** | 10.6 | **8,546** |

The ablation's answer to the review's qualification:
1. **LUT interpolation is NOT the dominant floor** (Q1 vs Q0 ≈ tied; pinned).
2. **The phase ACCUMULATOR WIDTH was** — at 24 bits, the α grid's rounding
   (±3e-8 rad/sample²) × n² = ±500 rad accumulated phase error = the measured
   ±250-unit tail error exactly (pinned: Q3 ≥ 5× below Q1).
3. **Correcting it still does not save the model** — Q3's residuals are 11×
   smaller yet the complete cost stays above the cascade (every position
   still codes ~±1-2 LUT ulp; an ideal H₀-coded residual ≈ 5.4 kB > 4,520).

**Science.md conclusion** (the review's wording, confirmed with the
ablation): the phase trajectory *successfully estimated nonstationary
frequency parameters but failed the complete-description-length test on an
independently generated linear sweep* — the current *decoder implementation*
loses (dominated by accumulator quantization, then by residual sparsity),
and the model itself has **insufficient MDL advantage on B2**.
Phase family CLOSED for B2-class fixtures; the qualification that "the
underlying phase-law representation is not proven intrinsically inefficient"
is answered in the model-favor direction by G3-B.1's finding that precision
is NOT the binding constraint — residual sparsity is.

### 8.6 G4 — additive modal models: the cascade's mechanism named (v0.8-dev)

G4's central question: does a jointly fitted small set of oscillatory
components beat the greedy cascade? Built (`src/modal.rs`, research-only):

- **G4-A greedy seed** = a spectral line scan (cos/sin correlation over a
  coarse 8 Hz grid → strongest line's (ω, φ), amplitude by 1-column varpro,
  then the next scan on the residual) — the greedy arm already in the
  f3 cascade, made explicit.
- **G4-B joint refine** = coordinate descent on each component's (φ, ω, A)
  against the summed reconstruction's COMPLETE billed cost, seeded by
  varpro LS amplitudes (K ≤ 2 closed-form).
- Two bugs the instruments caught en route: the accumulator-units
  conversion missed the /(2π) (a DC-ish 6.28×-overscaled "component" fit
  nothing — caught because the phase-advance arithmetic gives f =
  ω·fs/2^48/(2π), and the correlation magnitude collapsed ⇒ max|r| stayed
  ≈ the whole signal); and the scanned phase needed NO +π/2 (the
  correlation basis is (cos, sin) so atan2 IS the cos phase — caught by
  amp = 6.59 with comps never building).

Complete-cost table (billed == wire, exact decode, every fixture):

| Row | Fixture (independent) | modal best | portfolio | portfolio's arm |
| --- | --------------------- | ---------- | --------- | ---------------- |
| G4-1 | single 917.3 Hz tone | 11,036 | **2,839** | OSC (var 8) |
| G4-2 | two tones (917/1361) | 8,974 (K=3) | **6,052** | OSC (var 8) |
| G4-4 | two damped oscillators | 12,818 | **2,670** | OSC (var 8) |
| G4-6 | independent mixture (square + tone) | 11,350 | **7,584** | OSC closed-loop (var 2) |
| G4-7 | noise | 12,320 | **8,209** | RAW literal — rejection PINNED |
| G4-8 | saturated mixture | (exact) | | exactness enforced |
| B2 regression | linear sweep | 8,974 | **4,520** | OSC var 8 |

**Measured verdict — the additive arm DENIED, with the cascade's mechanism
NAMED**: the modal model's loss is the LUT-quantization floor (±1-2 ulp per
sample per component — the G3-B.1 ceiling reappears), which the f3 cascade
dodges by DECOMPOSING the signal into integer recurrences whose residual is
one rounding cell per position. The review's decisive inequality
(L_joint < min(greedy, cascade, portfolio)) never fires on any fixture;
the K-sweep shows diminishing returns (k=3 ≈ k=4) and even varpro's exact
amplitudes cannot close a 2-4× gap. The review's own counter-hypothesis
stands measured: **the cascade's advantage comes from its computational
structure (integer recurrences, per-stage exactness), not from
approximating several oscillatory components.**

The G2/G3-B.1/GB1 law, now measured three ways: the portfolio wins when
its representation reproduces the signal with cell-level error; every
LUT-per-sample family carries a quantization cost per sample that
compounding stages avoid.

### 8.7 G5 — innovations anatomy: the cascade's residuals are 66% coder-entropy (v0.8-dev)

The diagnostic (`src/g5.rs`; replays the winning cascade's stage loop — same
LS seed, candidate neighborhood, closed-loop reconstruction — and measures
per stage): on the G4-2 fixture (two independent tones, cascade winner):

| Measurement | Value | Meaning |
| ----------- | ----- | ------- |
| stage-0 H₀ | 7.809 bits | marginal information of the corrections |
| stage-0 H(R\|Rₚ₋₁) | **3.716** bits | **52% of the marginal entropy vanishes when the PREVIOUS correction is visible** |
| coder gap | 5,518 B coded vs 1,892 B conditional floor | **65.7%** conditional gap |
| ACF peak | lag 1, r = 0.505 | strong adjacent-position dependency |
| state-conditional spread | 2.18× | the recurrence's internal magnitude predicts the correction's size (G5-D) |
| stage 1 | residual NOT oscillator-like (LS degenerate) | greedy cascade = 1 stage here; multi-stage structure lives inside the one recurrence's corrections |

**G5-E confirmed the structure is exploitable**: coding the deltas through a
D1 CONTEXT (r'[n] = r[n] − r[n−1], same zigzag/RLE machinery, honest billing
both sides) = **4,489 B vs 5,518 B (−18.6%)** on the winning model's own
residual stream — measured, reproducible, and exactly what the conditional
entropy predicted (Hcond/H0 = 0.48 ⇒ big temporal redundancy).

**Decision (the review's rule)**: a reduction in conditional entropy was
demonstrated ⇒ a context/correlation model on the cascade's correction stream
is JUSTIFIED as the next wire milestone (payload VERSION 5; residual
D1-context field). No new generative family until that ships — the
"remaining bits" are not unpredictable; the coder hasn't been claiming them.

The law refined (the review's wording + measured): efficient audio
representation = compact dynamics + low-cost exact innovations; the
innovations were NOT at their floor, at a 65.7% coder gap.

G5-F (real audio) stays queued pending a corpus (the honest row).
### 8.8 G5-F — real-recording validation (CC0 corpus, hash-locked)

Corpus (`testdata/g5f/`, fetched + sha-256-verified by `scripts/g5f_fetch.sh`;
no resampling; stereo channels measured separately — `parse_wav_multichannel`
joined the LIB in this milestone for the harness):

| Recording | Format | whole-file portfolio | PCM | ratio |
| --------- | ------ | -------------------- | --- | ----- |
| example_word.wav ch0/ch1 | stereo 44.1k | 55,710/55,719 B | 77,824 B | 71.6% |
| can_you_stop.wav ch0 | mono 24k | 113,946 B | 165,866 B | 68.7% |
| sample2.wav ch0/ch1 | stereo 44.1k | 437,335/447,532 B | 1,217,024 B | 35.9-36.8% |

(The portfolio includes the corpus WAV header cost only at the 44-byte framing;
percentages are against raw PCM frames. Fixed-window form also recorded in the
test's stdout — the whole-file form wins everywhere.)

**Outcome 1 — residual predictability EXTENDS to real audio, stronger**:
the cascade's stage-0 residual Hcond/H0 ratio = 0.41/0.41/0.21/0.55/0.54
(synthetic fixture was 0.48): half or more of the residual entropy on real
speech is conditional structure.

**Outcome 2 — the naive promotion DENIED**: the D1-context residual coder
(the G5-E synthetic win, −18.6%) LOSES on real recordings in every row
(+10.8% example_word, +3.6% can_you_stop, +4.7% sample2 — billed both
sides). Mechanism: real speech's portfolio-extracted residuals carry large
isolated deltas (phonetic transitions); differencing spreads each large
delta across adjacent pairs, lengthening the zigzag codes. The conditional
redundancy is real but NOT lag-1-adjacent — an extractor keyed on the
stage's internal STATE (the G5-D 2.18× spread) is the remaining candidate,
not delta-adjacency.

**Outcome 3 — the complete portfolio on real sound**: the shipped v4 wire
already reconstructs real recordings at 36-72% of PCM (speech class) with
exact reconstruction — the strongest real-audio positive so far.

**Wire-milestone decision**: payload VERSION 5 (residual context coder) is
NOT justified by the lag-1 context as-is — the v5 milestone's design brief
changes to state-conditioned residual modeling (the G5-D instrument's bucket
structure), gated on first demonstrating positive ΔL against the shipped
coder on the hash-locked corpus through the release gate. The G3-B/G4
lesson repeated in the innovations domain: a coder hypothesis earns its
wire change only with the measured advantage — the synthetic-only win did
not survive contact with real sound.

### 8.9 v5-S — state-conditioned residual coding (DENIED in histogram form)

The G5-F review's hypothesis, built as `src/context.rs` (research-only; the
released wire untouched): residual VALUES unchanged, coding PROBABILITIES
bucketed on the decoder's causal state |y[n-1]| (LS-seeded stage-0 replay in
`g5::anatomy_positions`). S1 global histogram vs S2 (2 buckets) vs S3/S5
(4 buckets); thresholds synthetic-frozen (8192/16384/24576 — held-out rule
honored, real corpus never tuned); S6 = randomized-context negative control.

Synthetic two-tone: S1 4,945 B — S2 5,309 B — S3/5 5,701 B (MORE buckets
MONOTONICALLY WORSE: splitting a near-geometric residual alphabet across
more per-bucket histograms pays an estimation tax exceeding the conditional
gain; S6 control 7,664 B ≥ S3 — machinery sane).

Locked real corpus (shipped vs ideal-histogram forms): shipped coder WINS
in every cell — example_word 53,182 vs 61,543 B; can_you_stop 95,573 vs
166,597 B; sample2 430,712 vs 490,074 B. dL_complete NEGATIVE everywhere;
even the ONE-bucket histogram form loses to zigzag-RLE (the mask already
carries the zero/nonzero split for free).

**Verdict**: state-conditioned PROBABILITY modeling is DENIED in its
static-histogram form. The unexplored remainder is explicitly recorded: an
ADAPTIVE arithmetic (range) coder where P(r|c) is learned without transporting
per-bucket histograms — a machinery-level change, not a re-histogramming.
State information EXISTS (the G5-D 2.18x correlation, the S6 control), but no
measured exploit through this route. v5 remains NOT justified.
### 8.10 G5-G — adaptive probability learning (CLOSED; the v5 residual-context investigation ends)

The review's arms (`context.rs::g5g`, research-only): sequential Laplace log-
loss on the SAME masked corrections (zero/nonzero never re-coded — the review's
section 4 holds by construction), scale adapted by EMA over decoded history;
A1 adaptive-global, A2 2-state, A3 4-state (thresholds unchanged),
A5 randomized control. Gate-1 instrument only — no range coder built, because
the margins below make an engine moot.

| Stream | shipped (A0) | A1 global | A2 2-state | A3 4-state | A5 random |
| ------ | ------------ | --------- | ---------- | ---------- | --------- |
| synthetic | 690 B | 4,647 B | 4,657 B | 4,618 B | 4,679 B |
| example_word | 6,426 B | 43,599 B | 43,599 B | 43,599 B | 43,789 B |
| can_you_stop | 12,263 B | 79,387 B | 79,396 B | 79,399 B | 79,878 B |
| sample2 | 61,164/62,370 B | 434,139/438,876 B | identical | identical | +0.5% |

Findings, per the review's three gates:
1. **Gate 1 (conditioned beats global adaptive) FAILED** — A2 = A1 within
   0.02-0.5% (log-loss noise); the recurrence state adds NO measurable
   conditioning value beyond what global adaptation already extracts.
2. **Gate 2 (fewer bytes, same engine) unreachable** — the precondition
   (Gate 1) failed; the state hypothesis has nothing to pay for a coder.
3. **Gate 3 (beat shipped) FAILED by 6.7-7.1x** — the corrections are sparse
   and transient: an online scale model overshoots after the first few large
   values and pays a fixed per-symbol cost zigzag-RLE never pays. The 6.7x
   margin is beyond any engine refinement (range coding recovers fractions,
   not factors).

The review's boundary rule fires verbatim: adaptive state-conditioned coding
could not beat the global adaptive control NOR the shipped coder on the locked
corpus — the v5 residual-context investigation is CLOSED. Remaining recorded
positives: the state/residual correlation is real (G5-D) but no causal
probability exploit at or above the shipped coder's floor exists on tested
routes. Research redirected per the review: new acoustic structure, or a
broader recording corpus, not coder refinement.
### 8.11 G6 — four-dimensional acoustic dynamics (DENIED; event-compaction already latent in the portfolio)

The review's 4D experiment (`src/g6.rs`, research-only): mono PCM re-imagined
as observations of a small evolving integer state (q, v, e, d). Arms billed at
complete cost with IDENTICAL corrections coding (the shipped zero-run RLE +
zigzag): osc2 (the standing 2-state family), str4 (structured two-integrator
+ decay: v' = v − qκ − vρ, q' = q + v, Q15 fixed point, per-strike state
resets), gen4 (generic 4×4 linear state — the skeptic).

| Fixture (4,096 samples) | shipped portfolio | osc2 stage0 | str4 4D | gen4 |
| ----------------------- | ----------------- | ----------- | ------- | ---- |
| 4D-1 struck damped sine | **541 B** | 4,625 B | 11,253 B | 12,840 B |
| 4D-2 three identical strikes | **571 B** | 3,748 B | 10,244 B | — |
| 4D-3 coupled resonators (exact integer state) | **8,209 B** | 8,578 B | (corrections dense) | — |
| 4D-4 real speech (can_you_stop ch0) | **113,946 B** | 112,326 B | — | — |
| 4D-5 white noise | 8,209 B | — | 11,740 B | — |
| 4D-6 saturated (a0 = 3×full-scale) | exact ✓ | | | |

Findings:
1. **4D-2's event-compaction is ALREADY in the shipped portfolio** — one strike
   541 B vs three identical strikes 571 B: +30 B for two extra events. The
   standing families' periodic/loop structures carry repeated-event structure
   more cheaply than a 4-state model with resets.
2. The structured 4D arm loses 18-21× to shipped on its own target classes:
   the struck-resonator generator class IS inside the shipped oscillatory
   family's closure (the damped sine is exactly a 2-state Q15 recurrence with
   a decay term), so the extra (e, d) dimensions bill parameter bytes and
   dense corrections without new modeling power.
3. The generic 4×4 skeptic arm loses even harder (12,840 B; its 12 free
   entries buy no fit that the family search doesn't find cheaper) — the
   review's skeptical prediction ("may simply reproduce AR(4)") measured TRUE,
   with the additional wrinkle that the SHIPPED 2-state family is already the
   better AR(4)-class fit at lower parameter cost.
4. Noise REJECTED (4D-5 ✓): MDL picks the cheaper model; saturation exactness
   holds (4D-6 ✓).

**Verdict**: the four-dimensional DYNAMICAL hypothesis is DENIED for mono PCM
under the complete-cost metric. The 4D branch's real value (spacetime
acoustics, Green's functions, source-receiver geometry) requires spatial
measurements mono cannot provide — reserved for the spatial-audio future as
the review itself framed it.
### 8.12 G7 — acoustic structure attribution (audit + transfer + perturbation + persistence)

The G6 review's milestone (`src/g7.rs`, research-only), four arms:

**G7-A — the G6-4D-4 audit (the review's order)**: the 1.42% stage0-vs-
shipped "win" was an ACCOUNTING ARTIFACT. The stage-0 row billed
corrections-only costs (mask RLE + zigzag varints + 3 varint coefficients),
omitting the production framing (header, y0/y1 seeds, 4-byte-coefficient
fields, the raw mask words the kind-4 families actually write). Re-billed
with the EXACT wire via payload_len over every OSC variant:

| Stream | shipped | best single OSC variant | shipped advantage |
| ------ | ------- | ----------------------- | ----------------- |
| example_word ch0 | 55,710 B | 60,539 B (var2) | 4,829 B (8.7%) |
| can_you_stop ch0 | 113,946 B | 115,419 B (var2) | 1,473 B (1.3%) |
| sample2 ch0 | 437,335 B | 542,767 B (var8) | 105,432 B (24.1%) |

The COMPOSITE portfolio (cascade + its per-stage accounting) IS the winning
representation on real speech — no unexplained audit gap remains.

**G7-C — causal perturbation** (±2 Q15 units on a discovered stage's
coefficients, can_you_stop w0): base 304 bits → +104..+160 bits (+34-53%)
for every perturbation. Both coefficients are load-bearing; the compactness
is genuinely carried by the discovered (c, r), not the surrounding coding.

**G7-B — event transfer** (parameters from recording half A applied to half
B, vs B's own fresh fit): transfer LOSES by 8-16% on all three streams
(29,970 vs 24,175 B; 41,258 vs 37,895 B; 271,939 vs 249,088 B). Within one
recording, the per-window stage parameters do NOT transfer — the fitted
law is window-local, not a persistent physical system.

**G7-D — coefficient persistence**: stage-0 (c, r) pairs across windows:
99/100 distinct (sample2), 16/20 (can_you_stop), 9/9 (example_word) —
consistent with G7-B: no per-window persistent physical law. The compactness
the portfolio achieves on speech comes from the CASCADE's residual-splitting
(many cheap local stages), not from a globally valid acoustic equation.

**Implication for the ledger**: "which physical relationship won?" answered —
on real speech, NONE persists across windows; the win is the cascade
MACHINERY splitting residual structure into many short-lived local models.
A transfer/physics claim (G7-B's ambition) needs parameters valid across
events; measured here, they are not. Honest next ground per the original
review: a broader recording corpus (music/environmental) IF any transfer
hypothesis is revisited — with these floors as the starting baselines.
### 8.13 G8 — coefficient dynamics (CLOSED at G8-A/B; the budget is 0.1%)

The G7 review's next hypothesis (`src/g8.rs`, research-only): the cascade's
locally fitted coefficients may carry their own compact dynamics. The review's
G8-A gate — measure the coefficient budget BEFORE any adaptive machinery —
fires the early-closure clause verbatim.

**G8-A — the coefficient budget (the entire G8 upside)**: the shipped
family-3 cascade carries each stage's (c, r) as two zigzag varints:

| Stream | windows | payload bytes | coefficient bytes | share |
| ------ | ------- | ------------- | ----------------- | ----- |
| example_word ch0/ch1 | 9 | 53,182/53,230 B | 54 B | **0.10%** |
| can_you_stop ch0 | 20 | 95,573 B | 83 B | **0.09%** |
| sample2 ch0/ch1 | 100 | 430,712/440,541 B | 404/402 B | **0.09%** |

Even PERFECT elimination of coefficient transport (impossible — the decoder
needs the values) buys under 0.1% on every stream.

**G8-B — delta trajectories** (are the CHANGES predictable?): H(delta-pairs)
= 6.63 vs H(c,r-absolute) 6.62 on sample2 — the deltas carry NO additional
structure; 3.00 vs 3.17 / 3.91 vs 4.00 likewise within noise. And 0 of 125
stage r2 values sit within 64 Q15 units of Q15: every cascade stage is a
DAMPED pole — the coefficients genuinely roam (consistent with G7-D's
distinct-pairs finding) rather than tracing a recurrence near pure
oscillation.

**Verdict**: G8 CLOSED. The hierarchical hypothesis — an equation governing
which equations describe successive regions — is priced at under 0.1% of the
description on the locked corpus: there is nothing meaningful for coefficient
dynamics to save. This is the cleanest possible honest negative: the budget
ceiling was measured first, and it is two orders below the smallest gains the
ledger records anywhere.
### 8.14 G9 — bitstream anatomy (COMPLETE: where the winning bytes go)

The review's harness (validator 5/5) + the production-writer ledger
(`src/g9.rs`, test-gated): the walker reads the exact byte layout `serialize()`
emits — same headers, field order, encodings — with every byte covered exactly
once, no gaps/overlaps/unknown categories, asserted per stream (the review's
five checks as test invariants; the ledger comes from the real writer's
layout, killing the G7-A estimator class of error).

**Anatomy (locked G5-F corpus, whole-file payloads + two synthetic kinds)**:

| Stream (kind) | total | metadata | masks | residuals | coefficients | seeds/other |
| --- | --- | --- | --- | --- | --- | --- |
| example_word ch0 (D1) | 55,710 B | 17 | 0 | 55,693 (99.97%) | 0 | 0 |
| example_word ch1 (D1) | 55,719 B | 17 | 0 | 55,702 | 0 | 0 |
| can_you_stop ch0 (PERIOD-2) | 113,946 B | 17 | **10,368 (9.1%)** | 103,560 (90.9%) | 1 | 0 |
| sample2 ch0 (D1) | 437,335 B | 17 | 0 | 437,318 (>99.99%) | 0 | 0 |
| sample2 ch1 (D1) | 447,532 B | 17 | 0 | 447,515 | 0 | 0 |
| two-tone (OSC cascade) | 6,052 B | 17 | **512 (8.5%)** | 5,515 (91.1%) | 8 | 0 |
| silence (OP loop) | 23 B | 17 | 3 (RLE) | 0 | 0 | 3 |

**Findings**:
1. **The bytes are the residuals** — 91-100% of every real-speech payload.
   The portfolio's wins are won inside the zigzag-RLE correction stream; no
   other category registers above 9.1% anywhere measured.
2. **Masks are the only remaining measurable cost on real speech** — exactly
   9.1% of can_you_stop's payload (the flat u32 mask table on PERIOD/D1/D2/OSC
   families — the same structural floor P5 exposed; the mask RLE is wired
   ONLY for OP payloads, where it bills 3 B on silence). On tonal cascade
   payloads the per-stage mask segments cost 8.5%.
3. Metadata/coefficient/seeds/other: ≤0.02% combined on real speech —
   G8's 0.1% coefficient bound reconfirmed at the wire; there is nothing
   else to optimize outside masks + residuals.
4. The honest ceiling for any future masks work: ~9% on SOME recordings,
   ~0% on others — a candidate WIRE change (mask RLE outside OP needs a
   version bump + the full release gate), recorded for a version-bump
   decision, NOT made here.
### 8.15 G10 — uniform mask representation (measured; small real win, wire decision open)

The G9 review's mask-only ablation (`src/g10.rs` + `g9::extract_mask_segments`,
research-only): M0 = shipped fixed-width words (frozen control), M1 = the
verified OP zero-run-first RLE billed from the REAL writer, M2 = adaptive
min(M0, M1) + 1 B selector. Residual bytes untouched — ΔL_complete =
ΔL_mask − selector exactly.

Boundary cases verified (the review's required list): all-zero, all-one,
sparse, dense, set-word start, alternating runs, 127/128 and 16383/16384
varint-boundary runs, exact mask round-trips, per-stage segment boundaries
preserved, dense/random fallback to fixed (asserted), metric == bytes.

| Stream (kind) | mask words | M0 | M1 | M2 | ΔL_complete |
| --- | --- | --- | --- | --- | --- |
| example_word ch0/ch1 (D1 — no mask words) | 0 | 0 | 0 | 0 | 0 |
| can_you_stop ch0 (PERIOD-2) | 2,592 | 10,368 | 7,514 | 7,515 | **+2,853 B (2.5% of payload)** |
| sample2 ch0/ch1 (D1) | 0 | 0 | 0 | 0 | 0 |
| two-tone (OSC cascade, 1 stage) | 128 | 512 | 516 | 513 | **−1 B (RLE loses)** |

Findings:
1. RLE realizes its promise exactly where masks are SPARSE AT THE WORD LEVEL
   (period-lag streams with corrections clustered): −2,854 B on
   can_you_stop's mask table. On DENSE word masks (the tonal cascade's
   correction stream) it LOSES (516 vs 512 B; with the selector −1) — the
   review's "RLE can lose on irregular masks" measured true, and M2's
   adaptive selection is therefore REQUIRED, not optional.
2. The corpus-wide honest ceiling: +2,853 B of +607,000 B ≈ **0.47%** —
   G9's 9.1% single-payload bound was the ceiling; RLE realizes 27.5% of
   the mask bytes on the one stream where they exist, which is 2.5% of
   that payload.
3. D1 speech (the dominant kind on the corpus): zero mask bytes — there is
   nothing to save. The aggregate win is bounded by the PERIOD-class
   payload share.

Wire-milestone decision (OPEN, user's call): extending mask RLE beyond OP
would need a payload VERSION bump + the full five-stage release gate for a
measured ~0.5% corpus-wide saving (max measured 2.5% on PERIOD-class
payloads). Recorded honestly both directions: real but small; the residual
stream (91-100% of bytes) remains the materially larger program.
### 8.16 G11-A — residual information audit (measurement-only; the gap is real but its recoverable share is closed by evidence)

After G9 the audit question: are the residual bytes near their limit?
(`src/g11.rs` + `g9::ledger_bits`, research-only; the review's three
measurements per stream on the EXACT production symbol sequences:
H0 empirical symbol entropy, lag-1 conditional entropy, actual wire bytes;
G = actual − H0-coded ideal).

| Stream (kind) | symbols | actual | H0-coded | Hcond-coded | G vs H0 |
| --- | --- | --- | --- | --- | --- |
| example_word ch0 (D1) | 38,912 | 55,693 B | 44,460 B | 27,338 B* | 11,233 B (20%) |
| example_word ch1 (D1) | 38,912 | 55,702 B | 44,459 B | 27,333 B* | 11,243 B (20%) |
| can_you_stop ch0 (PERIOD-2, sparse corr + mask) | 57,991 | 113,926 B | 82,455 B | 27,605 B* | 31,471 B (28%) |
| sample2 ch0 (D1) | 412,187 | 437,318 B | 367,035 B | 354,004 B | 70,283 B (16.1%) |
| sample2 ch1 (D1) | 412,187 | 447,515 B | 373,670 B | 359,566 B | 73,845 B (16.5%) |
| two-tone (OSC cascade) | 4,073 | 6,027 B | 3,975 B | 1,890 B* | 2,052 B (34%) |
(*Hcond = lag-1 pair-counting — see the bias note.)

Findings, separated honestly:
1. **The stable, mechanism-explained gap: the integral varint floor** —
   long streams (sample2-class) sit 16-16.5% above their H0 code. The varint
   pays whole bytes per symbol; a fractional-bit backend would keep the
   difference. This is the ONLY gap that survives scrutiny on long streams.
2. **The conditional-entropy promises COLLAPSE under coder evidence**:
   example_word's Hcond says −38% and can_you_stop's says −58% — but
   adjacency-coded variants of exactly these streams ALREADY LOST when
   actually coded (G5-F: D1-context +3.6..+10.8%; the same files), and
   G5-G's adaptive-context closed the last conditioning route on both
   correction streams (A2 = A1) and vs the shipped coder (6.7-7.1x).
   The Hcond columns carry the small-n pair-counting bias visible in
   Hcond < H0/2 patterns at n < 60k (the review's own caveat, measured).
3. **The two-tone cascade's 34% gap** is the integral floor on a short
   dense stream (mean ≈7.8 b/sym, varint pays 8+); the same stream's
   contextual promises were priced and closed in G5-G.
4. **The recoverable-share arithmetic** the review asked for: corpus-wide,
   G vs H0 ≈ 16% on long streams; the fraction REACHABLE by an evidenced
   mechanism is 0% by G5-F/G5-G closure (context/conditioning LOST) plus
   whatever an arithmetic coder could keep of the varint floor — a real
   range-coder build gated on beating shipped by >0.5% while paying its
   own model/tables — the exact form the review's earlier boundary
   preserved as the "only surviving route."

**Next-step verdict**: on existing speech, the audit finds no evidence for
substantial recoverable savings beyond the (closed-by-evidence) floor; per
the review's own rule, the corpus expansion to instruments/environmental
recordings is the priority. Measurement-only retained; no predictor,
transform, or entropy coder introduced.
## 9. Standing principles (the honest-results contract)

1. Complete accounting, always: fields, masks, deltas, headers; selection
   metric == serialized bytes, tested per candidate.
2. Same fixture for both arms of any comparison; losses ship beside wins.
3. The three criteria — physical fidelity, numerical fidelity, information
   efficiency — stay separate; no composite score exists.
4. A passing test with the wrong cause is a bug: every research claim carries
   the test that would break if the engine changed underneath it.
5. The engine does not optimize approximation accuracy; **it searches for the
   smallest complete mathematical description that reproduces observations
   exactly** — oscillator dynamics now, nonlinear systems and spatial acoustic
   fields later.

## References

- [README.md](../README.md) — engine overview, wire layout, variant table.
- [CHANGELOG.md](../CHANGELOG.md) — per-milestone changes with measured notes.
- Work-order memos (2026-10-27): phase-coherent dynamics (P1–P4); exact
  discrete dynamics; independent-signal feasibility; rounding-cell geometry.
  The memos' hypotheses are quoted where they shaped an experiment; every
  verdict above is this repo's own measurement.