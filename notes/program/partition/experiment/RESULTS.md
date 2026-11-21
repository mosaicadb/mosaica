# Partition experiment: results

Experimentalist's results for `../EXPERIMENT-SPEC.md`, in the spec's order. The log of what was run is `NOTEBOOK.md`.
Every number here is **[measured]** on the 200k BioCLIP-2 sample (`vexact-data/bioclip2-200k`, 200,000 rows,
n = 768), unless marked otherwise, with these common conditions:

- Split: permutation seed 1; 149,922 fit rows, 49,978 held-out rows; the 100 item-query rows are in neither and in no
  count. Coding scores (S1) are means over the **49,978 held-out rows**, with centres and bases fitted on fit rows.
  S2, S3, S2+3, S4 and S5 are over **all 199,900 valid rows** (not held out: those bounds must hold for every row),
  for the **830 queries** of `setup.npz` (369 held-out text prompts, 361 in-sample species prompts, 100 held-out item
  queries), at answers of 0.01, 0.1 and 1%.
- Σ_Q held fixed: A = (Si + 0.1·tr(Si)/n·I)^½, and again with Sm for the coding scores. K = 200, G = 12.
- The level model is the floor's idealisation (a coded coordinate's error is sᵢ times the coordinate, water-filling
  per group), **not the built code's bytes**. Ratios between partitions are what these scores are for.
- Violations: every row decided by the cut or by the range, for every query and answer size, was checked against
  its exact score (slack 1e-5). **The count is 0 for every partition in this file.**
- The seed spread is the largest gap among A1, A2, A3 (build seeds 1, 2, 3). A difference smaller than twice it is
  not a result, and is said so.

Scripts: `lib.py` (all scores), `item12.py`, `item3.py`, `itemB.py`, `itemC.py`, `itemD.py`, `sweep.py`,
`report.py` (tables), `diag_groups.py`; logs and score files in `work/` (ignored by git).

## Item 1. How far the built partition is from a fixed point (P1.6)

`item12.py lloyd`, log `work/item12_lloyd.log`. A1 (build seed 1) continued by Lloyd on all fit rows: every row to
the stored centre (fit-row mean) of largest inner product, centres to the means, groups re-made by the build's rule
(fresh generator, seed 1) at each reported pass. It reached under 0.1% of fit rows moved at pass 53 (A1-fp); no
cell was emptied. S0 for the other partitions: A0 3.32%, A2 4.39%, A3 4.18% (all valid rows).

| partition | pass | fit rows moved in the pass | S0 (all valid rows) | mean key | mean sq. residual | S1d 16 B (Si) | S1b w*=.06 (Si) | S2+3 item .01% | S2+3 item 1% | S2+3 species 1% | S4a text | S4a item | smallest group (fit rows) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| A1 | 0 | 0.0000 | 0.0441 | 0.91826 | 0.15472 | 0.09635 | 37.366 | 0.4291 | 0.6530 | 0.8065 | 0.5810 | 0.8738 | 617 |
| A1-lloyd1 | 1 | 0.0433 | 0.0319 | 0.91842 | 0.15442 | 0.09568 | 36.703 | 0.4280 | 0.6520 | 0.8061 | 0.5821 | 0.8743 | 3090 |
| A1-lloyd2 | 2 | 0.0315 | 0.0274 | 0.91856 | 0.15418 | 0.09521 | 36.425 | 0.4275 | 0.6515 | 0.8059 | 0.5829 | 0.8744 | 4285 |
| A1-lloyd5 | 5 | 0.0200 | 0.0174 | 0.91894 | 0.15351 | 0.09518 | 36.435 | 0.4261 | 0.6506 | 0.8056 | 0.5844 | 0.8751 | 5272 |
| A1-lloyd10 | 10 | 0.0103 | 0.0095 | 0.91911 | 0.15321 | 0.09541 | 36.594 | 0.4250 | 0.6498 | 0.8054 | 0.5844 | 0.8752 | 1265 |
| A1-lloyd20 | 20 | 0.0042 | 0.0039 | 0.91918 | 0.15308 | 0.09536 | 36.572 | 0.4246 | 0.6496 | 0.8052 | 0.5847 | 0.8753 | 1252 |
| A1-fp | 53 | 0.0009 | 0.0010 | 0.91929 | 0.15287 | 0.09547 | 36.639 | 0.4235 | 0.6488 | 0.8049 | 0.5855 | 0.8756 | 1274 |
| seed spread (A1–A3) | | | 0.0023 | 0.00053 | 0.00088 | 0.00139 | 1.082 | 0.0036 | 0.0026 | 0.0049 | 0.0015 | 0.0019 | |

A1 against its fixed point:

| score | A1 | A1-fp | difference | relative | 2 × seed spread | a result? |
|---|---|---|---|---|---|---|
| S0_all | 0.04412 | 0.00099 | -0.04314 | -97.77% | 0.00461 | yes |
| mean_key | 0.91826 | 0.91929 | +0.00104 | +0.11% | 0.00106 | no: under twice the seed spread |
| msq_resid | 0.15472 | 0.15287 | -0.00185 | -1.20% | 0.00177 | yes |
| Si_S1d_16 | 0.09635 | 0.09547 | -0.00088 | -0.91% | 0.00278 | no: under twice the seed spread |
| Si_S1b_0.06 | 37.36593 | 36.63907 | -0.72686 | -1.95% | 2.16459 | no: under twice the seed spread |
| Sm_S1d_16 | 0.14745 | 0.14595 | -0.00150 | -1.02% | 0.00459 | no: under twice the seed spread |
| Sm_S1b_0.06 | 66.04955 | 64.87390 | -1.17565 | -1.78% | 3.30709 | no: under twice the seed spread |
| Si_S1a_mean | 0.20834 | 0.20636 | -0.00198 | -0.95% | 0.00342 | no: under twice the seed spread |
| S23_item_0.01% | 0.42909 | 0.42348 | -0.00561 | -1.31% | 0.00712 | no: under twice the seed spread |
| S23_item_0.1% | 0.51600 | 0.51063 | -0.00537 | -1.04% | 0.00748 | no: under twice the seed spread |
| S23_item_1% | 0.65303 | 0.64883 | -0.00420 | -0.64% | 0.00512 | no: under twice the seed spread |
| S23_species_0.01% | 0.67384 | 0.67081 | -0.00303 | -0.45% | 0.00958 | no: under twice the seed spread |
| S23_species_1% | 0.80651 | 0.80494 | -0.00157 | -0.20% | 0.00975 | no: under twice the seed spread |
| S23_text_0.01% | 0.97662 | 0.97619 | -0.00043 | -0.04% | 0.00127 | no: under twice the seed spread |
| S4a_mean_text | 0.58102 | 0.58551 | +0.00450 | +0.77% | 0.00310 | yes |
| S4a_mean_item | 0.87383 | 0.87565 | +0.00182 | +0.21% | 0.00379 | no: under twice the seed spread |
| S4b_item | 0.03621 | 0.03071 | -0.00550 | -15.20% | 0.01190 | no: under twice the seed spread |
| S4b_nn_owncell | 0.68065 | 0.69765 | +0.01700 | +2.50% | 0.04230 | no: under twice the seed spread |

**Baseline rule.** S0 for A1 is 4.41%, above 2%, so by the spec's rule **the baseline is A1-fp**. (The rule's
second clause, S2+3 and S1b at A1-fp within twice the seed spread of A1's, holds.) Every later item starts from A1-fp.

**Reading.** 4.4% of rows of the built partition do not have their largest key at their own stored centre (the
referee's toy had 11.7%), and 53 further Lloyd passes raise the mean key by 0.11% (toy: 4.7%) and lower the mean
square residual by 1.2%; only S0, the mean square residual and S4a for text queries move by more than twice the seed
spread. What finishing the fit is worth to the coding and the decisions (S1d, S1b, S2+3) is under twice the seed
spread, so the remark of P1.6 has little practical weight on this corpus at K = 200.

Notes. The groups are re-made at every reported pass, so the coding columns mix the cells' change with a new
grouping. A1's own grouping has a group of 617 fit rows (fewer than n), and A1-fp's smallest group has 1,274; see
item 2.

Full tables by pass (all scores):


Coding, Σ_Q = Si, held-out rows (mean over 49,978 rows)

| partition | S1d 16B | S1b w*=.06 (B) | S1d 4B | S1d 8B | S1d 32B | S1b .03 | S1b .12 | S1a mean (sec.) | S1a rms (sec.) |
|---|---|---|---|---|---|---|---|---|---|
| A1 | 0.09635 | 37.366 | 0.14828 | 0.12432 | 0.06697 | 83.104 | 10.741 | 0.20834 | 0.22271 |
| A1-lloyd1 | 0.09568 | 36.703 | 0.14736 | 0.12351 | 0.06643 | 81.894 | 10.487 | 0.20717 | 0.22125 |
| A1-lloyd2 | 0.09521 | 36.425 | 0.14675 | 0.12296 | 0.06609 | 81.525 | 10.340 | 0.20653 | 0.22052 |
| A1-lloyd5 | 0.09518 | 36.435 | 0.14663 | 0.12289 | 0.06608 | 81.495 | 10.356 | 0.20623 | 0.22022 |
| A1-lloyd10 | 0.09541 | 36.594 | 0.14688 | 0.12314 | 0.06625 | 81.778 | 10.402 | 0.20642 | 0.22034 |
| A1-lloyd20 | 0.09536 | 36.572 | 0.14682 | 0.12308 | 0.06622 | 81.745 | 10.390 | 0.20634 | 0.22027 |
| A1-fp | 0.09547 | 36.639 | 0.14690 | 0.12318 | 0.06631 | 81.836 | 10.420 | 0.20636 | 0.22035 |
| seed spread (A1-A3, max−min) | 0.00139 | 1.082 | 0.00177 | 0.00162 | 0.00111 | 1.791 | 0.46924 | 0.00171 | 0.00225 |

Coding, Σ_Q = Sm, held-out rows (mean over 49,978 rows)

| partition | S1d 16B | S1b w*=.06 (B) | S1d 4B | S1d 8B | S1d 32B | S1b .03 | S1b .12 | S1a mean (sec.) | S1a rms (sec.) |
|---|---|---|---|---|---|---|---|---|---|
| A1 | 0.14745 | 66.050 | 0.21704 | 0.18572 | 0.10510 | 124.513 | 25.638 | 0.28938 | 0.30985 |
| A1-lloyd1 | 0.14626 | 64.932 | 0.21545 | 0.18430 | 0.10412 | 122.821 | 25.086 | 0.28743 | 0.30708 |
| A1-lloyd2 | 0.14555 | 64.584 | 0.21458 | 0.18349 | 0.10359 | 122.414 | 24.832 | 0.28655 | 0.30607 |
| A1-lloyd5 | 0.14551 | 64.569 | 0.21442 | 0.18339 | 0.10358 | 122.344 | 24.858 | 0.28614 | 0.30567 |
| A1-lloyd10 | 0.14585 | 64.819 | 0.21480 | 0.18377 | 0.10385 | 122.711 | 24.982 | 0.28642 | 0.30585 |
| A1-lloyd20 | 0.14578 | 64.789 | 0.21470 | 0.18368 | 0.10380 | 122.672 | 24.963 | 0.28632 | 0.30576 |
| A1-fp | 0.14595 | 64.874 | 0.21483 | 0.18383 | 0.10395 | 122.774 | 25.021 | 0.28638 | 0.30591 |
| seed spread (A1-A3, max−min) | 0.00230 | 1.654 | 0.00285 | 0.00264 | 0.00186 | 2.363 | 0.92272 | 0.00285 | 0.00402 |

S2+3 (share of valid rows left by both cut and range), mean over queries

| partition | text 0.01% | text 0.1% | text 1% | species 0.01% | species 0.1% | species 1% | item 0.01% | item 0.1% | item 1% |
|---|---|---|---|---|---|---|---|---|---|
| A1 | 0.9766 | 0.9936 | 0.9993 | 0.6738 | 0.7299 | 0.8065 | 0.4291 | 0.5160 | 0.6530 |
| A1-lloyd1 | 0.9765 | 0.9936 | 0.9993 | 0.6732 | 0.7293 | 0.8061 | 0.4280 | 0.5149 | 0.6520 |
| A1-lloyd2 | 0.9763 | 0.9935 | 0.9993 | 0.6729 | 0.7291 | 0.8059 | 0.4275 | 0.5144 | 0.6515 |
| A1-lloyd5 | 0.9762 | 0.9935 | 0.9993 | 0.6720 | 0.7284 | 0.8056 | 0.4261 | 0.5133 | 0.6506 |
| A1-lloyd10 | 0.9762 | 0.9935 | 0.9993 | 0.6716 | 0.7280 | 0.8054 | 0.4250 | 0.5117 | 0.6498 |
| A1-lloyd20 | 0.9761 | 0.9935 | 0.9993 | 0.6712 | 0.7276 | 0.8052 | 0.4246 | 0.5116 | 0.6496 |
| A1-fp | 0.9762 | 0.9935 | 0.9993 | 0.6708 | 0.7272 | 0.8049 | 0.4235 | 0.5106 | 0.6488 |
| seed spread (A1-A3, max−min) | 0.0006 | 0.0004 | 0.0001 | 0.0048 | 0.0053 | 0.0049 | 0.0036 | 0.0037 | 0.0026 |

S2 (cut alone)

| partition | text 0.01% | text 0.1% | text 1% | species 0.01% | species 0.1% | species 1% | item 0.01% | item 0.1% | item 1% |
|---|---|---|---|---|---|---|---|---|---|
| A1 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.8377 | 0.8491 | 0.8673 |
| A1-lloyd1 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.8368 | 0.8483 | 0.8663 |
| A1-lloyd2 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.8375 | 0.8485 | 0.8663 |
| A1-lloyd5 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.8374 | 0.8486 | 0.8661 |
| A1-lloyd10 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.8370 | 0.8478 | 0.8660 |
| A1-lloyd20 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.8383 | 0.8484 | 0.8667 |
| A1-fp | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.8370 | 0.8475 | 0.8668 |
| seed spread (A1-A3, max−min) | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.0056 | 0.0052 | 0.0032 |

S3 (sorted range alone)

| partition | text 0.01% | text 0.1% | text 1% | species 0.01% | species 0.1% | species 1% | item 0.01% | item 0.1% | item 1% |
|---|---|---|---|---|---|---|---|---|---|
| A1 | 0.9766 | 0.9936 | 0.9993 | 0.6738 | 0.7299 | 0.8065 | 0.4422 | 0.5323 | 0.6733 |
| A1-lloyd1 | 0.9765 | 0.9936 | 0.9993 | 0.6732 | 0.7293 | 0.8061 | 0.4412 | 0.5313 | 0.6724 |
| A1-lloyd2 | 0.9763 | 0.9935 | 0.9993 | 0.6729 | 0.7291 | 0.8059 | 0.4404 | 0.5306 | 0.6718 |
| A1-lloyd5 | 0.9762 | 0.9935 | 0.9993 | 0.6720 | 0.7284 | 0.8056 | 0.4379 | 0.5283 | 0.6700 |
| A1-lloyd10 | 0.9762 | 0.9935 | 0.9993 | 0.6716 | 0.7280 | 0.8054 | 0.4367 | 0.5273 | 0.6693 |
| A1-lloyd20 | 0.9761 | 0.9935 | 0.9993 | 0.6712 | 0.7276 | 0.8052 | 0.4362 | 0.5267 | 0.6689 |
| A1-fp | 0.9762 | 0.9935 | 0.9993 | 0.6708 | 0.7272 | 0.8049 | 0.4354 | 0.5259 | 0.6682 |
| seed spread (A1-A3, max−min) | 0.0006 | 0.0004 | 0.0001 | 0.0048 | 0.0053 | 0.0049 | 0.0038 | 0.0030 | 0.0021 |

S2 at 1%: mean 2ρ (net) and mean β(E⁺−E⁻) (extent) over cells left

| partition | 2ρ text | 2ρ species | 2ρ item | extent text | extent species | extent item |
|---|---|---|---|---|---|---|
| A1 | 1.4889 | 1.1639 | 0.7207 | 0.0876 | 0.1167 | 0.1632 |
| A1-lloyd1 | 1.4872 | 1.1627 | 0.7203 | 0.0867 | 0.1168 | 0.1618 |
| A1-lloyd2 | 1.4854 | 1.1614 | 0.7206 | 0.0859 | 0.1169 | 0.1613 |
| A1-lloyd5 | 1.4839 | 1.1590 | 0.7189 | 0.0841 | 0.1173 | 0.1610 |
| A1-lloyd10 | 1.4897 | 1.1574 | 0.7200 | 0.0852 | 0.1171 | 0.1610 |
| A1-lloyd20 | 1.4897 | 1.1568 | 0.7197 | 0.0856 | 0.1172 | 0.1613 |
| A1-fp | 1.4893 | 1.1581 | 0.7191 | 0.0851 | 0.1171 | 0.1607 |
| seed spread (A1-A3, max−min) | 0.0023 | 0.0113 | 0.0031 | 0.0033 | 0.0034 | 0.0017 |

Order: S4a (mean share; ratio of means), S4b (share of rows visited to cover 9 of the 10 best), own-cell share of 10-NN

| partition | S4a text | S4a species | S4a item | S4a ratio text | S4a ratio species | S4a ratio item | S4b text | S4b species | S4b item | 10-NN own cell |
|---|---|---|---|---|---|---|---|---|---|---|
| A1 | 0.5810 | 0.8584 | 0.8738 | 0.6170 | 0.8762 | 0.9217 | 0.0425 | 0.0210 | 0.0362 | 0.6806 |
| A1-lloyd1 | 0.5821 | 0.8588 | 0.8743 | 0.6180 | 0.8764 | 0.9222 | 0.0389 | 0.0210 | 0.0364 | 0.6828 |
| A1-lloyd2 | 0.5829 | 0.8590 | 0.8744 | 0.6187 | 0.8766 | 0.9224 | 0.0364 | 0.0218 | 0.0358 | 0.6842 |
| A1-lloyd5 | 0.5844 | 0.8600 | 0.8751 | 0.6201 | 0.8775 | 0.9227 | 0.0385 | 0.0214 | 0.0351 | 0.6894 |
| A1-lloyd10 | 0.5844 | 0.8600 | 0.8752 | 0.6203 | 0.8774 | 0.9227 | 0.0372 | 0.0205 | 0.0336 | 0.6946 |
| A1-lloyd20 | 0.5847 | 0.8600 | 0.8753 | 0.6205 | 0.8774 | 0.9226 | 0.0386 | 0.0203 | 0.0327 | 0.6977 |
| A1-fp | 0.5855 | 0.8605 | 0.8756 | 0.6210 | 0.8778 | 0.9227 | 0.0391 | 0.0213 | 0.0307 | 0.6976 |
| seed spread (A1-A3, max−min) | 0.0015 | 0.0014 | 0.0019 | 0.0011 | 0.0009 | 0.0010 | 0.0055 | 0.0014 | 0.0060 | 0.0212 |

Layout and shape (all valid rows)

| partition | mean key | msq resid | n_c min | median | max | CV | rows in cells <500 | cells/family | cells/genus | cells/species | cells/species (row-wtd) | max/rms radius (median cell) | cells with max/rms>2 | key p1/median (median cell) | S1e B/row | viol S2 | viol S3 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| A1 | 0.9183 | 0.1547 | 134 | 885.0000 | 3658 | 0.6283 | 0.0852 | 104.3333 | 20.4889 | 6.1082 | 44.3004 | 1.5593 | 0.1550 | 0.9026 | 36.1574 | 0 | 0 |
| A1-lloyd1 | 0.9184 | 0.1544 | 133 | 878.5000 | 3691 | 0.6233 | 0.0845 | 103.3333 | 20.2222 | 6.0475 | 43.4688 | 1.5496 | 0.1400 | 0.9029 | 36.1574 | 0 | 0 |
| A1-lloyd2 | 0.9186 | 0.1542 | 134 | 874.5000 | 3702 | 0.6193 | 0.0828 | 103.0000 | 20.1333 | 6.0061 | 42.8463 | 1.5452 | 0.1550 | 0.9030 | 36.1574 | 0 | 0 |
| A1-lloyd5 | 0.9189 | 0.1535 | 134 | 860.0000 | 3730 | 0.6178 | 0.0764 | 104.0000 | 20.2222 | 5.9676 | 42.8465 | 1.5524 | 0.1400 | 0.9034 | 36.1574 | 0 | 0 |
| A1-lloyd10 | 0.9191 | 0.1532 | 134 | 857.5000 | 3743 | 0.6177 | 0.0840 | 102.3333 | 19.9111 | 5.9201 | 42.7981 | 1.5544 | 0.1300 | 0.9016 | 36.1574 | 0 | 0 |
| A1-lloyd20 | 0.9192 | 0.1531 | 134 | 841.0000 | 3742 | 0.6172 | 0.0815 | 103.3333 | 19.8444 | 5.8342 | 42.3578 | 1.5483 | 0.1400 | 0.9012 | 36.1574 | 0 | 0 |
| A1-fp | 0.9193 | 0.1529 | 134 | 833.0000 | 3742 | 0.6242 | 0.0778 | 102.3333 | 20.1111 | 5.8584 | 42.5876 | 1.5441 | 0.1350 | 0.8997 | 36.1574 | 0 | 0 |
| seed spread (A1-A3, max−min) | 0.0005 | 0.0009 | 133 | 31.5000 | 1740 | 0.0415 | 0.0057 | 3.3333 | 0.3111 | 0.0607 | 3.5218 | 0.0367 | 0.0150 | 0.0052 | 0.0000 | 0 | 0 |

## Item 2. The noise between build seeds

`item12.py seeds`, log `work/item12_seeds.log`. A1, A2, A3: the build's steps at seeds 1, 2, 3 (12 passes of
`core.py::kmeans` on 40,000 fit rows, every row to the sample centroid of largest inner product, stored centre the
fit-row mean, groups by `kmeans` on the unit centre directions, 20 passes). A0 is `setup.npz`'s partition (its cells
were fitted on all rows, so it is not held out at the cell level), with centres recomputed on fit rows.


