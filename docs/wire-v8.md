# ACOUSTICS wire v8 — normative specification (G23-A freeze)

Status: FROZEN for implementation (G23-A). This document is the authority
for every v8 encoder and decoder byte. Research billing conventions that
differ from these bytes are recorded in §8 of docs/science.md and are
reconciled item-by-item below (G23-G verifies
`L_billed = L_serialized` field-by-field).

Wire version history: v5 = D3 · v6 = D4 + adaptive mask layer · v7 = D5 ·
**v8 = Rice backends + reversible stereo geometry + parity planes +
multichannel framing**. Overlay components (gamma/theta/delta
neuro-acoustic layers) are forbidden in v8 payloads and reserved for a
separately versioned v8.1 container extension built on the v8 feature-bit
mechanism §7 reserves.

## 1. Frame layer

All multi-byte integers little-endian. All varints are the engine's
existing unsigned LEB128 (`write_varint`/`read_varint`).

```
offset  field                 notes
0..8    magic                 "ACUSTCS1" (unchanged)
8       VERSION               0x08
9       CHANNELS              0x01 = mono, 0x02 = stereo; every other
                              value is a decode error. Bounded: stereo
                              consumes exactly two component stacks.
10      FLAGS                 0x00 for v8.0 (v8 decoders must reject
                              unknown flag bits; see §7).
11..15  sample rate           LE32, nonzero (else decode error)
15      sample count varint   count; count == 0 → error; count >
                              MAX_DECODE_SAMPLES (1<<27) → error
```

Mono frames then continue at §3 with exactly one component stack.
Stereo frames continue at §2 with the spatial selector and two component
stacks sharing the single sample count.

## 2. Spatial layer (stereo only)

```
spatial selector varint   0 = C0 independent
                          1 = C1 mid/side:      M = R + floor((L-R)/2),
                                                A = half of (L−R),
                                                B = (L−R) mod 2
                          2 = C2 directional:   base = L, side = R−L
                          3 = C3 directional:   base = R, side = L−R
                          every other value is a decode error
```

Reconstruction identities (exact, i16-safe, from G18's validated lifting):

- C1: S = L−R (17-bit); A = floor_div2(S); B = S & 1; M = (R + A) clamped
  to i16 by construction; L = M + A + B, R = M − A computed in i64 then
  range-checked — any out-of-i16 reconstruction is a decode error, never a
  wrap.
- C2/C3: base decodes directly; side = 2A + B; L/R = base ± side in i64
  with the same range check.

The parity plane rides the SIDE stream's payload as its trailing section:
`ceil(side_len / 8)` raw bytes, one bit per sample, first sample in the
most-significant bit of the first byte, tail bits ZERO on encode and
UNREAD on decode (padding is defined, not validated). A payload section
shorter than the plane is a truncation error.

## 3. Mathematical + information layers (each component stack)

```
model kind byte   v7 kinds 0..=8 (RAW, D1, D2, PERIOD, OSC, OP, D3, D4, D5)
                  > 8 → decode error
param varint      PERIOD lag / OSC variant / OP arm; dense kinds omit it
model fields      bit-for-bit the v7 field set (P1-P4 OSC families, OP
                  battery arms, PERIOD lag + head residuals) — byte-identical
                  to the v7 writer; the v7 mask sections (adaptive masks for
                  PERIOD/OSC, RLE-wired masks for OP) are preserved exactly
backend byte      0 = R0 varint · 1 = R1 global Rice · 2 = R2 block Rice
                  every other value → decode error
backend body      per §4. The masked models' mask sections ride BEFORE the
                  residual backend body, exactly as v7 frames them.
```

## 4. Residual backends (normative)

Symbols are zigzag-encoded residuals exactly as v7 frames them (for masked
models: only masked positions; for RAW: the stream submode's full zigzag
sequence — RAW also keeps the bill-the-smaller-submode rule with the
submode byte; a Rice backend on RAW always implies submode 0).

- **R0** body: the v7 varint stream, byte-for-byte. A R0-selected frame is
  byte-identical to a v7 frame once the backend byte is removed.
- **R1** body: `symbol count varint`, then `k: u8` (0..=20; > 20 → decode
  error), then a MSB-first bit stream of exactly that many symbols: per
  symbol, quotient q = u >> k; q < 12 → unary(q) = q ones + one zero, then
  k remainder bits (omitted entirely when k = 0); q >= 12 → unary(12) + a
  bit-wise LEB128 varint of the full u; the stream is 0-padded to a byte at
  the END with zero bits. **CORRECTED SPEC** (implementation defect; v8.0
  initial freeze amend): the zero padding is decodable as phantom symbols,
  so the symbol count is carried explicitly and the decoder reads EXACTLY
  that many symbols, then REQUIRES the stream to end at a byte boundary
  with no leftover bits (a decoder that reads fewer/more symbols, or a
  body whose consumed bits differ, is malformed). Symbol counts are
  cross-checked against the component's mask sections where applicable.
- **R2** body: `symbol count varint`, then `nblocks varint` (ceil(count /
  256); a mismatch with the count or with the decoded blocks → decode
  error), then ONE continuous bit stream: per block, an 8-bit LEB128-encoded
  k (block parameter, 0..=19; > 19 → decode error) written bit-wise, then
  that block's Rice symbols under k (every block but the last codes
  exactly 256 symbols); the shared stream is 0-padded once at the end with
  zero bits, consumed exactly.

