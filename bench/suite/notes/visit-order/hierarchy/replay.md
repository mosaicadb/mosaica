# Hierarchies under and over the clusters: replays

2026-11-18. Offline replays on the three suite datasets (LAION text queries against 1M image vectors, glove-100,
yahoo-minilm-384) and their real fits. Nothing in `rust/` changes. Scripts in `replay/`, full tables in
`replay/results/tables.md`. Every count is **measured** in the replay; every microsecond figure is an **estimate**
built from counts and stated constants. 100 queries per dataset: the two halves of the queries give item ratios that
differ by up to about ±30% (`replay/results/extras.txt`), so differences under about 10% in items are noise.

## Summary

1. **Sub-clusters inside the existing clusters cut the items scanned 1.8 to 4 times at recall 0.9 to 0.95, with
   the invariant spread key, on all three datasets, and the gain holds under every visible set tried.** They raise
   the groups and clusters touched and the centres scored. With per-dataset cost constants taken from the real
   index's phase timings, the estimated query time at recall 0.9 falls 1.5 to 2.5 times for two-level probing
   (clusters first, then the sub-clusters of the first 32 to 64) and 1.1 to 1.4 times for ranking every sub-centre.
   Under a 1% random visible set the estimate is a loss: the rows are already cheap there and the extra centre
   scores cost more than the rows saved.
2. **A tree over the centres does not pay at the real K** (339 to 592): a beam that holds recall scores 0.5 to 1.1
   times the centres of the flat order at recall 0.9, and all of them at 0.95 and above. At K = 5,000 and 20,000
   one level of 256 nodes with a statistical node key cuts the centres scored to 13 to 58% (5,000) and 6 to 41%
   (20,000) for a recall loss under 0.01, depending on the dataset. Deeper trees are worse, not better: a narrow
   beam loses at the top level.
3. **The payoff question has a mixed answer.** More, smaller clusters do scan fewer items per recall (LAION at 0.9:
   36,722 items at K = 500, 11,815 at 5,000, 10,119 at 20,000), and with a one-level tree the order's cost stays
   near 800 to 3,500 centres. But 16 or 64 sub-clusters under the unchanged fit reach about the same (16,209 and
   9,043 items), ahead on glove and behind on LAION and yahoo at 16, and they keep the partition, the groups, the
   bases and the row order. Re-clustering raises the groups touched (LAION at 0.9: 8, 11 and 18 groups at K = 500,
   5,000 and 20,000) and breaks down under a visible set aligned with clusters (38 groups against 3).
4. **Thresholds**: sub-clusters help small answers (2.3 times fewer items at 0.01% of the corpus on LAION) and
   fade as the answer grows (1.5 at 1%, 1.26 at 10%), with 6 to 14 more groups touched.
5. **Negative results**: scoring sub-centres in the first coordinates of the group's basis (needs 64 to 128
   coordinates and the bases of every candidate's group); the non-invariant key on sub-clusters (fails on LAION
   under a sparse filter and on thresholds); trees deeper than one level; K = 20,000 at 1M items (no better than
   5,000 in items on glove and yahoo, more groups, 20,000 centres); limiting a probe to few groups (it no longer
   matters once group setup is priced at its measured cost).

## Method

A replay ranks leaves (clusters or sub-clusters) under a key for each query and probes them in order; a probed
leaf is taken as searched exactly, so recall@10 is decided by which leaves are probed (as in `../README.md`; the
flat rows here reproduce that README's glove figures to three digits). The same number of leaves is probed for
every query (as nprobe works) and the curve of mean recall against mean items is interpolated to recall targets.
Counted per variant: items scanned (visible items under a visible set), centres scored (cluster, sub-cluster and
tree-node centres: the cost of the order), groups touched and clusters touched (distinct groups and parent clusters
of the probed leaves).

- **Real fits** (`/home/user/suite-data/work/<ds>/vexact/index`): K = 500 (LAION), 592 (glove), 339 (yahoo), 64
  groups each. Cluster centres are the stored int8 centres.
- **Sub-clusters** (`replay/under.py`): each cluster split into 4, 16 or 64 by k-means on its own items (faiss, 10
  iterations). Clusters, groups, bases and rows are untouched: a sub-cluster is a set of rows of its cluster, with
  a centre (exact mean here), a radius and a count. The index would read it as it reads a visible set inside a
  cluster. Probing: **direct** (every sub-centre scored and ranked), **two-level** (the first P clusters by the flat
  spread key, then their sub-clusters ranked together), **group-limited** (the sub-clusters of every cluster in the
  groups of the first P clusters).
- **Keys.** qm = q·centre; sdK(q) the s.d. of q·m_c over the clusters (so every sub-cluster key is computable from
  the cluster scores); rad the RMS distance of a leaf's items to its centre, divided by its s.d. over the leaves.
  `spread f` = qm + f·sdK(q)·rad; `H` = qm + α·z(10, n)·σ(q) + 0.3·sdK(q)·rad with the leaf's own rank-4 covariance
  (`Hown4`) or its parent's rank-16 one scaled by the radius ratio (`Hpar`); `nonin .2` = qm + 0.2·rad (raw radius,
  the key of `reports/annideas`, not invariant); `gbT` = spread with the sub-centre score truncated to the first T
  coordinates of the group's basis; `zeta` = (qm − t)/σ(q) for threshold queries. The constants 0.4, 0.3 and α are
  those fitted for clusters; a grid of f is in the full tables, and 0.4 is within noise of the best f on glove and
  yahoo and behind 0.8 on LAION (16,209 against 13,835 items at 0.9).
- **Trees** (`replay/over.py`): over the real centres (the groups as one level; top-down k-means with 16, 32 and
  8×8 nodes) and over refits. **The refits are a different partition**: faiss k-means at K = 5,000 (400,000
  training rows, 10 iterations) and 20,000 (200,000 rows, 6 iterations; about 50 items a cluster), groups remade as
  `fit.py` makes them (k-means of the normalised centres into 64). Trees: 256 nodes; 16×16 and 32×32; 16×16×16.
  The k-means is not size-balanced (node sizes are in the logs).
- **Beams.** Two widths, because the first alone cannot save anything when nodes are large: `m` keeps the best
  m × nprobe nodes at each level (the width asked for); `w` keeps the best nodes until they hold m × nprobe
  clusters, so the clusters scored at the bottom are about m × nprobe. Node keys: `c` = q·M; `sp f` = q·M +
  f·sd(q)·R (the spread form one level up, R the node's RMS radius); `st a` = q·M + a·z(1, n)·sqrt(qᵀCq) + the
  leaf key's radius term at the node's mean radius, C the covariance of the node's centres (top 8 directions and an
  isotropic rest), n its clusters. The leaf key is the flat spread key, with sd(q) over all clusters; taking it
  over the scored clusters only changes recall by 0.003 or less (`st 1 local`).
- **Visible sets**: the suite's random 1% and 25%, and two correlated kinds made here at 1% and 25%: `clus` (whole
  clusters of the real fit, drawn at random) and `cap` (the items scoring highest against one corpus item: one
  region of the space). Truth is recomputed from the exact scores. Leaves without a visible item are dropped and
  z uses visible counts, as `knn2_fast` does. H falls back to spread under sparse sets by the rule in `q.rs`.
- **Thresholds**: the suite's thresholds giving 0.01%, 1% and 10% of the corpus; recall is the share of the answer
  set in the probed leaves.
- **Estimated time.** Constants per dataset from the real index's phase timings on these fits
  (`../results/phases-1-int8-order.txt`, unfiltered): a group 27 µs (LAION, n = 512), 14.5 (yahoo), 4 (glove, n =
  128), the group phase divided by the mean groups among the first P clusters; a centre 0.07, 0.067 and 0.045 µs
  (the order phase over K); a row 0.06, 0.04 and 0.027 µs (the rest of the query at nprobe 16 over the rows
  scanned, after 4 µs a cluster, which is the planner's range of 1.6 to 7.8 and is not separable in the phase
  figures). The fixed 50 to 190 µs of the stored-vector phase is left out of every row. The per-row figure is for
  whole clusters; the rows of a probed sub-cluster are those nearest the query, which the codes prune least, so
  the estimate favours sub-clusters by an amount not measured here.

## 1. Under: sub-clusters

Tables A (below) give, per dataset, items at recall 0.9, 0.95 and 0.99, groups and clusters touched, centres
scored and estimated time. Reading for LAION text queries at recall 0.9 / 0.95:

| | items | groups | clusters | centres | est. µs |
|---|---|---|---|---|---|
| flat spread | 36,393 / 75,565 | 8 / 14 | 17 / 36 | 500 | 2,498 / 5,088 |
| flat H r16 | 30,476 / 55,202 | 7 / 11 | 14 / 25 | 500 (+ spreads) | 2,100 / 3,736 |
| 16 sub-clusters, direct, spread 0.4 | 16,209 / 41,020 | 13 / 23 | 32 / 69 | 7,989 | 2,005 / 3,915 |
| 16, two-level P64, spread 0.4 | 17,751 / 42,754 | 12 / 18 | 31 / 54 | 1,524 | 1,619 / 3,383 |
| 16, two-level P64, Hown4 | 15,499 / 35,205 | 10 / 16 | 25 / 46 | 1,524 (+ spreads) | 1,415 / 2,834 |
| 64, two-level P64, spread 0.4 | 10,023 / 34,247 | 14 / 20 | 37 / 60 | 4,595 | 1,449 / 3,147 |
| 64, direct, spread 0.4 | 9,043 / 24,443 | 18 / 30 | 46 / 97 | 31,941 | 3,443 / 4,895 |

- **Items.** 16 sub-clusters: 2.2 / 1.8 times fewer on LAION, 2.4 / 1.8 on glove, 4.1 / 2.6 on yahoo (0.9 / 0.95).
  64: 4.0 / 3.1, 2.8 / 2.5, 7.8 / 6.2. 4: 1.3 to 1.7. At 0.99 the gain is smaller (1.4 to 3 at 16).
- **The invariant key carries over.** Centre score alone loses most of the gain (LAION, 16: 24,262 items at 0.9
  against 16,209). The H form with the sub-cluster's own rank-4 covariance is the best key at 0.95 on LAION and
  yahoo (29,905 and 9,263 items) and no better than spread on glove; it needs 4 directions per sub-cluster.
- **The non-invariant key** (centre + 0.2 × radius) is as good unfiltered (LAION 14,117 at 0.9) and fails where the
  theory says it should: LAION under random 1% touches 30 groups against 21 and scans 658 visible items against
  460; at 1% and 10% thresholds it is no better than the flat order (1.06 and 0.96 times) with 15 to 25 more
  groups. Glove and yahoo, whose queries are in distribution, do not show it.
