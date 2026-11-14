# Making the bulk cheaper

Saved by the coordinating session from the agent's returned text (the agent could not write files
itself); paths adjusted to this branch. Every number is the agent's. The agent was a restart of one
lost in a container restart; it re-checked its predecessor's runs before relying on them.

The question: the cost of a threshold query is dominated by the bulk, items 1–8 score standard
deviations (sd) from the threshold that must be read because the interval at zero or few bytes is
wider than their distance to t (`groups/REPORT.md` section 3.5). How can each such item be decided
with fewer bytes or less time, keeping the guarantee?

## Summary

The bulk can be made about a third cheaper in bytes, and several times cheaper in time, by letting
the query choose which columns to read. Nothing found escapes the floors by more than that.

All measurements use a 200k sample (seed 11) of the 1M set, with every interval and decision checked
against the exact score: **0 violations in every run**.

| Lever | Gain | Status |
|---|---|---|
| Query-chosen column order | Coordinates read fall 21–36% (exact coordinates). Bytes fall 11–25% against the design's shape on the same quantised code, and 28–44% for items 2–8 sd from the threshold. | measured |
| Columnar bulk pass (time) | 8.9 / 11.7 / 13.9 ns per item at 12 / 24 / 32 B, two checks included. The native prototype spends 39–48 ns per item at 19–29 B. | measured, kernel only, synthetic data |
| Randomised read part from a public dither | A further 7–10% at 0.01–0.1% answers, 18–25% at 1–5%. No stored scalars. | certificate proved, gain measured |
| Sign-only columns | A further 4–6%. | bound proved, gain measured |
| One-sided union bound for exclusions | About 1%. Not worth a change to the guarantee. | proved, measured |

- **Combined:** all four byte levers on the data-only dual basis take text queries from
  36 / 54 / 115 / 220 B to 24 / 33 / 71 / 141 B (−35 to −38%). Against the PCA baseline that is −42
  to −44%. On the text-fitted dual basis the gain is −26 to −35%, so it stacks with the dual basis.
- **The catch for item queries:** at 0.01–0.1% answers they get worse (5.5 → 9.3 B), because the
  query-chosen order needs block norms that cost 5 B more for every item touched. Reading those
  norms only when the single stored norm fails should remove this; that is computed, not run.
- **Absolute bytes are not competitive yet:** these 4-bit cells read 26–41 B at text 0.01% where the
  native escape code reads 18.6 B. Whether the order gain carries to an escape-style columnar code is
  conjectured from the exact-coordinate result.
- **Floors:** one-sided decisions do not escape the volume floor (proved). Randomising the unread part
  loses to deterministic codes in the bulk range (computed). An average-case guarantee would need 4 B
  where the conjectured per-item floor needs 18 B; the per-item requirement closes that route.

The native dual-basis result (`dualproto/REPORT.md`) agrees with this one in size, except at zero
read; see section 5.

## 1. What was inherited from the lost agent

- **`adapt.py`** (query-chosen order, exact coordinates): sound. Its fixed-order numbers reproduce
  `groups/REPORT.md` (PCA text 45.3 / 66.2 / 118.2 / 201.1 against 44.9 / 64.9 / 116.7 / 202.0 on the
  full 1M; data-only dual 38.4 / 56.5 / 101.7 / 175.0 against 37.7 / 54.9 / 99.9 / 175.9). Relied on:
  `adapt_pca.log` and `adapt_sq_item.log`, 93 text + 60 item queries, 2.448·10⁹ interval checks each,
  0 violations.
- **`adapt_sq_text`**: cut off, not rerun; the byte experiment covers that basis.
- **`quant.py`**: never ran and was not run. `qcol2.py` replaces it with a simpler two-plane code and
  adds the design-shaped baseline.
