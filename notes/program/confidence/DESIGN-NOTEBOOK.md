# Confidence track, design consequences (C3, C4): working notebook

Researcher: the confidence theorist (design consequences). Reports to the PI. Tags are those of `notes/README.md`,
symbols those of `notes/00-notation.md`; local symbols are declared where used. Nothing here is refereed. Claims go to
`DESIGN-CLAIMS.md` as they are reached. `NOTEBOOK.md` and `CLAIMS.md` in this directory belong to the lower-bounds
agent and are not edited here.

## 2026-11-21, entry 1: state after the reading pass (written after a dropped connection, before any derivation)

### Read
- `notes/README.md`, `notes/00-notation.md`.
- `SYSTEM.md` sections 1 to 5: deterministic levels end at 4, 8, 12, 16, 24, 32 B; randomised at 56, 96, 160, 256 B;
  blocks of 96, 128, 256, 288 coordinates (cuts at 1/8, 7/24, 5/8 of n = 768), d_j = 95, 127, 255, 287;
  h = c_ℓ·sd + deterministic terms, sd² = Σ_j (‖y_j‖ tan φ_j ‖Δ_j‖)²/d_j; an uncoded block costs ‖y_j‖‖Δ_j‖;
  c = 6.931 (56 B) and 7.963 (96 to 256 B) at 1M; measured B/item 2.9 / 5.8 / 18.6 / 133.2 / 276.2 in distribution
  and 20.6 / 26.1 / 67.0 / 184.5 / 322.8 out of distribution at answers 0.01% / 0.1% / 1% / 10% / 50%; stored bytes
  per item 2 (key) + 35 (levels to 32 B with norm codes) + 29.5 (24 B code + 5.5 B scalars) + 209.8.
- `notes/03-randomised.md` sections 3.1 to 3.3: Lemma 3.1 (E = A·u, A = ‖y‖ tan φ, u uniform on the sphere of y^⊥,
  any codebook); Proposition 3.4 (k′ sign bits of n_j: tan²φ → π n_j/(2k′) − 1) and the sentence after it, "the sketch
  beats a deterministic bound on the same block once k′ exceeds c²π/2, about 100 bits at c = 8": this is the PI's
  estimate for C3 (a), and it compares with the block's zero-bit Cauchy–Schwarz bound only.
- `notes/program/floors/CLAIMS.md` sections 0, 1 (L3, L4), 3, 4, 7: W(H) = Σᵢ √min(γᵢ, θ), Σᵢ ½ ln⁺(γᵢ/θ) = H
  [proved, unrefereed there]; V = κ_k√θ + Σ_{i>k} √γᵢ is 1 to 2% above W; group 12 F = 0.0728, n_eff = 76; W at
  0 / 4 / 8 / 16 / 32 / 96 B (plus 48 bits of scalars) = 0.0453 / 0.0379 / 0.0327 / 0.0258 / 0.0180 / 0.0068;
  escape code on Gaussian data 1.26 to 1.59 above V; real residuals 1.17 to 1.21 above W (`quant/REPORT.md`).
- `notes/program/codes/REPORT.md`: sphere bound tan φ = (4^R − 1)^(−½); built / bound = 1.625 (96, 2 bits), 1.767
  (96, 7 bits), 1.549 (128, 3 bits), 1.307 (256, 1 bit), 1.415 (256, 2 bits), 1.498 (256, 1/4 bit).
- `agent1/theory.py`, `agent1/theory.log`; spectra `agent1/spec_v1.json`, `spec2_v2.json`, `spec2_n128.json`.
- `sublinear/REPORT.md` section 4: Proposition 2 (shell identity): bytes = Σ_ℓ b_ℓ·|A_(ℓ−1)|, a non-match survives
  level ℓ exactly when its score is within the level's upper slack of t.
- `reports/bounds3/REPORT.txt`, sections 5 to 9 (the dump's regeneration command; stored scale 2.45% too large).
- The dump's record layout as the other notebook recorded it: float32 × 24 per record; word 0 a tag (bit 0 index plan,
  bits 1 to 5 answer size, bits 6+ query), word 1 the row, then level, t, truth − t, estimate − t, deterministic part
  of the half-width, sd, then per block (coded flag, a_j, R_j, e_j). I have not opened the dump and will not.

