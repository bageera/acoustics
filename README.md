# manifold-acoustics

**ACOUSTICS — Mathematical & Physics-Informed Audio Research Engine** — v0.11.1

A reversible audio-model codec research engine. Every payload decodes to the exact original samples; every experiment pays a measured, explicit information cost.

**Current state (2026-11):** wire v7 (`RAW`/`D1`/`D2`/`D3`/`D4`/`D5`/`PERIOD`/`OSC`/`OP 0..15`, adaptive mask-RLE on PERIOD/OSC, original RLE on OP); 94 tests + 11 corpus-gated tests; published to [crates.io](https://crates.io/crates/manifold-acoustics) as 0.7.0 → 0.8.0 → 0.8.1 → 0.8.2 → 0.9.0 (D3 promotion) → 0.10.0 (D4 promotion) → **0.11.0 (D5 + adaptive masks, the combined v7 candidate)** → **0.11.1 (decoder security and validation — the total-decode contract enforced, resource model documented)**. The release gate `scripts/release-gate.sh` runs five stages: fmt/clippy/release tests + exact-CLI roundtrip → `package --locked` + tarball audit → consumer FROM THE PACKAGED CRATE → numeric regression → publish behind `ACOUSTICS_RELEASE=1`. The research record runs the G-line — attribution (G7), coefficient budget (G8), bitstream anatomy (G9), uniform masks (G10), residual audit (G11-A), acoustic diversity (G12), D3 residual coding (G13), the D4 follow-up (G14), the D5 ladder probe (G15), the v7 four-arm gate (G16) — every result with its honest verdict in [docs/science.md](docs/science.md) §8; highlights below.


The measured research record — foundations, per-milestone experiments, and the
honest-results contract — lives in [docs/science.md](docs/science.md)
(index: [docs/README.md](docs/README.md)); every number there cites a
committed test that reproduces it.

## Foundations (v0.2–v0.5): feasibility geometry + description-length ranking (F1–F3)

Implemented in Rust with **no external crate dependencies**.

### Models

The encoder serializes each candidate completely *in the payload format itself*, then ships the smallest:

| Model | Representation | Body |
|-------|----------------|------|
| `RAW` (0) | exact reference | one zigzag varint per sample |
| `D1` (1) | first-order differences | residual stream (first sample coded directly) |
| `D2` (2) | second-order differences | second-derivative residual stream |
| `D3` (6, v5) | third-order differences | third-derivative residual stream — the G12/G13 class-conditional win: piano +14-17.8% vs the complete v4 portfolio (held-out, both channels) — selectable (broadband rejects, so portfolio picks it only when it wins) |
| `D4` (7, v6) | fourth-order differences | — the G14 class win: +3.1-4.0% beyond D3 on the REAL piano vs the complete v5 portfolio (held-out, both partitions); selectable (birds/river reject) |
| `D5` (8, v7) | fifth-order differences | — the G15 continuation (piano +0.8-1.2% vs the complete v6 portfolio, both partitions), shipped ONLY bundled with the adaptive mask layer (the G16 four-arm gate); selectable (birds/river/noise reject) |
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
| 6 | f0 | **exact discrete dynamics** — integer coefficient lattice q=1, zero per-step truncation (v0.4 P5) |
| 7 | f1 | exact discrete dynamics, closed loop (v0.4 P5) |
| 8–10 | f3 | residual cascade ×K stages (v0.4 mixtures): stage s closed-plan-fit on the residual seen so far, deltas coded, Σ rec_s == x exactly |

**f2 (P2 rational frequency + P3 re-anchoring):** exact rational frequency `p/q` on an
integer phase lattice with `q in {512, 1024, 2048, 4096}` — the LUT angle
`position * 4096 / q` is an exact integer, so lattice-native periods carry zero
frequency-representation error. Quarter-wave cosine LUT (Q15, baked constants); Q15
envelope pow with per-segment reset **anchors** every `2^(stride_sel+5)` samples
(P3's segment-vs-correction tradeoff); anchor positions are always coded.

**Exact discrete dynamics (v0.4 P5, the memo's second modeling family):** grid 2 sets
the coefficient lattice q = 1 — the recurrence runs with *zero* per-step truncation.
Stability closes the space exactly: R = r² must be a non-negative integer with r ≤ 1
forces R = 1, and |C| ≤ 2r forces C ∈ {−1, 0, 1} — precisely three undamped integer
oscillators, periods 3, 4, and 6. Damping on the integer lattice is structurally
impossible, which is *why* the continuous family needs fixed-point grids at all: the
two families are complementary, not redundant. Coefficient fields are zigzag varints
((1,1) costs 2 bytes vs the fixed 8-byte i32 pair).

**Residual cascade (v0.4 E1/P6, variants 8–10):** stage s runs a closed-loop plan on
the residual seen so far (greedy coefficient pick by residual energy, ≤3 stages),
codes its own deltas; rec_s reconstructs level_s exactly, the level ladder is
decoder-computable, and the sum of the reconstructed streams equals x exactly.
Measured honest negative: the greedy cascade *ties* closed-loop-single on the
two-part fixture (4,512 B both) — greedy stage fits leave little for later stages,
and prediction-only (uncoded) intermediate stages compound drift like open-loop f0.
Joint or re-anchored stage optimization is the named follow-up.

**P4 (description-length fitting) is the plan search itself:** least squares seeds a
coefficient neighborhood (±2 lattice steps around the MSE-rounding point); the encoder
evaluates the complete serialized payload — fields, masks, every delta — for each
candidate and the smallest wins. The MSE candidate is always inside the sweep set, so
DL selection ≤ LS selection structurally.

Binary layout: `ACUSTCS1` magic (8) + version (1) + model kind (1) + sample rate u32 LE (4)
+ sample count varint + model body. The body opens with the kind's parameter varint
where one exists (PERIOD lag, OSC variant id, OP arm id, RAW submode 0..1), then the
per-kind payload (dense kinds: pure residual varint stream; PERIOD/OSC: adaptive mask
section + field/header data + residual stream).

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
the scientific finding. Q30 (P1) bought ~10% on open-loop. Feeding corrections back
into the prediction (closed loop, P3's limiting case) bought >3×, because it removes
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

### Measured v0.4–v0.5 results

- **Exact discrete dynamics (P5)**: the three stable integer oscillators
  (periods 3/4/6) encode as header + mask table only — deltas vanish entirely;
  ~6.7% of PCM at n=3000 (~15×), with the mask table (4 B per 32 samples) as the
  structural floor. Exactness is proven, not fitted: coefficient lattice q = 1 has
  no truncation to drift.
- **Mixtures (P6, honest negative)**: the greedy residual cascade ties
  closed-loop-single on the two-part fixture (4,512 B both) — measured, recorded,
  and named as the v0.5+ target (joint/re-anchored stage optimization).
- **Feasibility tier separation (F1)**: on truncated dynamics the ideal-real-valued
  strip region is empty by n≈10 while the decoder-defined closed loop stays exact;
  on exact-evolving dynamics the polygon is nonempty at n=600 with the centroid at
  the true seed, and the tier-2 search recovers the generator bit-exactly.
- **Entropy instruments (F2)**: H0 ≥ conditional entropy holds on every fixture;
  shuffling leaves H0 invariant while cond₁ rises 2.22 → 3.39 bits — the two
  measurements are separate instruments, per the work order.
- **Latent vs closed loop (F3)**: latent-state wins anchor-free on exactly-
  representable dynamics (0 vs 2 coded samples); closed loop wins decisively on
  truncating dynamics (J 28 vs 1,706 bits). Neither direction is universal.

### Guarantees

- **Exact reversibility** — decode reconstructs the original samples bit-for-bit (not
  necessarily the original WAV file's metadata or layout).
- **Measured information cost** — model selection compares real serialized byte counts;
  a test asserts the selection metric equals actual serialized size for every candidate
  family, grid, and PERIOD lag.
- **Total decode** — malformed, truncated, or hostile payloads are rejected with an
  error, never a panic and never invented samples. The contract is enforced, not
  asserted, at every boundary:
  - varint overrun / u64 overflow; residual accumulation overflow (checked i64);
  - unstable or envelope-escaping dynamics; invalid lattice geometry;
  - every OSC/OP coefficient and selector field validated against the encoder's
    reachable set;
  - a 2^27-sample decoder ceiling restricting the maximum ACCEPTED payload —
    including otherwise wire-valid claims above it (resource model: SECURITY.md);
  - mask-run allocation caps; count-derived buffers propagate allocation failure
    as an error rather than aborting.

Sample-rate metadata rides in the payload header (no companion file).

## Usage

Requires a Rust toolchain (`rustup`). Quick start: `make check-deps`, then
`make` (fmt + clippy + release tests) — `make help` lists everything, including
`make roundtrip` and `make test-corpus`.

```
cargo run --release -- benchmark input.wav output.acoust   # encode + print model/size stats
cargo run --release -- decode output.acoust restored.wav   # exact decode to 16-bit WAV
cargo run --release -- info output.acoust                  # describe a payload

cargo test                                                  # 94-test suite
cargo test -- --nocapture p1_phase                          # print research measurements
```

Input: mono, 16-bit PCM WAV. Stereo, 8-bit, and exotic formats are rejected with an error.

### Makefile

`make help` lists targets. The crate itself has zero Cargo dependencies; the
Makefile manages the development prerequisites
(toolchain + rustfmt/clippy components, python3, and the two corpora):

```
make                    # fmt --check + clippy + release tests (gate stage 1)
make deps               # toolchain check + the small in-repo g5f corpus
make test-corpus        # the corpus-gated tests (fetches g12, ~262 MB, first run)
make roundtrip          # release CLI encode/decode must be PCM byte-identical
make gate V=0.11.1      # scripts/release-gate.sh (V must equal Cargo.toml)
make publish V=0.11.1   # gate + crates.io publish (sets ACOUSTICS_RELEASE=1)
make fmt-check clippy audit clean   # or run each goal individually
```

### Library

```rust
let payload = acoustics::encode(44100, &samples)?;                 // Vec<u8>
let (rate, samples, model) = acoustics::decode_payload(&payload)?; // exact + model used
```

`Model` is `{ kind: u8, param: usize }` — one of `MODEL_RAW` (0), `MODEL_D1` (1),
`MODEL_D2` (2), `MODEL_PERIOD` (3, `param` = lag in 2..=96), `MODEL_OSC` (4,
`param` = variant id 0..10, see the variant table above), `MODEL_OP` (5,
`param` = arm id 0..15), `MODEL_D3` (6), `MODEL_D4` (7), `MODEL_D5` (8) — the
D3/D4/D5 dense kinds carry no `param`.

The `acoustics::research` module exposes the measurement surface:
`osc_variant_costs`, `op_arm_len`, `period_control_cost`, `f2_stride_curve`,
`p4_ls_vs_dl`, `synth_phase_lattice` (exact integer f2 pipeline for fixtures),
`period_len_v6` / `osc_len_v6` / `arm_costs` (the G16 four-arm billing);
`acoustics::feas`
adds the v0.5 feasibility/entropy instruments (see its section below).

## Automated tests

`cargo test` covers: silence, fixed-frequency sine, i16 extremes + pseudo-random noise,
an integer-dynamics damped sinusoid (OSC selection + >8x compression), 1–2 sample inputs,
malformed-payload and bad-OSC-parameter rejections, WAV parse/write roundtrip,
stereo/8-bit rejection, zigzag+varint boundary values, metric==serialize for every
candidate — the four v0.3 acceptance tests P1–P4 (drift over 44k samples, rational
exact fit, anchor-stride tradeoff, LS-vs-DL fitting), the three v0.4 tests P5/P6/P6b
(exact integer dynamics, mixtures, stage-count cost), the six v0.5 feasibility
tests F1a–F3 (strip geometry, decoder-representable search, entropy instruments,
shuffle probe, latent-vs-closed-loop), the D3/D4/D5 wire acceptances, the v7
hostile-payload battery, and the v6-billing invariant. Suite: 94 tests (+ 11
corpus-gated G12/G13/g16 tests that run after `scripts/g12_fetch.sh` — or
`make test-corpus`).

The `feas` research module (`acoustics::feas`, v0.5; wire format untouched) implements
the description-length formulation as measurement machinery — strip-constraint
feasibility tiers, the H0/cond₁/binned-cross-entropy instruments, J(θ) ranking, and
the latent-vs-closed-loop runner. Its API surface: `fundamental_solutions`,
`real_feasible_region`, `decoder_feasible_search`, `residual_stats`, `j_cost`,
`latent_state_run`, `closed_loop_residual`. Findings summarized under "Measured
v0.4–v0.5 results" above.

## Honest limitations

(still current at 0.11.1 — each bullet measured, not assumed)

- One oscillator family per payload stage; f3 cascade capped at 3 stages (greedy
  per-stage coefficient pick — joint stage optimization not yet implemented, and
  the greedy cascade measured TIES on the two-part fixture).
- f0/f1 fit one (R, C) pair for the whole run; f2 anchors amplitude but phase restarts
  per segment rather than re-fitting locally.
- PERIOD lag capped at 96 samples; f2 lattices have `q | 4096`; envelope is Q15 pow
  evaluated per segment only.
- Mono 16-bit only. Encode sweeps 11 variants × candidate grids: description length,
  not encode speed, is the objective.
- The feasibility tier applies the strip geometry to exactly-evolving systems;
  feasibility polygons for truncated dynamics are decoder-defined, not half-plane-
  intersected (the measured distinction above).

## Research record: the G-line (research instrumentation; the wire it evaluates is now v7)

Every row measured under the standing gates: complete serialized-payload cost
(`L_measured(M) = |serialize(M)|`), exact reconstruction, identical downstream
corrections coding across arms, hash-locked real-audio corpora, negative
controls. Full tables in science.md §8.2-8.21; the honest ledger:

| Experiment | Verdict | Key numbers |
|---|---|---|
| G2 compositional echo | **positive on its class** | nested 195 B vs flat 4,129 / portfolio 580; noise reject |
| G7 attribution | the "win" was an accounting artifact | shipped beats the best single OSC variant by 1.3-24% on real speech; coefficients local (refit beats transfer 8-16%) |
| G8 coefficient budget | closed at the gate | stage coefficients = 0.09-0.10% of payload — even perfect prediction cannot matter |
| G9 bitstream anatomy | residuals own the payload | 91-100% of real-speech bytes; masks only 9.1% (PERIOD-2) / 8.5% (cascade); the ledger comes from the REAL writer |
| G10 uniform mask RLE | real, small | +2,853 B (2.5%) on PERIOD-2; RLE LOSES on dense word masks (adaptive M2 required); corpus-wide ~0.5%; SHIPPED in v7 (adaptive selector on PERIOD/OSC, OP untouched) |
| G13 D3 residual coding | **PROMOTED (v5 wire)** | piano +14.0-17.6% held-out vs D2; river -7.4..-8.1% (dies on stochastic streams, as the noise reject predicts) |
| G14 the D4 follow-up | **PROMOTED (v6 wire)** | D4 = +3.1-4.0% beyond D3 on the REAL piano vs the complete v5 portfolio (held-out, both partitions); birds/river reject |
| G15 the D-ladder's endgame | D5 marginally continues | piano +0.8-1.2% vs the complete v6 portfolio (both partitions); birds −18..−20% / river −26% reject; the continuation win shrinks ~4× per order — bounded by the varint coder, not stream structure |
| G11-A residual audit | floor identified | long streams 16-16.5% above H0 — the integral varint floor; only a fractional-bit backend reaches it (gated, unbuilt) |
| G12 acoustic diversity | the first genuine new signal | music/environment = D2 (62-72% of PCM, exact); lag-1 Hcond 42-67% below actual at n >= 700k — the structure speech never had |
| G16 the v7 four-arm gate | **SHIPPED (v7 = both layers)** | piano D-vs-A = −4,968/−4,766 B, pure D5 (masks contribute 0 on dense kinds); PERIOD-2 speech −2,853 B = the G10 anchor exact; birds/river/noise +0 B — zero regressions |

Corpus instrumentation: `scripts/g5f_fetch.sh` (speech, in-repo) and
`scripts/g12_fetch.sh` (music/environment, 262 MB, hash-locked, gitignored
with the sha-256s recorded in the script). Sources preserved exactly: 24-bit
material kept in original form, unmeasured until the engine supports it.

## Where the research goes next

The record names its follow-ups; none of them starts work until a gate asks
for it. The known open targets, each cited to the measurement that named it:

- **The residual backend itself.** The D-ladder measured its own endgame
  (G15: the continuation win shrinks ~4× per order — bounded by the varint
  coder, not stream structure), and G11-A put a number on that ceiling: long
  streams sit 16–16.5% above H0, the integral varint floor. MEASURED DONE
  (G17, §8.22): on v7 the gap holds (18–28%), and Golomb-Rice recovers most
  of it with full billing — R1 −77..95% of the order-0 gap, R2 33–98% of the
  lag-1-conditional gap; the piano payload translates 721,261 → 558,997 B.
  Wire promotion = USER's call.
- **The correction-stream context coder.** G5-E measured a D1-context on the
  cascade's deltas at −18.6% (4,489 vs 5,518 B) and justified it as a wire
  milestone; the v5–v7 promotions shipped first. Measured, not yet shipped.
- **Cascade stage optimization.** The greedy per-stage fit measured TIES on
  the two-part fixture (P6); joint or re-anchored stage optimization is the
  named follow-up.
- **24-bit material.** The corpora's 24-bit sources are preserved exactly
  and stay unmeasured until the engine supports them (corpus note above).
- **Spatial acoustics** — the acoustic-field models the project's name
  promises: multi-stage feasibility over re-anchored cascade segments,
  nonlinear dynamics, and the conditional-entropy regimes of real recordings
  (speech, instruments, room responses), where the irreducible quantity is
  the model-conditional entropy of the digitized signal, not the marginal
  entropy of physical noise.
- **Decoder resource budgets.** Configurable per-call maxima for samples,
  i64 intermediates, and mask tables — the 0.11.1 future-work item,
  without expanding the public API or touching the wire.

Reversibility and measured description length stay nonnegotiable.
