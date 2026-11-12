# Agent 3a — stronger guarantees per bit: is the wall real?

Saved by the coordinating session from the agent's returned text (the agent could not write files
itself). Condensed; every number is the agent's. **Not independently verified.**

Setup: 200k subset (`e00, e05, e10, e15`), K = 200 clusters, corpus items as queries, numpy only.
Notation as in FINDINGS: x = m + r, Δ = q − m, a rotated block has n_b dimensions, m_b = n_b − 1,
A = ‖y‖·tan φ is the stored error scale of an item's block.

## Outcome

| Question | Outcome | Status |
|---|---|---|
| Adaptive queries, d = 0 | Adaptivity costs nothing beyond the union bound, if the client sees only result sets | proved (T1) |
| Leakage | L leaked bits cost c² → c² + 2·ln2·L; an attack reaches ~0.8·√L sd | proved (T2) + measured |
| d > 0 with randomised certificates | Not adaptively safe (band decisions leak the estimate); fix: seed-independent answer rule | measured + proved |
| Query-time randomness | Cannot tighten an interval from stored bytes; buys only a Monte-Carlo rate | proved (T3) |
| Uniform over a k-dim query class | Half-width factor (√k + c)/√m_b; equals a deterministic two-scalar certificate | proved (T4) + measured |
| Realistic small class | Only finite classes are cheap (queries = corpus items: c ≈ 10.6 instead of 8.4 at N = 10⁹) | proved + measured |
| Deterministic wall | Floor proved for the Gaussian model; scheme is 1.36–1.41× above it up to 96 B | proved (T5) + measured |
| Box ∩ ellipsoid, exact | 3 / 7 / 14% tighter at 32 / 96 / 192 B, zero violations, CPU only | measured |
| Sketch the uncoded tail | Sub-1-bit rotated blocks: ~20% below the split scheme at 96–192 B, c = 8 | measured |
| Exact tail instead of sub-Gaussian | 5–20% tighter single-block bound | proved + computed |

No result moves the deterministic wall. The randomised side has about 20% left in it.

## Theorems

**T1 (adaptive, d = 0).** If every (q, t) fixed independently of ω has Pr[R ≠ S] ≤ ε, then for T
adaptively chosen queries where the client sees only result sets, Pr[some wrong answer] ≤ T·ε.
Proof: fix client coins; define the ideal query sequence by running the client against the exact
oracle (independent of ω); a first wrong answer at step i implies the ideal i-th query failed;
union bound. Also covers items the client inserts adaptively.

**T2 (leakage).** If the client also sees side information with at most 2^L values, the bound is
2^L·T·ε. To keep it, raise c² to c² + 2·ln2·L.
Measured tightness (`exp_b.py`): error in predicted sd from L leaked sign bits of one item's
error vector — n_b = 128: 3.0 / 5.6 / 8.4 / 10.3 / 11.0 at L = 16 / 64 / 256 / 1024 / 4096
(deterministic ceiling 11.3); n_b = 256: 3.1 / 6.0 / 10.0 / 13.6 / 15.2 (ceiling 16.0).
So c = 8 falls to ~130 leaked bits, c = 10 to 250–900.

**d > 0 is not adaptively safe with randomised certificates.** Whether a band item is returned
depends on its estimate, hence on ω; binary search on t reads the estimate. Fix: make the answer
seed-independent — either d = 0 with randomised certificates, or decide band items from a public
deterministic code and use the randomised certificate only outside the band (cost not measured).
Deterministic certificates need no fix.

**T3 (query-time randomness).** Fix the bytes read for an item; if the feasible set straddles t,
two corpora with identical bytes have different correct answers, so any decoder errs with
probability ≥ ½ unless it reads more. Sampling exact coordinates gives c·H/√k′ by Hoeffding —
rate bytes^−½, loses to the stream's bytes^−0.7 to −1.3.

**T4 (class-uniform).** For one rotated block and a k-dim subspace W fixed independently of ω,
with probability ≥ 1 − e^(−c²/2), for every Δ:
|⟨E, Δ⟩| ≤ A·[ ‖P_W Δ‖·(√(k/m_b) + c/√(m_b − 2)) + ‖P_W^⊥ Δ‖ ].
Measured max ‖P_W u‖·√m_b (40k items/block, 0 violations), random W / top-k query directions:
k=1: 4.07/4.13 (n_b=128), 5.66/4.18 (256); k=16: 6.65/6.61, 6.85/6.66; k=64: 9.68/9.81,
10.35/10.55; k=128 (n_b=256): 13.14/13.31.
Reduces to a deterministic certificate storing ‖P_W E‖ and ‖P_W^⊥ E‖. Real offsets are
full-dimensional in the whitened blocks (participation ratio 103 of 128, 160 of 256), so no
subspace class is cheap: 0.88–0.99 of plain Cauchy–Schwarz versus 0.47–0.71 for the per-query
bound.

Price list (half-width as a multiple of A‖Δ‖): per query c/√m_b; finite class of M queries
√(c² + 2 ln M)/√m_b; k-dim class (√k + c)/√m_b; L leaked bits √(c² + 1.39 L)/√m_b.

