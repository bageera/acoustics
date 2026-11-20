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
