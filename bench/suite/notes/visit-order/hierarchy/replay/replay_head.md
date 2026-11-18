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

