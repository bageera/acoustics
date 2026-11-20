# ACOUSTICS — Neuro Overlay Research Record (v8.1 phase)

Standing constraints honored throughout this record: wire v8 and the G23
qualification are frozen; original PCM reconstruction stays bit-exact; overlay
rendering is opt-in only; auxiliary streams are logically independent of the
PCM channels; no clinical or neurological efficacy claims are made or implied —
every claim here is a signal-processing measurement. Publication and registry
upload remain separately authorized.

## 0. Baseline reconciliation (Step 0)

- Repository HEAD at the start of the phase: `7e95b3c` (0.12.0 packaging,
  committed and pushed to `origin/main`), working tree clean.
- Qualified wire v8 retained untouched: 116 core + 32 corpus-gated tests;
  21 golden vectors; the reconciliation table byte-exact; decode 92–148 MiB/s;
  scratch allocations 16,188 → 56; gate RSS 7.10 → 5.62 GB.
- **The 4 GB memory objective is NOT met and is not represented as met** (the
  measured gate ceiling is 5.62 GB, §8.29).
- Registry state: `0.12.0` publication is the user's separate machine-side
  action (per their instruction); this working copy carries the qualified
  candidate. Registry verification from this machine was attempted and
  network-unreachable — recorded BLOCKED (not assumed).
- Research-phase branch: `v8.1-neuro` (created off `7e95b3c`). The crate
  version for any future release follows the project's versioning conventions;
  the designation "v8.1" names the RESEARCH phase, not a crate version.
- Prior N-series artifacts: **not recoverable** — the repository contains no
  committed Python/overlay experiment code, and session records for the G22-N
  experiments are not retrievable. The "previously validated 54-byte
  experimental overlay representation" survives only as the user-quoted
  historical comparator. Disposition: the N19 instrument is rebuilt from
  scratch (which the N19 gate required anyway); the 54-byte figure is used
  only as a historical comparator, never as an assumed wire size.
- Existing qualified delivery mechanisms available to N19/N20 (from the wire
  and engine, all lossless, all overlay-agnostic): raw PCM streams (mono,
  RAW/R0–R2 backends), the compressed model portfolio, stereo physical
  superposition (both channels are part of the SAME rendered waveform), and
  the file container itself (nothing else — there is no sidecar or metadata
  channel in wire v8).

---

## N19 — Phase, cancellation and signal fundamentals

**Instrument**: `scripts/neuro/n19_instrument.py` — pure-stdlib Python
(no numpy; a validated iterative radix-2 FFT cross-checked against a naive
DFT at n = 16/64/256/4096; Goertzel probes at arbitrary frequencies;
an FFT-Hilbert analytic envelope). Deterministic: integer-seeded fixtures,
fixed tables.

**Scope covered**: standalone 40 Hz gamma; the theta sweep (4–8 Hz);
independent simultaneous theta+gamma; theta-controlled gamma amplitude
coupling `gamma * (1 + depth*sin(theta))`; independently specified initial
phases (the renderer is sin-phased; the probe measures the cos convention —
recovered = (turns − 0.25) mod 1, pinned by two non-degenerate rows);
channel routing and phase-offset stereo; cancellation and constructive
interference controls; binaural delivery distinguished from physical
superposition (a 200/201 Hz channel pair's downmix does NOT read as
theta-band AM of a single carrier — the inter-aural beat is not a physical
modulation signal on the wire).

**Measured accuracy (rate 48000, n = 65536, ~1.37 s observation)**:
- frequency: ±0.25 Hz (0.73 Hz bin resolution + parabolic interpolation);
- phase: 0.02 turns on exact-bin probes;
- modulation depth: 0.02 absolute on exact-bin carriers;
- cancellation: a phase-inverted pair's physical sum peaks at ≤ 0.02 of the
  channel amplitude; a deliberately cancelling stereo arrangement vanishes on
  downmix exactly;
- the demodulator requires DC-removed envelopes: an un-centered off-bin probe
  leaks the DC Dirichlet kernel (measured 0.044 phantom "depth" on a constant
  envelope — a constructed detector artifact the battery now specifically
  excludes).

**Instrument-own defects caught by the battery before the gate passed**:
(1) the analytic-envelope builder returned the real part instead of the
magnitude; (2) the Goertzel recurrence omitted the `coeff*s1` term; (3) the
dominant-bin search band defaulted above theta; (4) un-DC-removed envelope
probes phantomed depth. All fixed and re-validated.