- **Two-level probing gives the same items for a fifth of the centres** once P is about 4 times the clusters the
  flat order needs: P = 64 at 0.9 and P = 128 at 0.95 on LAION and glove, P = 32 on yahoo.
- **Groups and clusters touched go up**, by 4 to 5 groups and about 2 times the clusters at 16 on LAION and glove
  (yahoo: unchanged groups). At the measured group cost this is a small part of the estimate (LAION, two-level
  P64 at 0.9: 12 groups = 324 µs of 1,619). An earlier version of this note priced a group at 85 µs and concluded
  that group setup cancels the gain; with the measured 4 to 27 µs it does not. Limiting the probe to the groups
  of the first P clusters holds the groups down (7 at P = 16) but scores more centres and is not better in the
  estimate.
- **Estimated time** at 0.9 / 0.95, flat spread against the best two-level spread variant: LAION 2,498 / 5,088 →
  1,619 / 3,383 (1.5 / 1.5 times); glove 2,242 / 4,440 → 1,259 / 2,916 (1.8 / 1.5); yahoo 800 / 1,347 → 326 / 759
  (2.5 / 1.8). Ranking every sub-centre directly gives 1.25, 1.4 and 1.3 times at 0.9, and 64 sub-clusters ranked
  directly are slower than the flat order (31,941 centres). [estimated]
- **Group-basis coordinates.** The first T coordinates of the group's basis carry R² = 0.61 (T = 16), 0.87 (64),
  0.93 (128) of the sub-centre's offset score on LAION (0.65 and 0.91 at 16 and 64 on glove, 0.76 and 0.96 on
  yahoo). Recall with T = 16 is clearly worse (LAION 30,970 items at 0.9); T = 64 is close to the full score
  on glove and yahoo and 5 to 7% behind on LAION. It also needs the basis of every candidate's group before
  ranking: 21 groups for 64 parents, against 12 touched. Not worth having.

**Visible sets** (table B: items ratio against the flat spread order at 0.9 and 0.95, and the change in groups).
16 sub-clusters, direct, at 0.9, LAION / glove / yahoo: random 1% 2.1 / 1.6 / 1.9; random 25% 1.9 / 1.9 / 3.4;
whole clusters 1% 1.9 / 1.5 / 1.8; whole clusters 25% 2.0 / 2.0 / 2.7; cap 1% 1.5 / 1.5 / 1.45; cap 25% 2.1 / 2.2 /
2.5. The gain is smallest on the concentrated 1% cap and never collapses. Groups touched rise by 1 to 9. Two
caveats on cost: (i) under random 1% the index scores visible rows directly and opens no group; a whole query
costs 48 µs at nprobe 16 on LAION, of which the order is 20, so 500 to 2,000 extra sub-centre scores (35 to 140 µs)
cost more than halving the rows saves: **in estimated time sub-clusters lose under a sparse random set**, and a
rule that expands a cluster only when it holds enough visible rows is needed; (ii) under the whole-cluster 1% set
only 5 clusters are live and the flat order is already at its floor.

**Thresholds** (16, direct, items ratio at answer-set recall 0.9, LAION / glove / yahoo): 0.01% answers 2.3 / 1.9
/ 3.1; 1% 1.55 / 1.45 / 1.7; 10% 1.26 / 1.2 / 1.3, with 6 to 14 more groups on LAION. The `zeta` key with the
leaf's own rank-4 spread is 5 to 20% better than spread at 0.01% and equal at 10%. At 10% answers every variant
reads 31 to 45% of the corpus for recall 0.9: an approximate threshold query of that size is a scan.

## 2. Over: trees and beams

Table C gives, at the nprobe where the flat order reaches recall 0.9: the beam's recall, the share of true
neighbours lost at each level (the subtree holding it dropped there) and the centres scored.

- **Real K.** With the groups as the tree and B = nprobe nodes (m1) LAION loses 0.062 at the group level with the
  centre key and 0.021 with the statistical key, scoring 46 to 54% of the centres; B = 2 × nprobe loses 0.021 and
  0.002 at 76 to 81%; B = 4 × nprobe is the flat order plus the node scores (113%). Glove: m1 loses 0.004 at 79%;
  yahoo: m1 loses 0.053 (centre) or 0.027 (statistical) at 34 to 36%, m2 0.014 or 0.008 at 51%. At recall 0.95 and
  0.99 no beam that keeps recall scores fewer centres than the flat order on any dataset. The order costs 23 to
  35 µs of a 1,000 to 2,400 µs query at nprobe 16, so there is nothing to win here.
- **Node keys.** The statistical key (c) loses 2 to 6 times less than the centre key (a) at the same width (LAION,
  K = 20,000, three levels, m1: 0.022 against 0.129 at the bottom level; m2: 0.009 against 0.061). The spread
  form one level up (b) is between them and sometimes worse than the centre key (glove at K = 5,000).
- **Where the loss is, and depth.** With the width in nodes (m), all loss is at the lowest tree level: upper
  levels have fewer nodes than the beam, or coarse nodes that are kept anyway. With the width in clusters (w), which
  is what makes the beam cheap, the loss is at the top level: at K = 20,000 with three levels and w4, 0.48 of the
  neighbours are lost at the first level on LAION, 0.31 on glove, 0.29 on yahoo, against 0.14, 0.13 and 0.23 for
  one level of 256 nodes at the same width. So the beam must cover more clusters the higher the level (in
  clusters covered, B must grow towards the root; in node counts it can stay constant only because nodes get
  larger). A second or third level saves at most the 256 node scores of a one-level tree and costs recall. One
  level is the useful depth at these K.
- **How narrow a beam holds recall** (one level of 256 nodes, statistical key, width in clusters): at K = 5,000,
  w16 loses under 0.01 everywhere and scores 23% (LAION), 58% (glove), 13% (yahoo) of the centres; w8 loses 0.007,
  0.004 and 0.04 at 14%, 32% and 9%. At K = 20,000, w16 scores 17%, 41% and 6% for a loss of 0.00, 0.006 and 0.025.
  Glove needs 164 of 5,000 clusters for recall 0.9, so 16 × nprobe is already half of them.
- **Visible sets** (table E): the same beams under random and correlated sets lose about what they lose
  unfiltered (w16 within 3% of the flat order's items in most cells, w4 20 to 100% more items). No collapse, no
  gain beyond the centre scores.

## 3. Payoff: more, smaller clusters with a hierarchical order

Table D gives the fewest items at recall 0.9 and 0.95 within a budget of centres scored, for each partition and
for sub-clusters under the real fit. Items / centres / groups at 0.9, flat spread order:

| | K real | K = 5,000 | K = 20,000 | real, 16 sub-clusters | real, 64 sub-clusters |
|---|---|---|---|---|---|
| LAION | 36,722 / 500 / 8 | 11,815 / 5,000 / 11 | 10,119 / 20,000 / 18 | 16,209 / 7,989 / 13 | 9,043 / 31,941 / 18 |
| glove | 74,335 / 592 / 14 | 40,848 / 5,000 / 21 | 43,302 / 20,000 / 35 | 31,553 / 9,472 / 18 | 26,276 / 37,888 / 29 |
| yahoo | 17,016 / 339 / 5 | 2,933 / 5,000 / 5 | 2,241 / 20,000 / 7 | 4,130 / 5,424 / 5 | 2,176 / 21,696 / 6 |

- **At a fixed order cost equal to the flat order's (K real centres), nothing hierarchical reaches recall 0.9**
  on any dataset: every tree over 5,000 or 20,000 clusters and every sub-cluster scheme needs more centres than
  that. At 2 times: K = 5,000 with the 256-node tree reaches 0.9 with 14,225 items on LAION (768 centres), 78,190
  on glove (worse than flat), 4,134 on yahoo. At 4 times: 12,127 (LAION), 43,396 (glove), 3,010 (yahoo), against
  17,751, 37,104 and 4,008 for 16 sub-clusters two-level. At 8 times: K = 20,000 gives 10,543, 74,366 and 2,430;
  64 sub-clusters two-level 12,566, 31,548 and 2,327.
- So **yes, smaller clusters with a one-level tree beat 500 flat clusters per item scanned** by 2 to 6 times at
  0.9 for 2 to 8 times the centre scores, and the order's cost stays under 10% of the estimated query. But the
  same is obtained, within the noise of these replays, by sub-clusters under the unchanged fit: ahead on glove,
  behind on LAION and yahoo at 16, level at 64.
- **K = 20,000 at 1M is not better than 5,000** on glove (43,302 against 40,848 items at 0.9) and little better
  on LAION and yahoo, while touching 18 to 35 groups against 11 to 21. It is a poor stand-in for a 1B index: its
  clusters hold 50 items, not 50,000, and the within-cluster spread that the keys model is mostly noise at 50.
  What transfers is the ratio of nodes to leaves and the per-level losses of section 2.
- **What re-clustering does to the two strengths.** It changes the partition, the groups, the bases and the row
  order. Groups touched at recall 0.9 rise from 8 to 11 and 18 (LAION), 14 to 21 and 35 (glove), 5 to 5 and 7
  (yahoo). Under the whole-cluster 1% visible set (5 live clusters in the real fit) the refits touch 38 and 31
  groups on LAION against 3, 47 against 3 on glove, 46 and 40 against 3 on yahoo, and score 700 to 2,900 centres
  against 4 to 7. Under random 1% they halve the visible items scanned (LAION 975 → 491 → 376) for 3,900 to 6,300
  live centres, which at 0.07 µs each is several times the whole query today. Sub-clusters as metadata change
  none of this: a visible set or a filter sees the same clusters and groups as now.

## 4. Thresholds and 5. filters

Covered in sections 1 to 3 and tables B and E. In short: under every visible set tried the ranking of the variants
is the same as unfiltered; the sub-cluster gain in items is 1.45 to 3.4 times at 16 (smallest on a concentrated 1%
set), and it is a loss in estimated time when the visible set is sparse and random; the refits lose their
advantage in groups and centres under filters, most of all under a set aligned with the real clusters.

## Deviations and limits

- 100 queries per dataset; recall targets are means over queries with the same probe count for all.
- Sub-centres are exact means, not int8; cluster centres are the stored int8 ones.
- The refits use faiss L2 k-means on a sample (200,000 rows for K = 20,000, 10 per centre), not `fit.py`'s
  k-means; `flat H r4` on the K = 20,000 refit is the spread order (the sparse fallback of `q.rs` applies at 59
  rows a cluster).
- The beams use sd(q) over all clusters in the leaf key (checked: 0.003 or less against the scored clusters only).
- Under a visible set the estimated-time columns are not given: the index's cost per visible row there (direct
  scoring, or the index path at about 0.19 µs per visible row under random 25%) differs from the unfiltered one.
