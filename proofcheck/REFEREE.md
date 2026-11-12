# Referee report on the floor proofs and related results

Saved by the coordinating session from the referee agent's returned text; condensed. The
consolidated write-up of surviving results is `CONSOLIDATED_PROOFS.md` in this directory. Scripts:
`constants.py`, `toyfloor.py`, `misc.py`. The referee read `agent1/theory.py` but did not re-run
it.

## Verdicts

| Result | Verdict |
|---|---|
| Theorem A / T5 (volume floor) | Correct with two small fixes |
| Claims (a), (b) | Correct |
| Claim (c) | Correct for the proved bound; a gap as a claim about the true optimum |
| Theorem B | Correct (the constant 1 can be 1/√2) |
| Theorem C | Correct; the sketch completes |
| Theorem D | Correct with a fix: the encoder must be a fixed map, and ℓ* is defined over encoder pre-images |
| "Factor 2 against byte-granular readers" | Correct with a fix: 2 from the second level on, 4 at the first level |
| T1, T2, T3 | Correct |
| T4 | Correct; the "√k unavoidable" remark is only a heuristic |
| Sub-bit law | Correct asymptotically in k′; E tan²φ is infinite for k′ ≤ 2 |
| Exact tail | Correct, and provable (cone argument) |
| Sharper-c list | Correct; table reproduced |

## Fixes to the floor

- "H(c) ≤ B ln 2 for variable-length codes" needs the code to be uniquely decodable; otherwise use
  the maximum length or add a log term. Agent 1's escape code is self-delimiting and truncated, so
  it is fine.
- The shared-bits term S/M_g needs the items to be independent, which T5 does not state. With
  near-duplicates, shared bits can inform every item.
- Agent 1's "eigendirections of QΛ" must be those of Q^½ΛQ^½ (QΛ is not symmetric). T5 states it
  correctly. Checked on 3,000 random non-commuting (Q, Λ): worst achieved/floor 1.13.

## Most important error: in the reported numbers, not the proofs

Agent 3a's ratios 1.40 / 1.36 / 1.36 / 1.41 / 1.55 / 2.02 divide a certificate that uses three
stored scalars by a floor that does not count them. The "achieved" figure is also a model formula
(expected weighted Cauchy–Schwarz width), not a measurement; its quantiser's outer cells are
unbounded, so without the scalars h = ∞. Against its own 48-scalar-bit column the ratios are
1.61 / 1.52 / 1.49 / 1.51 / 1.65 / 2.11. Agent 1's uniform column agrees with the corrected figure
(1.52 at 768 bits).

## Headline claim

"Within about 1.4× from 96 bytes up" survives only in a narrow form: agent 1's escape code sits
1.36–1.42× above the proved bound at 96–576 B, on synthetic Gaussian residuals with diagonal
spectra and independent Gaussian query offsets. The baseline scalar code is at 1.5–1.7× at 96–192 B
and 2.7–9× beyond.

No floor is proved for the real data:
- residuals are non-Gaussian;
- Q was taken diagonal in the residual basis rather than using the eigenvalues of Q^½ΛQ^½;
- queries are not Gaussian or independent of items;
- the ~600 bits per item of shared local bases (768² × 16 bits per group of ~15,600 items at 1M)
  are not counted;
- the stored m·e scalar is also information about r.

## Theorem D

- The swap argument needs the encoder to be a fixed map; if means and bases are recomputed from
  the corpus, swapping x changes what other items store.
- ℓ* must range over every unit vector the encoder maps to this prefix in this cluster; an x′ that
  the assignment rule would send to another cluster is not a valid alternative.
- The upper half holds only for a certificate computing the exact range over that pre-image; the
  scheme uses weighted Cauchy–Schwarz.
- Byte-granular factor is c_ℓ/(c_{ℓ−1}+1): below 2 from the second level on, 4 at the first level
  (1.6 if the 4 scalar bytes are charged at level 0), 3.66 at the final jump.

## T1–T4 and the public seed

- T1 relies on d = 0 (correct answers are unique, so carry no information about ω); fails as
  written for d > 0.
- T2: the side information must be the whole transcript; c² + 2L ln 2 is right.
- T3: correct.
- T4: correct (needs m_b ≥ 3). The "√k cannot be avoided" remark holds only for a uniformly random
  W, not as a lower bound for a given W.

Effect of the public seed: no proof changes. T1 covers only clients whose queries are functions of
their own coins and earlier answers; a client that reads the seed is outside it. T2 is vacuous
against such a client. T3 then applies: to someone who knows ω the scheme is deterministic, so no
guarantee below the deterministic certificate can hold against seed-aware queries. **The corpus
must also be independent of ω**: with live ingest, someone who reads the seed can insert items
built to fail for a fixed query, and none of T1–T4 covers this. T4 is the only result that holds
for seed-dependent queries (W and items must be independent of ω).

## Other checks

- κ_k/k = 1.649, 1.461, 1.161, 1.0266 at k = 1, 2, 10, 100, above 1 for every k ≤ 200,000.
- The bound is tight in one dimension at high rate (1.0001 at step 0.05σ); no tested code beat it
  (4,000 random product codes, optimised interval partitions, 300 Voronoi or merged non-convex
  partitions).
- Sub-bit law: mean tan²φ exceeds the formula by 78% at k′ = 4, 11% at 16, 2% at 64.
- Exact tail fails for s near 1 (m = 2 from s ≈ 0.946), so s ≤ 1/√2 is needed.
- Sharper c: reproduced 0.741 / 0.638 / 0.537 at m = 30 and 0.915 / 0.936 / 0.940 at m = 767. The
  scheme's own deterministic certificate on rotated codes depends on the seed and cannot define
  the union-bound set M; M depends on (q, t), so this bounds a reported, not promised, failure
  probability.
