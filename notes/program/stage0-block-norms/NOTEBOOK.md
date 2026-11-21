# Stage 0: block norms for out-of-distribution queries (notebook)

Researcher: exact-path experimentalist. Reports to the PI. Everything here is working material; the tables for the PI
are in `REPORT.md`.

## Brief (21 November 2026)

A deterministic level bounds the unread term by |Δ·e| ≤ ‖DΔ‖·‖D⁻¹e‖ with one stored norm per item and D fixed at build
from a query profile. Idea under test: store J block norms per item, giving Σⱼ‖Δⱼ‖‖eⱼ‖, the best bound over all weights
constant on the blocks, for every (query, item) without a query sample. Stop condition: if block norms in the data-only
fit do not cut the text half-width by at least 10 % against one norm, drop the idea.

Rules kept: numpy only, cores 2–3 at nice 10 with two BLAS threads, the 200k sample, nothing timed, outputs under this
directory only, no file over 100 MB.

## Setup decisions (and why)

- **Sample and partition.** 200k BioCLIP-2 sample (g0–g3, f16, normalised in float64 per chunk). Spherical k-means,
  K = 200, then G = 12 groups by k-means on the cluster means, as `reports/surge/ood/standin.py` does. Cluster sizes
  159 / 861 / 3,692 (min / median / max); group sizes 3,199 to 50,503. At 1M the groups report used K = 1,000 and G = 64,
  so cluster and group sizes match to within a factor of about 1.2.
- **Fits use training queries only.** Σ_Q data-only: 3,000 corpus items (the 100 held-out item queries excluded) × 40
  clusters drawn by size, Δ = q − (q·m̂)m̂ (`groups/scripts/sigmaq.py`). Σ_Q text: the 2,206 training prompts of
  `text_train.npy` the same way. Mixed: ½(Σ_item/tr + Σ_text/tr). Ridge 0.1·tr/n on every Σ_Q used for a basis.
- **Test queries.** 369 held-out prompts (`text_q.npy`), 361 in-sample species prompts (`text_insample.npy`), 100
  held-out corpus items. Note: the brief calls `text_insample.npy` the training prompts. It is not: its prompts are
  "a photo of <species>" for species with images in the sample (`threshold/text_sets.py`), and none of its prompts is
  in `text_train.json`. The training set every earlier fit used is `text_train.npy` (2,206 prompts, no prompt in
  common with `text_q.json`), and I fit (b) with that. `text_insample` is therefore a second held-out text test set.
- **Item queries are scored against every item, themselves included**, as in `groups/scripts/measure.py`. One item
  per item query has score 1; negligible.
- **Offset.** Δ = q − αm with α = q·m/‖m‖², so Δ ⊥ m and the known part is q·m + α·m·r (the key gives m·r). The
  same Δ for every bound, so the bounds differ only in how the unread term is bounded. Fitting Σ_Q used the same
  projection.
- **Basis.** Per group, the dual pair of `groups/REPORT.md` §2.4: B = A Σ_R A with A = Σ_Q^½, eigenpairs (γ, V),
  item side z = r·(A V γ^(−¼)), query side u = Δ·(A⁻¹ V γ^(¼)), so u·z = Δ·r exactly. Identity checked per chunk:
  largest |q·m + α m·r + u·z − q·x| = 1.4·10⁻¹⁵ (float64).
- **Level.** A level at reach k knows z₁…z_k exactly; e = (z_{k+1}, …, z_n) is bounded. k ∈ {0, 8, 16, 32, 64, 128,
  256}. A built deterministic level of 4 / 8 / 16 / 32 B touches at most 32 / 64 / 128 / 256 coordinates
  (`bench/suite/notes/visit-order/groups-setup.md` §2), with one to five bits each, so exact k overstates what a
  level of that reach knows; the tail here is the unread tail only, not a coding error spread over the coded part.
  `quant.py` covers the second case with a model quantiser.
- **Weights.** D_i = (λ_i / P_i)^¼, λ_i the group's item variance of z_i (= √γ_i) and P_i the profile
  diag(V_Qᵀ Σ V_Q) for Σ = Σ_item (a), Σ_text (b1) or Σ_mix (b2, in the mixed basis). This minimises
  E‖Du‖²·E‖D⁻¹e‖² over diagonal D. The built profile is the mean square against each pseudo-query's 4 nearest
  clusters; mine is over clusters drawn by size, the law Σ_Q was fitted with, because a text query meets every cluster.