- The per-row cost of reading a sub-cluster through the index path (rows selected by a mask, the cluster's key
  order kept) is not measured; it is the main unknown in the estimates.
- No timing was run.

## What I would run next

1. The per-row and per-cluster cost of an index-path read restricted to a row mask of 6 to 12% of a cluster, in
   the real index (the microbench harness): it decides whether the 1.5 to 2.5 times estimate survives.
2. Stopping on a price or a calibrated probability in place of a fixed probe count, on sub-clusters, with the
   expansion of a cluster decided by its visible rows against its sub-centre cost (the specification in
   `policy.md`); this replay only measures fixed counts.
3. 1,000-query sets and the LAION image-query control, to shrink the ±30% noise on ratios and check the
   in-distribution case.
4. Sub-clusters by a cheaper rule than k-means (a split along the cluster's key order, which the row order already
   has), since a contiguous range of rows is cheaper to read than a mask.
5. A 10M corpus for the tree, where K under the build rule is in the thousands and the clusters are full size.

## Tables


### A. Sub-clusters, k-NN, no filter

Items scanned at recall 0.9 / 0.95 / 0.99; groups and clusters touched; centres scored; estimated µs.
More rows (other f, H variants, P) in `replay/results/tables.md`.

**laion**

| structure, key | items at 0.9 | at 0.95 | at 0.99 | groups at 0.9 / 0.95 / 0.99 | clusters at 0.9 / 0.95 | centres | est. us at 0.9 / 0.95 |
|---|---|---|---|---|---|---|---|
| flat spread .4 | 36,393 | 75,565 | 223,275 | 8 / 14 / 30 | 17 / 36 | 500 | 2,498 / 5,088 |
| flat H r16 | 30,476 | 55,202 | 164,187 | 7 / 11 / 24 | 14 / 25 | 500 | 2,100 / 3,736 |
| x4 direct spread 0.4 | 25,970 | 55,065 | 172,595 | 10 / 17 / 35 | 23 / 46 | 2,000 | 2,063 / 4,084 |
| x16 direct centre | 24,262 | 65,883 | 186,424 | 18 / 32 / 48 | 47 / 104 | 7,989 | 2,692 / 5,783 |
| x16 direct nonin .2 | 14,117 | 31,879 | 157,966 | 13 / 22 / 50 | 32 / 63 | 7,989 | 1,891 / 3,318 |
| x16 direct spread 0.4 | 16,209 | 41,020 | 156,821 | 13 / 23 / 45 | 32 / 69 | 7,989 | 2,005 / 3,915 |
| x16 direct spread 0.8 | 13,835 | 28,587 | 131,655 | 12 / 19 / 44 | 29 / 54 | 7,989 | 1,842 / 3,017 |
| x16 direct Hown4 a0.75 | 14,346 | 29,905 | 124,091 | 10 / 16 / 37 | 25 / 46 | 7,989 | 1,800 / 2,978 |
| x16 direct gb16 0.4 | 30,970 | 54,781 | 197,724 | 14 / 21 / 45 | 39 / 65 | 7,989 | 2,963 / 4,687 |
| x16 direct gb64 0.4 | 16,938 | 43,725 | 194,476 | 12 / 22 / 48 | 29 / 65 | 7,989 | 2,005 / 4,028 |
| x16 two-level P32 spread 0.4 | 19,159 | - | - | 11 / - / - | 26 / - | 1,012 | 1,614 / - |
| x16 two-level P64 spread 0.4 | 17,751 | 42,754 | - | 12 / 18 / - | 31 / 54 | 1,524 | 1,619 / 3,383 |
| x16 two-level P128 spread 0.4 | 16,297 | 41,091 | 160,069 | 12 / 21 / 33 | 31 / 64 | 2,547 | 1,615 / 3,478 |
| x16 two-level P64 Hown4 a0.75 | 15,499 | 35,205 | - | 10 / 16 / - | 25 / 46 | 1,524 | 1,415 / 2,834 |
| x16 group-limited P16 spread 0.4 | 18,854 | - | - | 7 / - / - | 28 / - | 1,943 | 1,577 / - |
| x16 group-limited P32 spread 0.4 | 18,255 | 42,655 | - | 11 / 13 / - | 32 / 57 | 2,883 | 1,712 / 3,333 |
| x16 group-limited P64 spread 0.4 | 17,145 | 42,338 | 215,047 | 12 / 19 / 21 | 32 / 65 | 4,113 | 1,773 / 3,593 |
| x64 direct nonin .2 | 6,935 | 18,785 | 100,238 | 15 / 26 / 52 | 35 / 76 | 31,941 | 3,190 / 4,359 |
| x64 direct spread 0.4 | 9,043 | 24,443 | 85,930 | 18 / 30 / 49 | 46 / 97 | 31,941 | 3,443 / 4,895 |
| x64 two-level P64 spread 0.4 | 10,023 | 34,247 | - | 14 / 20 / - | 37 / 60 | 4,595 | 1,449 / 3,147 |
| x64 two-level P128 spread 0.4 | 9,347 | 25,270 | 121,768 | 16 / 25 / 33 | 43 / 79 | 8,688 | 1,778 / 3,107 |
| x64 group-limited P16 spread 0.4 | 12,809 | - | - | 7 / - / - | 37 / - | 6,265 | 1,557 / - |
| x64 group-limited P32 spread 0.4 | 10,445 | 40,587 | - | 11 / 13 / - | 41 / 87 | 10,023 | 1,795 / 3,831 |

**glove**

| structure, key | items at 0.9 | at 0.95 | at 0.99 | groups at 0.9 / 0.95 / 0.99 | clusters at 0.9 / 0.95 | centres | est. us at 0.9 / 0.95 |
|---|---|---|---|---|---|---|---|
| flat spread .4 | 74,680 | 149,041 | 334,237 | 14 / 23 / 39 | 36 / 74 | 592 | 2,242 / 4,440 |
| flat H r16 | 65,636 | 130,724 | 319,543 | 11 / 19 / 36 | 28 / 57 | 592 | 1,954 / 3,863 |
| x4 direct spread 0.4 | 56,620 | 114,025 | 249,463 | 18 / 28 / 42 | 48 / 94 | 2,368 | 1,898 / 3,672 |
| x16 direct centre | 51,229 | 114,254 | 290,510 | 25 / 38 / 52 | 77 / 153 | 9,472 | 2,217 / 4,276 |
| x16 direct nonin .2 | 35,631 | 78,157 | 223,632 | 20 / 31 / 49 | 56 / 112 | 9,472 | 1,692 / 3,112 |
| x16 direct spread 0.4 | 31,553 | 82,561 | 200,513 | 18 / 33 / 48 | 52 / 123 | 9,472 | 1,561 / 3,278 |
| x16 direct spread 0.8 | 50,123 | 102,013 | 271,202 | 28 / 40 / 55 | 93 / 169 | 9,472 | 2,263 / 4,017 |
| x16 direct Hown4 a0.75 | 38,254 | 93,830 | 243,240 | 19 / 32 / 49 | 52 / 117 | 9,472 | 1,742 / 3,554 |
| x16 direct gb16 0.4 | 51,923 | 99,215 | 288,623 | 20 / 31 / 50 | 59 / 107 | 9,472 | 2,144 / 3,655 |
| x16 direct gb64 0.4 | 38,552 | 89,879 | 233,873 | 20 / 33 / 50 | 58 / 123 | 9,472 | 1,776 / 3,476 |
| x16 two-level P32 spread 0.4 | - | - | - | - / - / - | - / - | 1,104 | - / - |
| x16 two-level P64 spread 0.4 | 37,104 | - | - | 17 / - / - | 49 / - | 1,616 | 1,338 / - |
| x16 two-level P128 spread 0.4 | 32,011 | 83,827 | - | 18 / 29 / - | 51 / 104 | 2,640 | 1,259 / 2,916 |
| x16 two-level P64 Hown4 a0.75 | 46,289 | - | - | 17 / - / - | 48 / - | 1,616 | 1,583 / - |
| x16 group-limited P16 spread 0.4 | 57,836 | - | - | 7 / - / - | 57 / - | 2,155 | 1,913 / - |
| x16 group-limited P32 spread 0.4 | 34,784 | 117,851 | - | 12 / 13 / - | 49 / 113 | 3,275 | 1,330 / 3,831 |
| x16 group-limited P64 spread 0.4 | 32,122 | 82,789 | - | 16 / 21 / - | 51 / 107 | 4,831 | 1,352 / 2,964 |
| x64 direct nonin .2 | 25,206 | 59,194 | 192,267 | 28 / 41 / 57 | 89 / 173 | 37,888 | 2,852 / 4,163 |
| x64 direct spread 0.4 | 26,276 | 59,644 | 191,480 | 29 / 42 / 57 | 93 / 177 | 37,888 | 2,902 / 4,190 |
| x64 two-level P64 spread 0.4 | 31,548 | - | - | 19 / - / - | 56 / - | 4,688 | 1,365 / - |
| x64 two-level P128 spread 0.4 | 27,821 | 57,693 | - | 24 / 30 / - | 76 / 110 | 8,784 | 1,548 / 2,514 |
| x64 group-limited P16 spread 0.4 | 46,521 | - | - | 7 / - / - | 69 / - | 6,844 | 1,867 / - |
| x64 group-limited P32 spread 0.4 | 32,035 | 132,823 | - | 12 / 13 / - | 74 / 149 | 11,324 | 1,721 / 4,741 |

**yahoo**

