# ACOUSTICS v8.1-neuro — Consolidated Release Record

**Qualified revision:** `c03b1fc` on `origin/v8.1-neuro`
**Verdict:** RELEASE-CANDIDATE-PASS-WITH-LIMITATIONS
**State:** implementation FROZEN; merge, tag and publication HELD (separate
authorizations). The G23 compression research is not reopened; primary wire
semantics unchanged; no new modulation families at this checkpoint.

## 1. The commit chain (ten commits, all pushed)

| commit | gate | summary |
|---|---|---|
| `320bc09` | N19 | the reference instrument — exit gate PASS (15/15); pure-stdlib Python, validated FFT, four instrument defects caught by its own battery |
| `bd69df9` | N20 | recovery + fidelity — theta-AM gamma SELECTED (depth/phase err 0.0000, build + holdout); PM REJECTED; adaptive carriers excluded |
| `fcc5510` | N21 | real-audio corpus — the measured class map; archival exactness verified by re-hashing |
| `d66be59` | N22 | playback robustness — 15 SUPPORTED / 1 DEGRADED / 0 FAILED; real AAC + MP3 transcodes; bit-exact seek/blocks/resume |
| `4000147` | N23 | the ACAUX spec freeze + ADR — A0 sidecar selected; A1 rejected on the measured v8 fail-closed termination; A2 deferred |
| `34dfd95` | N24 | the Rust aux subsystem — bit-exact Python↔Rust parity (11/11); preservation verified |
| `eec7328` | N25 | the public API + CLI — parse/verify_binding/render_into/mix; the four `acoustics aux` commands |
| `02fa9fb` | N26 | the security battery (recomputed-CRC semantic mutations; one validation gap found and fixed) + the performance qualification |
| `a0ee9a9` | N27 | the qualification record — docs, results.json, permanent sidecar golden vectors |
| `c03b1fc` | N24-closure | the corpus reconciliation receipt — 12/12 byte-exact |

## 2. Primary codec preservation (the decisive evidence)

- **Wire v8 golden vectors: 21/21 byte-identical** against their committed
  digests (`golden_vectors_decode_bit_exact`).
- **Full corpus reconciliation: 12/12 byte-exact** (six classes × Balanced /
  Maximum), re-run during N24 verification:

| recording | wire v7 | qualified wire v8 | Δ | reduction |
|---|---|---|---|---|
| piano | 1,451,685 | 1,102,429 | +5 B | 24.06% |
| birds | 45,063,494 | 35,754,394 | +9 B | 20.66% |
| river | 72,247,619 | 52,807,442 | +9 B | 26.91% |
| speech1 | 55,710 | 42,895 | +11 B | 23.00% |
| speech2 | 111,093 | 74,802 | +11 B | 32.67% |
| speech3 | 437,335 | 366,881 | +11 B | 16.11% |
| **total** | **119,366,936** | **90,148,843** | **+56 B vs the G21 research optimum** | **24.48%** |

Every additional byte above the research optimum is the normative
count-varint margin (`L_v8 = L_research + Δ`); unexplained wire drift:
**0 B**. The auxiliary subsystem shares no code path with the primary
decoder; audio without a sidecar decodes identically.

## 3. The auxiliary-stream technology (qualified)

- Grammar: the `.acaux` sidecar, ACAUX v1 — 16 B header (magic/version/
  flags/8-byte sha-256-derived binding/count/reserved), fixed 38 B stream
  descriptors, length u32 + CRC-16/CCITT-FALSE epilog — **60 B** single
  stream, **98 B** coupled dual, complete with every integrity byte billed.
- Selected representation: theta-AM gamma (40.28 Hz carrier, 5.86 Hz
  theta-depth coupling in the qualified fixtures); rejected permanently:
  PM phase coupling, adaptive carriers, wire appendion (A1 — the v8 decoder
  rejects every appended payload, measured).
- Normative render: pure-integer (48-bit fixed-point phase accumulators,
  the defined 16384-entry Q14 table with linear interpolation, forward-only
  coupling, bit-exact random access); headroom contract peak = A×(1+d);
  clamps counted, never silent.
- Permanent goldens: 21 primary wire vectors + 2 sidecar vectors
  (60 B / 98 B, generated through the Python reference, rendered bit-exact
  in Rust).

## 4. Test results

- **166 tests green** (132 core + 34 corpus-gated) on **debug and release**;
  clippy 0; fmt clean.
- Aux battery: 13 tests — parity (cross-language, 11 cases), container
  roundtrip, headroom contract, blockwise==one-shot, bit-exact seek,
  window bounds, debug/release extreme-field parity, mix policy (scale-down
  and omit), binding-mismatch rejection, 4 N26 security batteries, the
  sidecar golden render.
- Corpus commands: `cargo test --release --locked g23_corpus_reconciliation
  -- --ignored --nocapture` (exit 0, receipts in /tmp/n24_corpus.log).
- Performance: `cargo test --release -- n26_performance_qualification
  --ignored --nocapture`.

## 5. Measured rendering performance

RTF ≤ 0.0007 across single/uncoupled-dual/coupled-dual (throughput
69.5–173.0 Msamples/s at 48 kHz); block latency 914 ns (64-sample windows)
to 14.5 µs (1 024); rates 8–192 kHz all within RTF 0.0001–0.0007; seek
4–33 µs per 1 024-window at arbitrary offsets; 60 s streamed at ≈2.5 MB RSS
(O(window) memory); the mix headroom policy scales down or omits
(block-size independent by construction), never gain-escalates. These are
the renderer's numbers — the codec's encode memory investigation is
separate, and the unresolved 4 GB target is not claimed here.

## 6. Documented limitations (nothing hidden)

1. **x86-64 runtime parity: BLOCKED** — Rosetta absent on the qualification
   machine; the x86_64 binaries cross-compile clean, which is NOT runtime
   evidence. Close with the native battery under Rosetta or on another
   machine; compare golden PCM hashes against ARM64 (all parity/golden
   tests ran natively on ARM64).
2. **Registry verification of 0.12.0: BLOCKED** from this machine — verify
   the published artifact through a fresh external consumer per the
   five-stage gate.
3. The N21 narrowband-masker classes fail envelope recovery (17.0% /
   36.2% / 12.1%) — a physical delivery limitation of the representation,
   documented permanently.
4. The N22 support matrix is conditional on the tested signals,
   transformations and detector criteria; the 8 kHz brickwall row is
   DEGRADED.
5. No coverage-guided fuzzer on the aux path (libFuzzer wiring absent);
   the recomputed-CRC mutation battery + fixture set are the recorded
   substitute and are fuzzer-ready.
6. The 8-byte binding is file association, not authentication; CRC-16
   detects accidental corruption only.
7. Neurological effectiveness: **not established and not claimed** — every
   result in this record is signal-processing measurement. Digital
   amplitude does not establish a safe acoustic listening level.

## 7. Handoff

The remaining work before any release closure is exactly the two external
cells in §6 — everything else is frozen at `c03b1fc`. After closure, the
next phase is **G24 applied acoustic composition and verification**
(corpus: G24-A real musical genres → G24-H long-duration streaming),
not further codec architecture. The three-dimension status: lossless
compression QUALIFIED · generative auxiliary information QUALIFIED ·
applied application research NEXT.

Evidence set: `docs/neuro-overlay-spec.md` (the grammar),
`docs/neuro-overlay-research.md` (the full measurement record + board),
`docs/neuro-overlay-results.json` (machine-readable),
`docs/neuro-overlay-compatibility.md`, `docs/neuro-overlay-security.md`,
`docs/neuro-overlay-performance.md`, this file, on the frozen revision.