### Derived
Nothing yet.

### Next
C3 (a): the exchange rate between a bit spent deterministically and a bit spent on a rotated block.

### Failed
Nothing yet.

## 2026-11-21, entry 2: C3 (a) written

- Model fixed (DESIGN-CLAIMS section 0): amplitudes aᵢ = √γᵢ; deterministic part costs W (l1, water-filling); a rotated
  block costs c·F_T·tan φ/√d with dual weights before the rotation, so its zero-bit bound is F_T; prefix nats shrink
  amplitudes at Shannon's rate; layout width (0.1).
- Three break-evens, all closed form for a flat block: against the block's own Cauchy–Schwarz bound
  B₀ = (n/2) log₂(1 + c²/d) → 0.721c²; the sign sketch (π/2)c²n/(c² + d) → c²π/2 (the PI's figure: right for signs in
  a long block against the zero-bit bound, 2.18 times the ideal); against the floor at the same bits
  B* = (n/2) log₂(d/(d − c²)), which needs d > c².
- New to me: at high rate the block wins iff d·(GM/AM)² > c². The rotation pays AM/GM of the amplitudes. On the raw
  spectrum the top 96 coordinates never win at c = 8. So the prefix is what makes the randomised levels worth having,
  and the first-order condition is cos²φ = k_eff/n.
- `c3a.py`, `c3a.log`, `design_lib.py` written. Run time a few seconds.
- First draft of the claim said the built c·ρ_c/ρ_det was 9.5 to 11.4; recomputed as 9.0 / 9.7 / 10.4 / 11.6 at
  1 / 2 / 3 / 7 bits and corrected before saving.

Next: C3 (b), the optimisation. Plan: minimise h at fixed bits over (cuts, prefix nats per part, rates), contiguous
blocks in the sorted order, J ≤ 4, plus the continuous limit (a block per coordinate band) as a lower envelope; then
invert to bits at a margin.

## 2026-11-21, entry 3: C3 (b) optimiser written, first numbers

- `c3b.py group c curve`: for each budget (2 to 384 B) and J = 0 to 4 rotated blocks, coordinate descent on the cuts
  (grid 16 … 768) around two starting layouts, SLSQP inside on (prefix bits per part, rate per block) with the bit
  budget as an equality. Tail [n_J, n) is deterministic. Output `c3b_g<g>_c<c>_<curve>.json` and `.log`.
  "built" uses ρ_c(R)·1.0245 on tan φ, ρ_det ramping from 1 to 1.19 over the first prefix bits (the ramp is my
  estimate; 1.19 is measured at 52 to 296 stored bits), 16 scalar bits per coded block and 4 for D.
- Quick run, group 12, c = 8, ideal curves, h/F at 8 / 32 / 56 / 96 / 256 B: deterministic alone 0.566 / 0.279 /
  0.179 / 0.101 / 0.0183; best hybrid 0.566 (J = 0) / 0.197 / 0.102 / 0.0477 / 0.0067. So with ideal curves the
  hand-over is between 8 and 32 B, not at 32, and the hybrid uses a short prefix (42 bits at 32 B, 91 at 56 B, 200 at
  96 B) and sub-bit rates on wide blocks. One block of everything already gets most of it (0.230 at 32 B).
- Chain of eight configurations started in the background (`c3b_chain.sh`: groups 12 and 6, c = 8 and 4, ideal and
  built), about 3.5 minutes each.

Next while it runs: `c3c.py`, the built schedule in the same model.

## 2026-11-21, entry 4: first full frontier (group 12, c = 8, ideal), built schedule in the model, C4 script

- `c3b_g12_c8_ideal.log`, h/F by budget, deterministic alone against the best hybrid:
  2 B 0.800 / det; 4 B 0.695 / det; 8 B 0.566 / det; 12 B 0.482 / det (best hybrid 0.492); 16 B 0.421 / 0.386;
  24 B 0.337 / 0.266; 32 B 0.279 / 0.197; 40 B 0.237 / 0.154; 48 B 0.204 / 0.124; 56 B 0.179 / 0.102;
  72 B 0.140 / 0.073; 96 B 0.101 / 0.048; 128 B 0.068 / 0.029; 160 B 0.048 / 0.019; 208 B 0.029 / 0.011.
  Hand-over with ideal curves at c = 8: between 12 and 16 B. One block of everything crosses between 16 and 24 B.