| structure, key | items at 0.9 | at 0.95 | at 0.99 | groups at 0.9 / 0.95 / 0.99 | clusters at 0.9 / 0.95 | centres | est. us at 0.9 / 0.95 |
|---|---|---|---|---|---|---|---|
| flat spread .4 | 16,878 | 28,896 | 117,520 | 5 / 8 / 26 | 8 / 14 | 339 | 800 / 1,347 |
| flat H r16 | 12,618 | 23,106 | 92,956 | 4 / 6 / 21 | 6 / 11 | 339 | 604 / 1,082 |
| x4 direct spread 0.4 | 9,775 | 17,938 | 79,249 | 5 / 9 / 28 | 9 / 16 | 1,356 | 595 / 1,000 |
| x16 direct centre | 5,148 | 13,750 | 61,773 | 6 / 12 / 36 | 11 / 24 | 5,424 | 698 / 1,184 |
| x16 direct nonin .2 | 4,173 | 11,812 | 50,915 | 5 / 11 / 31 | 9 / 21 | 5,424 | 639 / 1,076 |
| x16 direct spread 0.4 | 4,130 | 11,085 | 39,265 | 5 / 10 / 26 | 9 / 20 | 5,424 | 636 / 1,035 |
| x16 direct spread 0.8 | 5,094 | 11,986 | 45,670 | 6 / 11 / 30 | 11 / 22 | 5,424 | 698 / 1,097 |
| x16 direct Hown4 a0.75 | 3,660 | 9,263 | 43,477 | 5 / 9 / 28 | 8 / 17 | 5,424 | 611 / 933 |
| x16 direct gb16 0.4 | 6,954 | 17,746 | 63,322 | 5 / 11 / 30 | 10 / 24 | 5,424 | 761 / 1,334 |
| x16 direct gb64 0.4 | 4,197 | 11,642 | 50,435 | 5 / 10 / 29 | 8 / 19 | 5,424 | 634 / 1,047 |
| x16 two-level P32 spread 0.4 | 4,092 | 12,431 | - | 5 / 10 / - | 9 / 20 | 851 | 326 / 779 |
| x16 two-level P64 spread 0.4 | 4,133 | 11,098 | 59,713 | 5 / 10 / 26 | 9 / 19 | 1,363 | 364 / 759 |
| x16 two-level P128 spread 0.4 | 4,131 | 11,088 | 39,085 | 5 / 10 / 25 | 9 / 20 | 2,387 | 433 / 830 |
| x16 two-level P64 Hown4 a0.75 | 3,692 | 9,262 | 58,471 | 5 / 9 / 26 | 8 / 17 | 1,363 | 339 / 656 |
| x16 group-limited P16 spread 0.4 | 4,008 | 11,724 | - | 5 / 8 / - | 8 / 18 | 1,144 | 336 / 727 |
| x16 group-limited P32 spread 0.4 | 4,136 | 11,708 | - | 5 / 10 / - | 9 / 20 | 1,771 | 391 / 808 |
| x16 group-limited P64 spread 0.4 | 4,133 | 11,085 | 52,480 | 5 / 10 / 25 | 9 / 20 | 2,848 | 464 / 860 |
| x64 direct nonin .2 | 2,177 | 4,514 | 25,745 | 6 / 10 / 31 | 11 / 19 | 21,696 | 1,674 / 1,855 |
| x64 direct spread 0.4 | 2,176 | 4,651 | 26,573 | 6 / 10 / 32 | 11 / 19 | 21,696 | 1,674 / 1,864 |
| x64 two-level P64 spread 0.4 | 2,218 | 4,664 | 45,910 | 6 / 10 / 27 | 11 / 18 | 4,435 | 517 / 696 |
| x64 two-level P128 spread 0.4 | 2,178 | 4,654 | 25,948 | 6 / 10 / 29 | 11 / 19 | 8,531 | 791 / 980 |
| x64 group-limited P16 spread 0.4 | 2,194 | 5,063 | - | 5 / 7 / - | 10 / 16 | 3,559 | 441 / 612 |
| x64 group-limited P32 spread 0.4 | 2,222 | 4,770 | - | 6 / 9 / - | 11 / 18 | 6,067 | 623 / 799 |

### B. Sub-clusters under visible sets and for thresholds

Per cell: the ratio of items at recall 0.9, at 0.95 (flat spread over the variant; `nan`: 0.95 not reached), and the
change in groups touched at 0.9. `2L` / `GL`: the two-level / group-limited run with the fewest parents within 5% of
the direct run's items at 0.9. `thr 0/1/2`: thresholds with answers of 0.01%, 1%, 10% of the corpus.

**laion**

| variant | knn | knn random-1 | knn random-25 | knn clus-1 | knn clus-25 | knn cap-1 | knn cap-25 | thr 0 | thr 1 | thr 2 |
|---|---|---|---|---|---|---|---|---|---|---|
| x4 direct spread 0.4 | 1.40x, 1.37x; +2 g | 1.41x, 1.25x; +3 g | 1.32x, 1.33x; +3 g | 1.30x, 1.21x; +1 g | 1.41x, 1.37x; +2 g | 1.15x, 1.11x; +3 g | 1.27x, 1.23x; +4 g | 1.48x, 1.29x; +2 g | 1.22x, 1.20x; +6 g | 1.13x, 1.11x; +6 g |
| x16 direct spread 0.4 | 2.25x, 1.84x; +5 g | 2.14x, 1.87x; +5 g | 1.87x, 1.87x; +7 g | 1.93x, 1.68x; +1 g | 2.00x, 1.94x; +5 g | 1.51x, 1.29x; +4 g | 2.08x, 1.49x; +6 g | 2.28x, 1.89x; +6 g | 1.55x, 1.47x; +14 g | 1.26x, 1.23x; +11 g |
| x16 direct nonin .2 | 2.58x, 2.37x; +5 g | 1.49x, 1.61x; +13 g | 2.13x, 1.71x; +8 g | 1.54x, 1.48x; +1 g | 2.34x, 2.01x; +5 g | 1.50x, 1.29x; +8 g | 2.02x, 1.66x; +11 g | 2.12x, 1.90x; +9 g | 1.06x, 1.05x; +25 g | 0.96x, 0.97x; +15 g |
| x16 2L | 2.23x, 1.84x; +5 g | 2.05x, 1.69x; +4 g | 1.86x, 1.56x; +5 g | - | 1.96x, 1.80x; +4 g | 1.44x, nanx; +1 g | 2.08x, 1.50x; +5 g | 2.28x, 1.87x; +5 g | 1.53x, 1.43x; +13 g | - |
| x16 GL | 2.24x, 1.86x; +5 g | 2.05x, 1.44x; +2 g | 1.84x, 1.45x; +2 g | - | 1.99x, 1.88x; +4 g | 1.50x, 1.29x; +1 g | 2.07x, 1.52x; +3 g | 2.23x, 1.76x; +4 g | 1.51x, 1.37x; +5 g | 1.25x, 1.21x; +3 g |
| x64 direct spread 0.4 | 4.02x, 3.09x; +10 g | 2.89x, 2.23x; +5 g | 3.78x, 2.91x; +9 g | 2.55x, 2.04x; +1 g | 3.77x, 3.09x; +8 g | 1.87x, 1.57x; +3 g | 3.64x, 2.08x; +9 g | 3.99x, 2.96x; +12 g | 1.97x, 1.82x; +23 g | 1.43x, 1.38x; +14 g |
| x64 2L | 3.89x, 2.99x; +8 g | 2.87x, 2.20x; +5 g | 3.68x, 2.68x; +8 g | - | - | 1.86x, 1.51x; +3 g | 3.47x, 2.02x; +7 g | 3.93x, 2.69x; +9 g | 1.93x, 1.72x; +18 g | - |

**glove**

| variant | knn | knn random-1 | knn random-25 | knn clus-1 | knn clus-25 | knn cap-1 | knn cap-25 | thr 0 | thr 1 | thr 2 |
|---|---|---|---|---|---|---|---|---|---|---|
| x4 direct spread 0.4 | 1.32x, 1.31x; +4 g | 1.24x, 1.30x; +5 g | 1.40x, 1.35x; +3 g | 1.27x, 1.24x; +1 g | 1.37x, 1.49x; +2 g | 1.11x, 1.22x; +4 g | 1.44x, 1.45x; +3 g | 1.36x, 1.35x; +4 g | 1.20x, 1.16x; +5 g | 1.09x, 1.07x; +4 g |
| x16 direct spread 0.4 | 2.37x, 1.81x; +5 g | 1.64x, 1.66x; +9 g | 1.90x, 1.77x; +9 g | 1.54x, 1.39x; +1 g | 1.99x, 2.04x; +5 g | 1.48x, 1.54x; +4 g | 2.18x, 1.76x; +6 g | 1.86x, 1.78x; +9 g | 1.45x, 1.36x; +10 g | 1.20x, 1.16x; +6 g |
| x16 direct nonin .2 | 2.10x, 1.91x; +6 g | 1.59x, 1.63x; +9 g | 1.77x, 1.66x; +9 g | 1.48x, 1.43x; +1 g | 1.88x, 1.79x; +5 g | 1.43x, 1.38x; +3 g | 1.89x, 1.64x; +7 g | 1.79x, 1.65x; +9 g | 1.47x, 1.37x; +9 g | 1.23x, 1.18x; +6 g |
| x16 2L | 2.33x, 1.78x; +4 g | 1.60x, nanx; +3 g | 1.91x, 1.79x; +8 g | - | 1.93x, nanx; +4 g | 1.50x, 1.51x; +4 g | 2.17x, 1.78x; +6 g | 1.86x, 1.76x; +9 g | - | - |
| x16 GL | 2.32x, 1.80x; +2 g | 1.61x, 0.93x; -8 g | 1.85x, 1.70x; +2 g | - | 2.00x, 1.66x; +4 g | 1.46x, 1.42x; -1 g | 2.17x, 1.79x; +5 g | 1.78x, 1.57x; +1 g | 1.44x, 1.34x; +5 g | 1.17x, 1.09x; -6 g |
| x64 direct spread 0.4 | 2.84x, 2.50x; +15 g | 2.17x, 1.97x; +10 g | 2.82x, 2.33x; +15 g | 1.92x, 1.83x; +2 g | 2.77x, 2.08x; +11 g | 1.71x, 1.57x; +5 g | 2.70x, 2.27x; +14 g | 2.48x, 2.22x; +19 g | 1.76x, 1.60x; +16 g | 1.34x, 1.28x; +8 g |
| x64 2L | 2.84x, 2.53x; +14 g | 2.12x, 1.81x; +8 g | 2.83x, 1.84x; +9 g | - | 2.64x, nanx; +5 g | 1.77x, 1.61x; +3 g | 2.63x, 1.49x; +8 g | 2.48x, 2.17x; +16 g | - | - |

**yahoo**