Coding, Σ_Q = Si, held-out rows (mean over 49,978 rows)

| partition | S1d 16B | S1b w*=.06 (B) | S1d 4B | S1d 8B | S1d 32B | S1b .03 | S1b .12 | S1a mean (sec.) | S1a rms (sec.) |
|---|---|---|---|---|---|---|---|---|---|
| A1 | 0.09635 | 37.366 | 0.14828 | 0.12432 | 0.06697 | 83.104 | 10.741 | 0.20834 | 0.22271 |
| A2 | 0.09596 | 36.868 | 0.14772 | 0.12387 | 0.06661 | 82.208 | 10.515 | 0.20744 | 0.22124 |
| A3 | 0.09496 | 36.284 | 0.14651 | 0.12271 | 0.06586 | 81.313 | 10.271 | 0.20663 | 0.22046 |
| A0 | 0.09557 | 36.635 | 0.14717 | 0.12338 | 0.06635 | 82.052 | 10.338 | 0.20699 | 0.22052 |
| seed spread (A1-A3, max−min) | 0.00139 | 1.082 | 0.00177 | 0.00162 | 0.00111 | 1.791 | 0.46924 | 0.00171 | 0.00225 |

Coding, Σ_Q = Sm, held-out rows (mean over 49,978 rows)

| partition | S1d 16B | S1b w*=.06 (B) | S1d 4B | S1d 8B | S1d 32B | S1b .03 | S1b .12 | S1a mean (sec.) | S1a rms (sec.) |
|---|---|---|---|---|---|---|---|---|---|
| A1 | 0.14745 | 66.050 | 0.21704 | 0.18572 | 0.10510 | 124.513 | 25.638 | 0.28938 | 0.30985 |
| A2 | 0.14666 | 65.206 | 0.21597 | 0.18480 | 0.10440 | 123.211 | 25.192 | 0.28782 | 0.30708 |
| A3 | 0.14515 | 64.396 | 0.21419 | 0.18308 | 0.10324 | 122.150 | 24.715 | 0.28653 | 0.30583 |
| A0 | 0.14609 | 65.017 | 0.21520 | 0.18411 | 0.10399 | 123.173 | 24.936 | 0.28719 | 0.30607 |
| seed spread (A1-A3, max−min) | 0.00230 | 1.654 | 0.00285 | 0.00264 | 0.00186 | 2.363 | 0.92272 | 0.00285 | 0.00402 |

S2+3 (share of valid rows left by both cut and range), mean over queries

| partition | text 0.01% | text 0.1% | text 1% | species 0.01% | species 0.1% | species 1% | item 0.01% | item 0.1% | item 1% |
|---|---|---|---|---|---|---|---|---|---|
| A1 | 0.9766 | 0.9936 | 0.9993 | 0.6738 | 0.7299 | 0.8065 | 0.4291 | 0.5160 | 0.6530 |
| A2 | 0.9773 | 0.9940 | 0.9994 | 0.6769 | 0.7343 | 0.8100 | 0.4273 | 0.5157 | 0.6538 |
| A3 | 0.9770 | 0.9938 | 0.9994 | 0.6786 | 0.7352 | 0.8114 | 0.4309 | 0.5194 | 0.6556 |
| A0 | 0.9759 | 0.9934 | 0.9993 | 0.6777 | 0.7351 | 0.8107 | 0.4287 | 0.5172 | 0.6545 |
| seed spread (A1-A3, max−min) | 0.0006 | 0.0004 | 0.0001 | 0.0048 | 0.0053 | 0.0049 | 0.0036 | 0.0037 | 0.0026 |

S2 (cut alone)

| partition | text 0.01% | text 0.1% | text 1% | species 0.01% | species 0.1% | species 1% | item 0.01% | item 0.1% | item 1% |
|---|---|---|---|---|---|---|---|---|---|
| A1 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.8377 | 0.8491 | 0.8673 |
| A2 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.8398 | 0.8508 | 0.8679 |
| A3 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.8433 | 0.8544 | 0.8705 |
| A0 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.8459 | 0.8571 | 0.8726 |
| seed spread (A1-A3, max−min) | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.0056 | 0.0052 | 0.0032 |

S3 (sorted range alone)

| partition | text 0.01% | text 0.1% | text 1% | species 0.01% | species 0.1% | species 1% | item 0.01% | item 0.1% | item 1% |
|---|---|---|---|---|---|---|---|---|---|
| A1 | 0.9766 | 0.9936 | 0.9993 | 0.6738 | 0.7299 | 0.8065 | 0.4422 | 0.5323 | 0.6733 |
| A2 | 0.9773 | 0.9940 | 0.9994 | 0.6769 | 0.7343 | 0.8100 | 0.4385 | 0.5304 | 0.6726 |
| A3 | 0.9770 | 0.9938 | 0.9994 | 0.6786 | 0.7352 | 0.8114 | 0.4416 | 0.5334 | 0.6747 |
| A0 | 0.9759 | 0.9934 | 0.9993 | 0.6777 | 0.7351 | 0.8107 | 0.4389 | 0.5303 | 0.6717 |
| seed spread (A1-A3, max−min) | 0.0006 | 0.0004 | 0.0001 | 0.0048 | 0.0053 | 0.0049 | 0.0038 | 0.0030 | 0.0021 |

S2 at 1%: mean 2ρ (net) and mean β(E⁺−E⁻) (extent) over cells left

| partition | 2ρ text | 2ρ species | 2ρ item | extent text | extent species | extent item |
|---|---|---|---|---|---|---|
| A1 | 1.4889 | 1.1639 | 0.7207 | 0.0876 | 0.1167 | 0.1632 |
| A2 | 1.4866 | 1.1572 | 0.7228 | 0.0857 | 0.1179 | 0.1649 |
| A3 | 1.4888 | 1.1526 | 0.7238 | 0.0889 | 0.1201 | 0.1646 |
| A0 | 1.4956 | 1.1559 | 0.7257 | 0.0869 | 0.1188 | 0.1627 |
| seed spread (A1-A3, max−min) | 0.0023 | 0.0113 | 0.0031 | 0.0033 | 0.0034 | 0.0017 |

Order: S4a (mean share; ratio of means), S4b (share of rows visited to cover 9 of the 10 best), own-cell share of 10-NN

| partition | S4a text | S4a species | S4a item | S4a ratio text | S4a ratio species | S4a ratio item | S4b text | S4b species | S4b item | 10-NN own cell |
|---|---|---|---|---|---|---|---|---|---|---|
| A1 | 0.5810 | 0.8584 | 0.8738 | 0.6170 | 0.8762 | 0.9217 | 0.0425 | 0.0210 | 0.0362 | 0.6806 |
| A2 | 0.5808 | 0.8595 | 0.8757 | 0.6169 | 0.8767 | 0.9224 | 0.0390 | 0.0210 | 0.0422 | 0.6857 |
| A3 | 0.5824 | 0.8598 | 0.8746 | 0.6180 | 0.8770 | 0.9214 | 0.0370 | 0.0197 | 0.0378 | 0.7018 |
| A0 | 0.5833 | 0.8594 | 0.8750 | 0.6193 | 0.8767 | 0.9211 | 0.0397 | 0.0201 | 0.0410 | 0.6856 |
| seed spread (A1-A3, max−min) | 0.0015 | 0.0014 | 0.0019 | 0.0011 | 0.0009 | 0.0010 | 0.0055 | 0.0014 | 0.0060 | 0.0212 |

Layout and shape (all valid rows)

| partition | mean key | msq resid | n_c min | median | max | CV | rows in cells <500 | cells/family | cells/genus | cells/species | cells/species (row-wtd) | max/rms radius (median cell) | cells with max/rms>2 | key p1/median (median cell) | S1e B/row | viol S2 | viol S3 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| A1 | 0.9183 | 0.1547 | 134 | 885.0000 | 3658 | 0.6283 | 0.0852 | 104.3333 | 20.4889 | 6.1082 | 44.3004 | 1.5593 | 0.1550 | 0.9026 | 36.1574 | 0 | 0 |
| A2 | 0.9188 | 0.1538 | 1 | 853.5000 | 4131 | 0.6557 | 0.0847 | 101.0000 | 20.3556 | 6.0475 | 41.5689 | 1.5226 | 0.1407 | 0.8974 | 36.1574 | 0 | 0 |
| A3 | 0.9186 | 0.1542 | 56 | 882.5000 | 5398 | 0.6698 | 0.0795 | 103.0000 | 20.6667 | 6.0597 | 45.0908 | 1.5401 | 0.1400 | 0.8980 | 36.1574 | 0 | 0 |
| A0 | 0.9189 | 0.1536 | 159 | 861.5000 | 3688 | 0.5875 | 0.0548 | 101.3333 | 20.4889 | 6.2568 | 42.1813 | 1.5464 | 0.1650 | 0.9013 | 36.1574 | 0 | 0 |
| seed spread (A1-A3, max−min) | 0.0005 | 0.0009 | 133 | 31.5000 | 1740 | 0.0415 | 0.0057 | 3.3333 | 0.3111 | 0.0607 | 3.5218 | 0.0367 | 0.0150 | 0.0052 | 0.0000 | 0 | 0 |

**Where the coding spread comes from** (`diag_groups.py`, log `work/diag_groups.log`, Si, held-out rows). The build's
grouping rule does not hold a group above n rows at this size. A1 has a group of 2 cells and 617 fit rows (fewer than
n = 768, so its pooled covariance is singular and floored) and one of 2,177 (under 3n); A2 has two groups of one
cell each with 1 and 3 fit rows (outlier rows that made their own cell and their own group; no held-out row falls in
them); A3's smallest group has 5,175. The 208 held-out rows of A1's 617-row group need **200 B** at w* = 0.06 against
a mean of 36 B, which by itself adds about 0.7 B to A1's mean. Over groups of at least 3n fit rows only, A1's S1b at
0.06 is 36.33 B (A2 36.87, A3 36.28), so the seed spread without that one group would be about 0.6 B instead of 1.08 B.
The spread is still taken as the spec defines it, with this group in.

| seed spread (max − min of A1, A2, A3) | value | as a share of A1-fp's value |
|---|---|---|
| S1d at 16 B, Si | 0.00139 | 1.5% |
| S1b at w* = 0.06, Si | 1.082 B | 3.0% |
| S1d at 16 B, Sm | 0.00230 | 1.6% |
| S1b at w* = 0.06, Sm | 1.654 B | 2.5% |
| S2+3, item queries, 0.01 / 0.1 / 1% | 0.0036 / 0.0037 / 0.0026 | 0.9 / 0.7 / 0.4% |
| S2+3, species prompts, 1% | 0.0049 | 0.6% |
| S2+3, text prompts, 0.01% | 0.0006 | 0.06% |
| S4a, text / item (mean share) | 0.0015 / 0.0019 | 0.3 / 0.2% |
| 10-NN own-cell share | 0.021 | 3.0% |

**Reading.** Between build seeds the headline coding scores move by 1.5% (S1d at 16 B) and 3.0% (S1b at 0.06), so
a coding difference must exceed about 3% (S1d) or 6% (S1b) to count here; S2+3 moves by under 1%. Much of the S1b
spread is one artefact of the build's grouping rule (a group under n rows in A1), which is itself a finding: at
200,000 rows and G = 12 the rule can leave a group whose basis is singular and whose held-out rows cost 5 times the
mean. The cut alone (S2) decides nothing for text and species prompts at any answer size (2ρ is 1.16 to 1.49), so
S2+3 equals S3 for them.

## Item 3. The price of the built grouping (P3.4)

`item3.py A1-fp`, log `work/item3.log`. All four share A1-fp's cells, so S2 to S5 are A1-fp's (item 1). A1-fp-rand:
the group labels of A1-fp's cells permuted (seed 1), keeping its numbers of cells per group (its smallest group then
holds 2,037 fit rows). A1-fp-one: one basis for all rows. A1-fp-cell: a basis per cell from 0.5·Σ_c + 0.5·Σ̄_g, Σ_c
on the cell's fit rows (106 to about 2,800, median 630). S1e is 36.2 B a row for the baseline, 3.7 for one group and
591 for a basis per cell.


Coding, Σ_Q = Si, held-out rows (mean over 49,978 rows)

| partition | S1d 16B | S1b w*=.06 (B) | S1d 4B | S1d 8B | S1d 32B | S1b .03 | S1b .12 | S1a mean (sec.) | S1a rms (sec.) |
|---|---|---|---|---|---|---|---|---|---|
| A1-fp | 0.09547 | 36.639 | 0.14690 | 0.12318 | 0.06631 | 81.836 | 10.420 | 0.20636 | 0.22035 |
| A1-fp-rand | 0.10908 | 44.406 | 0.16395 | 0.13913 | 0.07635 | 94.573 | 13.497 | 0.22262 | 0.23585 |
| A1-fp-one | 0.11339 | 47.085 | 0.16881 | 0.14387 | 0.07976 | 98.788 | 14.723 | 0.22719 | 0.24071 |
| A1-fp-cell | 0.08560 | 31.292 | 0.13381 | 0.11123 | 0.05931 | 73.826 | 7.988 | 0.19362 | 0.20678 |
| seed spread (A1-A3, max−min) | 0.00139 | 1.082 | 0.00177 | 0.00162 | 0.00111 | 1.791 | 0.46924 | 0.00171 | 0.00225 |
| ratio to A1-fp: ||||||||||
| A1-fp-rand / A1-fp | 1.1426 | 1.2120 | 1.1161 | 1.1295 | 1.1513 | 1.1556 | 1.2953 | 1.0788 | 1.0703 |
| A1-fp-one / A1-fp | 1.1878 | 1.2851 | 1.1492 | 1.1680 | 1.2027 | 1.2071 | 1.4130 | 1.1009 | 1.0924 |
| A1-fp-cell / A1-fp | 0.8966 | 0.8541 | 0.9109 | 0.9030 | 0.8944 | 0.9021 | 0.7666 | 0.9382 | 0.9384 |

Coding, Σ_Q = Sm, held-out rows (mean over 49,978 rows)

| partition | S1d 16B | S1b w*=.06 (B) | S1d 4B | S1d 8B | S1d 32B | S1b .03 | S1b .12 | S1a mean (sec.) | S1a rms (sec.) |
|---|---|---|---|---|---|---|---|---|---|
| A1-fp | 0.14595 | 64.874 | 0.21483 | 0.18383 | 0.10395 | 122.774 | 25.021 | 0.28638 | 0.30591 |
| A1-fp-rand | 0.16621 | 76.207 | 0.23896 | 0.20690 | 0.11941 | 138.414 | 31.080 | 0.30896 | 0.32730 |
| A1-fp-one | 0.17278 | 79.972 | 0.24611 | 0.21397 | 0.12472 | 143.451 | 33.302 | 0.31561 | 0.33437 |
| A1-fp-cell | 0.13090 | 57.552 | 0.19554 | 0.16596 | 0.09303 | 113.348 | 20.536 | 0.26804 | 0.28640 |
| seed spread (A1-A3, max−min) | 0.00230 | 1.654 | 0.00285 | 0.00264 | 0.00186 | 2.363 | 0.92272 | 0.00285 | 0.00402 |
| ratio to A1-fp: ||||||||||
| A1-fp-rand / A1-fp | 1.1388 | 1.1747 | 1.1123 | 1.1254 | 1.1488 | 1.1274 | 1.2422 | 1.0788 | 1.0699 |
| A1-fp-one / A1-fp | 1.1838 | 1.2327 | 1.1456 | 1.1639 | 1.1998 | 1.1684 | 1.3310 | 1.1020 | 1.0930 |
| A1-fp-cell / A1-fp | 0.8969 | 0.8871 | 0.9102 | 0.9027 | 0.8950 | 0.9232 | 0.8207 | 0.9359 | 0.9362 |

Differences from the baseline with the seed rule (Si; the Sm rows at the end):

