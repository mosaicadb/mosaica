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
- (later) `notes/program/library/theory.md` verdict 3 and `notes/program/codes/REPORT.md` were read in entry 2.

## 2026-11-21, entry 2: C1 written (after two interruptions; everything is in CLAIMS.md)

Files of this track (the `DESIGN-*` and `c3*`, `c4*` files in this directory are another researcher's):
- `CLAIMS.md`: C1.0 model; C1.1 achievability (F1, A1 to A3, Theorem A); C1.2 first findings (N1 to N3);
  C1.3 converse (B0 to B6, A4, gaps G1 and G2, the tax, the summary); C2 follows.
- `c1_converse.py`, `c1_converse.log`: Monte Carlo of four intervals; steepness; Theorem B and B6 numbers.
- `c1_pivot_check.py`, `c1_pivot_check.log`: the pivot interval's rates on 2,000,000 draws.

What changed against the brief.
1. The PI's converse (Δ Haar and independent of y) cannot give tan φ: under that average a centre shrunk by cos²φ
   gives q·sin φ. The worst case needs queries correlated with y (s = Δ·y not small).
2. The target q·tan θ_R is not the optimum. In the class 𝓛 (linear estimate, half-width a multiple of ‖Δ‖) the
   optimum is q·tan θ_R/√(1+k), k = q²tan²θ_R, attained by shrinking the centre by 1/(1+k) (A4), and proved as a
   lower bound (Theorem B) to first order when ln(1/δ) ≫ ln d.
3. Not proved: the general decoder (G1) and fixed δ (G2).

Failed or dropped.
- A Bayes argument with the flat prior on s for arbitrary cells: needs a trimmed Urysohn inequality (G1).
- Jensen in place of Markov for fixed δ: the bias event costs a factor 2 in δ (G2).
- A first Monte Carlo reading of the pivot's one-sided rate as 0.0103 > δ: noise, by the larger run.
- Theorem B with one Markov threshold gave 0.85 to 0.87 of the achievable at the index's sizes; the integrated
  form B6 gives 0.92 to 0.93.

Next: C2.

## 2026-11-21, entry 3: C2 written

Files added: `c2_union.py`, `c2_union.log` (the κ inequality; Corollary C3's table; Lemma C4's χ₀; a Monte Carlo of
Theorem C with 1 to 512 codewords). CLAIMS.md now ends with C2.0 to C2.5 and a summary.

What changed against the brief.
1. The construction is not N hand-placed items with a pairwise bound each. It is N items drawn independently and
   uniformly from a latitude of the query offset (all at the threshold). Given where the rotation sends the query,
   their failure events are exactly independent, for any code (Theorem C). Bonferroni then needs one number, the
   overlap π₂ of two random items.
2. π₂ is bounded in closed form for a random code (Lemma C2): the union over N is attained to 1% from 52 bits per
   block at N = 10⁹. For any code it is bounded through the cosine χ between the two errors (Lemma C4).
3. R9 as stated is false without a condition on the code: with one codeword the failures are nearly one event
   (Claim E, simulated: effective count 3.9 of 20).

Failed or not done.
- The built code: no bound on the tail of χ (G3). pairdep.py's correlations are bulk statistics.
- Several blocks in Lemma C2; the effective count for 1 ≪ M ≪ N; k-NN.
- My first χ₀ figures (0.32 at d = 95) were guessed before the computation and were wrong; the file has the
  computed 0.52 / 0.45 / 0.34 / 0.33.

State: C1 and C2 are written. Open gaps are G1 (general decoder), G2 (fixed δ), G3 (built code), and the two
sketches in C2.3. Nothing is running.