| variant | knn | knn random-1 | knn random-25 | knn clus-1 | knn clus-25 | knn cap-1 | knn cap-25 | thr 0 | thr 1 | thr 2 |
|---|---|---|---|---|---|---|---|---|---|---|
| x4 direct spread 0.4 | 1.73x, 1.61x; +1 g | 1.35x, 1.48x; +3 g | 1.79x, 1.59x; +0 g | 1.44x, 1.28x; +0 g | 1.58x, 1.52x; +1 g | 1.20x, 1.13x; -1 g | 1.46x, 1.56x; +2 g | 1.58x, 1.55x; +1 g | 1.31x, 1.30x; +5 g | 1.14x, 1.12x; +6 g |
| x16 direct spread 0.4 | 4.09x, 2.61x; +0 g | 1.88x, 1.69x; +5 g | 3.40x, 2.53x; +1 g | 1.83x, 1.58x; +0 g | 2.68x, 2.43x; +1 g | 1.45x, 1.30x; -2 g | 2.50x, 2.05x; +3 g | 3.14x, 2.45x; +1 g | 1.74x, 1.74x; +11 g | 1.32x, 1.27x; +10 g |
| x16 direct nonin .2 | 4.04x, 2.45x; +0 g | 1.92x, 1.77x; +4 g | 3.64x, 2.47x; +1 g | 1.85x, 1.63x; +0 g | 2.46x, 2.60x; +2 g | 1.43x, 1.30x; -3 g | 2.31x, 1.98x; +3 g | 3.19x, 2.42x; +1 g | 1.81x, 1.80x; +10 g | 1.37x, 1.31x; +9 g |
| x16 2L | 4.13x, 1.69x; -0 g | 1.81x, 1.49x; +4 g | 3.42x, 2.29x; +1 g | - | 2.69x, 2.29x; +1 g | 1.39x, 1.24x; -2 g | 2.51x, 2.05x; +2 g | 3.08x, 2.09x; +1 g | 1.69x, 1.54x; +10 g | 1.30x, 1.21x; +8 g |
| x16 GL | 4.21x, 2.46x; -0 g | 1.87x, 1.63x; +4 g | 3.29x, 1.75x; +0 g | - | 2.69x, 2.42x; +1 g | 1.45x, 1.30x; -2 g | 2.42x, 1.92x; +2 g | 3.13x, 2.28x; +1 g | 1.73x, 1.70x; +10 g | 1.32x, 1.26x; +8 g |
| x64 direct spread 0.4 | 7.76x, 6.21x; +1 g | 3.21x, 2.89x; +2 g | 6.56x, 5.40x; +2 g | 3.15x, 2.18x; +0 g | 5.34x, 3.99x; +2 g | 2.24x, 1.79x; -5 g | 5.70x, 4.23x; +2 g | 5.35x, 4.53x; +3 g | 2.38x, 2.40x; +19 g | 1.57x, 1.49x; +11 g |
| x64 2L | 7.61x, 6.20x; +1 g | 3.13x, 2.61x; +2 g | 6.35x, 3.96x; +1 g | - | 5.32x, 3.81x; +1 g | - | 5.64x, 4.04x; +2 g | 5.10x, 3.28x; +2 g | 2.37x, 2.37x; +18 g | 1.52x, 1.38x; +9 g |

### C, D, E. Trees

Per dataset: (flat) items / centres / groups at each recall for the flat orders on each partition; (C) beams at the
flat order's nprobe for recall 0.9: recall; share of true neighbours lost per tree level; centres scored (share of
K), for widths m1..m8 (nodes) and w2..w16 (clusters); (D) payoff: fewest items / groups within a budget of centres;
(E) items / centres / groups at recall 0.9 under visible sets and for thresholds (beam rows: the groups as the tree
on the real fit, 256 nodes on the refits, statistical key).

**laion, flat**

| partition, key | at recall 0.8 | 0.9 | 0.95 | 0.99 | est. us at 0.9 | at 0.95 |
|---|---|---|---|---|---|---|
| real flat centre | 21,937 / 500 / 5 | 61,341 / 500 / 11 | 115,895 / 500 / 18 | 287,741 / 500 / 33 | 4,114 | 7,669 |
| real flat spread | 18,373 / 500 / 5 | 36,722 / 500 / 8 | 77,936 / 500 / 14 | 216,146 / 500 / 30 | 2,519 | 5,242 |
| real flat H r4 | 17,104 / 500 / 4 | 34,162 / 500 / 7 | 62,819 / 500 / 12 | 210,297 / 500 / 28 | 2,344 | 4,238 |
| refit5000 flat centre | 6,468 / 5,000 / 8 | 20,446 / 5,000 / 15 | 59,128 / 5,000 / 29 | 231,298 / 5,000 / 52 | 2,351 | 5,742 |
| refit5000 flat spread | 4,670 / 5,000 / 6 | 11,815 / 5,000 / 11 | 39,065 / 5,000 / 24 | 138,865 / 5,000 / 45 | 1,566 | 4,052 |
| refit5000 flat H r4 | 4,013 / 5,000 / 5 | 11,032 / 5,000 / 10 | 30,509 / 5,000 / 18 | 127,357 / 5,000 / 41 | 1,452 | 3,173 |
| refit20000 flat centre | 4,430 / 20,000 / 17 | 15,771 / 20,000 / 31 | 41,161 / 20,000 / 44 | 254,005 / 20,000 / 59 | 4,606 | 8,564 |
| refit20000 flat spread | 3,087 / 20,000 / 9 | 10,119 / 20,000 / 18 | 25,750 / 20,000 / 29 | 170,290 / 20,000 / 54 | 3,214 | 5,533 |
| refit20000 flat H r4 | 3,087 / 20,000 / 9 | 10,119 / 20,000 / 18 | 25,750 / 20,000 / 29 | 170,290 / 20,000 / 54 | 3,214 | 5,533 |

**laion, C**

| partition | tree | node key | m1 | m2 | m4 | m8 | w2 | w4 | w8 | w16 |
|---|---|---|---|---|---|---|---|---|---|---|
| real np 18 rec 0.906 | groups | c | 0.874; 0.062; 232 (0.46) | 0.893; 0.021; 378 (0.76) | 0.906; 0.000; 564 (1.13) | 0.906; 0.000; 564 (1.13) | 0.731; 0.253; 106 (0.21) | 0.833; 0.127; 143 (0.29) | 0.870; 0.070; 215 (0.43) | 0.893; 0.024; 358 (0.72) |
| real np 18 rec 0.906 | groups | st 1 | 0.903; 0.021; 269 (0.54) | 0.906; 0.002; 406 (0.81) | 0.906; 0.000; 564 (1.13) | 0.906; 0.000; 564 (1.13) | 0.777; 0.214; 107 (0.21) | 0.868; 0.098; 143 (0.29) | 0.896; 0.039; 215 (0.43) | 0.906; 0.006; 357 (0.71) |
| refit5000 np 55 rec 0.903 | hkm 256 | c | 0.890; 0.048; 1,402 (0.28) | 0.898; 0.018; 2,523 (0.50) | 0.903; 0.000; 4,622 (0.92) | 0.903; 0.000; 5,256 (1.05) | 0.742; 0.252; 379 (0.08) | 0.797; 0.182; 487 (0.10) | 0.837; 0.131; 708 (0.14) | 0.875; 0.077; 1,150 (0.23) |
| refit5000 np 55 rec 0.903 | hkm 256 | st 1 | 0.901; 0.015; 1,612 (0.32) | 0.903; 0.003; 2,801 (0.56) | 0.903; 0.001; 4,833 (0.97) | 0.903; 0.000; 5,256 (1.05) | 0.814; 0.182; 380 (0.08) | 0.868; 0.113; 490 (0.10) | 0.896; 0.070; 709 (0.14) | 0.898; 0.031; 1,150 (0.23) |
| refit20000 np 197 rec 0.907 | hkm 256 | c | 0.907; 0.000; 16,327 (0.82) | 0.907; 0.000; 20,256 (1.01) | 0.907; 0.000; 20,256 (1.01) | 0.907; 0.000; 20,256 (1.01) | 0.663; 0.335; 704 (0.04) | 0.769; 0.220; 1,107 (0.06) | 0.843; 0.130; 1,893 (0.09) | 0.884; 0.070; 3,467 (0.17) |
| refit20000 np 197 rec 0.907 | hkm 256 | st 1 | 0.907; 0.000; 18,529 (0.93) | 0.907; 0.000; 20,256 (1.01) | 0.907; 0.000; 20,256 (1.01) | 0.907; 0.000; 20,256 (1.01) | 0.777; 0.222; 714 (0.04) | 0.844; 0.141; 1,110 (0.06) | 0.888; 0.078; 1,900 (0.09) | 0.910; 0.034; 3,468 (0.17) |
| refit20000 np 197 rec 0.907 | hkm 16x16x16 | c | 0.853; 0.000 0.000 0.129; 4,019 (0.20) | 0.895; 0.000 0.000 0.061; 5,857 (0.29) | 0.905; 0.000 0.000 0.031; 8,806 (0.44) | 0.908; 0.000 0.000 0.001; 14,616 (0.73) | 0.465; 0.486 0.046 0.001; 501 (0.03) | 0.499; 0.478 0.018 0.000; 957 (0.05) | 0.604; 0.377 0.008 0.000; 1,855 (0.09) | 0.753; 0.221 0.003 0.000; 3,639 (0.18) |
| refit20000 np 197 rec 0.907 | hkm 16x16x16 | st 1 | 0.911; 0.000 0.000 0.022; 6,088 (0.30) | 0.910; 0.000 0.000 0.009; 8,746 (0.44) | 0.910; 0.000 0.000 0.001; 12,740 (0.64) | 0.907; 0.000 0.000 0.000; 18,276 (0.91) | 0.555; 0.396 0.045 0.003; 495 (0.02) | 0.592; 0.386 0.013 0.000; 942 (0.05) | 0.676; 0.305 0.004 0.000; 1,825 (0.09) | 0.804; 0.164 0.001 0.000; 3,581 (0.18) |

**laion, D**

| partition | recall | <= 500 | <= 1,000 | <= 2,000 | <= 4,000 | any |
|---|---|---|---|---|---|---|
| real | 0.9 | 36,531 / 8 g (groups, st 0.5, m2; 371 c) | 36,531 / 8 g (groups, st 0.5, m2; 371 c) | 36,531 / 8 g (groups, st 0.5, m2; 371 c) | 36,531 / 8 g (groups, st 0.5, m2; 371 c) | 36,531 / 8 g (groups, st 0.5, m2; 371 c) |
| real | 0.95 | 77,037 / 14 g (hkm 8x8, sp 1, m1; 440 c) | 77,037 / 14 g (hkm 8x8, sp 1, m1; 440 c) | 77,037 / 14 g (hkm 8x8, sp 1, m1; 440 c) | 77,037 / 14 g (hkm 8x8, sp 1, m1; 440 c) | 77,037 / 14 g (hkm 8x8, sp 1, m1; 440 c) |
| refit5000 | 0.9 | - | 14,225 / 11 g (hkm 256, st 1, w8; 768 c) | 12,127 / 11 g (hkm 32x32, st 1, m2; 1,875 c) | 11,781 / 11 g (hkm 32x32, sp 1, m8; 3,718 c) | 11,781 / 11 g (hkm 32x32, sp 1, m8; 3,718 c) |
| refit5000 | 0.95 | - | 75,362 / 27 g (hkm 256, st 1, w2; 953 c) | 45,616 / 22 g (hkm 256, st 1, w4; 1,091 c) | 35,842 / 21 g (hkm 32x32, st 0.5, w16; 3,124 c) | 35,842 / 21 g (hkm 32x32, st 0.5, w16; 3,124 c) |
| refit20000 | 0.9 | - | - | 23,306 / 18 g (hkm 256, st 1, w4; 1,768 c) | 10,543 / 15 g (hkm 256, st 1, w16; 3,118 c) | 9,849 / 17 g (hkm 16x16x16, c, m8; 13,229 c) |
| refit20000 | 0.95 | - | - | - | 40,838 / 25 g (hkm 256, st 0.5, w4; 2,911 c) | 25,750 / 29 g (flat spread; 20,000 c) |
| real, x4 sub-clusters (spread 0.4) | 0.9 | - | 26,178 / 10 g (two-level P64; 756 c) | 25,970 / 10 g (two-level P256; 1,524 c) | 25,970 / 10 g (two-level P256; 1,524 c) | 25,970 / 10 g (two-level P256; 1,524 c) |
| real, x4 sub-clusters (spread 0.4) | 0.95 | - | 56,690 / 16 g (two-level P64; 756 c) | 54,558 / 17 g (two-level P128; 1,012 c) | 54,558 / 17 g (two-level P128; 1,012 c) | 54,558 / 17 g (two-level P128; 1,012 c) |
| real, x16 sub-clusters (spread 0.4) | 0.9 | - | - | 17,751 / 12 g (two-level P64; 1,524 c) | 16,297 / 12 g (two-level P128; 2,547 c) | 16,209 / 13 g (direct; 7,989 c) |
| real, x16 sub-clusters (spread 0.4) | 0.95 | - | - | 42,754 / 18 g (two-level P64; 1,524 c) | 41,091 / 21 g (two-level P128; 2,547 c) | 40,658 / 22 g (two-level P256; 4,594 c) |
| real, x64 sub-clusters (spread 0.4) | 0.9 | - | - | - | 12,566 / 11 g (two-level P32; 2,548 c) | 9,043 / 18 g (direct; 31,941 c) |
| real, x64 sub-clusters (spread 0.4) | 0.95 | - | - | - | - | 24,355 / 29 g (group-limited P256; 28,856 c) |

