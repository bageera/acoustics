# Neuro Overlay — Compatibility Record

## What this document establishes

The `.acaux` sidecar coexists with wire v8 by construction: **wire v8 files
are never modified** by the subsystem, and the sidecar is inert bytes without
its primary. This record carries the measured compatibility evidence and the
version policy.

## The version boundaries (three, deliberately independent)

| component | version policy |
|---|---|
| Acoustics primary wire | v8 — unchanged; no fractional primary wire version exists for the aux phase |
| ACAUX descriptor grammar | its OWN version number (currently 1); unknown versions reject fail-closed |
| Rust crate | follows the project's SemVer conventions; the next crate release after qualification records the subsystem |

"v8.1" names the RESEARCH PHASE, never a wire version or a crate version.

## Wire v8 preservation evidence

- The full test suite (131 core + 34 corpus-gated) is green on debug AND
  release; all 21 wire v8 golden vectors decode bit-exact against their
  committed digests (`sidecar_golden_vectors_render_bit_exact` and
  `golden_vectors_decode_bit_exact`).
- The G23 reconciliation gate re-ran during N24's verification window
  (piano rows byte-exact: 1,102,429 B, +5 B, both profiles).
- `v8_appended_descriptor_rejects` (N23): appending 1/16/43/128 bytes to a
  valid v8 frame rejects — **the decoder's fail-closed termination is the
  security property that makes sidecar extension (A0) the ONLY compatible
  auxiliary architecture**; architecture arm A1 (appension) is rejected on
  this measured evidence; A2 (a new versioned container) is deferred.
- The archival decode path (`acoustics decode`, `decode_v8`,
  `decode_payload`) references no aux code; the aux subsystem references no
  compression code. A sidecar that fails validation can never influence PCM,
  and audio without a sidecar produces unchanged baseline output.

## Audio without overlays / overlays without audio

- Default decoding NEVER activates overlays: the archival decoder
  reconstructs the original PCM only, and the CLI `decode` command's
  behavior is unchanged.
- Overlay renders are separate derived artifacts (`aux render`, `aux mix`);
  `aux verify` checks the 8-byte sha-256-derived file binding, the grammar,
  and the primary's rate/count bounds; a binding mismatch rejects.
- The derived playback mix is printed and labeled as DERIVED — it is not
  the archival PCM.

## Fail-closed compatibility contract

Unknown ACAUX version, unknown flag bits, unknown model ids, nonzero
reserved bytes, out-of-range frequencies/amplitudes/depths/offsets/
durations/rates, bad binding, bad CRC-16, bad declared length, count 0 or
over the 8-stream maximum, trailing bytes, oversized caller windows: ALL
reject. There is no skip-over-unknown mechanism in this freeze — a future
sidecar version may relax it deliberately, documented separately.

## The binding's honest strength

The 8-byte truncated SHA-256 binding is a compact file-ASSOCIATION mechanism
(the user's qualification record, N26-A): it is not a full SHA-256 identity
and not cryptographic authentication; the CRC-16 detects accidental
corruption only. Both distinctions are documented here rather than papered
over, and neither expands the frozen 60/98-byte representation.