- **`slack.py`, `model.py`**: run; logs `slack_pca.log`, `slack_sq_item.log`, `model.log`.
- **`colscan.c`**: run. Its scalar order selection cost 8.7 µs per cluster and dominated, hence
  `colscan2.c` and `colscan3.c`.

## 2. Where the zero-read slack sits (measured, `slack.py`, 41 text + 40 item queries)

- **More block norms do little.** Text, PCA: 7.4 sd with one norm, 6.55 with four, 6.44 with six,
  6.19 with eleven. Data-only dual: 5.86 / 5.59 / 5.50 / 5.34.
- **The slack is in the middle of the spectrum.** Coordinates 8–512 hold 75% of it; the tail block
  512–768 holds 0.8%. Sketching or dropping the tail is not a lever.
- **One query's offset is more concentrated than the group profile predicts.** Its 16 largest terms
  of Δᵢ²λᵢ hold 78% of the total; the first 16 coordinates in basis order hold 64% (PCA, text). This
  is what a query-chosen order uses.
- **The flat Gaussian model overstates the gain.** It predicts 2.3× fewer reads at half the slack
  (`model.log`); the data gives 1.3–1.5×, because the fixed order already follows a steep spectrum.

## 3. Which assumptions of the floors the bulk can escape

**3.1 One-sided decisions: no escape (proved).** Take the setting of Theorem 1 of
`proofcheck/CONSOLIDATED_PROOFS.md` and an upper certificate U(c, Δ) ≥ Δ·r for every r in the cell
K_c. Then U ≥ h_{K′_c}(g), and E_g[Δ·r] = 0 for each fixed r because g is centred and independent of
r. So E[U − Δ·r] ≥ E h_{K′_c}(g), which is exactly the quantity steps 2–5 of Theorem 1 bound. The
mean slack of an upper bound alone obeys the same floor as the mean half-width; the same holds for
Corollaries 1–2 and Theorems 2–3. ∎