- Shape of the optimum: four blocks, the first of 32 to 256 positions growing with the budget, rates falling by
  about a factor 2 from block to block (for example 2.0 / 0.94 / 0.36 / 0.05 at 56 B), prefix on the first block
  only and short (33 bits at 24 B, 91 at 56 B, 200 at 96 B, 300 at 160 B).
- `c3c.py 12 8` (optimum columns not yet filled): the built schedule in the model, ideal curves, h/F: 56 B 0.1965 of
  which c·sd is only 0.022 and the three uncoded blocks 0.174; 96 B 0.069; 160 B 0.024; 256 B 0.0094. Built cuts and
  prefix with free rates: 0.113 / 0.049 / 0.020 / 0.0072, with rates 0.73 / 0.6 / 0.15 / 0.02 at 56 B.
- Caution found: the model's block sums after the 256-bit prefix are 0.105 / 0.114 / 0.057 / 0.004 of F. The 50k
  dump's level-0 means (`reports/bounds3/dump_stats.log`) give block 0's Cauchy–Schwarz term about 0.039 against 0.043
  for blocks 1 to 3 together, so the real block 0 is about 1.5 times heavier relative to the rest than the model's.
  The built rates were designed for the real norms. So "built rates against free rates" in the model overstates the
  rates' loss; the uncoded share at 56 B is measured (73% of the width at c = 9.33) and does not depend on the model.
- `c4.py` written (not yet run: needs the frontier files).

## 2026-11-21, entry 5: built-curve frontier (partial) and the first C4 table

- `c3b_g12_c8_built.log` so far: deterministic alone is best through 32 B (0.336 against 0.346 for the best hybrid);
  at 40 B a hybrid wins (0.276 against 0.285). So with the built codes at c = 8 the hand-over belongs between 32 and
  40 B, where it is built. With ideal codes it would be at 12 to 16 B.
- `c4.py 12 8` → `c4_g12_c8.log`. Gaussian scores, σ_s = 0.068 (item) and 0.040 (text) [measured,
  `sublinear/REPORT.md` 5.4], item scale η = κ·F_g over the 16 groups, group 12's normalised curves for every group.
  Rows: (1) optimum frontier, continuous; (2) the frontier read at the ten built level sizes; (3) the built schedule
  with ideal curves; (4) with built curves; (5) with stored scalars and 2 B of key per candidate.
  In distribution, κ = 1 (no free parameter): (5) = 2.0 / 5.3 / 17.6 / 68 / 132 B per item at 0.01% / 0.1% / 1% /
  10% / 50% against 2.9 / 5.8 / 18.6 / 133 / 276 measured. Good at small answers, half at large ones.
  κ fitted to the measured candidate share at 1% (22%) gives 0.615 and everything 2 to 7 times too low: that
  calibration fails for item queries (the support-table cut is stronger than the zero-bit bound there).
  Out of distribution, κ = 1: 9 / 16 / 37 / 111 / 200 against 20.6 / 26.1 / 67.0 / 184.5 / 322.8. κ fitted to the
  candidate share (95%) = 1.74: 27 / 40 / 72 / 175 / 299, within 8% from 1% to 50% and 30 to 55% high below.
- The split of (5) is stable across rows: floor 27 to 32%, level spacing 14 to 24%, built layout 12 to 19%, code
  distance 23 to 29%, scalars and key 5 to 14%.
- Derived the local form of the law: at an optimum of (0.1), bits per halving of the width = Σ_j n_j cos²φ_j (and
  W/λ in the deterministic regime). Check on the frontier at 56 B: 96·0.939 + 96·0.728 + 128·0.393 + 448·0.067 = 240
  against a measured slope of 251 bits per halving between 48 and 72 B.

## 2026-11-21, entry 6: the dump test written and tried on synthetic data only

- `c4_dump_test.py` (not run on the dump, which I have not opened). One pass in 38 MB chunks plus a 46 MB join table.
  Per (query class, level): the spread of w = h/scale, the u at which half the entrants survive, the 10% and 90%
  points, rms tan φ per block; then the bytes of the randomised levels per (query, size) predicted from the level-0
  margins alone against the bytes the records imply.