Encoder k selection and block boundaries follow the G17 research coder
(verified bit-exact); selection rules are ENCODER-side policy — the
decoder only reads what is present.

## 5. Stereo frame assembly (normative order)

A stereo v8 frame is:
`[frame layer][spatial selector byte][component A: full stack][component B:
full stack][— for transformed geometries: nothing extra; each side stack's
parity plane is the trailing section of that stack's fields]`.

The selector bytes billed in the research harness become real framing:
one `backend byte` per component stack (§3), the spatial selector byte per
frame, and the parity plane per transformed side stack. **Reconciliation
vs the research billing:** (a) the research Rice arms' internal selector
byte (the 1u8/2u8 leading byte of rice_r1/rice_r2 bodies) is DROPPED in v8
— the per-stack backend byte replaces it (v8 is exactly 1 byte/component
leaner than the research cells for Rice arms); (b) the research geometry
selector-byte conventions (§8.23's ms two-byte / §8.24 one-byte artifact)
are unified to one spatial selector byte per frame. G23-G reconciles the
integrated totals against the §8.25 references item-by-item with these
byte deltas charged.

## 6. Resource limits, checked arithmetic, compatibility

- Sample count ceiling `MAX_DECODE_SAMPLES` (1<<27) applies to the frame;
  every count-derived buffer goes through `try_reserve_exact` exactly as
  v7 (the 0.11.1 guarantees carry over unchanged).
- All length/frame arithmetic is checked (`i64_sum_checked` /
  `checked_add`); overflow or negative = decode error.
- Rice decode bounds: unary runs terminate at the symbol budget; a unary
  run longer than the remaining bit budget → truncation error (bounded
  execution). Escape values decode through the varint reader's existing
  u64 guard.
- Cross-version: a v8 decoder accepts ONLY VERSION == 8. The v5/v6/v7
  decode paths are retained for historical payloads (the 0.11.1 battery
  governs them); v8 encoders never emit earlier versions. Historical
  VALID payloads decode exactly (the recorded floors stay pinned).
- Platform: pure integer arithmetic throughout; golden vectors must
  reproduce identical PCM on all supported targets.

## 7. Extensibility

The FLAGS header byte carries future OPTIONAL components. v8.0 writes
0x00 and v8 decoders reject any unknown bit (fail-closed). v8.1 overlay
containers (gamma/theta/delta) MUST define themselves as a new version or
a FLAGS bit with an explicit section grammar — they must not reuse v8.
field meanings.

## 8. Encoder profiles (same grammar, different search)

| Profile | Geometry policy | Predictor policy | Backend policy |
|---|---|---|---|
| Fast | C0 only | K1 shortlist (varint top-4 + dense family + baseline) | all three |
| Balanced | all C0-C3 | K1 per geometry | all three |
| Maximum | all C0-C3 | exhaustive per geometry | all three |

Profiles differ ONLY in encoder search; every profile emits the same wire
grammar and the decoder never learns which profile produced a frame.

## 9. Verification obligations (G23-E/F/G)

1. Complete-frame roundtrip: `encode_v8` → `decode_v8` byte-identical PCM
   on the full corpus, all six classes, both channels, every geometry and
   backend combination the research measured.
2. The §8.25/8.27 reference sizes re-measured through the REAL v8
   serializer with the §5 byte deltas reconciled line-by-line.
3. The 0.11.1 security battery stays green plus the new-path battery:
   truncated Rice streams, oversized unary runs, invalid k/nblocks,
   malformed escapes, truncated/short parity planes, invalid spatial
   selectors, out-of-range channel reconstruction, flagged frames,
   randomized mutations — no panics, clean errors only.
4. Golden vectors: mono RAW/D1/PERIOD/OSC/OP, stereo C0-C3 × R0/R1/R2,
   stored in-repo, byte-frozen, decoded on every test run.