**3.2 Reads chosen by the query: a real escape, bounded by a log factor.**
- Theorem 1 fixes the message before the query. Once the cell depends on g, the Urysohn step fails.
- Ceiling (proved; reconstructs the lost agent's "Proposition D"). Let g ~ N(0, I_D) and let
  a₁…a_M be unit vectors fixed in advance. Then E max over k-subsets A of ‖P_A g‖² ≤
  k(1 + 2√L + 2L) + √(πk) + 2, with L = ln(eM/k).
  *Proof:* for fixed A, ‖P_A g‖² is at most χ²_k in law. Laurent–Massart gives
  Pr[χ²_k ≥ k + 2√(kx) + 2x] ≤ e⁻ˣ. There are at most e^{kL} subsets; take x = kL + s and integrate
  over s. ∎
- Consequence: a query-chosen read of k exact projections removes at most that much offset energy, so
  an overcomplete dictionary buys only a logarithm for a multiple of the storage. For D = 160 and
  M = 768, halving the bound needs k ≥ 7 by the ceiling, against 35 in the flat model for an
  orthonormal basis.

**3.3 The ε allowed per query.**
- **Dither certificate (proved).** Store k = ⌊(z + u)/s⌋ with u uniform on (−s/2, s/2), drawn per
  coordinate and group from the public seed. Escapes are decided by |z| > T before the dither. For
  any z the reconstruction error is exactly uniform on (−s/2, s/2] and independent across
  coordinates. Since sinh(x)/x ≤ exp(x²/6), the read part Σ Δᵢeᵢ is sub-Gaussian with proxy
  Σ Δᵢ²sᵢ²/12, so each side fails with probability at most exp(−c²/2) at half-width
  c·√(Σ Δᵢ²sᵢ²/12). This is one number per (query, cluster, level): no stored scalar and no per-item
  work. The deterministic box bound stays valid as the backstop, and the interval takes the smaller.
- **One-sided union (proved).** An exclusion can only be wrong for a true match, so the failure
  probability is at most |S(t)|·L·exp(−c_out²/2) + N·L·exp(−c_in²/2). That lowers c from 9.48 to
  8.5–9.2. Measured gain: about 1%.
- **Randomising the unread part does not help (computed).** By Lemma 8 an ideal spherical code needs
  c²/(1.39R²) bits to bring the interval to R times the M-norm ball; the sign sketch of Proposition 13
  needs c²π/(2R²). At R = 0.5 that is 32 B and 70 B, against a conjectured deterministic floor of
  14–18 B.

**3.4 The gap to typical error (computed, `floor.py`).** Gaussian model on the fitted spectra, where
ℓ1 = 0.219, ℓ2 = 0.021 and the participation ratio is 88–110. Bytes to bring the bound to R times its
zero-read value, text-fitted spectrum:

| R | Proved floor | Conjectured floor | Average-case error × c | Exact coordinates in order |
|---|---|---|---|---|
| 0.75 | 0 | 4.7 | 0.7 | 13 |
| 0.5 | 6.9 | 18.4 | 4.0 | 44 |
| 0.33 | 28.1 | 40.3 | 11.9 | 86 |
| 0.25 | 47.0 | 58.3 | 20.2 | 118 |

The average-case column is what a guarantee over random item–query pairs would allow. It is not
attainable per item: Theorem 7 applies, since an aligned residual and a typical one store the same
bytes.

**3.5 Threshold known, tolerance, non-Gaussian item structure:** nothing new. Knowing t is already
used by early stopping.

**3.6 Time rather than bytes:** a real escape; see 4.3.

## 4. Measurements

### 4.1 Query-chosen order, exact coordinates (`adapt.py`; coordinates per item at 0.01 / 0.1 / 1 / 5%)

| Basis, queries | Fixed order | Query-chosen | Query-chosen, norms-only check |
|---|---|---|---|
| PCA, text | 45.3 / 66.2 / 118.2 / 201.1 | 31.2 / 47.2 / 89.3 / 159.7 | 37.3 / 56.3 / 105.5 / 186.6 |
| PCA, item | 4.5 / 10.7 / 38.7 / 116.3 | 2.9 / 7.2 / 27.9 / 89.9 | 3.4 / 8.5 / 32.8 / 105.1 |
| Dual data-only, text | 38.4 / 56.5 / 101.7 / 175.0 | 25.5 / 39.0 / 74.9 / 136.2 | 30.5 / 46.4 / 88.1 / 158.1 |
| Dual data-only, item | 3.1 / 7.1 / 24.0 / 70.2 | 2.0 / 4.8 / 17.3 / 53.3 | 2.4 / 5.6 / 20.1 / 61.8 |

- The order is by Δᵢ²λᵢ per (query, cluster); the unread part is bounded by block norms.
- By band, text at 0.01%, dual basis: items 2–4 sd away fall from 92 to 63 coordinates, items
  4–8 sd away from 17.6 to 10.5.
- The norms-only check keeps the whole block norm on the item side and so needs no per-item energy
  accounting. It keeps about half the gain.

### 4.2 Bytes on a quantised columnar code (`qcol2.py`, `tables_qcol2.txt`)

**The code.** 4-bit cells of width 0.6 sd with escapes, and a second 4-bit plane 16 times finer for
the first 128 coordinates of the fixed order. Each plane is read as its own column. Six block norms
at 1 B each, plus 2 B. "Design shape" is the fixed order with levels at 4 / 8 / 16 / 32 / 48 / 96 … B
and one stored weighted error norm per level, on the same cells.

**The runs.** 19 text + 10 item queries per basis. Interval checks: 1.94·10⁹ (PCA), 2.57·10⁹
(data-only dual), 1.94·10⁹ (text dual); 0 violations. The largest randomised read error was 5.53
proxy sd, against c = 9.48.

Text queries, bytes per item (scalars included) at 0.01 / 0.1 / 1 / 5%:

| Variant | PCA | Dual, data-only | Dual, text-fitted |
|---|---|---|---|
| Design shape, fixed order, stored norms | 40.8 / 59.8 / 127.7 / 243.9 | 36.4 / 53.5 / 114.6 / 219.8 | 26.2 / 39.7 / 91.4 / 186.5 |
| Fixed order, box bound | 44.9 / 64.4 / 136.1 / 262.3 | 36.3 / 52.8 / 113.5 / 223.4 | 28.7 / 42.0 / 93.8 / 193.0 |
| Query-chosen order, box bound | 34.3 / 50.0 / 112.3 / 230.9 | 27.2 / 40.1 / 92.3 / 195.1 | 22.2 / 32.5 / 76.3 / 167.8 |
| + dithered randomised read part | 32.1 / 45.1 / 93.1 / 176.1 | 25.4 / 36.0 / 76.0 / 147.1 | 20.9 / 29.4 / 63.6 / 128.9 |
| + sign columns, one-sided c | 29.7 / 41.9 / 87.9 / 169.0 | 23.5 / 33.3 / 71.4 / 140.8 | 19.5 / 27.4 / 59.8 / 123.3 |
| Query orders only the first 192 coordinates, randomised | not run | 26.9 / 38.8 / 84.2 / 164.3 | not run |

Item queries:

| Variant | PCA | Dual, data-only | Dual, text-fitted |
|---|---|---|---|
| Design shape | 6.1 / 11.9 / 52.1 / 143.7 | 5.5 / 9.6 / 39.5 / 101.6 | 5.9 / 10.7 / 46.5 / 121.1 |
| Query-chosen, randomised | 9.6 / 13.3 / 41.3 / 105.5 | 9.4 / 11.8 / 31.2 / 71.8 | 9.6 / 12.5 / 35.8 / 84.9 |
| + sign columns, one-sided c | 9.5 / 12.8 / 39.1 / 100.6 | 9.3 / 11.6 / 29.7 / 68.5 | 9.5 / 12.2 / 34.0 / 80.7 |

By distance band, text, data-only dual, bytes per item (0.01% answer / 1% answer):

| Variant | 1–2 sd | 2–4 sd | 4–8 sd |
|---|---|---|---|
| Design shape | 232 / 194 | 86.9 / 75.0 | 23.0 / 32.0 |
| Query-chosen, box | 176 / 147 | 58.2 / 52.5 | 17.9 / 23.1 |
| + randomised | 148 / 127 | 53.8 / 49.1 | 17.5 / 22.6 |
| + signs, one-sided c | 138 / 119 | 49.0 / 45.2 | 16.3 / 20.9 |

- **The order is the bulk lever.** It gives −25 / −25 / −19 / −11% on the data-only dual, and −33%
  at 2–4 sd.
- **The dither matters more near the threshold than in the bulk**: −7% at 2–4 sd, −16% at 1–2 sd,
  and −30% or more within 1 sd.
- **Nothing changes at zero read.** The order is irrelevant with nothing read, so the share undecided
  there is the same for every variant (text 0.01%: 73.5% PCA, 57.8% data-only dual, 51.6% text dual).
- **Storing only 192 coordinates as columns keeps most of the gain**: 6–12% worse than a free order.
- **Item queries at small answers lose** because every touched item pays 8 B of scalars against 3 B.
  If the block norms are read only where the single stored norm fails, the expected cost on the
  data-only dual is 4.8 / 7.7 / 27.6 / 68.3 B for items and 22.3 / 32.7 / 71.8 / 141.7 B for text
  (computed from the two undecided-at-zero columns, not run).
- **A cluster-wide stage costs about 9% more.** If every item of a cluster reads to the cluster's
  median need in one columnar stage, bytes rise 8–10% (text, text dual: 22.6 / 32.0 / 69.8 / 140.7).
  Reading to the 80th percentile costs 28–41% more.

**Against the floors.** Code bytes to halve the mean upper slack, text on the data-only dual: 56.6 B
fixed, 35.7 B query-chosen, 31.2 B with signs and dither. The conjectured non-adaptive floor on the
text spectrum is 18.4 B (a rough comparison: the basis is data-fitted, the floor's spectrum
text-fitted). So these cells sit about 3× above the conjectured floor in bytes, and the query's
order brings them to about 1.7×. None of the levers beats the conjectured floor.

**An earlier, cruder code agrees.** `qcol.py` stored the first 64 coordinates at 8 bits (PCA only,
24 text + 14 item queries, 0 violations). Text went from 54.8 / 77.1 / 138.2 / 247.7 to
41.7 / 58.3 / 104.3 / 186.5 with the query's order and the dither, and to 38.8 / 54.5 / 99.1 / 180.2
with signs. One stored error norm per block gave nothing after its 5 B.

### 4.3 Time: columnar kernel (`colscan3.c`, AVX-512, one thread, timed with no other load)

Synthetic data in the real layout: 1,000 clusters of 1,024 items, nibble columns, six block-norm
columns. Per cluster the kernel selects columns, builds tables, accumulates K1 columns, checks,
accumulates K2 − K1 more, and checks again. No compaction.

| Code read | Total, energy-accounting check | Total, norms-only check | Selection | Fetch | Accumulate | Two checks (energy / norms-only) |
|---|---|---|---|---|---|---|
| 12 B | 8.9 ns per item | 5.2 | 1.2 | 1.6 | 1.9 | 3.9 / 1.3 |
| 24 B | 11.7 | 7.6–8.0 | 1.3 | 2.9 | 3.4 | 3.9 / 1.5 |
| 32 B | 13.9 | 8.9 | 1.4 | 3.6 | 4.5 | 4.0 / 1.4 |

- With 4,000 clusters (1.5 GB of columns) the 24 B case costs 11.6 ns, so the figures hold out of
  cache.
- The vectorised selection takes 1.2–1.4 µs per cluster. The scalar quickselect took 8.7 µs and would
  have cost more than the scan.
- The native prototype (`rust/REPORT2.md`) spends 38.6 / 47.6 / 75.6 ms per 1M for text at
  18.6 / 29.0 / 64.8 B per item: 1.2–2.1 ns per byte against 0.4–0.5 here.
- This compares a kernel with an end-to-end prototype. An end-to-end gain of 2–4× on text at small
  answers is **conjectured**.

## 5. Comparison with the native dual-basis result (`dualproto/REPORT.md`)

- **Size of the gain agrees.** On these cells the design shape falls 10 / 19 / 24 / 29% for items with
  the data-only fit (native: 9–30%), and 36 / 34 / 28 / 24% for text with the text fit (native:
  25–34%).
- **Zero read disagrees.** In this harness the dual basis does tighten the zero-read bound: mean upper
  slack 7.10 → 6.00 → 5.50 sd for text (PCA, data-only dual, text dual), and items undecided at zero
  read 74.6% → 63.3% → 54.4% at 0.01%. The native report says the plain Cauchy–Schwarz slack barely
  moves (0.228 against 0.232). Conjectured explanation: the native level 0 bounds with a Euclidean
  norm, which the dual pair does not change. If so, storing ‖r‖ in the M⁻¹ norm at level 0 is an
  unclaimed gain there; `groups/REPORT.md` section 3.3 measured the same thing.
- **Both gains live in the later levels and they stack.** The query's order gives 15–18% (box bound)
  or 20–31% (with dither) on top of the text-fitted dual.

## 6. Ranking

1. **Columnar bulk pass with a query-chosen order.** The largest gain in both bytes and time.
   - Storage: none extra if the first ~128–192 coordinates of the stream are laid out as columns per
     cluster.
   - Complexity: a second layout, inserts that append to ~200 column tails per cluster, and block norms
     replacing per-level stored norms.
   - It needs the lazy block-norm read to avoid hurting item queries.
2. **Dithered cells.** Free in storage and time, since the half-width is one number per cluster and
   level. It changes the quantiser definition and puts ε on early levels. Worth most near the
   threshold.
3. **Sign columns.** 4–6% for a second kind of column and a per-block accumulator. Marginal.
4. **One-sided union bound.** About 1%. Do not do it.

## 7. Scaling (conjectured unless marked)

- **Dimension.**
  - The order gain depends on how much a query's offset departs from the group profile; flatter
    spectra should gain more (flat model: 1.4–4×).
  - Selection is O(n) per cluster: 1.2 µs at 768, about 8 µs at 5,000 unless the query orders only
    the stored columns.
  - The dither gain grows as √(cells read)/c, so there is none near n = 100 and more at 5,000.
- **N.**
  - Every lever is per item, so shares are unchanged at a fixed answer share.
  - At a fixed answer count the threshold moves out in sd and more of the bulk sits at 4–8 sd, where
    the order gain is 22–29% (measured at 1M).
  - At equal bytes the query's order lowers the mean upper slack by 0.7–1.0 sd between 8 and 32 B
    (measured). By the rule of `sublinear/REPORT.md` that is 2–3 times fewer survivors at those
    levels.

## 8. What did not work

- **One-sided bounds, tolerance, knowing t:** no escape (3.1, 3.5).
- **Randomised first certificate or a sketch of the unread part:** loses below about 60 B (3.3).
- **More block norms; the spectrum tail:** under 4% beyond four norms; the tail holds 0.8% of the
  slack.
- **Overcomplete dictionaries for query-chosen reads:** a log factor at best, for a multiple of the
  storage (3.2).
- **Box bound alone with a fixed order:** 3–10% worse than the stored weighted norm. The order must
  be query-chosen to pay for losing that norm.
- **A stored error norm per block:** no gain.
- **Scalar order selection:** costs more than the scan.

## 9. Limits of this evidence

- **Absolute bytes are not comparable with the native prototype.** These cells read 26–41 B at text
  0.01% against the native 18.6 B. Differences: 4-bit cells against an escape code, 8 B of scalars,
  no item skipping, 200k against 1M items, 29 against 569 queries. Only ratios inside one harness
  carry.
- **Small query sets** for the byte runs (19 text + 10 item per basis).
- **Sign columns are costed as sign-and-magnitude cells.** The dithered cell would need T lowered
  from 6.5 to 5.5 steps to fit; not simulated.
- **The kernel** uses synthetic data, has no compaction, and its second accumulation stands in for
  the energy tables.
- **The randomised bound** was checked empirically only to 5.5 proxy sd; its validity rests on the
  proof.
- **One dataset, one dimension.** The proofs in section 3 are unrefereed.

## Files (this directory)

- `tables_qcol2.txt` — all byte tables, bands and stage statistics; start here
- `qcol2.py`, `qcol2_pca.log`, `qcol2_sq_item.log`, `qcol2_sq_text.log` (with `.json`) — the byte
  experiment
- `adapt.py`, `adapt_pca.log`, `adapt_sq_item.log` — exact coordinates (from the lost agent)
- `colscan3.c`, `colscan3.log`, `colscan2.c`, `colscan2.log`, `colscan.c`, `colscan.log` — kernel
  timings (binaries not committed; build with gcc -O3 -march=native)
- `floor.py`, `floor_sq_text.log`, `floor_sq_item.log` — floors on the measured spectra
- `slack.py`, `slack_pca.log`, `slack_sq_item.log`, `model.py`, `model.log` — slack and concentration
  diagnostics
- `qcol.py`, `qcol_pca.log` — the earlier 8-bit-head variant
- `common.py` — the shared setup; `tab.py` — prints the tables from the result files
- `quant.py` — from the lost agent, not run
