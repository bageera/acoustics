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
  (OOB indexing, u32/i16 conversion overflow, division/remainder) are pinned
  by the malformed-input test suite.
- Decoder arithmetic is pure integer (`div_euclid` rounding); no float paths
  are reachable at decode time.
- The crate has **zero external dependencies** (supply-chain surface: none).
- No I/O, no filesystem, no network, no unsafe code.

## Scanners / SAST

- `cargo clippy --all-targets -- -D warnings` runs in the release gate
  (stage 1); `cargo audit` runs in CI for the advisory DB (advisories only;
  the crate itself is dependency-free).
- Malformed-input hardening status: see `docs/science.md` (fuzz coverage is
  the named future expansion of the invariant-test correctness contract).