| score | A1-fp | A1-fp-one | difference | relative | 2 × seed spread | a result? |
|---|---|---|---|---|---|---|
| Si_S1d_16 | 0.09547 | 0.11339 | +0.01793 | +18.78% | 0.00278 | yes |
| Si_S1b_0.06 | 36.63907 | 47.08471 | +10.44565 | +28.51% | 2.16459 | yes |
| Si_S1a_mean | 0.20636 | 0.22719 | +0.02083 | +10.09% | 0.00342 | yes |
| Sm_S1d_16 | 0.14595 | 0.17278 | +0.02683 | +18.38% | 0.00459 | yes |
| Sm_S1b_0.06 | 64.87390 | 79.97227 | +15.09837 | +23.27% | 3.30709 | yes |

| score | A1-fp | A1-fp-rand | difference | relative | 2 × seed spread | a result? |
|---|---|---|---|---|---|---|
| Si_S1d_16 | 0.09547 | 0.10908 | +0.01362 | +14.26% | 0.00278 | yes |
| Si_S1b_0.06 | 36.63907 | 44.40631 | +7.76724 | +21.20% | 2.16459 | yes |
| Si_S1a_mean | 0.20636 | 0.22262 | +0.01625 | +7.88% | 0.00342 | yes |
| Sm_S1d_16 | 0.14595 | 0.16621 | +0.02026 | +13.88% | 0.00459 | yes |
| Sm_S1b_0.06 | 64.87390 | 76.20716 | +11.33325 | +17.47% | 3.30709 | yes |

| score | A1-fp | A1-fp-cell | difference | relative | 2 × seed spread | a result? |
|---|---|---|---|---|---|---|
| Si_S1d_16 | 0.09547 | 0.08560 | -0.00987 | -10.34% | 0.00278 | yes |
| Si_S1b_0.06 | 36.63907 | 31.29175 | -5.34732 | -14.59% | 2.16459 | yes |
| Si_S1a_mean | 0.20636 | 0.19362 | -0.01274 | -6.18% | 0.00342 | yes |
| Sm_S1d_16 | 0.14595 | 0.13090 | -0.01505 | -10.31% | 0.00459 | yes |
| Sm_S1b_0.06 | 64.87390 | 57.55162 | -7.32229 | -11.29% | 3.30709 | yes |

**Against the predictions.** On S1d at 16 B, A1-fp-one is 18.8% wider than the baseline (predicted 5 to 25%: inside);
A1-fp-rand lies between, 14.3% wider (predicted: between); A1-fp-cell is 10.3% narrower (predicted: narrower). The
drop rule for B (A1-cell not narrower on held-out rows) **did not fire**. With Sm the three ratios are the same to
half a point (1.184, 1.139, 0.897).

**Reading.** The built grouping is worth 16% of S1d at 16 B over one basis and 22% of the bytes at w* = 0.06, and a
random grouping with the same sizes keeps only a quarter of that (S1d 14.3% wider than built, one basis 18.8%): the
groups' gain is mostly from which cells share a basis, not from having twelve bases. A basis per cell, shrunk half way
to its group, is a further 10% narrower at 16 B and 15% fewer bytes at 0.06, at 16 times the per-row state, so
there is room below the built grouping for B to reach, at most about that much.

## Item 4. Cells to bases by coding cost: B0 and B16 (P3.1, P3.2)

`itemB.py A1-fp`, log `work/itemB.log`, summary `work/itemB_summary.json`. From A1-fp's cells and groups; Σ_Q = Si in
the scheme (the scores are then taken with Si and Sm). Costs from per-cell scatter matrices: the sum over a cell's
rows of w_B(x | c, g)² is W_g·⟨C_c, T_g diag(s) T_gᵀ⟩, exact for the level model. Floor of 3n = 2,304 fit rows a group:
A1-fp already has one group of 1,274 fit rows, so moves *out of* it are refused (that is what "refused" counts below).
Cross-fitted: each cell's fit rows split in two by the parity of their rank in permutation order within the cell
(75,010 and 74,912 rows).

Objective per pass (mean square width per row; "in-sample value" is Σ_g N_g W_g²/N on the rows that fit the metrics,
which for the cross-fitted runs is half A):

| run | pass | cells moved | refused by the floor | objective, fit rows | objective, held-out rows | in-sample value |
|---|---|---|---|---|---|---|
| B0-in | 0 | 7 | 0 | 0.047702 | 0.048556 | 0.047702 |
| B0-in | 1 | 0 | 0 | 0.047483 | 0.048340 | 0.047483 |
| B0-cf | 0 | 8 | 3 | 0.048739 | 0.050591 | 0.047427 |
| B0-cf | 1 | 1 | 3 | 0.048521 | 0.050379 | 0.047202 |
| B0-cf | 2 | 0 | 3 | 0.048515 | 0.050371 | 0.047198 |
| B16-in | 0 | 7 | 0 | 0.010208 | 0.010527 | 0.010208 |
| B16-in | 1 | 2 | 0 | 0.010119 | 0.010433 | 0.010119 |
| B16-in | 2 | 3 | 0 | 0.010075 | 0.010392 | 0.010075 |
| B16-in | 3 | 0 | 0 | 0.009992 | 0.010314 | 0.009992 |
| B16-cf | 0 | 10 | 3 | 0.010627 | 0.011326 | 0.010075 |
| B16-cf | 1 | 2 | 3 | 0.010509 | 0.011208 | 0.009954 |
| B16-cf | 2 | 1 | 3 | 0.010456 | 0.011155 | 0.009901 |
| B16-cf | 3 | 0 | 3 | 0.010411 | 0.011115 | 0.009859 |

The in-sample objective never rose (B0-in 0.047702 → 0.047483; B16-in 0.010208 → 0.009992). The held-out objective
fell with it in every run: by 0.45% (B0-in), 0.43% (B0-cf), 2.03% (B16-in) and 1.85% (B16-cf) of the mean square
width.

Single-cell moves with both groups refitted, one pass in random order (seed 1), from each in-sample result:

| from | cells tried | moves kept | refused by the floor | Σ N_g W_g²/N before | after | change |
|---|---|---|---|---|---|---|
| B0-in | 198 | 29 | 2 | 0.047483 | 0.047024 | -0.97% |
| B16-in | 199 | 31 | 1 | 0.009992 | 0.009850 | -1.42% |

| grouping | cells whose group changed | mean angle, cell centre to its group mean direction (degrees) |
|---|---|---|
| A1-fp (baseline) | 0 | 18.49 |
| B0-in | 7 | 18.49 |
| B0-cf | 9 | 18.53 |
| B0-in-moves | 35 | 18.82 |
| B16-in | 12 | 18.56 |
| B16-cf | 13 | 18.56 |
| B16-in-moves | 42 | 18.96 |

