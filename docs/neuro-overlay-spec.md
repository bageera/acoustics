# ACOUSTICS — Mathematical Auxiliary Stream Specification (research freeze)

Status: RESEARCH FREEZE (v8.1 phase, gate N23). Not a published wire. The
grammar is what N24's Rust implementation must satisfy and N26's batteries
must attack. Publication remains separately authorized.

Normative companion: `docs/wire-v8.md` (the primary payload, unchanged). The
research record with the measured justification for every choice here:
`docs/neuro-overlay-research.md` (N19–N22).

## 1. Architecture decision record

| arm | definition | verdict | measured basis |
|---|---|---|---|
| A0 — independent sidecar descriptor | a separate `.acaux` file bound to an UNCHANGED wire v8 payload by a content hash | **SELECTED** | the primary bytes never change (N21 re-hash evidence); 56 B binds one stream incl. integrity; a lost sidecar can never corrupt the original |
| A1 — backward-compatible optional extension | descriptors appended to (or inside) a wire v8 frame | **REJECTED** | measured: the v8 decoder rejects EVERY appended payload (`v8_appended_descriptor_rejects`: 1/16/43/128 appended bytes on two golden fixtures) — the fail-closed termination that protected G23-B2/C is exactly what makes transparent extension impossible; any in-frame auxiliary channel would demand a new wire version |
| A2 — new versioned representation (primary + aux in one artifact) | a full new wire release | DEFERRED | heavier than the research phase justifies: a new wire version requires its own golden vectors, compat matrix and full five-stage qualification; nothing in N19–N22 needs single-artifact distribution; revisit only if a product requirement demands it |

Decision rationale (promotion policy: promote the smallest representation
that satisfies exactness, fidelity, compatibility, resource safety, robustness):
the sidecar is the smallest representation that satisfies every measured
requirement. It preserves archival bytes bit-for-bit, adds no parser surface
to the qualified v8 decoder, and is independently implementable (its grammar
involves no compression machinery).

## 2. Container grammar (`.acaux`, fixed-width, little-endian everywhere)

```
offset  size  field
0       4     magic       = "ACAU" (0x41 0x43 0x41 0x55)
4       1     version     = 1 (unknown version -> REJECT)
5       1     flags       bit0 = schedule-follows-payload (reserved=0 in this
                          freeze; unknown set bits -> REJECT; 0 recommended)
6       8     binding     = sha256(primary_payload_file)[0..8] (64-bit binding;
                          mismatch or an absent primary -> REJECT at validation
                          time, NOT at decode time — the .acaux is inert bytes
                          without its primary)
14      1     count       = the aux stream count, 1..=8 (0 or >8 -> REJECT)
15      1     reserved    = 0 (nonzero -> REJECT)
16      count×38  streams  (the per-stream descriptors, section 3)
16+38c  4     file_len    = the total .acaux byte length (u32 LE; must equal
                          the actual length -> else REJECT)
20+38c  2     crc16       = CRC-16/CCITT-FALSE over bytes [0, 16+38c) (u16 LE;
                          mismatch -> REJECT)
21+38c  = the file end; nothing may follow (trailing bytes -> REJECT)
```

One-stream file: 16 + 38 + 4 + 2 = **60 B**; two streams: 98 B. (The N20
43 B/stream comparator billed only the schedule fields; the sidecar adds the
16 B header with the 8 B binding hash and a 6 B epilog — full accounting,
every integrity byte billed.)

## 3. Per-stream descriptor (fixed 38 B, little-endian)

```
offset size field
0      1     model         0 = sine; 1 = sine with theta-AM
                           (other values -> REJECT)
1      1     flags         bit0 = enabled; bit1 = L, bit2 = R routing
                           shorthand (see routing below); unknown set bits ->
                           REJECT
2      4     carrier_freq  millihertz, u32 LE. Legal: 500..=20_000_000 mHz
                           (0.5 Hz..=20 kHz; outside -> REJECT)
6      2     carrier_phase Q16 turns (phase0 = value/65536 revolutions)
8      2     amplitude     Q15 (0..=32767). The render's PEAK contribution is
                           amplitude×(1+depth); the mixer must budget headroom
                           for that peak (the N20/N21 measured contract)
10     4     mod_freq      millihertz u32; model 1 only: 100..=12_000 mHz
                           (0.1..=12 Hz); model 0: MUST be 0 (nonzero -> REJECT)
14     2     mod_depth     Q16 (0..=65536 ⇒ depth 0..=1.0); model 0: MUST be 0
16     2     mod_phase     Q16 turns; model 0: MUST be 0
18     8     start_sample  u64 LE; 0..=primary_sample_count (outside -> REJECT)
26     8     duration      u64 LE samples; 0 = until the primary ends (the
                           render-time limit still applies)
34     1     routing       bitmask: bit0 = render into channel 0, bit1 =
                           channel 1; 0b00 or unknown bits -> REJECT
35     1     coupling_ref  0 = none; else the 1-based stream id of the master
                           stream whose THETA PHASE drives this stream's
                           mod_phase (a forward-only reference: a stream may
                           only reference a LOWER-indexed stream; a
                           self-reference or unknown ref -> REJECT; cycles are
                           structurally impossible under the forward-only rule)
36     2     reserved      = 0 (nonzero -> REJECT)
```

