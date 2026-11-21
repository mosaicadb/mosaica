# Confidence track: working notebook

Researcher: the confidence theorist. Reports to the PI. Tags are those of `notes/README.md`. Symbols are those of
`notes/00-notation.md`; local symbols are declared where used. Nothing here is refereed.

## 2026-11-21, entry 1: state after the reading pass (written at the PI's request after an interrupted run)

### Read so far
- `notes/README.md`, `notes/00-notation.md`.
- `proofcheck/CONSOLIDATED_PROOFS.md` in full (Lemma 8, Theorem 7, Propositions 10-13 are section 5).
- `reports/bounds3/REPORT.txt` (R1-R10) and `reports/bounds3/referee/REFEREE.txt`.
- `reports/surge/bounds2.txt` (T1-T5, the pair-dependence measurement) and `reports/surge/bounds2_work/pairdep.py`.
- `SYSTEM.md` sections 1-3 (schedule: deterministic levels end at 4, 8, 12, 16, 24, 32 B; randomised at 56, 96, 160,
  256 B; blocks of 96, 128, 256, 288 positions at n = 768, so d_j = 95, 127, 255, 287).
- `rust/REPORT_BOUNDS.md`, the quantiser passage (nested uniform binary expansion, tan² = 0.133 at one rise of 2 bits).
- `sublinear/REPORT.md` section 4 (Proposition 2, the shell identity; Proposition 3).
- `lit/REPORT.md` item 4.
- `agent1/REPORT.md`, `agent1/theory.py`, `agent1/theory.log`; the spectra files are dictionaries of 16 groups with
  `lam` (residual variances), `Qe` (query-offset variances) and `items`, so γᵢ = lamᵢ·Qeᵢ per coordinate.
- The verify dump's record layout, from `reports/bounds3/dump_stats.py`: float32 × 24 per record; word 0 a tag
  (bit 0 = index plan, bits 1-5 the answer size, bits 6+ the query), word 1 the row, then level, t, truth − t,
  estimate − t, deterministic part of the half-width, sd, then per block (coded flag, a_j, R_j, e_j).

### Settled
Nothing of C1-C4 yet. No claim has been written.

### Half done
Nothing. The next step is C1.

### Failed
Nothing yet.

### Not yet read
- `notes/program/library/theory.md` (the librarian's findings; the PI points at verdict 3).