Scores on held-out rows (S2 to S5 are A1-fp's, since the cells do not change):


Coding, Σ_Q = Si, held-out rows (mean over 49,978 rows)

| partition | S1d 16B | S1b w*=.06 (B) | S1d 4B | S1d 8B | S1d 32B | S1b .03 | S1b .12 | S1a mean (sec.) | S1a rms (sec.) |
|---|---|---|---|---|---|---|---|---|---|
| A1-fp | 0.09547 | 36.639 | 0.14690 | 0.12318 | 0.06631 | 81.836 | 10.420 | 0.20636 | 0.22035 |
| B0-in | 0.09514 | 36.446 | 0.14652 | 0.12282 | 0.06606 | 81.598 | 10.308 | 0.20600 | 0.21986 |
| B0-cf | 0.09512 | 36.432 | 0.14650 | 0.12280 | 0.06604 | 81.582 | 10.301 | 0.20599 | 0.21984 |
| B0-in-moves | 0.09409 | 35.826 | 0.14543 | 0.12172 | 0.06520 | 80.588 | 10.057 | 0.20496 | 0.21877 |
| B16-in | 0.09464 | 36.121 | 0.14604 | 0.12231 | 0.06564 | 81.148 | 10.165 | 0.20558 | 0.21938 |
| B16-cf | 0.09464 | 36.122 | 0.14605 | 0.12232 | 0.06564 | 81.153 | 10.166 | 0.20559 | 0.21939 |
| B16-in-moves | 0.09394 | 35.711 | 0.14531 | 0.12158 | 0.06505 | 80.419 | 10.005 | 0.20486 | 0.21865 |
| seed spread (A1-A3, max−min) | 0.00139 | 1.082 | 0.00177 | 0.00162 | 0.00111 | 1.791 | 0.46924 | 0.00171 | 0.00225 |
| ratio to A1-fp: ||||||||||
| B0-in / A1-fp | 0.9965 | 0.9947 | 0.9974 | 0.9971 | 0.9962 | 0.9971 | 0.9893 | 0.9982 | 0.9978 |
| B0-cf / A1-fp | 0.9963 | 0.9943 | 0.9973 | 0.9969 | 0.9959 | 0.9969 | 0.9886 | 0.9982 | 0.9977 |
| B0-in-moves / A1-fp | 0.9856 | 0.9778 | 0.9900 | 0.9882 | 0.9833 | 0.9847 | 0.9651 | 0.9932 | 0.9928 |
| B16-in / A1-fp | 0.9913 | 0.9859 | 0.9942 | 0.9929 | 0.9899 | 0.9916 | 0.9755 | 0.9962 | 0.9956 |
| B16-cf / A1-fp | 0.9913 | 0.9859 | 0.9942 | 0.9930 | 0.9899 | 0.9916 | 0.9756 | 0.9963 | 0.9956 |
| B16-in-moves / A1-fp | 0.9840 | 0.9747 | 0.9892 | 0.9870 | 0.9810 | 0.9827 | 0.9601 | 0.9927 | 0.9923 |

Coding, Σ_Q = Sm, held-out rows (mean over 49,978 rows)

| partition | S1d 16B | S1b w*=.06 (B) | S1d 4B | S1d 8B | S1d 32B | S1b .03 | S1b .12 | S1a mean (sec.) | S1a rms (sec.) |
|---|---|---|---|---|---|---|---|---|---|
| A1-fp | 0.14595 | 64.874 | 0.21483 | 0.18383 | 0.10395 | 122.774 | 25.021 | 0.28638 | 0.30591 |
| B0-in | 0.14545 | 64.648 | 0.21428 | 0.18330 | 0.10355 | 122.525 | 24.844 | 0.28586 | 0.30519 |
| B0-cf | 0.14542 | 64.631 | 0.21425 | 0.18327 | 0.10352 | 122.510 | 24.831 | 0.28584 | 0.30516 |
| B0-in-moves | 0.14383 | 63.751 | 0.21262 | 0.18162 | 0.10221 | 121.274 | 24.345 | 0.28428 | 0.30352 |
| B16-in | 0.14468 | 64.228 | 0.21354 | 0.18252 | 0.10290 | 122.022 | 24.565 | 0.28520 | 0.30442 |
| B16-cf | 0.14468 | 64.231 | 0.21355 | 0.18253 | 0.10290 | 122.030 | 24.565 | 0.28521 | 0.30444 |
| B16-in-moves | 0.14360 | 63.603 | 0.21244 | 0.18142 | 0.10197 | 121.078 | 24.248 | 0.28412 | 0.30331 |
| seed spread (A1-A3, max−min) | 0.00230 | 1.654 | 0.00285 | 0.00264 | 0.00186 | 2.363 | 0.92272 | 0.00285 | 0.00402 |
| ratio to A1-fp: ||||||||||
| B0-in / A1-fp | 0.9966 | 0.9965 | 0.9974 | 0.9971 | 0.9962 | 0.9980 | 0.9929 | 0.9982 | 0.9977 |
| B0-cf / A1-fp | 0.9963 | 0.9963 | 0.9973 | 0.9969 | 0.9959 | 0.9979 | 0.9924 | 0.9981 | 0.9976 |
| B0-in-moves / A1-fp | 0.9855 | 0.9827 | 0.9897 | 0.9880 | 0.9833 | 0.9878 | 0.9730 | 0.9927 | 0.9922 |
| B16-in / A1-fp | 0.9913 | 0.9900 | 0.9940 | 0.9929 | 0.9899 | 0.9939 | 0.9818 | 0.9959 | 0.9951 |
| B16-cf / A1-fp | 0.9913 | 0.9901 | 0.9941 | 0.9929 | 0.9900 | 0.9939 | 0.9818 | 0.9959 | 0.9952 |
| B16-in-moves / A1-fp | 0.9839 | 0.9804 | 0.9889 | 0.9869 | 0.9810 | 0.9862 | 0.9691 | 0.9921 | 0.9915 |

The headline differences with the seed rule:

| grouping | S1d 16 B (Si) | S1b w*=.06 (Si) | S1a mean (Si, sec.) | S1d 16 B (Sm) | S1b w*=.06 (Sm) |
|---|---|---|---|---|---|
| B0-in | -0.35% | -0.53% | -0.18% | -0.34% | -0.35% |
| B0-cf | -0.37% | -0.57% | -0.18% | -0.37% | -0.37% |
| B0-in-moves | -1.44% | -2.22% | -0.68% | -1.45% | -1.73% |
| B16-in | -0.87% | -1.41% | -0.38% | -0.87% | -1.00% |
| B16-cf | -0.87% | -1.41% | -0.37% | -0.87% | -0.99% |
| B16-in-moves | -1.60% | -2.53% | -0.73% | -1.61% | -1.96% |
| twice the seed spread, as a share of A1-fp | 2.91% | 5.91% | 1.66% | 3.15% | 5.10% |

**Drop rules.** For B: "the gain in S1b at w* = 0.06 is under 1%, or under twice the seed spread". B0 (both
variants) gains 0.5% in S1b: under 1% **and** under twice the seed spread: **fires**. B16-in and B16-cf gain 1.4%: over
1% but under twice the seed spread (5.9%): **fires**. With the extra pass of single moves B16 gains 2.5% and B0 2.2%,
still under twice the seed spread: **fires**. The angle clause does not fire (the mean angle rises from 18.49° to at
most 18.96°, not by half). The zero-read clause (B0 gains in S1a and not in S1d or S1b) does not fire: B0's gains are
smaller in S1a (0.2%) than in S1d (0.35%) and S1b (0.5%).

**Reading.** Regrouping the baseline's cells by coding cost moves 7 to 13 of 200 cells and stops in 2 to 4 passes; it
lowers the held-out width at 16 B by 0.9% (B16) and the bytes at w* = 0.06 by 1.4%, and a pass of single moves with
refitting doubles that (1.6% and 2.5%). Every one of these is in the predicted direction, on held-out rows, for both
Σ_Q, and the in-sample and cross-fitted runs agree to 0.01%, but none exceeds twice the seed spread, so by the spec's
rule B is not a result: on this data the built grouping is within a few percent of what the cell-level scheme reaches,
against the 10% that a basis per cell shows exists (item 3). (The seed spread is a between-partition noise; B
changes only the grouping of fixed cells, so its paired noise is smaller than this bar, but the spec's bar is the
one applied.)

## Item 5. Rows to the cell where their interval is narrowest: C (P3.5)

`itemC.py`, logs `work/itemC_*.log`. From B16-in's cells and groups (the in-sample B16, before single moves; chosen
because the spec names B16's result and the in-sample scheme is the one the descent covers). Σ_Q = Si in the scheme.
Candidates per row: the 4 baseline centres of largest inner product, fixed, **plus the row's starting cell** (added so
that the tie rule "keep its cell" is always available; under 1% of rows have a starting cell outside their top 4, by
S0 of A1-fp). For C16-in the 4 are the largest among the cells of the row's own group (B16-in grouping). The cost is
the level model's w_B(x | c)² with the group's basis refitted on fit rows after each pass. Cmix weights: w_B² divided
by its fit-row mean at the baseline (A1-fp's cells and groups): 0.02418, 0.01701, 0.01021, 0.00490 at 4, 8, 16, 32 B.

**First step, from B16-in, with no refit** (`itemC.py A1-fp B16-in first`, `work/itemC_first.json`). Shares of
149,922 fit rows; widths on the 49,978 held-out rows, each placed by the same rule; the held-out mean of w₁₆ before
the step is 0.094637.

| rule | fit rows changing cell | fit rows changing group | held-out rows changing cell | held-out mean w₁₆ after | change |
|---|---|---|---|---|---|
| C16 | 9.96% | 2.15% | 12.20% | 0.094246 | −0.41% |
| C0 | 11.55% | 1.57% | 12.64% | 0.094589 | −0.05% |
| Cmix | 9.14% | 2.01% | 11.13% | 0.094251 | −0.41% |
| C16-in | 7.97% | 0 | 9.68% | 0.094510 | −0.13% |

Fit rows whose cheapest candidate differs between C0 and C16: **14.7%** (the referee's toy: 12%).

C16's first-step rule ("fewer than 5% of rows change cell") **does not fire** (9.96%), so C16 was run to its stopping
point and C16-in, Cmix and C0 after it.

**Objective per pass** (each rule run to 15 passes; none reached under 0.1% of rows moved, the last pass moved 0.21
to 0.48%; no cell fell under 32 fit rows; "refused" counts fit rows kept out of A1-fp's 1,274-row group's way by
the 3n floor). The rule's own cost on fit rows never rose in any run (0 rises in 60 passes).

| rule | pass | fit rows moved | refused by the floor | cells alive | fit: mean w₁₆² | fit: the rule's cost | held-out: mean w₁₆² | held-out: the rule's cost | fit rows off their starting cell | ... off their starting group |
|---|---|---|---|---|---|---|---|---|---|---|
| C16 | 0 (B16-in) | | | 200 | 0.009992 | 0.009992 | 0.010314 | | | |
| C16 | 1 | 0.0996 | 3 | 200 | 0.009868 | 0.009868 | 0.010160 | 0.010160 | 0.0996 | 0.0215 |
| C16 | 2 | 0.0410 | 4 | 200 | 0.009846 | 0.009846 | 0.010151 | 0.010151 | 0.1364 | 0.0277 |
| C16 | 3 | 0.0242 | 5 | 200 | 0.009839 | 0.009839 | 0.010147 | 0.010147 | 0.1558 | 0.0305 |
| C16 | 4 | 0.0165 | 6 | 200 | 0.009835 | 0.009835 | 0.010145 | 0.010145 | 0.1681 | 0.0322 |
| C16 | 5 | 0.0121 | 6 | 200 | 0.009833 | 0.009833 | 0.010143 | 0.010143 | 0.1768 | 0.0333 |
| C16 | 6 | 0.0094 | 6 | 200 | 0.009831 | 0.009831 | 0.010142 | 0.010142 | 0.1834 | 0.0340 |
| C16 | 7 | 0.0077 | 6 | 200 | 0.009830 | 0.009830 | 0.010141 | 0.010141 | 0.1885 | 0.0347 |
| C16 | 8 | 0.0066 | 6 | 200 | 0.009828 | 0.009828 | 0.010140 | 0.010140 | 0.1929 | 0.0352 |
| C16 | 9 | 0.0055 | 6 | 200 | 0.009827 | 0.009827 | 0.010139 | 0.010139 | 0.1962 | 0.0355 |
| C16 | 10 | 0.0045 | 6 | 200 | 0.009827 | 0.009827 | 0.010138 | 0.010138 | 0.1990 | 0.0358 |
| C16 | 11 | 0.0042 | 6 | 200 | 0.009826 | 0.009826 | 0.010138 | 0.010138 | 0.2016 | 0.0360 |
| C16 | 12 | 0.0039 | 6 | 200 | 0.009825 | 0.009825 | 0.010138 | 0.010138 | 0.2040 | 0.0361 |
| C16 | 13 | 0.0035 | 6 | 200 | 0.009825 | 0.009825 | 0.010137 | 0.010137 | 0.2061 | 0.0362 |
| C16 | 14 | 0.0030 | 6 | 200 | 0.009825 | 0.009825 | 0.010137 | 0.010137 | 0.2077 | 0.0363 |
| C16 | 15 | 0.0025 | 6 | 200 | 0.009825 | 0.009825 | 0.010137 | 0.010137 | 0.2093 | 0.0364 |
| C16-in | 0 (B16-in) | | | 200 | 0.009992 | 0.009992 | 0.010314 | | | |
| C16-in | 1 | 0.0797 | 0 | 200 | 0.009959 | 0.009959 | 0.010272 | 0.010272 | 0.0797 | 0.0000 |
| C16-in | 2 | 0.0347 | 0 | 200 | 0.009951 | 0.009951 | 0.010268 | 0.010268 | 0.1111 | 0.0000 |
| C16-in | 3 | 0.0215 | 0 | 200 | 0.009947 | 0.009947 | 0.010266 | 0.010266 | 0.1289 | 0.0000 |
| C16-in | 4 | 0.0153 | 0 | 200 | 0.009945 | 0.009945 | 0.010264 | 0.010264 | 0.1404 | 0.0000 |
| C16-in | 5 | 0.0115 | 0 | 200 | 0.009944 | 0.009944 | 0.010263 | 0.010263 | 0.1487 | 0.0000 |
| C16-in | 6 | 0.0090 | 0 | 200 | 0.009942 | 0.009942 | 0.010262 | 0.010262 | 0.1553 | 0.0000 |
| C16-in | 7 | 0.0073 | 0 | 200 | 0.009941 | 0.009941 | 0.010261 | 0.010261 | 0.1603 | 0.0000 |
| C16-in | 8 | 0.0066 | 0 | 200 | 0.009940 | 0.009940 | 0.010260 | 0.010260 | 0.1647 | 0.0000 |
| C16-in | 9 | 0.0055 | 0 | 200 | 0.009939 | 0.009939 | 0.010259 | 0.010259 | 0.1682 | 0.0000 |
| C16-in | 10 | 0.0049 | 0 | 200 | 0.009939 | 0.009939 | 0.010259 | 0.010259 | 0.1717 | 0.0000 |
| C16-in | 11 | 0.0044 | 0 | 200 | 0.009938 | 0.009938 | 0.010258 | 0.010258 | 0.1745 | 0.0000 |
| C16-in | 12 | 0.0039 | 0 | 200 | 0.009938 | 0.009938 | 0.010258 | 0.010258 | 0.1772 | 0.0000 |
| C16-in | 13 | 0.0034 | 0 | 200 | 0.009937 | 0.009937 | 0.010258 | 0.010258 | 0.1795 | 0.0000 |
| C16-in | 14 | 0.0032 | 0 | 200 | 0.009937 | 0.009937 | 0.010257 | 0.010257 | 0.1817 | 0.0000 |
| C16-in | 15 | 0.0026 | 0 | 200 | 0.009937 | 0.009937 | 0.010257 | 0.010257 | 0.1833 | 0.0000 |
| Cmix | 0 (B16-in) | | | 200 | 0.009992 | 3.923152 | 0.010314 | | | |
| Cmix | 1 | 0.0913 | 4 | 200 | 0.009872 | 3.878408 | 0.010163 | 3.987755 | 0.0913 | 0.0200 |
| Cmix | 2 | 0.0385 | 6 | 200 | 0.009851 | 3.870610 | 0.010154 | 3.984639 | 0.1260 | 0.0259 |
| Cmix | 3 | 0.0222 | 6 | 200 | 0.009843 | 3.868039 | 0.010150 | 3.983227 | 0.1438 | 0.0287 |
| Cmix | 4 | 0.0154 | 6 | 200 | 0.009839 | 3.866632 | 0.010148 | 3.982442 | 0.1554 | 0.0302 |
| Cmix | 5 | 0.0114 | 6 | 200 | 0.009837 | 3.865785 | 0.010146 | 3.981911 | 0.1635 | 0.0313 |
| Cmix | 6 | 0.0084 | 7 | 200 | 0.009836 | 3.865261 | 0.010145 | 3.981543 | 0.1694 | 0.0320 |
| Cmix | 7 | 0.0068 | 7 | 200 | 0.009835 | 3.864899 | 0.010145 | 3.981295 | 0.1739 | 0.0324 |
| Cmix | 8 | 0.0058 | 7 | 200 | 0.009834 | 3.864569 | 0.010144 | 3.981044 | 0.1777 | 0.0328 |
| Cmix | 9 | 0.0049 | 7 | 200 | 0.009832 | 3.864026 | 0.010143 | 3.980744 | 0.1811 | 0.0333 |
| Cmix | 10 | 0.0040 | 7 | 200 | 0.009831 | 3.863730 | 0.010142 | 3.980563 | 0.1837 | 0.0336 |
| Cmix | 11 | 0.0034 | 7 | 200 | 0.009831 | 3.863477 | 0.010142 | 3.980457 | 0.1858 | 0.0338 |
| Cmix | 12 | 0.0031 | 7 | 200 | 0.009830 | 3.863318 | 0.010142 | 3.980353 | 0.1879 | 0.0341 |
| Cmix | 13 | 0.0027 | 7 | 200 | 0.009830 | 3.863212 | 0.010141 | 3.980271 | 0.1895 | 0.0342 |
| Cmix | 14 | 0.0023 | 7 | 200 | 0.009829 | 3.863116 | 0.010141 | 3.980209 | 0.1910 | 0.0344 |
| Cmix | 15 | 0.0021 | 7 | 200 | 0.009829 | 3.863047 | 0.010141 | 3.980159 | 0.1923 | 0.0344 |
| C0 | 0 (B16-in) | | | 200 | 0.009992 | 0.047267 | 0.010314 | | | |
| C0 | 1 | 0.1150 | 70 | 200 | 0.009948 | 0.046600 | 0.010265 | 0.047313 | 0.1150 | 0.0152 |
| C0 | 2 | 0.0501 | 80 | 200 | 0.009947 | 0.046470 | 0.010263 | 0.047236 | 0.1589 | 0.0195 |
| C0 | 3 | 0.0310 | 76 | 200 | 0.009949 | 0.046413 | 0.010264 | 0.047198 | 0.1831 | 0.0215 |
| C0 | 4 | 0.0229 | 75 | 200 | 0.009951 | 0.046379 | 0.010266 | 0.047174 | 0.1997 | 0.0227 |
| C0 | 5 | 0.0183 | 73 | 200 | 0.009953 | 0.046354 | 0.010267 | 0.047157 | 0.2126 | 0.0234 |
| C0 | 6 | 0.0148 | 71 | 200 | 0.009953 | 0.046336 | 0.010268 | 0.047146 | 0.2224 | 0.0239 |
| C0 | 7 | 0.0120 | 70 | 200 | 0.009954 | 0.046326 | 0.010269 | 0.047141 | 0.2302 | 0.0243 |
| C0 | 8 | 0.0096 | 72 | 200 | 0.009955 | 0.046319 | 0.010270 | 0.047137 | 0.2362 | 0.0246 |
| C0 | 9 | 0.0086 | 69 | 200 | 0.009955 | 0.046313 | 0.010270 | 0.047134 | 0.2411 | 0.0249 |
| C0 | 10 | 0.0079 | 69 | 200 | 0.009955 | 0.046307 | 0.010271 | 0.047131 | 0.2455 | 0.0252 |
| C0 | 11 | 0.0064 | 69 | 200 | 0.009955 | 0.046304 | 0.010271 | 0.047129 | 0.2494 | 0.0253 |
| C0 | 12 | 0.0057 | 69 | 200 | 0.009955 | 0.046301 | 0.010271 | 0.047128 | 0.2526 | 0.0255 |
| C0 | 13 | 0.0051 | 70 | 200 | 0.009955 | 0.046298 | 0.010271 | 0.047126 | 0.2553 | 0.0257 |
| C0 | 14 | 0.0052 | 71 | 200 | 0.009955 | 0.046295 | 0.010271 | 0.047124 | 0.2580 | 0.0258 |
| C0 | 15 | 0.0048 | 73 | 200 | 0.009955 | 0.046293 | 0.010271 | 0.047123 | 0.2605 | 0.0259 |

| rule | mean w₁₆² from B16-in to the end, fit rows | held-out rows | fit rows off their starting cell at the end | off their starting group |
|---|---|---|---|---|
| C16 | −1.68% | −1.71% | 20.9% | 3.6% |
| C16-in | −0.56% | −0.55% | 18.3% | 0 |
| Cmix | −1.63% | −1.67% | 19.2% | 3.4% |
| C0 | −0.37% | −0.41% | 26.1% | 2.6% |

**Scores at the stopping point** (C16, C16-in, Cmix, C0 against the baseline and against their start B16-in):


Coding, Σ_Q = Si, held-out rows (mean over 49,978 rows)

| partition | S1d 16B | S1b w*=.06 (B) | S1d 4B | S1d 8B | S1d 32B | S1b .03 | S1b .12 | S1a mean (sec.) | S1a rms (sec.) |
|---|---|---|---|---|---|---|---|---|---|
| A1-fp | 0.09547 | 36.639 | 0.14690 | 0.12318 | 0.06631 | 81.836 | 10.420 | 0.20636 | 0.22035 |
| B16-in | 0.09464 | 36.121 | 0.14604 | 0.12231 | 0.06564 | 81.148 | 10.165 | 0.20558 | 0.21938 |
| C16 | 0.09396 | 35.769 | 0.14531 | 0.12154 | 0.06515 | 80.643 | 9.981 | 0.20643 | 0.22018 |
| C16-in | 0.09439 | 36.014 | 0.14575 | 0.12200 | 0.06549 | 81.013 | 10.105 | 0.20663 | 0.22049 |
| Cmix | 0.09398 | 35.781 | 0.14525 | 0.12153 | 0.06517 | 80.669 | 9.983 | 0.20618 | 0.21989 |
| C0 | 0.09456 | 36.046 | 0.14611 | 0.12230 | 0.06553 | 81.016 | 10.134 | 0.20360 | 0.21708 |
| seed spread (A1-A3, max−min) | 0.00139 | 1.082 | 0.00177 | 0.00162 | 0.00111 | 1.791 | 0.46924 | 0.00171 | 0.00225 |
| ratio to A1-fp: ||||||||||
| B16-in / A1-fp | 0.9913 | 0.9859 | 0.9942 | 0.9929 | 0.9899 | 0.9916 | 0.9755 | 0.9962 | 0.9956 |
| C16 / A1-fp | 0.9842 | 0.9762 | 0.9892 | 0.9867 | 0.9824 | 0.9854 | 0.9579 | 1.0003 | 0.9992 |
| C16-in / A1-fp | 0.9887 | 0.9829 | 0.9922 | 0.9904 | 0.9876 | 0.9899 | 0.9697 | 1.0013 | 1.0006 |
| Cmix / A1-fp | 0.9844 | 0.9766 | 0.9888 | 0.9866 | 0.9827 | 0.9857 | 0.9580 | 0.9991 | 0.9979 |
| C0 / A1-fp | 0.9905 | 0.9838 | 0.9947 | 0.9929 | 0.9883 | 0.9900 | 0.9726 | 0.9866 | 0.9851 |

Coding, Σ_Q = Sm, held-out rows (mean over 49,978 rows)

| partition | S1d 16B | S1b w*=.06 (B) | S1d 4B | S1d 8B | S1d 32B | S1b .03 | S1b .12 | S1a mean (sec.) | S1a rms (sec.) |
|---|---|---|---|---|---|---|---|---|---|
| A1-fp | 0.14595 | 64.874 | 0.21483 | 0.18383 | 0.10395 | 122.774 | 25.021 | 0.28638 | 0.30591 |
| B16-in | 0.14468 | 64.228 | 0.21354 | 0.18252 | 0.10290 | 122.022 | 24.565 | 0.28520 | 0.30442 |
| C16 | 0.14366 | 63.767 | 0.21244 | 0.18138 | 0.10213 | 121.446 | 24.264 | 0.28594 | 0.30506 |
| C16-in | 0.14431 | 64.099 | 0.21310 | 0.18206 | 0.10267 | 121.877 | 24.472 | 0.28630 | 0.30559 |
| Cmix | 0.14369 | 63.789 | 0.21238 | 0.18138 | 0.10217 | 121.478 | 24.273 | 0.28566 | 0.30474 |
| C0 | 0.14455 | 64.116 | 0.21365 | 0.18250 | 0.10273 | 121.858 | 24.505 | 0.28329 | 0.30213 |
| seed spread (A1-A3, max−min) | 0.00230 | 1.654 | 0.00285 | 0.00264 | 0.00186 | 2.363 | 0.92272 | 0.00285 | 0.00402 |
| ratio to A1-fp: ||||||||||
| B16-in / A1-fp | 0.9913 | 0.9900 | 0.9940 | 0.9929 | 0.9899 | 0.9939 | 0.9818 | 0.9959 | 0.9951 |
| C16 / A1-fp | 0.9843 | 0.9829 | 0.9889 | 0.9866 | 0.9826 | 0.9892 | 0.9698 | 0.9985 | 0.9972 |
| C16-in / A1-fp | 0.9888 | 0.9881 | 0.9920 | 0.9904 | 0.9877 | 0.9927 | 0.9781 | 0.9997 | 0.9990 |
| Cmix / A1-fp | 0.9845 | 0.9833 | 0.9886 | 0.9866 | 0.9829 | 0.9894 | 0.9701 | 0.9975 | 0.9962 |
| C0 / A1-fp | 0.9904 | 0.9883 | 0.9945 | 0.9927 | 0.9883 | 0.9925 | 0.9794 | 0.9892 | 0.9877 |

S2+3 (share of valid rows left by both cut and range), mean over queries

| partition | text 0.01% | text 0.1% | text 1% | species 0.01% | species 0.1% | species 1% | item 0.01% | item 0.1% | item 1% |
|---|---|---|---|---|---|---|---|---|---|
| A1-fp | 0.9762 | 0.9935 | 0.9993 | 0.6708 | 0.7272 | 0.8049 | 0.4235 | 0.5106 | 0.6488 |

| C16 | 0.9777 | 0.9941 | 0.9994 | 0.6754 | 0.7325 | 0.8096 | 0.4306 | 0.5180 | 0.6562 |
| C16-in | 0.9777 | 0.9941 | 0.9994 | 0.6745 | 0.7315 | 0.8090 | 0.4291 | 0.5165 | 0.6549 |
| Cmix | 0.9774 | 0.9940 | 0.9994 | 0.6748 | 0.7317 | 0.8088 | 0.4296 | 0.5169 | 0.6551 |
| C0 | 0.9768 | 0.9936 | 0.9993 | 0.6740 | 0.7307 | 0.8077 | 0.4268 | 0.5131 | 0.6492 |
| seed spread (A1-A3, max−min) | 0.0006 | 0.0004 | 0.0001 | 0.0048 | 0.0053 | 0.0049 | 0.0036 | 0.0037 | 0.0026 |
| ratio to A1-fp: ||||||||||
| C16 / A1-fp | 1.0015 | 1.0006 | 1.0001 | 1.0069 | 1.0073 | 1.0058 | 1.0169 | 1.0144 | 1.0114 |
| C16-in / A1-fp | 1.0016 | 1.0006 | 1.0001 | 1.0055 | 1.0060 | 1.0051 | 1.0132 | 1.0115 | 1.0093 |
| Cmix / A1-fp | 1.0013 | 1.0005 | 1.0000 | 1.0060 | 1.0062 | 1.0048 | 1.0144 | 1.0124 | 1.0096 |
| C0 / A1-fp | 1.0006 | 1.0000 | 1.0000 | 1.0048 | 1.0048 | 1.0035 | 1.0078 | 1.0049 | 1.0005 |

S2 (cut alone)

| partition | text 0.01% | text 0.1% | text 1% | species 0.01% | species 0.1% | species 1% | item 0.01% | item 0.1% | item 1% |
|---|---|---|---|---|---|---|---|---|---|
| A1-fp | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.8370 | 0.8475 | 0.8668 |

| C16 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.8432 | 0.8544 | 0.8719 |
| C16-in | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.8426 | 0.8538 | 0.8715 |
| Cmix | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.8423 | 0.8531 | 0.8712 |
| C0 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.8246 | 0.8367 | 0.8542 |
| seed spread (A1-A3, max−min) | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.0056 | 0.0052 | 0.0032 |
| ratio to A1-fp: ||||||||||
| C16 / A1-fp | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0074 | 1.0081 | 1.0059 |
| C16-in / A1-fp | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0067 | 1.0074 | 1.0054 |
| Cmix / A1-fp | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0063 | 1.0066 | 1.0050 |
| C0 / A1-fp | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.9851 | 0.9872 | 0.9855 |

S3 (sorted range alone)

| partition | text 0.01% | text 0.1% | text 1% | species 0.01% | species 0.1% | species 1% | item 0.01% | item 0.1% | item 1% |
|---|---|---|---|---|---|---|---|---|---|
| A1-fp | 0.9762 | 0.9935 | 0.9993 | 0.6708 | 0.7272 | 0.8049 | 0.4354 | 0.5259 | 0.6682 |

| C16 | 0.9777 | 0.9941 | 0.9994 | 0.6754 | 0.7325 | 0.8096 | 0.4420 | 0.5327 | 0.6745 |
| C16-in | 0.9777 | 0.9941 | 0.9994 | 0.6745 | 0.7315 | 0.8090 | 0.4408 | 0.5313 | 0.6735 |
| Cmix | 0.9774 | 0.9940 | 0.9994 | 0.6748 | 0.7317 | 0.8088 | 0.4411 | 0.5317 | 0.6735 |
| C0 | 0.9768 | 0.9936 | 0.9993 | 0.6740 | 0.7307 | 0.8077 | 0.4440 | 0.5346 | 0.6762 |
| seed spread (A1-A3, max−min) | 0.0006 | 0.0004 | 0.0001 | 0.0048 | 0.0053 | 0.0049 | 0.0038 | 0.0030 | 0.0021 |
| ratio to A1-fp: ||||||||||
| C16 / A1-fp | 1.0015 | 1.0006 | 1.0001 | 1.0069 | 1.0073 | 1.0058 | 1.0151 | 1.0129 | 1.0095 |
| C16-in / A1-fp | 1.0016 | 1.0006 | 1.0001 | 1.0055 | 1.0060 | 1.0051 | 1.0123 | 1.0103 | 1.0079 |
| Cmix / A1-fp | 1.0013 | 1.0005 | 1.0000 | 1.0060 | 1.0062 | 1.0048 | 1.0130 | 1.0110 | 1.0079 |
| C0 / A1-fp | 1.0006 | 1.0000 | 1.0000 | 1.0048 | 1.0048 | 1.0035 | 1.0198 | 1.0165 | 1.0120 |

S2 at 1%: mean 2ρ (net) and mean β(E⁺−E⁻) (extent) over cells left

| partition | 2ρ text | 2ρ species | 2ρ item | extent text | extent species | extent item |
|---|---|---|---|---|---|---|
| A1-fp | 1.4893 | 1.1581 | 0.7191 | 0.0851 | 0.1171 | 0.1607 |

| C16 | 1.4898 | 1.1579 | 0.7219 | 0.0866 | 0.1168 | 0.1676 |
| C16-in | 1.4905 | 1.1569 | 0.7215 | 0.0847 | 0.1171 | 0.1675 |
| Cmix | 1.4891 | 1.1580 | 0.7213 | 0.0865 | 0.1167 | 0.1673 |
| C0 | 1.4806 | 1.1565 | 0.7208 | 0.0836 | 0.1126 | 0.1433 |
| seed spread (A1-A3, max−min) | 0.0023 | 0.0113 | 0.0031 | 0.0033 | 0.0034 | 0.0017 |

Order: S4a (mean share; ratio of means), S4b (share of rows visited to cover 9 of the 10 best), own-cell share of 10-NN

| partition | S4a text | S4a species | S4a item | S4a ratio text | S4a ratio species | S4a ratio item | S4b text | S4b species | S4b item | 10-NN own cell |
|---|---|---|---|---|---|---|---|---|---|---|
| A1-fp | 0.5855 | 0.8605 | 0.8756 | 0.6210 | 0.8778 | 0.9227 | 0.0391 | 0.0213 | 0.0307 | 0.6976 |

| C16 | 0.5791 | 0.8567 | 0.8700 | 0.6144 | 0.8746 | 0.9175 | 0.0436 | 0.0220 | 0.0401 | 0.7027 |
| C16-in | 0.5803 | 0.8574 | 0.8707 | 0.6155 | 0.8752 | 0.9181 | 0.0438 | 0.0219 | 0.0351 | 0.7011 |
| Cmix | 0.5805 | 0.8574 | 0.8707 | 0.6158 | 0.8752 | 0.9179 | 0.0420 | 0.0211 | 0.0384 | 0.7129 |
| C0 | 0.5822 | 0.8633 | 0.8843 | 0.6187 | 0.8808 | 0.9338 | 0.0347 | 0.0211 | 0.0227 | 0.6460 |
| seed spread (A1-A3, max−min) | 0.0015 | 0.0014 | 0.0019 | 0.0011 | 0.0009 | 0.0010 | 0.0055 | 0.0014 | 0.0060 | 0.0212 |
| ratio to A1-fp: |||||||||||
| C16 / A1-fp | 0.9890 | 0.9957 | 0.9936 | 0.9893 | 0.9964 | 0.9943 | 1.1163 | 1.0326 | 1.3058 | 1.0072 |
| C16-in / A1-fp | 0.9912 | 0.9965 | 0.9943 | 0.9911 | 0.9969 | 0.9950 | 1.1200 | 1.0270 | 1.1440 | 1.0049 |
| Cmix / A1-fp | 0.9915 | 0.9964 | 0.9944 | 0.9915 | 0.9970 | 0.9947 | 1.0739 | 0.9899 | 1.2491 | 1.0219 |
| C0 / A1-fp | 0.9944 | 1.0034 | 1.0099 | 0.9963 | 1.0034 | 1.0120 | 0.8876 | 0.9905 | 0.7405 | 0.9259 |

Layout and shape (all valid rows)

| partition | mean key | msq resid | n_c min | median | max | CV | rows in cells <500 | cells/family | cells/genus | cells/species | cells/species (row-wtd) | max/rms radius (median cell) | cells with max/rms>2 | key p1/median (median cell) | S1e B/row | viol S2 | viol S3 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| A1-fp | 0.9193 | 0.1529 | 134 | 833.0000 | 3742 | 0.6242 | 0.0778 | 102.3333 | 20.1111 | 5.8584 | 42.5876 | 1.5441 | 0.1350 | 0.8997 | 36.1574 | 0 | 0 |

| C16 | 0.9181 | 0.1551 | 131 | 844.0000 | 3816 | 0.6555 | 0.0869 | 103.3333 | 20.5333 | 5.8483 | 42.2821 | 1.6040 | 0.1500 | 0.8883 | 36.1574 | 0 | 0 |
| C16-in | 0.9183 | 0.1546 | 133 | 830.0000 | 3808 | 0.6527 | 0.0932 | 102.3333 | 20.2222 | 5.8605 | 43.0547 | 1.6124 | 0.1400 | 0.8912 | 36.1574 | 0 | 0 |
| Cmix | 0.9183 | 0.1548 | 131 | 839.0000 | 3780 | 0.6499 | 0.0835 | 103.3333 | 20.5333 | 5.8028 | 42.2319 | 1.5994 | 0.1650 | 0.8902 | 36.1574 | 0 | 0 |
| C0 | 0.9179 | 0.1553 | 133 | 825.0000 | 4097 | 0.6833 | 0.0832 | 98.0000 | 20.2000 | 6.1284 | 41.0515 | 1.4770 | 0.0550 | 0.9044 | 36.1574 | 0 | 0 |
| seed spread (A1-A3, max−min) | 0.0005 | 0.0009 | 133 | 31.5000 | 1740 | 0.0415 | 0.0057 | 3.3333 | 0.3111 | 0.0607 | 3.5218 | 0.0367 | 0.0150 | 0.0052 | 0.0000 | 0 | 0 |

Differences from the baseline with the seed rule:

C16:

| score | A1-fp | C16 | difference | relative | 2 × seed spread | a result? |
|---|---|---|---|---|---|---|
| Si_S1d_16 | 0.09547 | 0.09396 | -0.00151 | -1.58% | 0.00278 | no: under twice the seed spread |
| Si_S1b_0.06 | 36.63907 | 35.76860 | -0.87047 | -2.38% | 2.16459 | no: under twice the seed spread |
| Si_S1a_mean | 0.20636 | 0.20643 | +0.00007 | +0.03% | 0.00342 | no: under twice the seed spread |
| Sm_S1d_16 | 0.14595 | 0.14366 | -0.00229 | -1.57% | 0.00459 | no: under twice the seed spread |
| Sm_S1b_0.06 | 64.87390 | 63.76696 | -1.10694 | -1.71% | 3.30709 | no: under twice the seed spread |
| S23_item_0.01% | 0.42348 | 0.43065 | +0.00716 | +1.69% | 0.00712 | yes |
| S23_item_1% | 0.64883 | 0.65620 | +0.00737 | +1.14% | 0.00512 | yes |
| S23_species_1% | 0.80494 | 0.80962 | +0.00469 | +0.58% | 0.00975 | no: under twice the seed spread |
| S23_text_0.01% | 0.97619 | 0.97767 | +0.00148 | +0.15% | 0.00127 | yes |
| mean_key | 0.91929 | 0.91807 | -0.00122 | -0.13% | 0.00106 | yes |
| msq_resid | 0.15287 | 0.15509 | +0.00222 | +1.45% | 0.00177 | yes |
| S4a_mean_text | 0.58551 | 0.57905 | -0.00646 | -1.10% | 0.00310 | yes |
| S4a_mean_item | 0.87565 | 0.87004 | -0.00561 | -0.64% | 0.00379 | yes |

C16-in:

| score | A1-fp | C16-in | difference | relative | 2 × seed spread | a result? |
|---|---|---|---|---|---|---|
| Si_S1d_16 | 0.09547 | 0.09439 | -0.00108 | -1.13% | 0.00278 | no: under twice the seed spread |
| Si_S1b_0.06 | 36.63907 | 36.01382 | -0.62525 | -1.71% | 2.16459 | no: under twice the seed spread |
| Si_S1a_mean | 0.20636 | 0.20663 | +0.00027 | +0.13% | 0.00342 | no: under twice the seed spread |
| Sm_S1d_16 | 0.14595 | 0.14431 | -0.00164 | -1.12% | 0.00459 | no: under twice the seed spread |
| Sm_S1b_0.06 | 64.87390 | 64.09903 | -0.77487 | -1.19% | 3.30709 | no: under twice the seed spread |
| S23_item_0.01% | 0.42348 | 0.42906 | +0.00558 | +1.32% | 0.00712 | no: under twice the seed spread |
| S23_item_1% | 0.64883 | 0.65488 | +0.00605 | +0.93% | 0.00512 | yes |
| S23_species_1% | 0.80494 | 0.80901 | +0.00407 | +0.51% | 0.00975 | no: under twice the seed spread |
| S23_text_0.01% | 0.97619 | 0.97772 | +0.00152 | +0.16% | 0.00127 | yes |
| mean_key | 0.91929 | 0.91833 | -0.00096 | -0.10% | 0.00106 | no: under twice the seed spread |
| msq_resid | 0.15287 | 0.15462 | +0.00175 | +1.15% | 0.00177 | no: under twice the seed spread |
| S4a_mean_text | 0.58551 | 0.58033 | -0.00518 | -0.88% | 0.00310 | yes |
| S4a_mean_item | 0.87565 | 0.87069 | -0.00496 | -0.57% | 0.00379 | yes |

Cmix:

| score | A1-fp | Cmix | difference | relative | 2 × seed spread | a result? |
|---|---|---|---|---|---|---|
| Si_S1d_16 | 0.09547 | 0.09398 | -0.00149 | -1.56% | 0.00278 | no: under twice the seed spread |
| Si_S1b_0.06 | 36.63907 | 35.78086 | -0.85821 | -2.34% | 2.16459 | no: under twice the seed spread |
| Si_S1a_mean | 0.20636 | 0.20618 | -0.00018 | -0.09% | 0.00342 | no: under twice the seed spread |
| Sm_S1d_16 | 0.14595 | 0.14369 | -0.00226 | -1.55% | 0.00459 | no: under twice the seed spread |
| Sm_S1b_0.06 | 64.87390 | 63.78857 | -1.08533 | -1.67% | 3.30709 | no: under twice the seed spread |
| S23_item_0.01% | 0.42348 | 0.42959 | +0.00611 | +1.44% | 0.00712 | no: under twice the seed spread |
| S23_item_1% | 0.64883 | 0.65506 | +0.00623 | +0.96% | 0.00512 | yes |
| S23_species_1% | 0.80494 | 0.80883 | +0.00389 | +0.48% | 0.00975 | no: under twice the seed spread |
| S23_text_0.01% | 0.97619 | 0.97745 | +0.00125 | +0.13% | 0.00127 | no: under twice the seed spread |
| mean_key | 0.91929 | 0.91826 | -0.00104 | -0.11% | 0.00106 | no: under twice the seed spread |
| msq_resid | 0.15287 | 0.15476 | +0.00189 | +1.24% | 0.00177 | yes |
| S4a_mean_text | 0.58551 | 0.58053 | -0.00499 | -0.85% | 0.00310 | yes |
| S4a_mean_item | 0.87565 | 0.87072 | -0.00493 | -0.56% | 0.00379 | yes |

C0:

| score | A1-fp | C0 | difference | relative | 2 × seed spread | a result? |
|---|---|---|---|---|---|---|
| Si_S1d_16 | 0.09547 | 0.09456 | -0.00091 | -0.95% | 0.00278 | no: under twice the seed spread |
| Si_S1b_0.06 | 36.63907 | 36.04602 | -0.59304 | -1.62% | 2.16459 | no: under twice the seed spread |
| Si_S1a_mean | 0.20636 | 0.20360 | -0.00276 | -1.34% | 0.00342 | no: under twice the seed spread |
| Sm_S1d_16 | 0.14595 | 0.14455 | -0.00140 | -0.96% | 0.00459 | no: under twice the seed spread |
| Sm_S1b_0.06 | 64.87390 | 64.11563 | -0.75827 | -1.17% | 3.30709 | no: under twice the seed spread |
| S23_item_0.01% | 0.42348 | 0.42679 | +0.00331 | +0.78% | 0.00712 | no: under twice the seed spread |
| S23_item_1% | 0.64883 | 0.64916 | +0.00033 | +0.05% | 0.00512 | no: under twice the seed spread |
| S23_species_1% | 0.80494 | 0.80773 | +0.00279 | +0.35% | 0.00975 | no: under twice the seed spread |
| S23_text_0.01% | 0.97619 | 0.97677 | +0.00058 | +0.06% | 0.00127 | no: under twice the seed spread |
| mean_key | 0.91929 | 0.91787 | -0.00142 | -0.15% | 0.00106 | yes |
| msq_resid | 0.15287 | 0.15533 | +0.00246 | +1.61% | 0.00177 | yes |
| S4a_mean_text | 0.58551 | 0.58225 | -0.00327 | -0.56% | 0.00310 | yes |
| S4a_mean_item | 0.87565 | 0.88433 | +0.00868 | +0.99% | 0.00379 | yes |


Net figure, Σ_Q = Si: (S2+3) × (S1b at 0.06), ratio to A1-fp (below 1 is better)

| partition | text 0.01% | text 0.1% | text 1% | species 0.01% | species 0.1% | species 1% | item 0.01% | item 0.1% | item 1% |
|---|---|---|---|---|---|---|---|---|---|
| C16 | 0.9777 | 0.9768 | 0.9763 | 0.9829 | 0.9833 | 0.9819 | 0.9928 | 0.9903 | 0.9873 |
| C16-in | 0.9845 | 0.9835 | 0.9830 | 0.9883 | 0.9888 | 0.9879 | 0.9959 | 0.9942 | 0.9921 |
| Cmix | 0.9778 | 0.9770 | 0.9766 | 0.9824 | 0.9826 | 0.9813 | 0.9907 | 0.9886 | 0.9859 |
| C0 | 0.9844 | 0.9838 | 0.9838 | 0.9885 | 0.9885 | 0.9872 | 0.9915 | 0.9887 | 0.9843 |

Net figure, Σ_Q = Sm: (S2+3) × (S1b at 0.06), ratio to A1-fp (below 1 is better)

| partition | text 0.01% | text 0.1% | text 1% | species 0.01% | species 0.1% | species 1% | item 0.01% | item 0.1% | item 1% |
|---|---|---|---|---|---|---|---|---|---|
| C16 | 0.9844 | 0.9835 | 0.9830 | 0.9897 | 0.9901 | 0.9887 | 0.9996 | 0.9971 | 0.9941 |
| C16-in | 0.9896 | 0.9886 | 0.9881 | 0.9935 | 0.9940 | 0.9931 | 1.0011 | 0.9994 | 0.9973 |
| Cmix | 0.9845 | 0.9837 | 0.9833 | 0.9891 | 0.9893 | 0.9880 | 0.9974 | 0.9954 | 0.9927 |
| C0 | 0.9889 | 0.9883 | 0.9883 | 0.9930 | 0.9931 | 0.9917 | 0.9960 | 0.9932 | 0.9888 |

**Against the predictions and the drop rules.**

- C16. No size was predicted; Archer and Leen's figure would be S1d 12 to 19% narrower. Measured: S1d at 16 B 1.6%
  narrower than the baseline (0.7% narrower than its start, B16-in), S1b at 0.06 2.4% fewer bytes (1.0% from
  B16-in), all under twice the seed spread. Drop rule: first step under 5% of rows: **does not fire** (9.96%); S1b at
  0.06 gains under 3% at the stopping point: **fires** (2.4% against the baseline, 1.0% against its start); the net
  figure does not fall for both item and text queries: does not fire (it falls for both, by 0.7 to 2.4% with Si and
  0.04 to 1.7% with Sm, item queries least). **C16 is dropped** by its second clause.
- C16-in against C16 (reported only). From the common start B16-in, C16-in gains 0.26% of S1d at 16 B and 0.30% of
  S1b at 0.06, against C16's 0.72% and 0.98%: the within-group flattening (P1.4) is about a third of C16's gain and
  the change of basis (rows moving between groups, 3.6% of them) about two thirds.
- C0 against C16. Predicted: C0 gains more in S1a and less in S1d and S1b. Measured: S1a 1.34% narrower for C0 against
  +0.03% for C16; S1d at 16 B 0.95% against 1.58%; S1b at 0.06 1.62% against 2.38%: the direction predicted on every
  column. The drop condition ("C0 does as well as C16 in S1b") does not hold on the measured values (36.05 B against
  35.77 B), but the gap between them, 0.28 B, is far under twice the seed spread, so the two are not distinguished
  by this experiment. C0 is also the only rule that lowers S2 for item queries (by 1.3 to 1.5%; over twice the seed
  spread at 0.01% and 1%), and the only one that lowers the 10-NN own-cell share (0.698 to 0.646, over twice the seed
  spread); its S4b for item queries is 26% lower, but under twice that column's seed spread.
- Cmix is indistinguishable from C16 on every score (within 0.05% on coding).

**Reading.** Moving rows to the cell where their idealised 16 B interval is narrowest moves a fifth of the rows and
descends as proved (no rise in 60 passes), but on held-out rows it buys 1.7% of mean square width (about 0.7% of
mean width and 1% of bytes) beyond the cell-level regrouping, and 1.6% of S1d and 2.4% of S1b beyond the baseline:
an order of magnitude below the published figure for image blocks, and under the seed noise. It pays for that in
the other uses, by more than twice the seed spread on several: the item queries' S2+3 rises 1.1 to 1.7%, the mean
square residual 1.5%, and the share of score variance the cells explain falls 1.1% for text queries; the
approximate order visits 31% more rows for item queries (S4b 0.031 to 0.040), but twice that column's seed spread
is 0.012, so that is not a result. Within one cell of 750 rows the rows of this corpus do not split into kinds whose residuals want
different bases strongly enough to matter at this K.

## Item 6. Balanced k-means: D (P4.1)

`itemD.py A1-fp`, log `work/itemD.log`. From A1-fp's centres, 12 passes on fit rows: rows in decreasing order of
(best − second-best inner product with the unit centre directions), each to its best centre holding fewer than
1.1·149,922/200 = 825 fit rows; centres to the fit-row means. Then the held-out rows by the same greedy rule with
cap 1.1·49,978/200 (91.3% got their first choice), and the 100 item-query rows to their nearest centre; groups by the
build's rule (seed 1). The fit rows moved 27.6% in the first pass and still 5.4% in the twelfth (the spec fixes 12
passes, so D is not at a fixed point).


Coding, Σ_Q = Si, held-out rows (mean over 49,978 rows)

| partition | S1d 16B | S1b w*=.06 (B) | S1d 4B | S1d 8B | S1d 32B | S1b .03 | S1b .12 | S1a mean (sec.) | S1a rms (sec.) |
|---|---|---|---|---|---|---|---|---|---|
| A1-fp | 0.09547 | 36.639 | 0.14690 | 0.12318 | 0.06631 | 81.836 | 10.420 | 0.20636 | 0.22035 |
| D | 0.09583 | 36.725 | 0.14793 | 0.12386 | 0.06646 | 81.924 | 10.496 | 0.20924 | 0.22339 |
| seed spread (A1-A3, max−min) | 0.00139 | 1.082 | 0.00177 | 0.00162 | 0.00111 | 1.791 | 0.46924 | 0.00171 | 0.00225 |
| ratio to A1-fp: ||||||||||
| D / A1-fp | 1.0038 | 1.0023 | 1.0070 | 1.0055 | 1.0023 | 1.0011 | 1.0073 | 1.0139 | 1.0138 |

Coding, Σ_Q = Sm, held-out rows (mean over 49,978 rows)

| partition | S1d 16B | S1b w*=.06 (B) | S1d 4B | S1d 8B | S1d 32B | S1b .03 | S1b .12 | S1a mean (sec.) | S1a rms (sec.) |
|---|---|---|---|---|---|---|---|---|---|
| A1-fp | 0.14595 | 64.874 | 0.21483 | 0.18383 | 0.10395 | 122.774 | 25.021 | 0.28638 | 0.30591 |
| D | 0.14645 | 64.963 | 0.21618 | 0.18475 | 0.10416 | 122.849 | 25.090 | 0.28981 | 0.30946 |
| seed spread (A1-A3, max−min) | 0.00230 | 1.654 | 0.00285 | 0.00264 | 0.00186 | 2.363 | 0.92272 | 0.00285 | 0.00402 |
| ratio to A1-fp: ||||||||||
| D / A1-fp | 1.0034 | 1.0014 | 1.0063 | 1.0050 | 1.0020 | 1.0006 | 1.0027 | 1.0120 | 1.0116 |

S2+3 (share of valid rows left by both cut and range), mean over queries

| partition | text 0.01% | text 0.1% | text 1% | species 0.01% | species 0.1% | species 1% | item 0.01% | item 0.1% | item 1% |
|---|---|---|---|---|---|---|---|---|---|
| A1-fp | 0.9762 | 0.9935 | 0.9993 | 0.6708 | 0.7272 | 0.8049 | 0.4235 | 0.5106 | 0.6488 |
| D | 0.9764 | 0.9933 | 0.9992 | 0.6790 | 0.7369 | 0.8138 | 0.4336 | 0.5209 | 0.6569 |
| seed spread (A1-A3, max−min) | 0.0006 | 0.0004 | 0.0001 | 0.0048 | 0.0053 | 0.0049 | 0.0036 | 0.0037 | 0.0026 |
| ratio to A1-fp: ||||||||||
| D / A1-fp | 1.0002 | 0.9997 | 0.9999 | 1.0122 | 1.0134 | 1.0110 | 1.0238 | 1.0201 | 1.0125 |

S2 (cut alone)

| partition | text 0.01% | text 0.1% | text 1% | species 0.01% | species 0.1% | species 1% | item 0.01% | item 0.1% | item 1% |
|---|---|---|---|---|---|---|---|---|---|
| A1-fp | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.8370 | 0.8475 | 0.8668 |
| D | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 0.8374 | 0.8495 | 0.8657 |
| seed spread (A1-A3, max−min) | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.0000 | 0.0056 | 0.0052 | 0.0032 |
| ratio to A1-fp: ||||||||||
| D / A1-fp | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0000 | 1.0005 | 1.0023 | 0.9987 |

S3 (sorted range alone)

| partition | text 0.01% | text 0.1% | text 1% | species 0.01% | species 0.1% | species 1% | item 0.01% | item 0.1% | item 1% |
|---|---|---|---|---|---|---|---|---|---|
| A1-fp | 0.9762 | 0.9935 | 0.9993 | 0.6708 | 0.7272 | 0.8049 | 0.4354 | 0.5259 | 0.6682 |
| D | 0.9764 | 0.9933 | 0.9992 | 0.6790 | 0.7369 | 0.8138 | 0.4462 | 0.5368 | 0.6776 |
| seed spread (A1-A3, max−min) | 0.0006 | 0.0004 | 0.0001 | 0.0048 | 0.0053 | 0.0049 | 0.0038 | 0.0030 | 0.0021 |
| ratio to A1-fp: ||||||||||
| D / A1-fp | 1.0002 | 0.9997 | 0.9999 | 1.0122 | 1.0134 | 1.0110 | 1.0249 | 1.0207 | 1.0141 |

S2 at 1%: mean 2ρ (net) and mean β(E⁺−E⁻) (extent) over cells left

| partition | 2ρ text | 2ρ species | 2ρ item | extent text | extent species | extent item |
|---|---|---|---|---|---|---|
| A1-fp | 1.4893 | 1.1581 | 0.7191 | 0.0851 | 0.1171 | 0.1607 |
| D | 1.4953 | 1.1588 | 0.7200 | 0.0861 | 0.1168 | 0.1593 |
| seed spread (A1-A3, max−min) | 0.0023 | 0.0113 | 0.0031 | 0.0033 | 0.0034 | 0.0017 |

Order: S4a (mean share; ratio of means), S4b (share of rows visited to cover 9 of the 10 best), own-cell share of 10-NN

| partition | S4a text | S4a species | S4a item | S4a ratio text | S4a ratio species | S4a ratio item | S4b text | S4b species | S4b item | 10-NN own cell |
|---|---|---|---|---|---|---|---|---|---|---|
| A1-fp | 0.5855 | 0.8605 | 0.8756 | 0.6210 | 0.8778 | 0.9227 | 0.0391 | 0.0213 | 0.0307 | 0.6976 |
| D | 0.5790 | 0.8572 | 0.8700 | 0.6152 | 0.8751 | 0.9169 | 0.0473 | 0.0218 | 0.0468 | 0.6287 |
| seed spread (A1-A3, max−min) | 0.0015 | 0.0014 | 0.0019 | 0.0011 | 0.0009 | 0.0010 | 0.0055 | 0.0014 | 0.0060 | 0.0212 |
| ratio to A1-fp: |||||||||||
| D / A1-fp | 0.9889 | 0.9962 | 0.9935 | 0.9906 | 0.9969 | 0.9937 | 1.2113 | 1.0225 | 1.5250 | 0.9012 |

Layout and shape (all valid rows)

| partition | mean key | msq resid | n_c min | median | max | CV | rows in cells <500 | cells/family | cells/genus | cells/species | cells/species (row-wtd) | max/rms radius (median cell) | cells with max/rms>2 | key p1/median (median cell) | S1e B/row | viol S2 | viol S3 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| A1-fp | 0.9193 | 0.1529 | 134 | 833.0000 | 3742 | 0.6242 | 0.0778 | 102.3333 | 20.1111 | 5.8584 | 42.5876 | 1.5441 | 0.1350 | 0.8997 | 36.1574 | 0 | 0 |
| D | 0.9176 | 0.1559 | 133 | 1100.0000 | 1100 | 0.2031 | 0.0180 | 102.6667 | 19.9778 | 6.2073 | 41.8874 | 1.6297 | 0.1700 | 0.8993 | 36.1574 | 0 | 0 |
| seed spread (A1-A3, max−min) | 0.0005 | 0.0009 | 133 | 31.5000 | 1740 | 0.0415 | 0.0057 | 3.3333 | 0.3111 | 0.0607 | 3.5218 | 0.0367 | 0.0150 | 0.0052 | 0.0000 | 0 | 0 |


Net figure, Σ_Q = Si: (S2+3) × (S1b at 0.06), ratio to A1-fp (below 1 is better)

| partition | text 0.01% | text 0.1% | text 1% | species 0.01% | species 0.1% | species 1% | item 0.01% | item 0.1% | item 1% |
|---|---|---|---|---|---|---|---|---|---|
| D | 1.0026 | 1.0021 | 1.0022 | 1.0146 | 1.0157 | 1.0133 | 1.0262 | 1.0225 | 1.0149 |

Net figure, Σ_Q = Sm: (S2+3) × (S1b at 0.06), ratio to A1-fp (below 1 is better)

| partition | text 0.01% | text 0.1% | text 1% | species 0.01% | species 0.1% | species 1% | item 0.01% | item 0.1% | item 1% |
|---|---|---|---|---|---|---|---|---|---|
| D | 1.0016 | 1.0011 | 1.0012 | 1.0136 | 1.0147 | 1.0123 | 1.0252 | 1.0215 | 1.0139 |

| score | A1-fp | D | difference | relative | 2 × seed spread | a result? |
|---|---|---|---|---|---|---|
| Si_S1d_16 | 0.09547 | 0.09583 | +0.00036 | +0.38% | 0.00278 | no: under twice the seed spread |
| Si_S1b_0.06 | 36.63907 | 36.72481 | +0.08575 | +0.23% | 2.16459 | no: under twice the seed spread |
| Si_S1a_mean | 0.20636 | 0.20924 | +0.00287 | +1.39% | 0.00342 | no: under twice the seed spread |
| Sm_S1d_16 | 0.14595 | 0.14645 | +0.00050 | +0.34% | 0.00459 | no: under twice the seed spread |
| Sm_S1b_0.06 | 64.87390 | 64.96260 | +0.08869 | +0.14% | 3.30709 | no: under twice the seed spread |
| S23_item_0.01% | 0.42348 | 0.43355 | +0.01007 | +2.38% | 0.00712 | yes |
| S23_item_0.1% | 0.51063 | 0.52088 | +0.01025 | +2.01% | 0.00748 | yes |
| S23_item_1% | 0.64883 | 0.65694 | +0.00811 | +1.25% | 0.00512 | yes |
| S23_species_0.01% | 0.67081 | 0.67901 | +0.00819 | +1.22% | 0.00958 | no: under twice the seed spread |
| S23_species_1% | 0.80494 | 0.81376 | +0.00882 | +1.10% | 0.00975 | no: under twice the seed spread |
| S23_text_0.01% | 0.97619 | 0.97641 | +0.00021 | +0.02% | 0.00127 | no: under twice the seed spread |
| S3_item_1% | 0.66820 | 0.67759 | +0.00939 | +1.41% | 0.00426 | yes |
| mean_key | 0.91929 | 0.91759 | -0.00170 | -0.19% | 0.00106 | yes |
| msq_resid | 0.15287 | 0.15595 | +0.00308 | +2.01% | 0.00177 | yes |
| S4a_mean_text | 0.58551 | 0.57900 | -0.00651 | -1.11% | 0.00310 | yes |
| S4a_mean_item | 0.87565 | 0.86997 | -0.00568 | -0.65% | 0.00379 | yes |
| S4b_item | 0.03071 | 0.04683 | +0.01612 | +52.50% | 0.01190 | yes |
| S4b_nn_owncell | 0.69765 | 0.62870 | -0.06895 | -9.88% | 0.04230 | yes |
| S5_cv | 0.62423 | 0.20312 | -0.42111 | -67.46% | 0.08301 | yes |
| S5_share_under500 | 0.07783 | 0.01799 | -0.05984 | -76.89% | 0.01145 | yes |

**Against the predictions and the drop rule.** Predicted: S1b within 1% of the baseline (measured +0.23% at
w* = 0.06: inside); S3 up by 0 to 3% (measured +1.1 to +2.5% for species and item queries, flat for text: inside);
no cell under 500 rows (measured: **10 cells under 500, the smallest 133 rows**: the prediction fails, because a cap
from above does nothing for cells that are small to start with; 1.8% of rows are in cells under 500, against 7.8% for
the baseline). Drop rule: S1b rises by more than 1% (no: 0.23%), or S2+3 by more than 3% (no: at most 2.4%, item
queries at 0.01%): **does not fire**.

**Reading.** Capping cells at 1.1 times the mean cuts the coefficient of variation of cell sizes from 0.62 to 0.20
and leaves the coding scores where they were (all within 0.4%, under the seed spread, except S1a, +1.4%, also under
it), but costs the decisions and the order: S2+3 for item queries rises 1.3 to 2.4% (over twice the seed spread),
the explained share of score variance falls 1.1% for text queries, the approximate order visits 53% more rows for item
queries to cover 9 of the 10 best (0.031 to 0.047, over twice the seed spread), and the share of a row's 10 nearest
rows in its own cell falls from 0.70 to 0.63. The net figure rises 0.2% for text and 1.5 to 2.6% for item queries.
The layout gain is real and the price is a few percent of the decisions; it is reported as run, not dropped.

## Item 7. The sweep for P2 (one group, only K moves)

`sweep.py`, logs `work/sweep_*.log`, results `work/sweep/*.json`, tables `sweep_report.py`. The build's steps at
seed 1 (sample of 200 rows a centre, all pool rows when that is more), one group, each continued by Lloyd on the
pool's rows to under 0.1% moved (29 to 73 passes). Pool: all 149,922 fit rows (K = 50 to 800), or a random 37,500 of
them (seed 7; K = 50, 100, 200). Centres, W_P and the single basis come from the pool's rows; coding on the 49,978
held-out rows; S2+3 and S4a on every valid row placed at its nearest stored centre. The 150k, K = 200 run reproduces
A1-fp with one group exactly (S1b at 0.06: 47.08 B in both), as it should, since it is the same partition.

| series | K | pool rows | Lloyd passes to < 0.1% | smallest cell (pool rows) | mean sq. residual (pool) | S1d 4 B | S1d 16 B | S1d 32 B | S1b .03 | S1b .06 | S1b .12 | S1d 16 B (Sm) | S1b .06 (Sm) | S2+3 text .01% | S2+3 species 1% | S2+3 item .01% | S2+3 item 1% | S4a text | S4a item | violations |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 150k | 50 | 149922 | 63 | 336 | 0.17970 | 0.18274 | 0.12083 | 0.08426 | 102.55 | 50.20 | 16.82 | 0.18340 | 83.56 | 0.9863 | 0.8445 | 0.5146 | 0.7272 | 0.5051 | 0.8360 | 0 |
| 150k | 100 | 149922 | 68 | 214 | 0.16555 | 0.17562 | 0.11724 | 0.08213 | 100.82 | 48.75 | 15.83 | 0.17832 | 81.91 | 0.9803 | 0.8216 | 0.4696 | 0.6857 | 0.5502 | 0.8608 | 0 |
| 150k | 200 | 149922 | 53 | 106 | 0.15272 | 0.16881 | 0.11339 | 0.07976 | 98.79 | 47.08 | 14.72 | 0.17278 | 79.97 | 0.9762 | 0.8049 | 0.4235 | 0.6488 | 0.5855 | 0.8756 | 0 |
| 150k | 400 | 149922 | 73 | 1 | 0.14206 | 0.16347 | 0.11029 | 0.07780 | 97.03 | 45.68 | 13.86 | 0.16829 | 78.31 | 0.9703 | 0.7912 | 0.3896 | 0.6206 | 0.6174 | 0.8884 | 0 |
| 150k | 800 | 149922 | 33 | 12 | 0.13156 | 0.15837 | 0.10724 | 0.07582 | 95.18 | 44.23 | 12.96 | 0.16381 | 76.57 | 0.9667 | 0.7780 | 0.3551 | 0.5919 | 0.6458 | 0.9002 | 0 |
| 37.5k | 50 | 37500 | 52 | 88 | 0.18077 | 0.18333 | 0.12121 | 0.08454 | 102.92 | 50.43 | 16.93 | 0.18397 | 83.89 | 0.9855 | 0.8385 | 0.5175 | 0.7259 | 0.4995 | 0.8376 | 0 |
| 37.5k | 100 | 37500 | 35 | 89 | 0.16520 | 0.17594 | 0.11746 | 0.08233 | 101.15 | 48.92 | 15.89 | 0.17868 | 82.19 | 0.9803 | 0.8228 | 0.4692 | 0.6864 | 0.5508 | 0.8599 | 0 |
| 37.5k | 200 | 37500 | 29 | 20 | 0.15168 | 0.16984 | 0.11412 | 0.08028 | 99.45 | 47.52 | 14.95 | 0.17389 | 80.57 | 0.9769 | 0.8090 | 0.4272 | 0.6532 | 0.5865 | 0.8773 | 0 |

S1b against log2 K (bytes per doubling of K, least squares; negative is a fall):

| series | Ks | w* = 0.03 | w* = 0.06 | w* = 0.12 | S1d 16 B, relative change per doubling | S1b .06 with Sm |
|---|---|---|---|---|---|---|
| 150k | [50, 100, 200, 400, 800] | -1.85 | -1.50 | -0.97 | -2.95% | -1.76 |
| 37.5k | [50, 100, 200] | -1.74 | -1.45 | -0.99 | -2.97% | -1.66 |

Model U (a): slope of ln γᵢ in ln K, 150k series, least squares over K = [50, 100, 200, 400, 800]:

| coordinates i | mean slope | median slope | smallest | largest |
|---|---|---|---|---|
| 1 to 20 | -0.1774 | -0.1684 | -0.2240 | -0.1512 |
| 21 to 100 | -0.1127 | -0.1080 | -0.1722 | -0.0758 |
| 101 to 300 | -0.0599 | -0.0584 | -0.0894 | -0.0413 |
| 1 to 300 | -0.0818 | -0.0677 | -0.2240 | -0.0413 |
| 301 to 768 | -0.0346 | -0.0339 | -0.0493 | -0.0258 |

Ratio of mean slopes, i ≤ 20 over 100 < i ≤ 300: 2.96
Same at 37.5k over K = [50, 100, 200]: i ≤ 20 -0.2196, 100 < i ≤ 300 -0.0550, i ≤ 300 -0.0831
150k over the same K = [50, 100, 200]: i ≤ 20 -0.2043, 100 < i ≤ 300 -0.0522, i ≤ 300 -0.0793

Model U (b): ln γᵢ at equal K, 150k against 37.5k, against one doubling of K and the seed spread (mean over i ≤ 300 of |Δ ln γᵢ|):

| K | 150k − 37.5k, mean Δ ln γ (signed) | mean |Δ| | one doubling at 150k, mean |ln γ(2K) − ln γ(K)| | seed spread at K = 200 (largest pairwise mean |Δ| among A1–A3) |
|---|---|---|---|---|
| 50 | +0.0032 | 0.0102 | 0.0501 | 0.0073 |
| 100 | +0.0078 | 0.0111 | 0.0599 | 0.0073 |
| 200 | +0.0085 | 0.0109 | 0.0545 | 0.0073 |

Assumption (d): a cell's own dual spectrum (row-weighted mean over cells, sorted) over the group's (W_P), at i:

| series | K | i=1 | i=2 | i=5 | i=10 | i=20 | i=50 | i=100 | i=200 | i=300 | i=500 | Σ own / Σ group |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 150k | 50 | 1.128 | 1.585 | 1.030 | 0.881 | 0.696 | 0.576 | 0.454 | 0.432 | 0.446 | 0.373 | 1.000 |
| 150k | 100 | 1.121 | 1.475 | 1.063 | 0.942 | 0.767 | 0.554 | 0.400 | 0.358 | 0.353 | 0.245 | 1.000 |
| 150k | 200 | 1.120 | 1.471 | 1.033 | 1.015 | 0.825 | 0.539 | 0.367 | 0.286 | 0.251 | 0.121 | 1.000 |
| 150k | 400 | 1.124 | 1.401 | 1.046 | 1.045 | 0.849 | 0.514 | 0.302 | 0.185 | 0.122 | 0.028 | 1.000 |
| 150k | 800 | 1.126 | 1.426 | 1.086 | 1.077 | 0.852 | 0.454 | 0.202 | 0.054 | 0.016 | 0.001 | 1.000 |
| 37.5k | 50 | 1.136 | 1.614 | 1.086 | 0.855 | 0.695 | 0.568 | 0.419 | 0.345 | 0.305 | 0.169 | 1.000 |
| 37.5k | 100 | 1.139 | 1.465 | 1.044 | 0.927 | 0.766 | 0.517 | 0.327 | 0.197 | 0.131 | 0.025 | 1.000 |
| 37.5k | 200 | 1.139 | 1.520 | 1.036 | 1.020 | 0.792 | 0.455 | 0.216 | 0.070 | 0.024 | 0.002 | 1.000 |

Bytes per entering item, (S2+3) × (S1b at 0.06), Si:

| series | K | text 0.01% | text 0.1% | text 1% | species 0.01% | species 0.1% | species 1% | item 0.01% | item 0.1% | item 1% |
|---|---|---|---|---|---|---|---|---|---|---|
| 150k | 50 | 49.51 | 50.05 | 50.19 | 36.74 | 39.25 | 42.39 | 25.83 | 30.18 | 36.50 |
| 150k | 100 | 47.78 | 48.50 | 48.73 | 34.27 | 36.71 | 40.05 | 22.89 | 27.10 | 33.42 |
| 150k | 200 | 45.96 | 46.78 | 47.05 | 31.59 | 34.24 | 37.90 | 19.94 | 24.04 | 30.55 |
| 150k | 400 | 44.33 | 45.27 | 45.63 | 29.82 | 32.51 | 36.14 | 17.80 | 21.88 | 28.35 |
| 150k | 800 | 42.76 | 43.77 | 44.17 | 27.73 | 30.59 | 34.41 | 15.71 | 19.66 | 26.18 |
| 37.5k | 50 | 49.69 | 50.26 | 50.41 | 36.52 | 38.98 | 42.28 | 26.10 | 30.43 | 36.60 |
| 37.5k | 100 | 47.95 | 48.67 | 48.89 | 34.37 | 36.88 | 40.25 | 22.95 | 27.20 | 33.58 |
| 37.5k | 200 | 46.42 | 47.22 | 47.49 | 32.20 | 34.87 | 38.45 | 20.30 | 24.48 | 31.04 |

**Against the predictions and the drop rules.**

- Model U (a): predicted that ln γᵢ falls by the same amount for every i, with a slope in ln K of −0.08 to −0.14 over
  the first 300 coordinates. Measured: the mean slope over i ≤ 300 is −0.082 (inside, at the edge), but it is not
  uniform: −0.177 over i ≤ 20, −0.113 over 21 to 100, −0.060 over 101 to 300, −0.035 beyond. Drop rule: the slope over
  i ≤ 20 more than twice that over 100 < i ≤ 300: **fires** (ratio 2.96 over K = 50 to 800; 3.9 and 4.0 over K = 50
  to 200 at 150k and 37.5k). So it is model M, and P2.2's table overstates the fall at depth.
- Model U (b): predicted that at equal K the spectra at 150,000 and 37,500 rows agree within the seed spread.
  Measured: mean |Δ ln γᵢ| over i ≤ 300 is 0.010 to 0.011 at K = 50, 100, 200, against 0.0073 between seeds A1 to A3
  at K = 200 (those are not fixed points) and 0.050 to 0.060 for one doubling of K. The prediction narrowly fails
  (1.4 to 1.5 times the seed spread; the 150k spectra are higher by 0.3 to 0.9% on average), but the drop rule
  (differ by more than one doubling of K) **does not fire**: at equal K a cell's covariance depends on K and hardly on N.
- P2.2: predicted S1b falls 0.8 to 2.4 B per doubling of K, more at the smaller w*. Measured at 150k: 1.85, 1.50, 0.97
  B per doubling at w* = 0.03, 0.06, 0.12 (37.5k: 1.74, 1.45, 0.99): inside, and more at the smaller w*. Drop rule
  (outside 0.4 to 5 B at every w*): **does not fire**. S1d at 16 B falls 3.0% per doubling.
- Assumption (d) (a group's spectrum stands for a cell's): not a drop rule, reported. A cell's own sorted spectrum is
  1.1 to 1.6 times the pooled one in the first 5 coordinates and falls below it with depth: at K = 50 (3,000 rows a
  cell, so no rank limit before i = 3,000) it is 0.45 of the pooled at i = 100 and 0.45 at i = 300. Past i ≈ rows a
  cell the ratio is a rank artefact (K = 800: about 190 rows a cell, ratio 0.05 at i = 200). The trace agrees by
  construction (ratio 1.000).

**Reading.** On this corpus a doubling of K buys 1.5 B at w* = 0.06 (3% of S1d at 16 B) and costs a doubling of the
centres; the fall is concentrated in the leading coordinates (slope −0.18 against −0.06 at depth), so model M, not U,
is the right one, and the cell's covariance depends on K, not on N, to within 1% of ln γ. The decisions improve
faster: S2+3 for item queries at 0.01% falls from 0.515 to 0.355 across K = 50 to 800 (about 9% a doubling), so the
bytes per entering item fall from 25.8 to 15.7 there; for text prompts S2+3 stays at 0.97 to 0.99 and the bytes per
entering item fall only with S1b. At K = 400 and 800 the fixed point has cells of 1 and 12 pool rows.

## Other predictions in section 5, read off the tables above

| claim | prediction | measured (A1-fp unless named) | drop rule |
|---|---|---|---|
| P1.6, the fixed point | no size; toy: S0 11.7%, mean key +4.7% | S0 4.4% (A1), mean key +0.11% at the fixed point | reported, not dropped; S0 is over 2%, so the remark has some weight, but what it buys is under the seed spread |
| P4.7, stragglers | largest radius of a cell 1.2 to 1.5 times its rms | median over cells 1.54 (A1 to A3: 1.52 to 1.56); 13.5% of cells above 2 | above 2 in most cells: does not fire |
| P4.2, the neighbour-graph headroom | not predicted | 69.8% of a row's 10 nearest rows are in its own cell (A1 to A3: 68.1 to 70.2%; 2,000 sample rows) | reported only: under 80%, so a graph partition is not ruled out by this test |

The key's 1st percentile over its median is 0.90 in the median cell (A1-fp), so a cell's lowest keys sit about 10%
below its typical key.

## Summary table: every partition at K = 200

Coding on held-out rows (S1d and S1b first, S1a secondary), and the main decision and order scores on all valid rows.
"n/a" where the cells are A1-fp's and so the score is A1-fp's.


Coding, Σ_Q = Si, held-out rows (mean over 49,978 rows)

| partition | S1d 16B | S1b w*=.06 (B) | S1d 4B | S1d 8B | S1d 32B | S1b .03 | S1b .12 | S1a mean (sec.) | S1a rms (sec.) |
|---|---|---|---|---|---|---|---|---|---|
| A1 | 0.09635 | 37.366 | 0.14828 | 0.12432 | 0.06697 | 83.104 | 10.741 | 0.20834 | 0.22271 |
| A2 | 0.09596 | 36.868 | 0.14772 | 0.12387 | 0.06661 | 82.208 | 10.515 | 0.20744 | 0.22124 |
| A3 | 0.09496 | 36.284 | 0.14651 | 0.12271 | 0.06586 | 81.313 | 10.271 | 0.20663 | 0.22046 |
| A0 | 0.09557 | 36.635 | 0.14717 | 0.12338 | 0.06635 | 82.052 | 10.338 | 0.20699 | 0.22052 |
| A1-fp | 0.09547 | 36.639 | 0.14690 | 0.12318 | 0.06631 | 81.836 | 10.420 | 0.20636 | 0.22035 |
| A1-fp-rand | 0.10908 | 44.406 | 0.16395 | 0.13913 | 0.07635 | 94.573 | 13.497 | 0.22262 | 0.23585 |
| A1-fp-one | 0.11339 | 47.085 | 0.16881 | 0.14387 | 0.07976 | 98.788 | 14.723 | 0.22719 | 0.24071 |
| A1-fp-cell | 0.08560 | 31.292 | 0.13381 | 0.11123 | 0.05931 | 73.826 | 7.988 | 0.19362 | 0.20678 |
| B0-in | 0.09514 | 36.446 | 0.14652 | 0.12282 | 0.06606 | 81.598 | 10.308 | 0.20600 | 0.21986 |
| B0-cf | 0.09512 | 36.432 | 0.14650 | 0.12280 | 0.06604 | 81.582 | 10.301 | 0.20599 | 0.21984 |
| B0-in-moves | 0.09409 | 35.826 | 0.14543 | 0.12172 | 0.06520 | 80.588 | 10.057 | 0.20496 | 0.21877 |
| B16-in | 0.09464 | 36.121 | 0.14604 | 0.12231 | 0.06564 | 81.148 | 10.165 | 0.20558 | 0.21938 |
| B16-cf | 0.09464 | 36.122 | 0.14605 | 0.12232 | 0.06564 | 81.153 | 10.166 | 0.20559 | 0.21939 |
| B16-in-moves | 0.09394 | 35.711 | 0.14531 | 0.12158 | 0.06505 | 80.419 | 10.005 | 0.20486 | 0.21865 |
| C16 | 0.09396 | 35.769 | 0.14531 | 0.12154 | 0.06515 | 80.643 | 9.981 | 0.20643 | 0.22018 |
| C16-in | 0.09439 | 36.014 | 0.14575 | 0.12200 | 0.06549 | 81.013 | 10.105 | 0.20663 | 0.22049 |
| Cmix | 0.09398 | 35.781 | 0.14525 | 0.12153 | 0.06517 | 80.669 | 9.983 | 0.20618 | 0.21989 |
| C0 | 0.09456 | 36.046 | 0.14611 | 0.12230 | 0.06553 | 81.016 | 10.134 | 0.20360 | 0.21708 |
| D | 0.09583 | 36.725 | 0.14793 | 0.12386 | 0.06646 | 81.924 | 10.496 | 0.20924 | 0.22339 |
| seed spread (A1-A3, max−min) | 0.00139 | 1.082 | 0.00177 | 0.00162 | 0.00111 | 1.791 | 0.46924 | 0.00171 | 0.00225 |
| ratio to A1-fp: ||||||||||
| A1 / A1-fp | 1.0092 | 1.0198 | 1.0095 | 1.0093 | 1.0099 | 1.0155 | 1.0307 | 1.0096 | 1.0107 |
| A2 / A1-fp | 1.0051 | 1.0063 | 1.0056 | 1.0056 | 1.0045 | 1.0045 | 1.0091 | 1.0052 | 1.0040 |
| A3 / A1-fp | 0.9947 | 0.9903 | 0.9974 | 0.9961 | 0.9932 | 0.9936 | 0.9857 | 1.0013 | 1.0005 |
| A0 / A1-fp | 1.0011 | 0.9999 | 1.0019 | 1.0016 | 1.0005 | 1.0026 | 0.9921 | 1.0031 | 1.0008 |
| A1-fp-rand / A1-fp | 1.1426 | 1.2120 | 1.1161 | 1.1295 | 1.1513 | 1.1556 | 1.2953 | 1.0788 | 1.0703 |
| A1-fp-one / A1-fp | 1.1878 | 1.2851 | 1.1492 | 1.1680 | 1.2027 | 1.2071 | 1.4130 | 1.1009 | 1.0924 |
| A1-fp-cell / A1-fp | 0.8966 | 0.8541 | 0.9109 | 0.9030 | 0.8944 | 0.9021 | 0.7666 | 0.9382 | 0.9384 |
| B0-in / A1-fp | 0.9965 | 0.9947 | 0.9974 | 0.9971 | 0.9962 | 0.9971 | 0.9893 | 0.9982 | 0.9978 |
| B0-cf / A1-fp | 0.9963 | 0.9943 | 0.9973 | 0.9969 | 0.9959 | 0.9969 | 0.9886 | 0.9982 | 0.9977 |
| B0-in-moves / A1-fp | 0.9856 | 0.9778 | 0.9900 | 0.9882 | 0.9833 | 0.9847 | 0.9651 | 0.9932 | 0.9928 |
| B16-in / A1-fp | 0.9913 | 0.9859 | 0.9942 | 0.9929 | 0.9899 | 0.9916 | 0.9755 | 0.9962 | 0.9956 |
| B16-cf / A1-fp | 0.9913 | 0.9859 | 0.9942 | 0.9930 | 0.9899 | 0.9916 | 0.9756 | 0.9963 | 0.9956 |
| B16-in-moves / A1-fp | 0.9840 | 0.9747 | 0.9892 | 0.9870 | 0.9810 | 0.9827 | 0.9601 | 0.9927 | 0.9923 |
| C16 / A1-fp | 0.9842 | 0.9762 | 0.9892 | 0.9867 | 0.9824 | 0.9854 | 0.9579 | 1.0003 | 0.9992 |
| C16-in / A1-fp | 0.9887 | 0.9829 | 0.9922 | 0.9904 | 0.9876 | 0.9899 | 0.9697 | 1.0013 | 1.0006 |
| Cmix / A1-fp | 0.9844 | 0.9766 | 0.9888 | 0.9866 | 0.9827 | 0.9857 | 0.9580 | 0.9991 | 0.9979 |
| C0 / A1-fp | 0.9905 | 0.9838 | 0.9947 | 0.9929 | 0.9883 | 0.9900 | 0.9726 | 0.9866 | 0.9851 |
| D / A1-fp | 1.0038 | 1.0023 | 1.0070 | 1.0055 | 1.0023 | 1.0011 | 1.0073 | 1.0139 | 1.0138 |

Coding, Σ_Q = Sm, held-out rows (mean over 49,978 rows)

| partition | S1d 16B | S1b w*=.06 (B) | S1d 4B | S1d 8B | S1d 32B | S1b .03 | S1b .12 | S1a mean (sec.) | S1a rms (sec.) |
|---|---|---|---|---|---|---|---|---|---|
| A1 | 0.14745 | 66.050 | 0.21704 | 0.18572 | 0.10510 | 124.513 | 25.638 | 0.28938 | 0.30985 |
| A2 | 0.14666 | 65.206 | 0.21597 | 0.18480 | 0.10440 | 123.211 | 25.192 | 0.28782 | 0.30708 |
| A3 | 0.14515 | 64.396 | 0.21419 | 0.18308 | 0.10324 | 122.150 | 24.715 | 0.28653 | 0.30583 |
| A0 | 0.14609 | 65.017 | 0.21520 | 0.18411 | 0.10399 | 123.173 | 24.936 | 0.28719 | 0.30607 |
| A1-fp | 0.14595 | 64.874 | 0.21483 | 0.18383 | 0.10395 | 122.774 | 25.021 | 0.28638 | 0.30591 |
| A1-fp-rand | 0.16621 | 76.207 | 0.23896 | 0.20690 | 0.11941 | 138.414 | 31.080 | 0.30896 | 0.32730 |
| A1-fp-one | 0.17278 | 79.972 | 0.24611 | 0.21397 | 0.12472 | 143.451 | 33.302 | 0.31561 | 0.33437 |
| A1-fp-cell | 0.13090 | 57.552 | 0.19554 | 0.16596 | 0.09303 | 113.348 | 20.536 | 0.26804 | 0.28640 |
| B0-in | 0.14545 | 64.648 | 0.21428 | 0.18330 | 0.10355 | 122.525 | 24.844 | 0.28586 | 0.30519 |
| B0-cf | 0.14542 | 64.631 | 0.21425 | 0.18327 | 0.10352 | 122.510 | 24.831 | 0.28584 | 0.30516 |
| B0-in-moves | 0.14383 | 63.751 | 0.21262 | 0.18162 | 0.10221 | 121.274 | 24.345 | 0.28428 | 0.30352 |
| B16-in | 0.14468 | 64.228 | 0.21354 | 0.18252 | 0.10290 | 122.022 | 24.565 | 0.28520 | 0.30442 |
| B16-cf | 0.14468 | 64.231 | 0.21355 | 0.18253 | 0.10290 | 122.030 | 24.565 | 0.28521 | 0.30444 |
| B16-in-moves | 0.14360 | 63.603 | 0.21244 | 0.18142 | 0.10197 | 121.078 | 24.248 | 0.28412 | 0.30331 |
| C16 | 0.14366 | 63.767 | 0.21244 | 0.18138 | 0.10213 | 121.446 | 24.264 | 0.28594 | 0.30506 |
| C16-in | 0.14431 | 64.099 | 0.21310 | 0.18206 | 0.10267 | 121.877 | 24.472 | 0.28630 | 0.30559 |
| Cmix | 0.14369 | 63.789 | 0.21238 | 0.18138 | 0.10217 | 121.478 | 24.273 | 0.28566 | 0.30474 |
| C0 | 0.14455 | 64.116 | 0.21365 | 0.18250 | 0.10273 | 121.858 | 24.505 | 0.28329 | 0.30213 |
| D | 0.14645 | 64.963 | 0.21618 | 0.18475 | 0.10416 | 122.849 | 25.090 | 0.28981 | 0.30946 |
| seed spread (A1-A3, max−min) | 0.00230 | 1.654 | 0.00285 | 0.00264 | 0.00186 | 2.363 | 0.92272 | 0.00285 | 0.00402 |
| ratio to A1-fp: ||||||||||
| A1 / A1-fp | 1.0103 | 1.0181 | 1.0103 | 1.0103 | 1.0111 | 1.0142 | 1.0246 | 1.0104 | 1.0129 |
| A2 / A1-fp | 1.0049 | 1.0051 | 1.0053 | 1.0053 | 1.0044 | 1.0036 | 1.0068 | 1.0050 | 1.0038 |
| A3 / A1-fp | 0.9945 | 0.9926 | 0.9970 | 0.9959 | 0.9932 | 0.9949 | 0.9878 | 1.0005 | 0.9998 |
| A0 / A1-fp | 1.0009 | 1.0022 | 1.0017 | 1.0015 | 1.0004 | 1.0033 | 0.9966 | 1.0028 | 1.0005 |
| A1-fp-rand / A1-fp | 1.1388 | 1.1747 | 1.1123 | 1.1254 | 1.1488 | 1.1274 | 1.2422 | 1.0788 | 1.0699 |
| A1-fp-one / A1-fp | 1.1838 | 1.2327 | 1.1456 | 1.1639 | 1.1998 | 1.1684 | 1.3310 | 1.1020 | 1.0930 |
| A1-fp-cell / A1-fp | 0.8969 | 0.8871 | 0.9102 | 0.9027 | 0.8950 | 0.9232 | 0.8207 | 0.9359 | 0.9362 |
| B0-in / A1-fp | 0.9966 | 0.9965 | 0.9974 | 0.9971 | 0.9962 | 0.9980 | 0.9929 | 0.9982 | 0.9977 |
| B0-cf / A1-fp | 0.9963 | 0.9963 | 0.9973 | 0.9969 | 0.9959 | 0.9979 | 0.9924 | 0.9981 | 0.9976 |
| B0-in-moves / A1-fp | 0.9855 | 0.9827 | 0.9897 | 0.9880 | 0.9833 | 0.9878 | 0.9730 | 0.9927 | 0.9922 |
| B16-in / A1-fp | 0.9913 | 0.9900 | 0.9940 | 0.9929 | 0.9899 | 0.9939 | 0.9818 | 0.9959 | 0.9951 |
| B16-cf / A1-fp | 0.9913 | 0.9901 | 0.9941 | 0.9929 | 0.9900 | 0.9939 | 0.9818 | 0.9959 | 0.9952 |
| B16-in-moves / A1-fp | 0.9839 | 0.9804 | 0.9889 | 0.9869 | 0.9810 | 0.9862 | 0.9691 | 0.9921 | 0.9915 |
| C16 / A1-fp | 0.9843 | 0.9829 | 0.9889 | 0.9866 | 0.9826 | 0.9892 | 0.9698 | 0.9985 | 0.9972 |
| C16-in / A1-fp | 0.9888 | 0.9881 | 0.9920 | 0.9904 | 0.9877 | 0.9927 | 0.9781 | 0.9997 | 0.9990 |
| Cmix / A1-fp | 0.9845 | 0.9833 | 0.9886 | 0.9866 | 0.9829 | 0.9894 | 0.9701 | 0.9975 | 0.9962 |
| C0 / A1-fp | 0.9904 | 0.9883 | 0.9945 | 0.9927 | 0.9883 | 0.9925 | 0.9794 | 0.9892 | 0.9877 |
| D / A1-fp | 1.0034 | 1.0014 | 1.0063 | 1.0050 | 1.0020 | 1.0006 | 1.0027 | 1.0120 | 1.0116 |

S2+3 (share of valid rows left by both cut and range), mean over queries

| partition | text 0.01% | text 0.1% | text 1% | species 0.01% | species 0.1% | species 1% | item 0.01% | item 0.1% | item 1% |
|---|---|---|---|---|---|---|---|---|---|
| A1 | 0.9766 | 0.9936 | 0.9993 | 0.6738 | 0.7299 | 0.8065 | 0.4291 | 0.5160 | 0.6530 |
| A2 | 0.9773 | 0.9940 | 0.9994 | 0.6769 | 0.7343 | 0.8100 | 0.4273 | 0.5157 | 0.6538 |
| A3 | 0.9770 | 0.9938 | 0.9994 | 0.6786 | 0.7352 | 0.8114 | 0.4309 | 0.5194 | 0.6556 |
| A0 | 0.9759 | 0.9934 | 0.9993 | 0.6777 | 0.7351 | 0.8107 | 0.4287 | 0.5172 | 0.6545 |
| A1-fp | 0.9762 | 0.9935 | 0.9993 | 0.6708 | 0.7272 | 0.8049 | 0.4235 | 0.5106 | 0.6488 |
| A1-fp-rand | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| A1-fp-one | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| A1-fp-cell | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| B0-in | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| B0-cf | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| B0-in-moves | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| B16-in | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| B16-cf | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| B16-in-moves | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| C16 | 0.9777 | 0.9941 | 0.9994 | 0.6754 | 0.7325 | 0.8096 | 0.4306 | 0.5180 | 0.6562 |
| C16-in | 0.9777 | 0.9941 | 0.9994 | 0.6745 | 0.7315 | 0.8090 | 0.4291 | 0.5165 | 0.6549 |
| Cmix | 0.9774 | 0.9940 | 0.9994 | 0.6748 | 0.7317 | 0.8088 | 0.4296 | 0.5169 | 0.6551 |
| C0 | 0.9768 | 0.9936 | 0.9993 | 0.6740 | 0.7307 | 0.8077 | 0.4268 | 0.5131 | 0.6492 |
| D | 0.9764 | 0.9933 | 0.9992 | 0.6790 | 0.7369 | 0.8138 | 0.4336 | 0.5209 | 0.6569 |
| seed spread (A1-A3, max−min) | 0.0006 | 0.0004 | 0.0001 | 0.0048 | 0.0053 | 0.0049 | 0.0036 | 0.0037 | 0.0026 |
| ratio to A1-fp: ||||||||||
| A1 / A1-fp | 1.0004 | 1.0001 | 1.0000 | 1.0045 | 1.0037 | 1.0020 | 1.0132 | 1.0105 | 1.0065 |
| A2 / A1-fp | 1.0011 | 1.0005 | 1.0001 | 1.0091 | 1.0097 | 1.0063 | 1.0091 | 1.0099 | 1.0077 |
| A3 / A1-fp | 1.0008 | 1.0003 | 1.0001 | 1.0117 | 1.0110 | 1.0080 | 1.0175 | 1.0172 | 1.0104 |
| A0 / A1-fp | 0.9997 | 0.9999 | 0.9999 | 1.0103 | 1.0109 | 1.0072 | 1.0124 | 1.0129 | 1.0087 |
| A1-fp-rand / A1-fp | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| A1-fp-one / A1-fp | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| A1-fp-cell / A1-fp | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| B0-in / A1-fp | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| B0-cf / A1-fp | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| B0-in-moves / A1-fp | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| B16-in / A1-fp | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| B16-cf / A1-fp | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| B16-in-moves / A1-fp | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| C16 / A1-fp | 1.0015 | 1.0006 | 1.0001 | 1.0069 | 1.0073 | 1.0058 | 1.0169 | 1.0144 | 1.0114 |
| C16-in / A1-fp | 1.0016 | 1.0006 | 1.0001 | 1.0055 | 1.0060 | 1.0051 | 1.0132 | 1.0115 | 1.0093 |
| Cmix / A1-fp | 1.0013 | 1.0005 | 1.0000 | 1.0060 | 1.0062 | 1.0048 | 1.0144 | 1.0124 | 1.0096 |
| C0 / A1-fp | 1.0006 | 1.0000 | 1.0000 | 1.0048 | 1.0048 | 1.0035 | 1.0078 | 1.0049 | 1.0005 |
| D / A1-fp | 1.0002 | 0.9997 | 0.9999 | 1.0122 | 1.0134 | 1.0110 | 1.0238 | 1.0201 | 1.0125 |

Order: S4a (mean share; ratio of means), S4b (share of rows visited to cover 9 of the 10 best), own-cell share of 10-NN

| partition | S4a text | S4a species | S4a item | S4a ratio text | S4a ratio species | S4a ratio item | S4b text | S4b species | S4b item | 10-NN own cell |
|---|---|---|---|---|---|---|---|---|---|---|
| A1 | 0.5810 | 0.8584 | 0.8738 | 0.6170 | 0.8762 | 0.9217 | 0.0425 | 0.0210 | 0.0362 | 0.6806 |
| A2 | 0.5808 | 0.8595 | 0.8757 | 0.6169 | 0.8767 | 0.9224 | 0.0390 | 0.0210 | 0.0422 | 0.6857 |
| A3 | 0.5824 | 0.8598 | 0.8746 | 0.6180 | 0.8770 | 0.9214 | 0.0370 | 0.0197 | 0.0378 | 0.7018 |
| A0 | 0.5833 | 0.8594 | 0.8750 | 0.6193 | 0.8767 | 0.9211 | 0.0397 | 0.0201 | 0.0410 | 0.6856 |
| A1-fp | 0.5855 | 0.8605 | 0.8756 | 0.6210 | 0.8778 | 0.9227 | 0.0391 | 0.0213 | 0.0307 | 0.6976 |
| A1-fp-rand | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| A1-fp-one | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| A1-fp-cell | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| B0-in | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| B0-cf | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| B0-in-moves | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| B16-in | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| B16-cf | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| B16-in-moves | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| C16 | 0.5791 | 0.8567 | 0.8700 | 0.6144 | 0.8746 | 0.9175 | 0.0436 | 0.0220 | 0.0401 | 0.7027 |
| C16-in | 0.5803 | 0.8574 | 0.8707 | 0.6155 | 0.8752 | 0.9181 | 0.0438 | 0.0219 | 0.0351 | 0.7011 |
| Cmix | 0.5805 | 0.8574 | 0.8707 | 0.6158 | 0.8752 | 0.9179 | 0.0420 | 0.0211 | 0.0384 | 0.7129 |
| C0 | 0.5822 | 0.8633 | 0.8843 | 0.6187 | 0.8808 | 0.9338 | 0.0347 | 0.0211 | 0.0227 | 0.6460 |
| D | 0.5790 | 0.8572 | 0.8700 | 0.6152 | 0.8751 | 0.9169 | 0.0473 | 0.0218 | 0.0468 | 0.6287 |
| seed spread (A1-A3, max−min) | 0.0015 | 0.0014 | 0.0019 | 0.0011 | 0.0009 | 0.0010 | 0.0055 | 0.0014 | 0.0060 | 0.0212 |
| ratio to A1-fp: |||||||||||
| A1 / A1-fp | 0.9923 | 0.9976 | 0.9979 | 0.9935 | 0.9981 | 0.9989 | 1.0868 | 0.9873 | 1.1792 | 0.9756 |
| A2 / A1-fp | 0.9920 | 0.9989 | 1.0001 | 0.9934 | 0.9987 | 0.9996 | 0.9972 | 0.9841 | 1.3730 | 0.9829 |
| A3 / A1-fp | 0.9946 | 0.9992 | 0.9989 | 0.9951 | 0.9991 | 0.9986 | 0.9463 | 0.9233 | 1.2303 | 1.0059 |
| A0 / A1-fp | 0.9961 | 0.9987 | 0.9992 | 0.9973 | 0.9987 | 0.9983 | 1.0168 | 0.9419 | 1.3360 | 0.9828 |
| A1-fp-rand / A1-fp | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| A1-fp-one / A1-fp | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| A1-fp-cell / A1-fp | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| B0-in / A1-fp | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| B0-cf / A1-fp | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| B0-in-moves / A1-fp | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| B16-in / A1-fp | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| B16-cf / A1-fp | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| B16-in-moves / A1-fp | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| C16 / A1-fp | 0.9890 | 0.9957 | 0.9936 | 0.9893 | 0.9964 | 0.9943 | 1.1163 | 1.0326 | 1.3058 | 1.0072 |
| C16-in / A1-fp | 0.9912 | 0.9965 | 0.9943 | 0.9911 | 0.9969 | 0.9950 | 1.1200 | 1.0270 | 1.1440 | 1.0049 |
| Cmix / A1-fp | 0.9915 | 0.9964 | 0.9944 | 0.9915 | 0.9970 | 0.9947 | 1.0739 | 0.9899 | 1.2491 | 1.0219 |
| C0 / A1-fp | 0.9944 | 1.0034 | 1.0099 | 0.9963 | 1.0034 | 1.0120 | 0.8876 | 0.9905 | 0.7405 | 0.9259 |
| D / A1-fp | 0.9889 | 0.9962 | 0.9935 | 0.9906 | 0.9969 | 0.9937 | 1.2113 | 1.0225 | 1.5250 | 0.9012 |

Layout and shape (all valid rows)

| partition | mean key | msq resid | n_c min | median | max | CV | rows in cells <500 | cells/family | cells/genus | cells/species | cells/species (row-wtd) | max/rms radius (median cell) | cells with max/rms>2 | key p1/median (median cell) | S1e B/row | viol S2 | viol S3 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| A1 | 0.9183 | 0.1547 | 134 | 885.0000 | 3658 | 0.6283 | 0.0852 | 104.3333 | 20.4889 | 6.1082 | 44.3004 | 1.5593 | 0.1550 | 0.9026 | 36.1574 | 0 | 0 |
| A2 | 0.9188 | 0.1538 | 1 | 853.5000 | 4131 | 0.6557 | 0.0847 | 101.0000 | 20.3556 | 6.0475 | 41.5689 | 1.5226 | 0.1407 | 0.8974 | 36.1574 | 0 | 0 |
| A3 | 0.9186 | 0.1542 | 56 | 882.5000 | 5398 | 0.6698 | 0.0795 | 103.0000 | 20.6667 | 6.0597 | 45.0908 | 1.5401 | 0.1400 | 0.8980 | 36.1574 | 0 | 0 |
| A0 | 0.9189 | 0.1536 | 159 | 861.5000 | 3688 | 0.5875 | 0.0548 | 101.3333 | 20.4889 | 6.2568 | 42.1813 | 1.5464 | 0.1650 | 0.9013 | 36.1574 | 0 | 0 |
| A1-fp | 0.9193 | 0.1529 | 134 | 833.0000 | 3742 | 0.6242 | 0.0778 | 102.3333 | 20.1111 | 5.8584 | 42.5876 | 1.5441 | 0.1350 | 0.8997 | 36.1574 | 0 | 0 |
| A1-fp-rand | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | 36.1574 | n/a | n/a |
| A1-fp-one | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | 3.7171 | n/a | n/a |
| A1-fp-cell | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | 590.5920 | n/a | n/a |
| B0-in | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | 36.1574 | n/a | n/a |
| B0-cf | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | 36.1574 | n/a | n/a |
| B0-in-moves | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | 36.1574 | n/a | n/a |
| B16-in | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | 36.1574 | n/a | n/a |
| B16-cf | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | 36.1574 | n/a | n/a |
| B16-in-moves | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | 36.1574 | n/a | n/a |
| C16 | 0.9181 | 0.1551 | 131 | 844.0000 | 3816 | 0.6555 | 0.0869 | 103.3333 | 20.5333 | 5.8483 | 42.2821 | 1.6040 | 0.1500 | 0.8883 | 36.1574 | 0 | 0 |
| C16-in | 0.9183 | 0.1546 | 133 | 830.0000 | 3808 | 0.6527 | 0.0932 | 102.3333 | 20.2222 | 5.8605 | 43.0547 | 1.6124 | 0.1400 | 0.8912 | 36.1574 | 0 | 0 |
| Cmix | 0.9183 | 0.1548 | 131 | 839.0000 | 3780 | 0.6499 | 0.0835 | 103.3333 | 20.5333 | 5.8028 | 42.2319 | 1.5994 | 0.1650 | 0.8902 | 36.1574 | 0 | 0 |
| C0 | 0.9179 | 0.1553 | 133 | 825.0000 | 4097 | 0.6833 | 0.0832 | 98.0000 | 20.2000 | 6.1284 | 41.0515 | 1.4770 | 0.0550 | 0.9044 | 36.1574 | 0 | 0 |
| D | 0.9176 | 0.1559 | 133 | 1100.0000 | 1100 | 0.2031 | 0.0180 | 102.6667 | 19.9778 | 6.2073 | 41.8874 | 1.6297 | 0.1700 | 0.8993 | 36.1574 | 0 | 0 |
| seed spread (A1-A3, max−min) | 0.0005 | 0.0009 | 133 | 31.5000 | 1740 | 0.0415 | 0.0057 | 3.3333 | 0.3111 | 0.0607 | 3.5218 | 0.0367 | 0.0150 | 0.0052 | 0.0000 | 0 | 0 |

## Drop rules: which fired

| candidate or claim | rule | fired? |
|---|---|---|
| baseline (item 1) | S0 under 2% for A1 and S2+3, S1b at A1-fp within twice the seed spread, else A1-fp | S0 = 4.4%: baseline is **A1-fp** |
| price of the grouping (P3.4) | A1-cell not narrower than the baseline on held-out rows | no (10.3% narrower) |
| B0, B16 (both variants, and with single moves) | S1b at 0.06 gains under 1% or under twice the seed spread | **yes, every variant** (0.5 to 2.5% against a bar of 5.9%) |
| B, angle clause | gain under 3% and the mean angle rises by more than half | no (18.49° to at most 18.96°) |
| B0, zero-read clause | B0 gains in S1a and not in S1d or S1b | no (B0's gains are smallest in S1a) |
| C16 | under 5% of rows change cell in the first step | no (9.96%) |
| C16 | S1b at 0.06 gains under 3% at the stopping point | **yes** (2.4% against the baseline, 1.0% against B16-in) |
| C16 | the net figure does not fall for both item and text queries | no (falls for both) |
| C0 against C16 | C0 does as well as C16 in S1b | no on the measured values (C0 1.6%, C16 2.4%), but the gap is under the seed spread |
| D | S1b rises by more than 1%, or S2+3 by more than 3% | no (+0.2%; at most +2.4%) |
| model U (a) | slope over i ≤ 20 more than twice that over 100 < i ≤ 300 | **yes** (2.96): model M |
| model U (b) | spectra at 150k and 37.5k differ by more than one doubling of K | no (0.011 against 0.05 to 0.06) |
| P2.2 | S1b's fall per doubling outside 0.4 to 5 B at every w* | no (0.97 to 1.85) |
| P4.7 | largest radius above 2 times the rms in most cells | no (13.5% of cells) |

Predictions that failed without a drop rule: D's "no cell under 500 rows" (10 cells, smallest 133); model U (b)'s
"within the seed spread" (1.4 to 1.5 times it).

## Limits of the evidence

- One dataset, one sample of 200,000 rows, one split, K = 200 and G = 12 except in the sweep. The sample holds 3
  families and 3 orders, so the taxon columns are near-trivial; genus and species are given beside them.
- The level cost is the floor's idealisation (water-filling per group, a coded coordinate's error sᵢ times the
  coordinate), not the built code's bytes; nothing here says whether a 1 to 2% gain in S1d or S1b survives into the
  built code.
- The seed spread is a between-partition noise from three build seeds, and it is inflated by one artefact of the
  build's grouping rule (A1's group of 617 fit rows). B and C change the baseline only partly (B the grouping of fixed
  cells), so their paired noise is smaller than the bar applied; the bar is the spec's, and gains of 1 to 2.5% that
  fall under it are reported as not results even where they are consistent in sign across Si, Sm and every level.