- **Bounds.** (a) one norm, data-only D; (b1) one norm, same basis, D from text prompts; (b2) the built remedy's
  analogue: mixed basis and mixed weights, its own first k coordinates exact; (c) J ∈ {2, 4, 8, 16} block norms on
  the data-only basis, D inside each block, so J = 1 is (a) exactly; blocks cut over [k, n) by equal counts or by
  equal shares of Σλ; (d) Σ|u_i||e_i|, which is the best diagonal weighting for each pair; b2c4, b2c16, b2d the same
  on the mixed basis. Each stored-norm bound is repeated with the item norm(s) rounded up to the 2^(1/16) grid the
  built code stores (suffix r; no range limit, no escapes).
- **Soundness check.** Every bound against |s − known| with s = q·x in float64, every (query, item) pair, every k.
  A violation is actual − bound > 10⁻⁹; the largest (actual − bound) is also kept.

## Log

- 01:53 prep.py (32 s): thresholds, mean over text_q: 0.618 / 0.592 / 0.566 / 0.545; in-sample species 0.769 / 0.743
  / 0.706 / 0.665; items 0.914 / 0.890 / 0.851 / 0.802.
- 01:58 measure.py on group 8 alone (18 s, peak RSS 458 MB): 0 violations; text mean h at k = 32: a 0.209, cvar4 0.198,
  d 0.126. First sign: blocks by position buy about 5 %, the per-pair limit about 40 %.
- 02:00 measure.py on all groups started (pid 115920). Done at 02:19 (1,144 s): 0 violations over 830 queries ×
  200,000 items × 22 bounds × 7 k; largest (actual − bound) −2.7·10⁻⁵. Output: `work/report_measure.txt` (report.py).
- 02:19 quant.py (model Lloyd-Max quantiser, greedy allocation by the data-only profile, 4–32 B) started (pid 127928),
  done in 356 s: 0 violations; largest (actual − bound) −2.3·10⁻³. About 105 of 768 coordinates hold bits at 32 B.
  The Lloyd iterations (100) leave the 6–8-bit distortions slightly above optimum; this only affects the allocation.
  Output: `work/report_quant.txt` (report_quant.py).
- The harness refused a separate REPORT.md from a subagent; the report went to the PI in the hand-back message. This
  notebook holds the findings in short form below.

## Findings (all [measured]; 200k sample, 369 held-out prompts, 361 in-sample species prompts, 100 items)

- Text held-out, mean half-width against one norm (a), idealised k = 0…256: equal-variance blocks J = 4 0.90→0.97,
  J = 16 0.84→0.95; equal counts are weaker (J = 16 0.89→0.96). In the model quantiser: J = 4 0.91–0.92, J = 8 0.89–0.90,
  J = 16 0.87–0.89. The 10 % bar is passed only at J ≥ 8 (idealised only for k ≤ 16–32).
- Not text-specific: per-coordinate limit d / a ≈ 0.57–0.64 for every query kind, items included. Blocks give items
  0.89–0.97 (idealised J = 16) and in-sample species 0.90–0.95.
- Text-fitted diagonal weights (b1): text 0.91–0.98, items 1.03–1.16. Blocks J = 4 equal it with no query sample, and
  J ≥ 8 beat it. The mixed-fit basis (b2) gives text 0.67–0.79 but in-sample species ≈ 1.00 and items 1.06–1.19.
  Blocks on top of b2 (J = 16) add about 0.03–0.08 more for text.
- Cost against saving (quantiser, bytes at equal undecided share against a, with cumulative norm cost (J−1)/2 per
  level): only J = 2 nets positive, and only for held-out text (+0.7 to +1.8 B at 0.01 %, about 0 at 1 %). J ≥ 4 loses
  from the second level on, and every J loses for items.
- Verdict: dropped as an out-of-distribution remedy.

## Open leads, if the PI wants them

- J = 2 equal-variance at the first one or two levels only (the one positive cell); needs the built quantiser.
- Block norms on the mixed basis stack (b2 + J = 16 adds 3–8 %), but cost the same bytes.
- The d gap (0.6 for all kinds) is the real prize and is generic. A sketch of e that is cheaper than coordinates
  (signs of the largest |eᵢ|?) is a separate question.