**laion, E**

| partition, order | knn random-1 | knn random-25 | knn clus-1 | knn clus-25 | knn cap-1 | knn cap-25 | thr 0 | thr 1 | thr 2 |
|---|---|---|---|---|---|---|---|---|---|
| real flat spread | 975 / 499 / 17 | 11,104 / 500 / 9 | 5,424 / 5 / 3 | 27,876 / 125 / 9 | 4,883 / 217 / 18 | 26,490 / 498 / 14 | 59,635 / 500 / 12 | 203,282 / 500 / 28 | 449,685 / 500 / 48 |
| real st 1 w4 | 1,078 / 277 / 16 | 13,278 / 169 / 8 | 5,424 / 10 / 3 | 32,036 / 113 / 10 | 4,905 / 237 / 18 | 30,278 / 233 / 12 | 71,470 / 205 / 10 | 203,339 / 467 / 28 | 449,685 / 564 / 48 |
| real st 1 w16 | 975 / 563 / 17 | 11,107 / 402 / 9 | 5,424 / 10 / 3 | 27,876 / 175 / 9 | 4,883 / 267 / 18 | 26,490 / 562 / 14 | 59,635 / 519 / 12 | 203,282 / 564 / 28 | 449,685 / 564 / 48 |
| refit5000 flat spread | 491 / 3,874 / 24 | 5,022 / 4,885 / 16 | 3,151 / 727 / 38 | 11,191 / 4,369 / 24 | 3,067 / 800 / 24 | 11,335 / 4,195 / 21 | 21,959 / 5,000 / 17 | 135,459 / 5,000 / 45 | 366,763 / 5,000 / 59 |
| refit5000 st 1 w4 | 520 / 1,034 / 22 | 7,146 / 770 / 16 | 3,151 / 877 / 38 | 13,609 / 1,139 / 23 | 3,101 / 717 / 24 | 14,375 / 940 / 20 | 31,745 / 834 / 17 | 138,411 / 2,879 / 43 | 366,763 / 5,256 / 59 |
| refit5000 st 1 w16 | 491 / 3,240 / 24 | 5,167 / 1,746 / 15 | 3,151 / 877 / 38 | 11,176 / 3,194 / 24 | 3,067 / 944 / 24 | 11,458 / 2,576 / 21 | 22,331 / 1,885 / 16 | 135,459 / 5,256 / 45 | 366,763 / 5,256 / 59 |
| refit20000 flat spread | 376 / 6,330 / 23 | 3,656 / 16,782 / 20 | 2,026 / 1,375 / 31 | 6,923 / 13,332 / 26 | 2,775 / 1,373 / 23 | 7,107 / 11,810 / 22 | 16,623 / 20,000 / 24 | 114,980 / 20,000 / 50 | 335,085 / 20,000 / 59 |
| refit20000 st 1 w4 | 480 / 1,415 / 22 | 7,117 / 1,910 / 20 | 2,123 / 1,299 / 30 | 11,786 / 2,463 / 25 | 2,909 / 1,100 / 21 | 14,116 / 2,115 / 23 | 30,962 / 2,260 / 21 | 124,562 / 8,446 / 45 | 335,085 / 20,256 / 59 |
| refit20000 st 1 w16 | 376 / 3,978 / 23 | 3,739 / 3,894 / 17 | 2,026 / 1,527 / 31 | 7,179 / 5,924 / 24 | 2,775 / 1,509 / 23 | 7,258 / 4,566 / 21 | 17,600 / 5,067 / 20 | 114,980 / 20,256 / 50 | 335,085 / 20,256 / 59 |

**glove, flat**

| partition, key | at recall 0.8 | 0.9 | 0.95 | 0.99 | est. us at 0.9 | at 0.95 |
|---|---|---|---|---|---|---|
| real flat centre | 38,966 / 592 / 7 | 100,417 / 592 / 15 | 180,210 / 592 / 24 | 424,058 / 592 / 41 | 2,961 | 5,284 |
| real flat spread | 27,404 / 592 / 6 | 74,335 / 592 / 14 | 149,312 / 592 / 23 | 363,344 / 592 / 40 | 2,231 | 4,448 |
| real flat H r4 | 26,866 / 592 / 5 | 70,207 / 592 / 12 | 140,716 / 592 / 21 | 348,140 / 592 / 38 | 2,092 | 4,163 |
| refit5000 flat centre | 17,328 / 5,000 / 10 | 58,965 / 5,000 / 23 | 149,318 / 5,000 / 39 | 389,288 / 5,000 / 56 | 2,613 | 6,335 |
| refit5000 flat spread | 10,666 / 5,000 / 8 | 40,848 / 5,000 / 21 | 96,896 / 5,000 / 36 | 291,915 / 5,000 / 53 | 2,012 | 4,503 |
| refit5000 flat H r4 | 11,322 / 5,000 / 7 | 40,601 / 5,000 / 18 | 99,647 / 5,000 / 32 | 335,294 / 5,000 / 53 | 1,897 | 4,357 |
| refit20000 flat centre | 21,798 / 20,000 / 34 | 60,291 / 20,000 / 49 | 110,702 / 20,000 / 56 | 272,680 / 20,000 / 61 | 5,729 | 9,793 |
| refit20000 flat spread | 14,240 / 20,000 / 19 | 43,302 / 20,000 / 35 | 80,152 / 20,000 / 46 | 278,560 / 20,000 / 60 | 3,985 | 6,724 |
| refit20000 flat H r4 | 14,240 / 20,000 / 19 | 43,302 / 20,000 / 35 | 80,152 / 20,000 / 46 | 278,560 / 20,000 / 60 | 3,985 | 6,724 |

**glove, C**

| partition | tree | node key | m1 | m2 | m4 | m8 | w2 | w4 | w8 | w16 |
|---|---|---|---|---|---|---|---|---|---|---|
| real np 38 rec 0.906 | groups | c | 0.905; 0.004; 469 (0.79) | 0.906; 0.000; 656 (1.11) | 0.906; 0.000; 656 (1.11) | 0.906; 0.000; 656 (1.11) | 0.796; 0.186; 147 (0.25) | 0.876; 0.079; 224 (0.38) | 0.898; 0.020; 376 (0.64) | 0.906; 0.000; 656 (1.11) |
| real np 38 rec 0.906 | groups | st 1 | 0.906; 0.003; 499 (0.84) | 0.906; 0.000; 656 (1.11) | 0.906; 0.000; 656 (1.11) | 0.906; 0.000; 656 (1.11) | 0.839; 0.146; 150 (0.25) | 0.885; 0.075; 227 (0.38) | 0.904; 0.023; 375 (0.63) | 0.906; 0.000; 656 (1.11) |
| refit5000 np 164 rec 0.907 | hkm 256 | c | 0.908; 0.004; 3,803 (0.76) | 0.907; 0.000; 5,256 (1.05) | 0.907; 0.000; 5,256 (1.05) | 0.907; 0.000; 5,256 (1.05) | 0.812; 0.178; 599 (0.12) | 0.869; 0.100; 926 (0.19) | 0.895; 0.055; 1,584 (0.32) | 0.907; 0.010; 2,894 (0.58) |
| refit5000 np 164 rec 0.907 | hkm 256 | st 1 | 0.907; 0.000; 4,164 (0.83) | 0.907; 0.000; 5,256 (1.05) | 0.907; 0.000; 5,256 (1.05) | 0.907; 0.000; 5,256 (1.05) | 0.846; 0.146; 600 (0.12) | 0.880; 0.089; 932 (0.19) | 0.903; 0.043; 1,585 (0.32) | 0.908; 0.006; 2,895 (0.58) |
| refit20000 np 492 rec 0.910 | hkm 256 | c | 0.910; 0.000; 20,256 (1.01) | 0.910; 0.000; 20,256 (1.01) | 0.910; 0.000; 20,256 (1.01) | 0.910; 0.000; 20,256 (1.01) | 0.756; 0.242; 1,315 (0.07) | 0.846; 0.141; 2,303 (0.12) | 0.879; 0.081; 4,278 (0.21) | 0.904; 0.030; 8,219 (0.41) |
| refit20000 np 492 rec 0.910 | hkm 256 | st 1 | 0.910; 0.000; 20,256 (1.01) | 0.910; 0.000; 20,256 (1.01) | 0.910; 0.000; 20,256 (1.01) | 0.910; 0.000; 20,256 (1.01) | 0.797; 0.201; 1,325 (0.07) | 0.862; 0.125; 2,307 (0.12) | 0.894; 0.070; 4,288 (0.21) | 0.904; 0.022; 8,212 (0.41) |
| refit20000 np 492 rec 0.910 | hkm 16x16x16 | c | 0.874; 0.000 0.000 0.090; 6,385 (0.32) | 0.902; 0.000 0.000 0.021; 10,634 (0.53) | 0.910; 0.000 0.000 0.000; 18,774 (0.94) | 0.910; 0.000 0.000 0.000; 23,332 (1.17) | 0.525; 0.447 0.024 0.003; 1,191 (0.06) | 0.656; 0.310 0.028 0.000; 2,325 (0.12) | 0.765; 0.180 0.022 0.001; 4,552 (0.23) | 0.843; 0.094 0.001 0.000; 8,980 (0.45) |
| refit20000 np 492 rec 0.910 | hkm 16x16x16 | st 1 | 0.910; 0.000 0.000 0.007; 11,864 (0.59) | 0.910; 0.000 0.000 0.003; 16,435 (0.82) | 0.910; 0.000 0.000 0.000; 20,991 (1.05) | 0.910; 0.000 0.000 0.000; 23,332 (1.17) | 0.560; 0.418 0.021 0.000; 1,182 (0.06) | 0.693; 0.286 0.013 0.000; 2,312 (0.12) | 0.786; 0.178 0.009 0.000; 4,554 (0.23) | 0.875; 0.065 0.001 0.000; 9,016 (0.45) |