- Σ_Q is held fixed at the stage-0 fit for every partition; the build would refit it to each partition's centres.
- The centres are fit-row means in float64, not rounded to 8 bits as `fit.py` stores them.
- Item 1 re-makes the groups at each reported pass with a fresh generator; the coding columns there mix cells and
  groupings.
- C: candidates include the row's starting cell as a fifth (spec: 4); C16-in takes the 4 best centres within the row's
  group (spec: "restricted to cells of its own group", read this way); the floor is enforced by reverting, within a
  pass, every move out of a group that would fall below 3n (it bit only on A1-fp's 1,274-row group, which was already
  below); every C run stopped at 15 passes with 0.2 to 0.5% of rows still moving, not at the 0.1% criterion.
- D: held-out rows are placed by the same greedy rule with their own cap, separately from the fit rows; D runs the
  spec's 12 passes and is not at a fixed point (5.4% still moving).
- The sweep's 37,500-row series is scored on the same held-out rows and places every row at its nearest centre; its
  W_P and basis come from 37,500 rows. Its seed spread for spectra is taken from A1 to A3 (not fixed points).
- Nothing was timed. The machine rules of the PI's brief (4 cores, 4 threads, 12 GB) replaced the spec's one thread
  and 100 MB; the rows were held in memory.

## Files

All under `notes/program/partition/experiment/`: `lib.py`, `prep_cache.py`, `check_bytes.py`, `item12.py`, `item3.py`,
`itemB.py`, `itemC.py`, `itemD.py`, `sweep.py`, `diag_groups.py`, `report.py`, `tab_compare.py`, `tab_chist.py`,
`net.py`, `sweep_report.py`, `NOTEBOOK.md`, this file. In `work/` (ignored by git, 0.7 GB): every partition's
assignment (`parts/`), every score (`scores/*.json`), the sweep (`sweep/`), C's passes (`C/`), B's summary, and the logs.