- Tried on a synthetic dump of 472k records built to obey the law (in the session scratchpad, not kept): it recovers
  the planted widths to 0.3%, and predicted over actual bytes is 0.996 pooled, 1.2% rms per (query, size).
- Unknown until the dump exists: whether R_j is filled for coded blocks. The script checks and falls back to the
  level-0 half-width as the item's scale.
- C3 (b) formulation and the bits-per-halving identity saved to DESIGN-CLAIMS. Corrected one line there (the identity
  follows from the rate condition, not the prefix condition).

## 2026-11-21, entry 7: C3 (b) group 12 saved; C3 (c) numbers for c = 8

- C3b.3 written into DESIGN-CLAIMS (tables for group 12, c = 8, ideal and built).
- Noticed from the c = 4 log: once D is empty, h = c·S, so the layout that minimises S does not depend on c. The c = 4
  optimum at 16 and 24 B has the same cuts and rates as the c = 8 one. c decides only where the hand-over is and how
  much of the tail stays in D.
- The identity bits-per-halving = Σ n cos²φ checked along the whole ideal frontier (`c3_tables.log`): 133 / 132,
  165 / 163, 241 / 251, 338 / 341, 396 / 393, 446 / 451, 504 / 500, 550 / 565.
- `c3c_g12_c8.log`. Built schedule over the optimum at the same bytes, width ratio (bytes ratio at the same width):
  ideal curves: 16 B 1.09 (1.12), 24 B 1.27 (1.29), 32 B 1.42 (1.41), 56 B 1.92 (1.74), 96 B 1.45 (1.28),
  160 B 1.25 (1.12), 256 B 1.41 (1.15);
  built curves, stored bytes: deterministic levels 1.03 to 1.10 (1.07 to 1.11), 56 B 1.68 (1.48), 96 B 1.71 (1.40),
  160 B 1.40 (1.18), 256 B 1.81 (1.26).
- Built cuts and the 256-bit prefix with free rates, ideal curves: within 10 / 2 / 5 / 7% of the optimum at
  56 / 96 / 160 / 256 B. So in the model the cuts and the prefix length cost little and the rates nearly everything,
  and the rates are where the model's block norms are least trustworthy (entry 4).
- A model-free reading of the dump's shares: at the best rates for fixed blocks, each block's share of the variance
  is n_j cos²φ_j / Σ n cos²φ. With group 0's rates [3, 1, ¼, 0] at 96 B that is about [0.34, 0.34, 0.27, 0.05]; the
  50k dump's mean shares for item queries at that level are [0.336, 0.398, 0.26, 0.005]. At 160 B: [0.22, 0.28, 0.43,
  0.07] against [0.371, 0.227, 0.385, 0.017]; at 256 B [0.18, 0.23, 0.44, 0.15] against [0.147, 0.173, 0.622, 0.058].
  Caveat: the 50k index's rates are not logged where I can read them, so the rates are assumed. On that assumption the
  built rates are near the optimum for the real norms at 96 B and off by one block at 160 and 256 B.

## 2026-11-21, entry 8: c = 4, group 6, measured shells; C3 (b) and (c) saved

- C3b.4 and C3 (c) written into DESIGN-CLAIMS. Corrected before saving: the variance shares the rate condition
  predicts for group 0's rates are [0.36, 0.36, 0.28, 0] at 96 B and [0.23, 0.30, 0.46, 0] at 160 B (block 3 is
  uncoded there); entry 7 gave them with block 3 coded. The 50k dump codes all four blocks from 96 B, so its rates are
  not group 0's and the comparison with its shares is indicative only.
- c = 4, group 12: a hybrid first wins at 4 B (ideal) and 8 B (built, by 2%; by 13% at 12 B). From 24 B (ideal) and
  56 B (built) the c = 4 layouts are the c = 8 layouts with half the width, as h = c·S says they must be.
  Built schedule over the optimum at c = 4, built curves: 1.27 / 1.49 / 1.80 / 2.06 at 12 / 16 / 24 / 32 B, 3.10 at
  56 B, 1.79 / 1.58 / 1.81 at 96 / 160 / 256 B (`c3c_g12_c4.log`).