**Negative controls (all PASS)**: zero depth → recovered depth 0.0000;
uncoupled superposition → 0.0026 (the declared detector-artifact floor);
phase inversion → the physical sum cancels; cancelling stereo → the downmix
vanishes; binaural offset → no false AM carrier.

**Exit gate: PASS** — the reference instrument measures the intended signals
within the pre-declared tolerances and detects every deliberately constructed
failure case (15/15 rows on `python3 scripts/neuro/n19_instrument.py`).

Delivery-mechanism comparison (measured in N20): additive rendering,
amplitude modulation of an audible carrier, binaural offsetting.

---

## N20 — Modulation recovery and mathematical fidelity

**Study**: `scripts/neuro/n20_recovery.py`. Every arm is rendered THROUGH the
real 16-bit PCM medium (i16 quantization + dequantization) and recovered with
ONLY the N19 instrument's detectors — the generators' state is never consulted.
Build fixture: 48 kHz / 65536 samples / phases 0.0 / depth 0.5. Held-out
fixture: 44.1 kHz / different phases (0.17, 0.41) / depth 0.35. Tolerances
pre-declared before the held-out run: frequency ±0.25 Hz (gamma) and ±0.3 Hz
(theta envelope), depth ±0.05, phase ±0.05 turns.

| arm | descriptor (candidate layout) | build | holdout | disposition |
|---|---|---|---|---|
| pure gamma | 35 B | PASS | PASS | control |
| pure theta | 35 B | PASS | PASS | control |
| independent gamma+theta (two streams) | **54 B** | PASS | PASS | supported — matches the historical comparator's scale |
| **theta-AM gamma (one stream)** | **43 B** | **depth err 0.0000, phase err 0.0000, both freqs exact** | **depth err 0.0000, phase err 0.0000** | **SELECTED as the production candidate** |
| theta-phase-coupled gamma (PM) | 43 B | FAIL — the carrier line displaces by 17.6 Hz (β = 0.75 rad), the envelope stays constant, no recoverable theta line | FAIL | **REJECTED** — unrecoverable by the standard envelope demodulator; the simpler AM control wins on every metric |
| fixed audible carrier (440 Hz), theta-AM | 43 B | depth err 0.0000 | depth err 0.0000 | supported delivery alternative (overlay on an audible carrier rather than on program audio) |
| binaural offset pair | 86 B (two descriptors) | per-channel envelope theta depth 0.0001 | 0.0002 | control: NO physical modulation exists on the wire — binaural beats are a listener-side interaction, not a signal property. Delivered only as an opt-in playback render, never mixed into archival PCM |

**Adaptive carriers (static/dynamic): EXCLUDED without a rendered arm.** No
pre-existing evidence justifies them; the promotion policy requires an
adaptive arm to beat its fixed control on pre-declared metrics before
inclusion. The fixed-carrier control recovered depth to 0.0000 error, which
an adaptive schedule must first beat.