**glove, D**

| partition | recall | <= 592 | <= 1,184 | <= 2,368 | <= 4,736 | any |
|---|---|---|---|---|---|---|
| real | 0.9 | 74,335 / 14 g (flat spread; 592 c) | 74,330 / 14 g (hkm 16, st 0.5, w16; 583 c) | 74,330 / 14 g (hkm 16, st 0.5, w16; 583 c) | 74,330 / 14 g (hkm 16, st 0.5, w16; 583 c) | 74,330 / 14 g (hkm 16, st 0.5, w16; 583 c) |
| real | 0.95 | 149,312 / 23 g (flat spread; 592 c) | 149,294 / 23 g (hkm 16, sp 1, w8; 589 c) | 149,294 / 23 g (hkm 16, sp 1, w8; 589 c) | 149,294 / 23 g (hkm 16, sp 1, w8; 589 c) | 149,294 / 23 g (hkm 16, sp 1, w8; 589 c) |
| refit5000 | 0.9 | - | 78,190 / 21 g (hkm 256, st 0.5, w2; 856 c) | 43,396 / 20 g (hkm 32x32, st 0.5, m1; 2,263 c) | 40,485 / 20 g (hkm 256, c, m1; 3,484 c) | 40,485 / 20 g (hkm 256, c, m1; 3,484 c) |
| refit5000 | 0.95 | - | - | 107,757 / 32 g (hkm 256, st 0.5, w4; 1,940 c) | 95,369 / 34 g (hkm 256, c, w8; 3,229 c) | 95,369 / 34 g (hkm 256, c, w8; 3,229 c) |
| refit20000 | 0.9 | - | - | - | 74,366 / 29 g (hkm 256, st 1, w4; 3,286 c) | 43,302 / 35 g (hkm 32x32, sp 1, m2; 20,784 c) |
| refit20000 | 0.95 | - | - | - | - | 80,152 / 46 g (hkm 32x32, st 0.5, m1; 20,747 c) |
| real, x4 sub-clusters (spread 0.4) | 0.9 | - | 56,598 / 17 g (two-level P128; 1,104 c) | 54,254 / 12 g (group-limited P32; 1,263 c) | 54,254 / 12 g (group-limited P32; 1,263 c) | 54,254 / 12 g (group-limited P32; 1,263 c) |
| real, x4 sub-clusters (spread 0.4) | 0.95 | - | 119,965 / 28 g (two-level P128; 1,104 c) | 113,515 / 27 g (group-limited P128; 2,125 c) | 113,515 / 27 g (group-limited P128; 2,125 c) | 113,515 / 27 g (group-limited P128; 2,125 c) |
| real, x16 sub-clusters (spread 0.4) | 0.9 | - | - | 37,104 / 17 g (two-level P64; 1,616 c) | 31,565 / 18 g (two-level P256; 4,688 c) | 31,553 / 18 g (direct; 9,472 c) |
| real, x16 sub-clusters (spread 0.4) | 0.95 | - | - | - | 82,454 / 32 g (two-level P256; 4,688 c) | 82,121 / 29 g (group-limited P128; 6,726 c) |
| real, x64 sub-clusters (spread 0.4) | 0.9 | - | - | - | 31,548 / 19 g (two-level P64; 4,688 c) | 26,276 / 29 g (direct; 37,888 c) |
| real, x64 sub-clusters (spread 0.4) | 0.95 | - | - | - | - | 57,693 / 30 g (two-level P128; 8,784 c) |

**glove, E**

| partition, order | knn random-1 | knn random-25 | knn clus-1 | knn clus-25 | knn cap-1 | knn cap-25 | thr 0 | thr 1 | thr 2 |
|---|---|---|---|---|---|---|---|---|---|
| real flat spread | 2,065 / 591 / 29 | 26,364 / 592 / 18 | 4,779 / 7 / 3 | 35,242 / 141 / 11 | 3,113 / 468 / 31 | 34,338 / 592 / 19 | 115,182 / 592 / 19 | 400,547 / 592 / 43 | 684,422 / 592 / 55 |
| real st 1 w4 | 1,993 / 465 / 26 | 28,650 / 294 / 14 | 4,779 / 13 / 3 | 36,921 / 123 / 11 | 3,113 / 518 / 31 | 37,073 / 317 / 16 | 127,819 / 322 / 16 | 400,547 / 656 / 43 | 684,422 / 656 / 55 |
| real st 1 w16 | 2,065 / 655 / 29 | 26,364 / 656 / 18 | 4,779 / 13 / 3 | 35,242 / 192 / 11 | 3,113 / 528 / 31 | 34,338 / 656 / 19 | 115,182 / 656 / 19 | 400,547 / 656 / 43 | 684,422 / 656 / 55 |
| refit5000 flat spread | 1,193 / 4,137 / 37 | 13,149 / 4,994 / 25 | 3,210 / 1,697 / 47 | 19,784 / 4,866 / 32 | 2,397 / 2,181 / 39 | 13,145 / 4,855 / 22 | 70,087 / 5,000 / 29 | 305,101 / 5,000 / 54 | 608,581 / 5,000 / 60 |
| refit5000 st 1 w4 | 1,250 / 1,918 / 34 | 18,980 / 1,410 / 25 | 3,210 / 1,914 / 47 | 23,240 / 1,673 / 29 | 2,448 / 1,941 / 38 | 19,429 / 1,214 / 22 | 80,423 / 1,483 / 26 | 305,099 / 5,181 / 54 | 608,581 / 5,256 / 60 |
| refit5000 st 1 w16 | 1,193 / 4,383 / 37 | 13,784 / 3,580 / 25 | 3,210 / 1,914 / 47 | 19,785 / 4,917 / 32 | 2,397 / 2,391 / 39 | 13,428 / 2,935 / 22 | 70,107 / 4,559 / 29 | 305,101 / 5,256 / 54 | 608,581 / 5,256 / 60 |
| refit20000 flat spread | 975 / 6,896 / 37 | 10,326 / 17,848 / 33 | 2,754 / 2,853 / 47 | 15,498 / 16,135 / 38 | 2,339 / 3,552 / 38 | 16,911 / 15,243 / 36 | 62,033 / 20,000 / 42 | 264,716 / 20,000 / 59 | 550,547 / 20,000 / 63 |
| refit20000 st 1 w4 | 1,199 / 2,655 / 33 | 18,383 / 3,088 / 28 | 2,765 / 2,871 / 46 | 23,559 / 3,505 / 31 | 2,553 / 2,495 / 34 | 26,921 / 3,307 / 30 | 94,075 / 4,140 / 34 | 272,616 / 13,062 / 56 | 550,547 / 20,256 / 63 |
| refit20000 st 1 w16 | 975 / 7,089 / 37 | 10,623 / 6,619 / 27 | 2,754 / 3,030 / 47 | 15,931 / 9,122 / 35 | 2,339 / 3,720 / 38 | 17,149 / 8,072 / 33 | 63,548 / 10,844 / 38 | 264,716 / 20,256 / 59 | 550,547 / 20,256 / 63 |

**yahoo, flat**

| partition, key | at recall 0.8 | 0.9 | 0.95 | 0.99 | est. us at 0.9 | at 0.95 |
|---|---|---|---|---|---|---|
| real flat centre | 7,453 / 339 / 2 | 15,898 / 339 / 4 | 29,217 / 339 / 7 | 110,649 / 339 / 23 | 753 | 1,356 |
| real flat spread | 7,177 / 339 / 2 | 17,016 / 339 / 5 | 28,133 / 339 / 7 | 116,780 / 339 / 26 | 806 | 1,312 |
| real flat H r4 | 6,794 / 339 / 2 | 15,224 / 339 / 4 | 26,090 / 339 / 7 | 92,606 / 339 / 21 | 724 | 1,217 |
| refit5000 flat centre | 1,337 / 5,000 / 3 | 3,242 / 5,000 / 5 | 7,593 / 5,000 / 9 | 32,611 / 5,000 / 27 | 627 | 984 |
| refit5000 flat spread | 1,382 / 5,000 / 3 | 2,933 / 5,000 / 5 | 6,397 / 5,000 / 9 | 24,978 / 5,000 / 24 | 607 | 898 |
| refit5000 flat H r4 | 1,167 / 5,000 / 3 | 2,603 / 5,000 / 5 | 5,420 / 5,000 / 8 | 22,466 / 5,000 / 21 | 573 | 806 |
| refit20000 flat centre | 835 / 20,000 / 5 | 2,220 / 20,000 / 9 | 6,090 / 20,000 / 17 | 21,889 / 20,000 / 38 | 1,812 | 2,556 |
| refit20000 flat spread | 794 / 20,000 / 4 | 2,241 / 20,000 / 7 | 4,716 / 20,000 / 12 | 20,785 / 20,000 / 32 | 1,756 | 2,179 |
| refit20000 flat H r4 | 794 / 20,000 / 4 | 2,241 / 20,000 / 7 | 4,716 / 20,000 / 12 | 20,785 / 20,000 / 32 | 1,756 | 2,179 |

**yahoo, C**

