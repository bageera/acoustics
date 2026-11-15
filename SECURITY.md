# Security Policy

## Supported versions

| Version | Supported |
| ------- | --------- |
| 0.9.x   | yes (wire v5) |
| < 0.9   | best effort; the wire-format guarantees are per-payload-version |

## Reporting a vulnerability

Email **security@nocturnalinc.com** with:

1. The affected version(s) and commit hash.
2. A minimal reproducer (bytes or a WAV that triggers the issue).
3. The expected vs actual behavior (e.g. panic vs error-string, wrong output).

**Please do not open a public issue for an unpatched vulnerability.**

## Security model

`acoustics` consumes byte payloads and 16-bit PCM WAV files.

- **decode()/decode_payload() is the trust boundary**: all header/param/mask/
  varint reads are length-checked and return typed `Err(String)` values —
  malformed input must error, never panic. The known panic-capable shapes
  (OOB indexing, u32/i16 conversion overflow, division/remainder, residual
  accumulation overflow, hostile OSC coefficients/selectors outside the
  encoder's reachable set, header counts above the 2^27-sample decoder
  ceiling, mask RLE runs beyond the declared table) are pinned by the
  malformed-input and hostile-payload test suites.
- Decoder arithmetic is pure integer (`div_euclid` rounding); no float paths
  are reachable at decode time.
- The crate has **zero external dependencies** (supply-chain surface: none).
- No I/O, no filesystem, no network, no unsafe code.

## Resource exhaustion (distinct from panic prevention)

The guarantees above are about controlled rejection: hostile bytes produce a
typed `Err`, never a panic. A second class of risk is resource exhaustion —
input that is structurally valid but whose decode costs memory out of
proportion to the payload's size. This section is the decoder's resource
model, enforced since 0.11.1:

- **Decoder ceiling**: the header sample count is capped at 2^27 (~134 M
  samples ≈ 47 min at 48 kHz mono). Headers claiming more error cleanly
  BEFORE any count-sized buffer is reserved. The wire format itself could
  express larger counts, so payloads that are otherwise internally
  consistent — including historical payloads above the ceiling that larger
  0.11.0-era hosts could materialize — are rejected by 0.11.1.
- **Peak memory inside the ceiling**: the i16 PCM output is ≤ 256 MiB; the
  largest i64 working buffers (PERIOD prev-stream, OSC f3 accumulator, f1/f2
  reconstruction streams, OP base/y streams) are ≤ 1 GiB each, and a single
  decode can hold several simultaneously — worst-known peak ≈ 3.3 GiB plus
  mask tables (≤ 17 MiB). Encode-side costs are unchanged from 0.11.0.
- **Allocation failures propagate, where the decoder owns the allocation**:
  every count-derived buffer reserves through `try_reserve_exact`, so an
  allocator failure returns the typed error
  `decoder buffer allocation declined (resource budget)` instead of
  aborting the process. The two shared synthesis helpers (`osc0_generate`,
  `osc2_synth`) still allocate internally — ceiling-bounded ≤ 1 GiB, but a
  failure there aborts (Rust's only escalation for an allocation inside a
  helper whose Option/error channel is already committed to a different
  meaning). This abort boundary is the resource analogue of the panic
  boundary above: recorded, ceiling-bounded, and not hidden.
- **Future work (recorded for a later release, deliberately NOT this
  patch's API)**: a configurable decode-resource budget — per-call maxima
  for samples, i64 intermediates, and mask tables, without expanding the
  public API and without touching the wire.

## Scanners / SAST

- `cargo clippy --all-targets -- -D warnings` runs in the release gate
  (stage 1); `cargo audit` runs in CI for the advisory DB (advisories only;
  the crate itself is dependency-free).
- Malformed-input hardening status: see `docs/science.md` (fuzz coverage is
  the named future expansion of the invariant-test correctness contract).
