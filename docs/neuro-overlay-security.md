# Neuro Overlay — Security Qualification

Threat model, battery, results. Companion to `docs/neuro-overlay-spec.md`
(the fail-closed grammar). Methodology carries forward the G23 battery
practices; the defining lesson applied here is that **integrity-layer
mutations must have their CRC-16 recomputed** so hostile inputs reach the
semantic parser and renderer — otherwise the deeper layers are never tested
(the G23 mutation that exposed the unchecked side-stack parity plane is the
precedent).

## Threat model and results (`src/aux/security.rs`)

| surface | control | result |
|---|---|---|
| descriptor lengths | checked arithmetic before every slice (the parser operates on `buf.len()` bounds declared up front) | PASS (truncation sweep: EVERY prefix of the canonical file rejects, all 98 positions) |
| stream count | 1..=8 enforced in the container header (0 and >8 reject) | PASS |
| audio binding | the 8-byte sha-256-derived binding checked at `verify_binding`; mismatch rejects | PASS |
| reserved fields | byte-level nonzero reserved rejects (tested with a resealed byte flip at the reserved offset) | PASS |
| coupling references | forward-only, existing, master-must-modulate; self-references and cycles reject structurally | PASS |
| offset and duration | `validate_against_primary` rejects start > primary_count unconditionally (a duration-0 stream included — the battery FOUND a validation gap here: duration-0 streams had skipped the check; fixed in commit 02fa9fb) | PASS (after the fix) |
| frequency fields | carrier 0.5–20 000 Hz; mod 0.1–12 Hz; out-of-range rejects; zero-depth AM and model-0-with-modulation reject | PASS |
| render window | the caller's buffers must cover the window: oversized requests REJECT (no slice panics); the work limit IS the caller's window (no unbounded internal allocation) | PASS |
| truncated/appended bytes | fail-closed (full-prefix sweep + appension) | PASS |
| excessive work | renders are caller-bounded; the streaming design materializes only the window | PASS |
| combined amplitude | mixing is i64-widened; the headroom policy scales down or omits; saturating clamps are counted, never silent | PASS |

Structured semantic mutations (12 cases, checksums RESEALED so they reach
the parser): unknown model id, unknown flag bits, carrier below/above range,
model-0-with-modulation-fields, zero AM depth, routing 0b00, unknown routing
bits, modulation-frequency overflow, unknown coupling reference,
self-reference, nonzero reserved — **every one rejected at the grammar or
validation layer**.

Debug and release batteries both run (overflow behavior differs between
profiles; debug panics on overflow and the wide-path arithmetic is
overflow-safe by construction — 128-bit step×index products, i64 accumulation).

## Recorded honesty

- The binding and CRC-16 provide association and accidental-corruption
  detection, NOT authentication (see the compatibility record). An attacker
  who can rewrite the sidecar can recompute both; any stronger integrity
  requires a design decision to expand the frozen representation, and none
  is made here.
- No unbounded work: rendering cannot exceed the caller's declared window,
  and long streams are O(window) memory (measured: 60 s streamed from
  ~2.5 MB RSS).
- No coverage-guided fuzzer was run on the aux path in this phase (the tool
  chain lacks libFuzzer wiring here); the mutation battery's recomputed-CRC
  method is recorded as the limitation, with the fixture set libFuzzer-ready.
- This record makes no safety, therapeutic, or efficacy claims of any kind.

No unexpected panics; controlled rejection everywhere; the failing inputs
are preserved as permanent regression fixtures in `src/aux/security.rs` and
`src/aux/mod.rs`.
