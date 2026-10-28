# Changelog

All notable changes to ACOUSTICS are documented here.
Format based on Keep a Changelog; versioning is SemVer.

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