| partition | tree | node key | m1 | m2 | m4 | m8 | w2 | w4 | w8 | w16 |
|---|---|---|---|---|---|---|---|---|---|---|
| real np 10 rec 0.919 | groups | c | 0.897; 0.053; 117 (0.34) | 0.914; 0.014; 172 (0.51) | 0.917; 0.003; 277 (0.82) | 0.919; 0.000; 403 (1.19) | 0.830; 0.147; 87 (0.26) | 0.890; 0.066; 107 (0.31) | 0.916; 0.020; 147 (0.43) | 0.917; 0.004; 227 (0.67) |
| real np 10 rec 0.919 | groups | st 1 | 0.916; 0.027; 122 (0.36) | 0.919; 0.008; 178 (0.52) | 0.919; 0.001; 285 (0.84) | 0.919; 0.000; 403 (1.19) | 0.888; 0.094; 87 (0.26) | 0.908; 0.049; 107 (0.31) | 0.917; 0.017; 147 (0.43) | 0.919; 0.001; 227 (0.67) |
| refit5000 np 22 rec 0.911 | hkm 256 | c | 0.887; 0.050; 687 (0.14) | 0.897; 0.027; 1,121 (0.22) | 0.907; 0.010; 1,988 (0.40) | 0.911; 0.000; 3,738 (0.75) | 0.691; 0.298; 313 (0.06) | 0.770; 0.209; 355 (0.07) | 0.840; 0.120; 444 (0.09) | 0.870; 0.068; 620 (0.12) |
| refit5000 np 22 rec 0.911 | hkm 256 | st 1 | 0.905; 0.022; 780 (0.16) | 0.908; 0.011; 1,285 (0.26) | 0.911; 0.000; 2,250 (0.45) | 0.911; 0.000; 4,009 (0.80) | 0.737; 0.254; 314 (0.06) | 0.825; 0.154; 360 (0.07) | 0.871; 0.089; 446 (0.09) | 0.890; 0.049; 625 (0.13) |
| refit20000 np 55 rec 0.900 | hkm 256 | c | 0.897; 0.009; 4,362 (0.22) | 0.900; 0.002; 8,709 (0.44) | 0.900; 0.000; 17,609 (0.88) | 0.900; 0.000; 20,256 (1.01) | 0.587; 0.408; 416 (0.02) | 0.673; 0.316; 523 (0.03) | 0.773; 0.196; 744 (0.04) | 0.846; 0.107; 1,182 (0.06) |
| refit20000 np 55 rec 0.900 | hkm 256 | st 1 | 0.900; 0.004; 5,161 (0.26) | 0.900; 0.001; 9,929 (0.50) | 0.900; 0.000; 18,729 (0.94) | 0.900; 0.000; 20,256 (1.01) | 0.644; 0.349; 420 (0.02) | 0.748; 0.234; 528 (0.03) | 0.817; 0.152; 744 (0.04) | 0.875; 0.074; 1,188 (0.06) |
| refit20000 np 55 rec 0.900 | hkm 16x16x16 | c | 0.800; 0.000 0.023 0.157; 1,302 (0.07) | 0.856; 0.000 0.004 0.096; 2,392 (0.12) | 0.887; 0.000 0.000 0.049; 4,593 (0.23) | 0.899; 0.000 0.000 0.020; 6,047 (0.30) | 0.502; 0.290 0.202 0.002; 176 (0.01) | 0.572; 0.290 0.126 0.004; 308 (0.02) | 0.648; 0.290 0.042 0.003; 574 (0.03) | 0.720; 0.238 0.019 0.000; 1,092 (0.05) |
| refit20000 np 55 rec 0.900 | hkm 16x16x16 | st 1 | 0.896; 0.000 0.008 0.041; 1,677 (0.08) | 0.900; 0.000 0.000 0.020; 3,071 (0.15) | 0.900; 0.000 0.000 0.009; 5,809 (0.29) | 0.900; 0.000 0.000 0.002; 8,019 (0.40) | 0.585; 0.248 0.162 0.002; 173 (0.01) | 0.678; 0.248 0.064 0.003; 305 (0.02) | 0.699; 0.248 0.031 0.003; 563 (0.03) | 0.734; 0.229 0.013 0.000; 1,077 (0.05) |

**yahoo, D**

| partition | recall | <= 339 | <= 678 | <= 1,356 | <= 2,712 | any |
|---|---|---|---|---|---|---|
| real | 0.9 | 16,385 / 5 g (hkm 8x8, st 1, m1; 142 c) | 16,385 / 5 g (hkm 8x8, st 1, m1; 142 c) | 16,385 / 5 g (hkm 8x8, st 1, m1; 142 c) | 16,385 / 5 g (hkm 8x8, st 1, m1; 142 c) | 16,385 / 5 g (hkm 8x8, st 1, m1; 142 c) |
| real | 0.95 | 27,909 / 7 g (hkm 8x8, st 0.5, m1; 174 c) | 27,909 / 7 g (hkm 8x8, st 0.5, m1; 174 c) | 27,909 / 7 g (hkm 8x8, st 0.5, m1; 174 c) | 27,909 / 7 g (hkm 8x8, st 0.5, m1; 174 c) | 27,909 / 7 g (hkm 8x8, st 0.5, m1; 174 c) |
| refit5000 | 0.9 | - | 4,134 / 5 g (groups, st 1, w16; 557 c) | 3,010 / 5 g (hkm 256, st 1, m2; 1,211 c) | 2,923 / 5 g (hkm 32x32, sp 1, m8; 2,370 c) | 2,923 / 5 g (hkm 32x32, sp 1, m8; 2,370 c) |
| refit5000 | 0.95 | - | 17,856 / 7 g (groups, st 1, w4; 626 c) | 7,623 / 10 g (hkm 256, st 1, w16; 1,127 c) | 6,309 / 9 g (hkm 16x16, sp 1, m2; 2,615 c) | 6,309 / 9 g (hkm 16x16, sp 1, m2; 2,615 c) |
| refit20000 | 0.9 | - | - | 4,325 / 9 g (hkm 256, st 1, w8; 1,141 c) | 2,430 / 7 g (hkm 32x32, st 0.5, m1; 2,488 c) | 2,171 / 7 g (hkm 256, st 0.5, m2; 9,350 c) |
| refit20000 | 0.95 | - | - | - | 5,515 / 12 g (hkm 256, st 1, w16; 2,497 c) | 4,566 / 12 g (hkm 16x16x16, c, m8; 8,804 c) |
| real, x4 sub-clusters (spread 0.4) | 0.9 | - | 9,772 / 5 g (two-level P32; 467 c) | 9,772 / 5 g (two-level P32; 467 c) | 9,772 / 5 g (two-level P32; 467 c) | 9,772 / 5 g (two-level P32; 467 c) |
| real, x4 sub-clusters (spread 0.4) | 0.95 | - | 17,944 / 9 g (two-level P64; 595 c) | 17,938 / 9 g (two-level P128; 851 c) | 17,938 / 9 g (two-level P128; 851 c) | 17,938 / 9 g (two-level P128; 851 c) |
| real, x16 sub-clusters (spread 0.4) | 0.9 | - | 4,089 / 5 g (two-level P16; 595 c) | 4,008 / 5 g (group-limited P16; 1,144 c) | 4,008 / 5 g (group-limited P16; 1,144 c) | 4,008 / 5 g (group-limited P16; 1,144 c) |
| real, x16 sub-clusters (spread 0.4) | 0.95 | - | 17,064 / 8 g (two-level P16; 595 c) | 11,724 / 8 g (group-limited P16; 1,144 c) | 11,088 / 10 g (two-level P128; 2,387 c) | 11,084 / 10 g (group-limited P256; 5,596 c) |
| real, x64 sub-clusters (spread 0.4) | 0.9 | - | - | - | 2,327 / 6 g (two-level P32; 2,387 c) | 2,176 / 6 g (group-limited P128; 16,327 c) |
| real, x64 sub-clusters (spread 0.4) | 0.95 | - | - | - | 5,390 / 9 g (two-level P32; 2,387 c) | 4,640 / 10 g (group-limited P64; 10,376 c) |

**yahoo, E**

| partition, order | knn random-1 | knn random-25 | knn clus-1 | knn clus-25 | knn cap-1 | knn cap-25 | thr 0 | thr 1 | thr 2 |
|---|---|---|---|---|---|---|---|---|---|
| real flat spread | 485 / 339 / 12 | 5,667 / 339 / 6 | 6,066 / 4 / 3 | 12,616 / 85 / 5 | 4,906 / 68 / 23 | 7,161 / 339 / 8 | 23,638 / 339 / 6 | 106,005 / 339 / 24 | 330,741 / 339 / 51 |
| real st 1 w4 | 480 / 164 / 12 | 5,876 / 113 / 6 | 6,066 / 8 / 3 | 13,898 / 73 / 5 | 4,906 / 93 / 23 | 7,161 / 125 / 7 | 24,585 / 115 / 6 | 106,353 / 280 / 24 | 330,741 / 403 / 51 |
| real st 1 w16 | 485 / 403 / 12 | 5,668 / 245 / 6 | 6,066 / 8 / 3 | 12,616 / 130 / 5 | 4,906 / 93 / 23 | 7,161 / 303 / 8 | 23,639 / 252 / 6 | 106,005 / 403 / 24 | 330,741 / 403 / 51 |
| refit5000 flat spread | 241 / 3,481 / 20 | 1,221 / 4,995 / 7 | 2,326 / 987 / 46 | 3,418 / 4,655 / 16 | 3,028 / 343 / 29 | 1,926 / 4,825 / 10 | 5,549 / 5,000 / 8 | 63,359 / 5,000 / 43 | 258,582 / 5,000 / 63 |
| refit5000 st 1 w4 | 282 / 849 / 20 | 2,183 / 515 / 9 | 2,326 / 1,159 / 46 | 4,257 / 756 / 16 | 3,028 / 427 / 29 | 3,046 / 611 / 12 | 8,983 / 521 / 9 | 65,852 / 2,283 / 41 | 258,582 / 5,256 / 63 |
| refit5000 st 1 w16 | 241 / 2,270 / 20 | 1,297 / 844 / 7 | 2,326 / 1,159 / 46 | 3,361 / 1,818 / 16 | 3,028 / 427 / 29 | 1,981 / 1,151 / 10 | 5,878 / 921 / 8 | 63,359 / 5,256 / 43 | 258,582 / 5,256 / 63 |
| refit20000 flat spread | 175 / 5,158 / 18 | 1,014 / 18,255 / 11 | 1,396 / 1,756 / 40 | 1,897 / 15,065 / 17 | 2,227 / 744 / 24 | 1,345 / 15,101 / 12 | 4,231 / 20,000 / 11 | 47,981 / 20,000 / 49 | 216,987 / 20,000 / 63 |
| refit20000 st 1 w4 | 249 / 1,002 / 20 | 2,170 / 1,124 / 13 | 1,516 / 1,426 / 37 | 3,644 / 1,519 / 19 | 2,227 / 831 / 24 | 2,434 / 1,122 / 14 | 9,128 / 1,230 / 14 | 55,280 / 6,377 / 46 | 216,987 / 20,256 / 63 |
| refit20000 st 1 w16 | 179 / 2,415 / 18 | 1,152 / 2,018 / 10 | 1,396 / 1,937 / 40 | 2,119 / 3,254 / 17 | 2,227 / 831 / 24 | 1,407 / 2,196 / 11 | 4,878 / 2,228 / 11 | 47,980 / 20,075 / 49 | 216,987 / 20,256 / 63 |