- Scope note: the approximate mode reads the deterministic levels with a statistical width, not W, so the c = 4
  figures are for a certificate of form (0.1) at a smaller constant, not for that mode. Not modelled.
- Group 6 (F = 0.279, n_eff 89) repeats group 12: hand-over at 16 B ideal and 40 B built at c = 8; det/best 1.44 at
  32 B, 1.75 at 56 B, 2.11 at 96 B, 2.74 at 256 B (ideal); schedule over optimum 1.12 / 1.30 / 1.44 at 16 / 24 /
  32 B and 1.94 / 1.45 / 1.24 / 1.42 at 56 / 96 / 160 / 256 B (`c3c_g6_c8.log`, ideal rows).
  The first `c3c.py 6 8` run read a half-written built frontier; its built rows from 96 B are wrong and it is re-run
  below.
- `c4b.py` (text queries, measured shells of `sublinear/REPORT.md` 5.4 in place of the Gaussian tail), row (5):
  κ = 1: 26.1 / 9.3 / 5.0 B per item at 1% / 0.1% / 0.01% against 67.0 / 26.1 / 20.6 measured;
  κ = 1.742: 54.0 / 24.9 / 16.5. So with the measured shells one fitted scale puts all three within 20% (0.81, 0.95,
  0.80 of measured), where the Gaussian tail overshot the two small answers by 55% and 33%.
