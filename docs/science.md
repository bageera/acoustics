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