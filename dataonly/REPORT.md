# Cost of a data-only build

Saved by the coordinating session from the agent's returned text; condensed. Every number is the
agent's. Scripts (`run3.py`, `go.sh`, `report3.py`, `profiles.py`, run chains) and logs in this
directory; outputs were left in the scratch area. The 1M cost attribution was stopped by request.

## Summary

- **BioCLIP-2 text queries, data-only (`D0`, corpus items as pseudo-queries, no text anchors):**
  +17.6 / +12.9 / +7.4 / +4.9% bytes per item at 1M (answers 0.01 / 0.1 / 1 / 5%); +17.4 / +13.6 /
  +9.4 / +6.6% at 200k. Item queries −5 to +2%. (measured)
- **ada-002, 3-large, GloVe, BioCLIP-2.5: zero cost.** Those builds never used text (mixture
  profile equals the item profile; no text anchors). `D0` reruns were bit-identical. (measured)
- **Where the BioCLIP text cost comes from** (200k, one switch at a time): deterministic-level
  profile +7.7 / 5.8 / 3.5 / 1.7%; randomised-level profile +3.4 / 3.7 / 4.2 / 4.5%; text anchors
  +5.1 / 3.3 / 1.4 / 0.4%. Near-additive. (measured)
- **The anchor part cannot be recovered from data with single-anchor tables:** no data-derived
  anchor set brought text queries closer than the cluster centres (mean cosine 0.75 vs 0.85 with
  text anchors). (cosines measured; unrecoverability conjectured)
- **Recommended profile `H25`:** Q = (0.75·√Qi + 0.25·√iso)², Qi the item profile, iso isotropic with
  Qi's trace, at both deterministic and randomised levels; no text anchors. Against the mixture-tuned
  build at 1M: text +8.3 / +5.6 / +2.5 / +1.1%, items −1.4 / −0.7 / −0.7 / −0.9%. On in-domain
  datasets it costs items −0.7 to +2.3%. Halves the text cost of `D0`. (measured) Modelled
  Cauchy–Schwarz width loss for text falls from 1.14–1.19× to 1.06–1.08×. (modelled)
- **Zero violations** in every run; largest randomised error 5.0–6.1 sd vs c ≈ 9.47.

## Where the query sample entered the build

1. `PROF=Qmix`: deterministic certificate weights, bit allocation, escape step order.
2. The randomised levels always used the mixture (whitening, block membership, rate design, α);
   `RPROF` switch added.
3. Text anchors: 256 k-means centres of `text_train` in the support table; `ANCH=items` added.
4. The mixture profile itself (`combined/build.py`, `quant/build.py`).
5. kNN code reads the same `Qmix` and anchors (not re-measured).
6. Calibration does not use text.
7. Schedule constants were chosen looking at text results; kept as fixed constants.
8. Test thresholds and query sets are evaluation only.

## Results (bytes per item; change vs mixture-tuned build)

**BioCLIP-2 1M, text (369 queries):**

| Build | B/item | Change | Touched % |
|---|---|---|---|
| Mixture | 22.79 / 32.21 / 57.66 / 99.28 | – | 64.1 / 74.2 / 90.3 / 99.1 |
| `D0` | 26.79 / 36.35 / 61.91 / 104.19 | +17.6 / +12.9 / +7.4 / +4.9% | 74.8 / 82.7 / 94.2 / 99.6 |
| `H25` | 24.69 / 34.02 / 59.07 / 100.38 | +8.3 / +5.6 / +2.5 / +1.1% | 74.8 / 82.7 / 94.2 / 99.6 |

**BioCLIP-2 1M, items (200 queries):** mixture 2.12 / 5.21 / 18.69 / 58.46; `D0` +1.0 / +1.7 /
+0.8 / −0.3%; `H25` −1.4 / −0.7 / −0.7 / −0.9%.

Totals at N = 10⁹ (% of dataset): text mixture 1.549 / 2.163 / 3.822 / 6.543, `D0` 1.810 / 2.433 /
4.099 / 6.862, `H25` 1.673 / 2.281 / 3.914 / 6.614; items `H25` 0.182 / 0.387 / 1.265 / 3.850.

**BioCLIP-2 200k attribution (90 text, 60 item queries):**

| Build | Text change | Items change |
|---|---|---|
| Deterministic profile only (`dP`) | +7.7 / +5.8 / +3.5 / +1.7% | −4.9 / −3.2 / −0.5 / +0.2% |
| Randomised profile only (`dR`) | +3.4 / +3.7 / +4.2 / +4.5% | −0.6 / −1.1 / −1.3 / −1.3% |
| Anchors only (`dA`) | +5.1 / +3.3 / +1.4 / +0.4% | 0 |
| Both profiles, text anchors kept (`dPR`) | +11.3 / +9.6 / +7.8 / +6.2% | −5.4 / −4.3 / −2.0 / −1.2% |
| `D0` | +17.4 / +13.6 / +9.4 / +6.6% | −5.4 / −4.3 / −2.0 / −1.2% |
| `H25` | +8.9 / +6.5 / +3.7 / +1.9% | −4.7 / −3.8 / −2.1 / −1.3% |

Anchors change which items are touched (text touched 60.7 / 68.1 / 79.1 / 89.9% → 72.5 / 78.3 /
86.0 / 93.6%); profiles widen bounds.

**Other datasets** (`D0` = current; change for `H25`, then `H50`):

| Dataset | Current B/item | `H25` | `H50` |
|---|---|---|---|
| ada-002, 1,536 | 56.26 / 81.93 / 134.0 / 250.8 | +0.9 / +1.1 / +2.3 / +2.3% | +2.6 / +2.1 / +3.9 / +4.1% |
| 3-large, 3,072 | 101.3 / 141.4 / 294.7 / 763.5 | +0.1 / +0.8 / +1.6 / +1.6% | +2.2 / +3.7 / +5.2 / +5.8% |
| GloVe, 100 | 18.07 / 25.16 / 37.88 / 56.99 | −0.7 / −0.6 / −0.5 / −0.4% | −0.7 / −0.7 / −0.5 / −0.4% |
| BioCLIP-2.5, 1,024 | 1.89 / 3.74 / 10.00 / 23.27 | +0.2 / +0.3 / +0.7 / +1.0% | +5.2 / +5.5 / +7.2 / +7.9% |

## Choosing the hedge (200k, change vs `D0`)

| Profile | Text | Items |
|---|---|---|
| `H12` (w = 0.125) | −4.4 / −3.8 / −3.5 / −3.4% | −0.2 / −0.4 / −0.6 / −0.6% |
| `H25` (w = 0.25) | −7.3 / −6.2 / −5.2 / −4.4% | +0.8 / +0.5 / −0.1 / −0.1% |
| `H37` (w = 0.375) | −8.3 / −6.8 / −5.4 / −4.4% | +3.1 / +2.8 / +1.9 / +1.7% |
| `H50` (w = 0.5) | −9.2 / −7.7 / −6.2 / −5.0% | +6.4 / +5.6 / +3.8 / +3.0% |
| `HS` (0.25 det, 0.5 rand) | −8.2 / −7.3 / −6.3 / −5.3% | +1.4 / +1.8 / +1.9 / +2.1% |

`H25` is the knee. Its weight was chosen on BioCLIP-2 text, the only out-of-domain set available;
not independent evidence that 0.25 suits other out-of-domain pairs (conjectured).

## Not done

1M attribution (stopped by request); schedule constants left as tuned; kNN path not re-measured;
data-derived multi-anchor or subspace bounds not built; learning the profile from the live query
log at compaction.