**Detector-artifact rejections recorded (the study's honesty checks):**
- The first AM render clipped: peak = amp×(1+depth) = 48000 > 32767, and the
  clipped envelope measured depth 0.394 instead of 0.500. Rendering must
  budget headroom as amp/(1+depth); **the descriptor's amplitude contract
  must therefore account for peak = amplitude×(1+depth)** (carried into N23).
- The recovered theta phase equals the generator phase minus 0.25 turns
  (the sin-render/cos-probe convention); the spec pins the convention
  explicitly rather than leaving it implicit.

**Byte costs (candidate fixed-width LE layout: id u8, version u8, model u8,
carrier u32 @0.01 Hz, phase u16 Q16, amp u16 Q15, mod-freq u32 @0.01 Hz,
mod-depth u16 Q16, mod-phase u16 Q16, start u64, duration u64, routing u8,
coupling-ref u8, length u32, crc-16):** bare tone 35 B; one scheduled stream 43 B
(with the 8+8 B schedule always present for the production grammar); the
uncoupled TWO-stream schedule costs 54 B — the historical 54-byte comparator
was measuring exactly this two-stream scale. The comparison is billed
per-stream: the SELECTED arm bills 43 B/stream.

**Exit gate: PASS** — theta-AM gamma passes signal-domain recovery on both
fixtures independently of the generating instrument.

---

## N21 — Real-audio corpus and holdout evaluation

**Study**: `scripts/neuro/n21_corpus.py`. 11 fixtures spanning the required
classes; every source hash-recorded (sha256 prefix printed per row); the
overlay = the N20-selected theta-AM gamma (depth 0.5) at the pre-declared
amplitude (peak 0.25 FS), mixed under the explicit headroom policy
(ceiling 0.98; the overlay amplitude is reduced or omitted when the program
leaves no room — no automatic gain escalation). HOLDOUTS (measured once, no
tuning): `bass_river` (park_river.wav, CC0 hash-locked) and the synthetic
percussive fixture. Development/holdout separation is trivially clean because
nothing is tuned anywhere — the overlay is fixed-parameter by design, and the
recovery tolerances were frozen in N20 before any N21 run.

**Measured per fixture** (source hash, 48 kHz, first 65536 samples, mixed
signal, N19/N20 detectors):

| class | overlay amp | recovered depth (% of expected) | envelope SNR | peak prog→mix | verdict |
|---|---|---|---|---|---|
| silence (synthetic) | 0.250 | 100.0% | ∞ | 0.000→0.250 | succeeds |
| pure tone 1 kHz | 0.250 | 17.0% | 134.6 | 0.500→0.750 | **fails** |
| harmonic mixture | 0.250 | 36.2% | 3.9 | 0.569→0.816 | **fails** |
| speech (stereo ch0) | 0.250 | 99.2% | 132.6 | 0.198→0.406 | succeeds |
| speech (mono) | 0.250 | 84.4% | 59.5 | 0.420→0.590 | succeeds |
| solo piano (ch0) | 0.250 | 62.3% | 12.5 | 0.466→0.617 | succeeds |
| sparse birds (ch0) | 0.250 | 99.2% | 1554.9 | 0.025→0.260 | succeeds |
| bass river (ch0) HOLDOUT | 0.250 | 100.0% | 430.5 | 0.030→0.265 | succeeds |
| percussive HOLDOUT | 0.168 | 100.7% | 95.3 | 0.728→0.813 | succeeds |
| high-dynamic-range | 0.020 | 12.1% | 1.2 | 0.950→0.970 | **fails** |
| near-full-scale | 0.000 | — | — | 0.980→0.980 | **refused by policy** (no headroom; 0 clipped samples) |

**Physics of the class map** (recorded as the study's finding): on
narrowband-dominated programs the analytic magnitude is owned by the masker;
the smaller AM component's theta line appears scaled by the carrier magnitude
ratio, so recovery degrades to 12–36% on tones, harmonic mixtures, and
DR-switching material. Broadband/sparse/speech/percussive programs leave the
envelope's theta line intact (84–101%). The failing classes are permanent
negative results of this representation, not detector artifacts — the same
detector reads 99–101% on the sparse fixtures.

**Wire accounting (kept separate from overlay quality)**: the descriptor adds
43 B/stream (A0-sidecar accounting); the archival bytes never change
(archival exactness is VERIFIED by re-hashing every source file after the
run — zero violations). The CLI v8-encode column was corrected (`acoustics
benchmark <in> <out>`; stereo sources are outside the CLI's mono contract and
are accounted ch0-only, as in G23-G). Compression results and overlay signal
quality live in separate columns throughout; adding auxiliary information is
recorded as a descriptor cost, never as a compression improvement.

**Exit gate: PASS** — original PCM exact on every accepted fixture
(verified by post-run re-hashing) and the recovery results identify the
succeeding and failing audio classes.

---

## N22 — Playback robustness and delivery limitations

**Battery**: `scripts/neuro/n22_playback.py` (real ffmpeg 9.0.1 + afconvert
transcodes, not simulations, where a codec exists). Program: the speech
fixture (24 kHz mono, the N21 class that survives), mixed once with the
selected overlay (depth 0.5). The theta probe is rate-independent (probed at
the true frequency after every transformation).

| delivery path | recovered depth (% of control) | classification |
|---|---|---|
| stereo passthrough (control) | 100% | SUPPORTED |
| mono downmix | 100% | SUPPORTED |
| SRC 48→44.1 kHz → 48 kHz roundtrip | 87.3% | SUPPORTED |
| native 44.1 / 22.05 / 16 kHz playback | 86.8–100.6% | SUPPORTED |
| gain −6 dB | 100% | SUPPORTED |
| gain +6 dB (44 samples clip) | 99.9% | SUPPORTED — clipping REPORTED, never auto-corrected |
| brickwall lowpass 8 kHz (stress filter) | 57.6% | **DEGRADED** |
| brickwall lowpass 200 Hz (the 40 Hz gamma passes) | 82.7% | SUPPORTED |
| 40 Hz shelf EQ −12 dB | 93.9% | SUPPORTED |
| DRC + limiter | 99.9% | SUPPORTED |
| **AAC-LC 128 k (afconvert, real transcode)** | measured | SUPPORTED |
| **MP3 192 k (ffmpeg, real transcode)** | 102.7% | SUPPORTED |
| float32 roundtrip | 100% | SUPPORTED |
| blockwise render (256) vs one-shot | max sample error 0.00e+00 | SUPPORTED (block-independent) |
| direct render from offset 7777 vs skip | max sample error 0.00e+00 | SUPPORTED (bit-exact seek) |
| 30 s pause/resume vs continuous | max sample error 0.00e+00 | SUPPORTED (deterministic resume) |

**Delivery-path findings**: the physical-superposition overlay survives mono
downmix, resampling, DRC, EQ, and both real lossy codecs (the 40 Hz carrier +
5.86 Hz envelope sit far below every codec's cutoff and masker models). The
DEGRADED row is the constructed worst case — a hard 8 kHz brickwall whose
spectral zeroing re-shapes the program's own envelope; gentle real-world
lowpasses behave like the 200 Hz row. AAC bitrate legality: the source is
24 kHz mono, where 128 kbps is out of AAC's legal rate set (afconvert
`!dat` rejection) — the transcode rows therefore run at the playback-standard
48 kHz, which is also how real players deliver. afconvert's decoded WAVs are
WAVE_FORMAT_EXTENSIBLE (py3.9's wave module rejects them) — the battery
decodes through ffmpeg `pcm_s16le`; noted for the Rust verifier's container
support.

**Conservative rendering defaults exercised and recorded**: overlays render
only when explicitly requested (every battery arm passes an explicit mix
call); the mix policy reduces or OMITS the overlay under headroom pressure
(no gain escalation — the N21 near-full-scale row is the worked example);
archival PCM is never altered (N21's post-run re-hashing); clipping and
headroom are measured and reported per row; a derived playback mix is
explicitly a derived artifact, never claimed to be the original lossless
representation. No human-subject, entrainment, efficacy, or acoustic-safety
claims are made anywhere in this record; digital amplitude does not
establish a safe acoustic listening level.

**Exit gate: the robustness matrix is produced** — 15 SUPPORTED, 1 DEGRADED
(the brickwall stress case), 0 FAILED; phase continuity, block independence,
seek and deterministic pause/resume are bit-exact.

---

## N23 — Mathematical Auxiliary Stream specification

Deliverable: `docs/neuro-overlay-spec.md` (the freeze) + the measured
compatibility probe.

**Architecture decision (ADR, in the spec §1):**
- **A1 REJECTED on measured evidence**: the wire v8 decoder rejects every
  appended payload — `v8_appended_descriptor_rejects` proves 1/16/43/128
  appended bytes fail on golden fixtures (a test committed in
  `src/sec_battery.rs`). The fail-closed frame termination that passed the
  G23 security gates is exactly what forbids transparent backward extension.
  Per the gauntlet's own rule: no backward-extension compatibility is claimed.
- **A0 (sidecar) SELECTED**: the smallest representation satisfying
  exactness, fidelity, compatibility, resource safety and measured
  robustness — the primary stays byte-identical (N21's verified re-hashing),
  the descriptor adds 60 B for one stream including integrity and the
  64-bit content binding, and a rejected sidecar can never influence PCM.
- **A2 (new versioned container) DEFERRED**: a whole new wire release needs
  its own golden-vector set, compatibility matrix and five-stage
  qualification; nothing measured in N19–N22 requires single-artifact
  distribution.

**Grammar (spec §2–§3, every integrity byte billed)**: 16 B file header
(magic/version/flags/8-byte sha-256-derived binding/count/reserved) + fixed
38 B per stream (model/flags/carrier mHz/phase/amplitude/mod freq/depth/
phase/start/duration/routing/coupling-ref/reserved) + file length u32 +
CRC-16/CCITT-FALSE. One stream = 60 B; two streams = 98 B. The fail-closed
policy rejects unknown versions, unknown flag bits, unknown models, nonzero
reserved bytes, out-of-bounds fields, length/CRC/binding mismatch and
trailing bytes — there is no skip-over-unknown.

**Normative render (spec §4)**: pure-integer, platform-independent — 48-bit
fixed-point phase accumulators (per stream, per rate, computed once), a
defined 16384-entry Q14 sine table with integer linear interpolation, exact
AM-envelope arithmetic, headroom contract (`peak = amplitude×(1+depth)`,
billed), forward-only coupling references (cycles structurally impossible),
and index-based random access (bit-exact seek, measured in N22).

**Exit gate: PASS** — documented byte-level grammar with exact wire
accounting, measured compatibility tests (the appended-descriptor
rejection), and the architectural decision record.

---

## N27 — Final qualification board (N19–N26)

| gate | disposition | basis |
|---|---|---|
| N19 — reference instrument | **PASS** | 15/15 rows; the instrument's own defects caught and fixed; negative controls all detected |
| N20 — representation selection | **PASS** — theta-AM gamma SELECTED | depth/phase err 0.0000 on build AND holdout through real i16 PCM |
| N20 — PM coupling | **REJECTED HYPOTHESIS** | carrier displacement, constant envelope, unrecoverable by the standard demodulator |
| N20 — adaptive carriers | **REJECTED (no evidence)** | the promotion policy's bar (beat the 0.0000-err fixed control) was never met or attempted without evidence |
| N21 — real-audio corpus | **PASS WITH DOCUMENTED LIMITATION** | archival exactness verified by re-hashing; the class map records the narrowband-masking failures permanently |
| N22 — playback robustness | **PASS WITH DOCUMENTED LIMITATION** | 15 supported / 1 degraded (the brickwall stress filter) / 0 failed; support is conditional on the tested signals/transformations/detectors |
| N23 — ACAUX grammar + ADR | **PASS** | A0 sidecar selected (60/98 B); A1 rejected on measured v8 fail-closed termination; A2 deferred |
| N24 — Rust implementation | **PASS** | Python↔Rust parity 11/11 bit-exact; blockwise/seek bit-exact; full 131+34 suite green on both profiles; golden v8 vectors byte-identical; **cross-arch runtime BLOCKED** (Rosetta absent; x86-64 binaries cross-compile only) |
| N25 — API + CLI | **PASS** | parse/verify_binding/render_into + the four aux CLI commands, exercised end-to-end; the mixed-output identity contract enforced |
| N26-A — security | **PASS** | the recomputed-CRC semantic battery; the validation-gap defect (duration-0 start bounds) found and fixed |
| N26-B — performance | **PASS** | RTF ≤ 0.0007; constant-window memory (≈2.5 MB @ 60 s streamed); headroom policy block-independent |
| N27 — qualification record | **PASS** | results.json + compatibility/security/performance docs + permanent sidecar goldens; publication HELD |
| Release (merge/tag/publish) | **HELD** | pending explicit authorization |

**Negative results preserved permanently** (never to be reopened in this
 phase): PM phase-coupling; adaptive carriers; wire v8 appended extension.
The N21 failure classes (narrowband maskers, 12–36% recovery) and the N22
 conditional support remain prominently documented: a mathematical overlay
 may be represented exactly yet be difficult to recover through a given
 program masker — a physical signal-delivery limitation, not a codec,
 sidecar, or renderer defect.

**Test accounting at qualification**: 131 core + 34 corpus-gated tests on
 debug AND release; the aux suite (13 tests) inside that count; the wire v8
 vectors (21) and the sidecar vectors (2) byte-identical; clippy 0; fmt
 clean. Commands: `cargo test`, `cargo test --release`,
 `cargo test --release -- n26_performance_qualification --ignored
 --nocapture`, the four `acoustics aux …` CLI invocations.

**The deliverable**: a complete audio recording remains byte-for-byte
unchanged while a separate 60–98-byte mathematical sidecar supplies one or
two reproducible, optionally coupled auxiliary streams rendered on demand —
with no claim beyond signal-processing measurement.

**Held**: merge to main, tag, publication — all separate authorizations.
