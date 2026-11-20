# Neuro Overlay — Performance Qualification

All numbers from `src/aux/perf.rs::n26_performance_qualification`
(`cargo test --release -- n26_performance_qualification --ignored --nocapture`),
measured on the qualification machine (Apple Silicon, aarch64, release).

## Real-time factors (RTF = T_render / T_audio; <1 = faster than real time)

| workload (10 s @ 48 kHz) | render time | RTF | throughput |
|---|---|---|---|
| single tone | 2.8 ms | **0.0003** | 173.0 Msamples/s |
| uncoupled dual | 6.9 ms | **0.0007** | 69.5 Msamples/s |
| coupled dual | 6.8 ms | **0.0007** | 70.8 Msamples/s |

## Block latency (coupled dual; linear in the block size)

| block (samples) | avg render time |
|---|---|
| 64 | 914 ns |
| 128 | 1 820 ns |
| 256 | 3 643 ns |
| 512 | 7 403 ns |
| 1 024 | 14 537 ns |

## Rates

8, 44.1, 48, 96 and 192 kHz all measured: RTF 0.0001–0.0007 across the
board (the theta probe rate-independence carries the correctness evidence;
N22).

## Seek and random access

Direct renders at offsets 0 / 1 / 10 000 / n−1024: 18–33 µs per 1 024-sample
window. A window past the primary's end: 4 µs (a silent-window OK). Bit-exact
seek equality is proven separately (`aux_seek_is_bit_exact`).

## Memory

- 60 s streamed (2.88 M samples) in 1 024-sample windows: 45.5 ms render
  time; the test binary's RSS ≈ 2.5 MB — constant-window memory confirmed
  (the renderer materializes ONLY the caller's window).
- Allocations: initialization is O(1) per parse; steady-state rendering is
  O(windows), never O(samples) (asserted against the counting allocator).

## Mixed playback

The headroom-policy mixer (`mix_into_primary`): on a loud primary
(peak 31 000) the combined Σ P_max forces the scale DOWN (Q16 4068) with
ZERO clamps; with no headroom the overlay is omitted (scale 0, the primary
untouched). The scale is computed ONCE from the declared peaks — the
no-headroom decision is independent of the render block size by
construction.

## Separation statement

These measurements are the AUXILIARY renderer's. The codec's encoding memory
is measured separately (G21-M/G23-M; peak RSS 5.62 GB on the six-class gate)
and **the unresolved 4 GB codec-encode target is NOT addressed by this
phase** — an efficient auxiliary renderer does not change codec memory
behavior, and that target must not be represented as solved. The
disabled-overlay baseline (the frozen wire v8 codec with no aux involvement)
is the full suite, green on both profiles.