## 4. Normative deterministic rendering (bit-exact contract for N24)

All arithmetic is INTEGER. No floating-point appears anywhere in the
normative render — platform float differences cannot touch the output.

- Phase accumulator: 48-bit fixed point, in units of 2^-48 revolutions.
  `step(rate) = round(mod_freq_or_carrier_mHz × 2^48 / (rate × 1000))`
  computed once per (stream, rate); `phase(i) = (phase0_u48 + step×i) mod 2^48`
  where `phase0_u48 = carrier_phase_or_mod_phase × 2^32` (the Q16 field
  shifted; exact integer).
- Waveform table: 16384 entries, `TABLE[k] = round(sin(2π·k/16384) × 2^14)`
  (Q14; the entry for k=16384 is TABLE[0] — wrap).
- Evaluation: `idx = phase(i) × 16384 >> 48` (floor); `frac = phase(i) &
  (2^48−1) × 16384 >> 48` (Q14 fraction); the interpolated value
  `v = TABLE[idx] + ((TABLE[idx+1] − TABLE[idx]) × frac) >> 14`, then
  `>> 14` scaled output as Q14.
- AM envelope (the theta signal): the SAME table/accumulator arithmetic on
  the mod fields; `env = 2^15 + (depth_q16 × theta_q14) >> 14` (Q15).
- Sample output (per channel with the routing mask):
  `out = amplitude × v × env >> (14 + 15)` — an exact integer in
  Q14×Q15 arithmetic; the result is in [-32768, 32767] integer PCM units
  multiplied by the Q15 amplitude's own scale (amplitude = full-scale
  fraction × 32768/32768 ⇒ the FINAL pcm value is
  `amplitude × v × env >> 29` clamped to i16 with the clamp COUNTED and
  reported, never silent).
- Coupling (when `coupling_ref != 0`): the stream's effective mod_phase at
  sample i = `phase0_u48 + step_rate×i` of the MASTER stream's phase (the
  coupled stream shares the master's theta phase, enabling phase-aligned
  multi-carrier rendering); the forward-only rule makes evaluation
  order-independent.
- Stereo routing: a stream with both routing bits renders one value into each
  channel (identical samples — physical superposition); per-channel PHASE
  OFFSETS are distinct streams (the N19 routing experiments).
- Random access: `phase(i)` depends only on the absolute sample index →
  rendering from any offset equals skipping a continuous render EXACTLY
  (measured bit-exact in N22).

## 5. Limits (N26 enforces before any expensive work)

```
MAX_STREAMS            = 8
MAX_TOTAL_DESCRIPTORS  = one file ≤ 16 + 8×38 + 6 = 326 B
MAX_DURATION           = min(primary sample count, 2^32) samples per stream
FREQ_BOUNDS            = carrier 0.5..=20000 Hz; mod 0.1..=12 Hz
RATE_BOUNDS            = 8000..=192000 Hz (the primary's rate; the .acaux
                         carries no rate — the rate comes from the primary)
RENDER_WINDOW          = a render request may bound [start, start+count];
                         requests outside -> REJECT
```

Incremental rendering: a stream is evaluated sample-by-sample from its
68-bit-independent phase state; no render larger than the requested window is
materialized (N26 measures the streaming property).

## 6. Fail-closed policy (normative)

Unknown version, unknown flag bits, unknown model id, nonzero reserved,
out-of-bounds fields, count 0 or >8, length/crc/binding mismatch, missing
primary, trailing bytes ⇒ the VALIDATOR REJECTS the whole .acaux. There is
no skip-over-unknown mechanism in this freeze (a future sidecar version may
upgrade the policy; this one is fail-closed because it is cheap to re-issue).
A .acaux that fails validation never influences any PCM render, and the
primary decodes unchanged regardless.