**T5 (deterministic floor, Gaussian model).** r ~ N(0, Σ_R), Δ ~ N(0, Σ_Q) independent, γ_i the
eigenvalues of Σ_Q^½ Σ_R Σ_Q^½. Decoder holds ≤ B bits of entropy per item (code + scalars) plus
S shared bits per group of M_g items. Then the mean half-width satisfies
E h ≥ max_k E‖g_k‖·ω_k^(−1/k)·√(2πe)·(γ_1⋯γ_k)^(1/2k)·2^(−(B + S/M_g)/k) ≈ max_k k·GM_k(√γ)·2^(−B/k).
Proof sketch: whiten by Σ_Q^½; project the hull of the feasible set onto the top-k eigenspace
(Jensen); Urysohn; entropy of a variable supported on a set ≤ log volume, conditioning on H bits
removes at most H; Jensen. Covers variable rate, stored scalars, non-convex/lattice cells, shared
group information.

Measured against it (`exp_c.py`):

| Bits | 32 | 128 | 256 | 768 | 1536 | 3072 |
|---|---|---|---|---|---|---|
| Floor (optimal k) | 0.0978 (219) | 0.0752 (288) | 0.0566 (337) | 0.0240 (477) | 0.0086 (534) | 0.0015 (692) |
| Achieved | 0.1373 | 0.1025 | 0.0769 | 0.0338 | 0.0133 | 0.0030 |
| Ratio | 1.40 | 1.36 | 1.36 | 1.41 | 1.55 | 2.02 |

Only hole: non-Gaussian structure (J bits of negentropy per item lower the floor by 2^(−J/k)).

## Sub-bit tail sketch

Keep the signs of the first k′ of n_b rotated coordinates: tan²φ = π·n_b/(2k′) − 1, so the
block's error sd is ‖y‖‖Δ‖·√(π/(2k′) − 1/n_b), independent of n_b. Break-even vs deterministic at
k′ = c²π/2 ≈ 100 bits (c = 8), 157 (c = 10). Synthetic check (`exp_d.py`, 2M pairs): matches
prediction to 3 digits.

On the data (`exp_a.py`, 20M pairs/run, 0 violations, max 5.5 sd), mean half-width over matches
at t = 0.9:

| n | Bytes | Deterministic | Split c=8 / 10 | This design c=8 / 10 | Scalars split / this |
|---|---|---|---|---|---|
| 768 | 32 | 0.0150 | 0.0132 / 0.0141 | 0.0129 / 0.0144 | 14 / 11 |
| 768 | 96 | 0.0065 | 0.0038 / 0.0042 | 0.0030 / 0.0037 | 18 / 13 |
| 768 | 192 | 0.0027 | 0.0012 / 0.0014 | 0.0010 / 0.0012 | 18 / 13 |
| 256* | 64 | 0.0071 | 0.0045 / 0.0050 | 0.0038 / 0.0047 | 18 / 11 |
| 128* | 64 | 0.0030 | 0.0021 / 0.0023 | 0.0018 / 0.0023 | 18 / 9 |

\* PCA truncations of the same data, not native embeddings of that size.

Exact tail of a sphere coordinate (`tail.py`), as a fraction of the deterministic bound
[sub-Gaussian in brackets]: n_b = 48: 0.85 [1.17] (c=8); 128: 0.61 [0.71]; 256: 0.45 [0.50];
768: 0.27 [0.29]; 5000: 0.107 [0.113].

## Owner's metric (like-for-like, NOT comparable with the FINDINGS baseline)

Three levels (32, 96, 192 B) then the full vector; stored scalars charged at 2 bytes each when a
level is read; 200k sample where true matches are 2.22 / 4.80 / 9.47 / 17.34%.

| Scheme, dataset read % | 0.95 | 0.9 | 0.8 | 0.7 |
|---|---|---|---|---|
| Deterministic, all levels | 2.90 | 3.08 | 4.05 | 10.24 |
| Split c = 8, all levels | 4.18 | 4.31 | 4.96 | 9.22 |
| Split c = 8, deterministic first level | 2.76 | 2.89 | 3.62 | 8.01 |
| This design c = 8, deterministic first level | 2.72 | 2.83 | 3.45 | 7.13 |
| This design c = 10, deterministic first level | 2.75 | 2.87 | 3.54 | 7.59 |

Stored scalars decide it: a randomised certificate at the first level reads 22–28 B of scalars
per item and is worse than deterministic at 0.95–0.8. It belongs only at levels read for few
items.

## Dependence on n

- Randomisation gain is √PR/c, PR = (Σaᵢ)²/Σaᵢ² ≤ n. Measured PR at n = 768: 214 (4 B) to 536
  (192 B). At n ≈ 100 and c = 10, c/√m ≥ 1, so only the exact tail gives anything.
- The sub-bit sketch is dimension-free, so long weak tails at n = 5,000 cost no more.
- Class-uniform and leakage budgets scale with block size.
- T1–T3 do not depend on n.

## Did not work / not done

- Class-uniform guarantees gain 1–12% on real offsets. Group moments, lattices, shared
  information are covered by T5 or dominated. Query-time sampling loses to the next level.
- First all-randomised sub-bit design was worse at n = 128 and at c = 10, 32 B.
- Not done: text queries, a native second dimension, nested prefixes, timings, the cost of the
  d > 0 answer rule. The Alon–Klartag statement in FINDINGS was not verified.

## Files (this directory)

`exp_a.py` (+ logs, `results_a*.json`), `report_a.py`, `exp_b.py`, `exp_c.py`, `exp_d.py`,
`exp_e.py`, `tail.py` and their logs.