- `c4.py 6 8` (group 6's normalised curves for every group): in distribution, κ = 1, row (5) = 2.2 / 6.0 / 19.4 /
  73.6 / 141.2 against group 12's 2.0 / 5.3 / 17.6 / 68.1 / 131.7. The choice of group moves the prediction by 7 to
  13%.

Next: re-run `c3c.py 6 8`, write C4 into DESIGN-CLAIMS, then the summary tables once the chain's last two
configurations (group 6, c = 4) finish.

## 2026-11-21, entry 9: all four parts written; state at hand-back

- The chain of eight `c3b.py` configurations finished (05:10). `c3_tables.log` rebuilt over all eight. First budget
  at which a hybrid beats the deterministic code alone: group 12: 16 B (c = 8 ideal), 40 B (c = 8 built), 4 B (c = 4
  ideal), 8 B (c = 4 built); group 6: 12 B (a tie, 0.7%; 11% at 16 B), 40 B, 2 B (a tie; 24% at 4 B), 8 B.
- `c3c.py 6 8` re-run with the finished frontier, and `c3c.py 6 4` run: group 6 matches group 12 to 0.03 at c = 8 and
  to 0.06 at c = 4.
- C4 written into DESIGN-CLAIMS (statement, Gaussian evaluation, the split, the dump test, what failed, files).
  Corrected after writing: group 6's frontier is 6 to 9% above group 12's in u (I had written "within 8%"); the
  deterministic-only floors were attached to the wrong cells; the level-to-level width ratios are 1.2 to 2.9.
- A slip of mine, no harm done: a wait loop written as `until ! pgrep -f c3b.py` matched its own shell and never
  ended; it was stopped by its task id after three minutes. It only slept. Nothing else was running under it.
- Nothing of mine is running now. Every job ran on core 4 at nice 10, one thread, small numpy and scipy. I opened no
  large data file and did not open the verify dump; the synthetic dump used to try the test script was 45 MB in the
  session scratchpad.

### Settled
C3 (a), (b), (c) and C4 are written, each with its tag. All numbers are [model] or [computed in the model] except
where a measured source is named.

### Not established (also listed at the end of DESIGN-CLAIMS)
- No lower bound on the frontier over all layouts; the search is local.
- The built-curve optimum's long prefixes rest on 1.19·W beyond 296 bits.
- The loss assigned to the built rates at 96 to 256 B is an upper bound: the model's block norms are not the index's.
- C4's absolute level: refuted in the Gaussian one-scale form for in-distribution queries at large answers.
- c = 4 is not the approximate mode as built.

### What would move this on
- The dump after 08:45: run `c4_dump_test.py` (arguments in C4.4) and read T1 to T5.
- The build log's rates for the 50k index, to make the variance-share check of C3 (c) item 4 exact.
- Per-level entrant counts of one verify run at 1M, for the deterministic part of the staircase.

## 2026-11-21, entry 10: C5 opened at the PI's request (the cost of stored scalars)

The owner's point: each coded block at a level makes the query read and decode that block's scalars per item, and a
native trial that coded three blocks at 56 B instead of one was 8 to 24% slower although fewer items went on
(`rust/REPORT_BOUNDS.md`). My C3 (c) item 1 recommends that direction and charged only 16 bits of scalars per coded
block in the built variant and nothing in time. So it cannot stand as written. Plan: (1) per-block costs in bytes and
nanoseconds from the sources; (2) the layout programme with those costs, re-solved for group 12, c = 8, built curves,
at 56 to 256 B, and the scalar cost at which the answer flips; (3) C3 (c)'s table and C4.3's split redone; (4) what a
native test must measure. C3 and C4 stay as written; C5 says where they change.

## 2026-11-21, entry 11: C5 step 1 (costs from the sources) and the first-level alternatives

Sources read: SYSTEM.md 2.1, 4.2, 5; `rust/REPORT_BOUNDS.md` (Part 1, the dump table, `--design-exact`, the last
level's depth, "The next bottleneck"); `rust/REPORT_RAND.md` (steps 1 and 2, the breakdown); `rust/REPORT_SETUP.md`
(no per-item constant for scalars); `rust/results/system/bd_p2c.out` (3.7 kB: the build line and the four breakdown
rows); `rust/src/v2/q.rs` line 2032 (the planner's per-candidate cost).

- Bytes [measured, bd_p2c.out build line: stored bytes per item cumulative by level 6.5, 11.0, 15.5, 20.0, 28.5, 37.0,
  66.5, 106.9, 176.8, 276.3, key included]: first randomised level 29.5 B = 24 code + 5.5 scalars = 4 × 1 B of |y|
  (all four blocks, coded or not) + 1.5 B for block 0's κ (8 bits) and tan φ (4 bits). Later levels 40.4, 69.9,
  99.5 B; group 0's rates give 36, 68 and 93 B of code, so 12.8 B of scalars over 10 rises, 1.3 B per rise. So a
  (coded block, rise) costs 1 to 1.5 B and a block's |y| 1 B once. My STORED list in c3c.py and c4.py was 0.5 to
  2.6 B off at four levels (27, 36, 65.5, 107.5 where the log gives 26.5, 35, 64.5, 104.9 without the key).
- Time [measured phases, bd_p2c.out, profiled; entrant counts inferred]: scalar decode 10.78 ms at item 50%, 4.20
  at text 1%, 0.58 at item 1%, 11.61 at text 50%. Rises decoded per query at item 50%: about 0.91 + 3·0.88 + 3·0.50 +
  4·0.21 = 5.9 per item of N if 96 B is entered by 0.88 and 160 B by 0.50; those two are poorly fixed by the bytes
  and the table counts (0.44 to 0.91 for the first), so 10.78 ms is 1.4 to 2.8 ns per (item, block, rise) for the
  scalar decode alone.
- The `--design-exact` trial, all-in: +8 to +24% of 71.3 ms (item 50%) over 0.91·2 extra block-rises per item is 3.1
  to 9.4 ns per extra (item, block); the same at text 1% (37.7 ms, 0.46 entering) is 3.3 to 9.8 ns. The report does
  not say which cell is the 8 and which the 24.
- Other constants: level decode 18.23 ms over about 137 B of code per item at item 50% = 0.13 ns per code byte;
  everything else per (item, level) about 3.2 ns (8.17 item-levels per item from the table counts); a full vector
  104 to 110 ns. So one block-rise costs as much time as decoding 11 to 75 bytes of code.
- Found while doing this: the constant is 6.931 with one block coded and 7.963 with several, and my model held it
  fixed. With it, and with menu rates, the model's gain from three blocks at 56 B falls from 21% of width to 15%.
- `c5a.py`, `c5a.log`: alternatives for the first randomised level (built prefix and cuts, 192 code bits). In time
  no alternative with three or four blocks pays above 1.1 ns per block-rise in any cell; two blocks at free rates
  pay below 1.5 to 3.3 ns. The trial's own figure (2% fewer entering 96 B) is far below the model's 13 to 26% for
  three blocks at menu rates, so the model overstates the gain as well as omitting the cost.

Next: `c5b.py`, the one-shot programme with a charge per coded block and the constant by number of blocks.
