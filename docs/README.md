# docs/ — study and specification index

Measured records for ACOUSTICS, the reversible audio-model codec research
engine (`github.com/bageera/acoustics`). Every number in this directory is
produced by a committed test (`cargo test -- --nocapture <filter>` reproduces
each table); nothing is quoted from an un-rerun session.

## Principles

- [science.md](science.md) — the foundations document: exact reversibility as a
  feasibility problem, the three-tier outcome taxonomy, measured description
  length as the sole ranking objective, and the twelve measured principles
  (P1–P6, E1, F1–F3, plus the v0.6 decidability result) with per-claim receipts.

## Research records (by milestone)

Records are ordered by milestone; each cites the test that produces its
numbers. Wire format is v2 (`ACUSTCS1`, layout in README); the research layers
are instrumentation and do not touch it.

- [science.md §"Phase coherence"](science.md#p1--p4--phase-coherence-v03) —
  the P1–P4 investigations: Q15 vs Q30 grids, closed-loop regeneration,
  rational-frequency lattices, anchor strides, description-length fitting.
  Key measured result: the closed-loop family beat the PERIOD control on the
  fixture where v0.2 lost (31,283 vs 40,526 B).
- Exact discrete dynamics + mixtures (v0.4, tests `p5_*`, `p6_*`) — the
  stability theorem for integer oscillators (periods 3/4/6 only; damping
  structurally impossible), plus the honest cascade tie recorded as a negative.
- Feasibility geometry (v0.5, tests `f1a`–`f3_*`) — strip constraints, the
  real-valued/decoder-representable/infeasible taxonomy, entropy instruments,
  and the latent-vs-closed-loop head-to-head. See science.md
  §"Feasibility geometry".
- Rounding-cell geometry (v0.6, test `f1d`) — the zero-correction decidability
  result on the v0.5 experiment's fixture. See science.md
  §"Rounding-cell geometry (v0.6)".

## The pre-dimensionality rule

Before any dimensionality increase (more stages, coupled systems, higher-order
recurrences), the encoder tests the simple discrete operations — superposition,
echo, loop/repeat, patch/copy, integer scale — under the same serialize-and-count
discipline; dimensionality is bought only when every simple operation loses on
measured J. See [science.md §8](science.md#8-the-pre-dimensionality-rule).

## Conventions

- **Same-fixture rule**: comparisons quote identical inputs and complete
  payload costs (fields, masks, deltas, headers) — never selective accounting
  or different coders on the two arms.
- **Losses ship beside wins**: honest negatives (the P6 cascade tie, the F1c
  feasibility collapse) are part of the record.
- **Three criteria stay separate**: physical fidelity, numerical fidelity,
  information efficiency — no composite score is computed anywhere.
- **Reproduction is a test, not a memory**: every claimed number has a
  `#[test]` that fails if the engine changes underneath it.