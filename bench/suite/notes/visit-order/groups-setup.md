# A group's setup: where it goes and what an exact cheaper one could save

Question: can opening a group cost less while every value the index reads stays the same, bit for bit? Analysis from
the code (`rust/src/v2/q.rs` at `c6374f0`: `group_q2r`, `setup_groups`, `get_gq2`, `cluster_q2w`, `NuCut`,
`run_cluster`; `rust/src/v2/setup.rs`) and from operation and byte counts. Nothing was timed here: the cycle figures
are static estimates (LLVM's Sapphire Rapids model through `llvm-mca`; this machine reports that core's AMX and
VNNI), and the only measurements quoted are the ones I was given (median of 100 queries on the suite's indexes: 4 µs
per group at n = 128, 15 at 384, 28 at 512). The timing that would settle the open figures is prepared:
`scripts/groups_setup_time.sh` (section 7).

**Answer.** Not by computing less of the basis product: every cluster opened in a group reads every coordinate of
`V_q^T q`, so the n² int8 bytes of the basis are read in full whatever the levels reached (section 2). What can be
removed exactly is around it: the post-processing's serial sums, the per-group rotations of `get_gq2`'s path, and the
basis kernel's reductions when its bases are in cache. By the counts these are about 5 to 15% of a group's setup at
n = 384 to 512 (the upper end on the one-group path only), under 2% of an unfiltered approximate query: **below
n = 768 the room is small, so I stopped after the analysis** and changed nothing in the library. Above it one item grows faster than the basis: a group set up
alone (`get_gq2`, which exact k-NN's seed uses for every group it probes) reads all four f64 rotations, 2.4 n² bytes,
2.4 times the basis; reading them once for all the probed groups is exact and is the change worth making when indexes
of n ≥ 768 return (section 6).

## 1. What a group's setup computes

Per query and group g (`group_q2r`), n the padded dimension, `ndl` deterministic levels (2 in the suite's indexes:
16 and 32 bytes), J = 4 randomised blocks (`BOUNDS`; at n = 512 they hold 64, 80, 176 and 192 positions):

| Phase (profiler slot) | Output | Work | Bytes read |
|---|---|---|---|
| Basis product (`P_BASIS`) | `qv` = V_q^T q: per row c, the exact integer S_c = Σ_i v8[c][i]·fx_i in two 13-bit halves, then vsc[c]·S_c·2^-24 | 2n² 16-bit multiply-adds | n² (the int8 basis) + 8n |
| Post-processing (`P_GPOST`) | `wq[..n]` = qv·σ; `qvd2`, `cq` per deterministic level; `qvt_pos` = qv·dt by position; `qdt2` per block; `a2` per block; `l1det` | about (8 + 6·ndl)·n flops; serial sums: per level four lanes of n/4 terms (`cq`), one per block (`a2`, n terms in all), n terms (`l1det`) | about (42 + 4·ndl)·n of group data; writes (56 + 4·ndl)·n |
| Rotations (`P_ROT`, or inside `P_GPOST` on one group's path) | `wq[n..]` = R_j (qv·dt) per block; `l1j` | Σ_j nb_j² f64 multiply-adds ≈ 0.30 n² | 8 Σ nb_j² ≈ 2.4 n²: once per group (`get_gq2` → `matvec64`) or once per call for all groups set up together (`setup_groups` → `dgemm`) |

Counts per group (ndl = 2; blocks in the suite's proportions; the p0 cycles are the LLVM model's: the VNNI
multiply-add runs on port 0 only, so the basis loop is 8 cycles per 4 rows × 32 columns, n²/16 per group, and each
4-row block ends with 8 horizontal sums of about 28 cycles, 22 of them on port 0):

| n | basis bytes | basis p0 cycles (loop + sums) | rotation MACs | rotation bytes (one group alone) | post-processing cycles, library ≈ 12n / fused ≈ 7n | 64 groups' bases |
|---|---|---|---|---|---|---|
| 128 | 16 KB | 1,024 + 704 | 5,120 | 41 KB | 1,540 / 900 | 1.0 MB |
| 384 | 147 KB | 9,216 + 2,112 | 43,520 | 348 KB | 4,610 / 2,690 | 9.4 MB |
| 512 | 262 KB | 16,384 + 2,816 | 78,336 | 627 KB | 6,140 / 3,580 | 16.8 MB |
| 768 | 590 KB | 36,864 + 4,224 | 176,256 | 1.41 MB | 9,220 / 5,380 | 37.7 MB |
| 2048 | 4.19 MB | 262,144 + 11,264 | 1,253,376 | 10.0 MB | 24,580 / 14,340 | 268 MB |

The post-processing figures: the library's serial sums take about 10n cycles at 4 cycles per dependent add (`cq`
ndl·n, `a2` 4n, `l1det` 4n) plus about 2n of other work; fused, `l1det`'s chain (4n, its terms in order) hides the
level sums and the block sums run side by side (4 × the largest block ≈ 1.5n). The rotations' own |.| sums (`l1j`)
add another 4n chain on the one-group path.

Against the measured 28 µs at n = 512 (64,000 to 84,000 cycles at 2.3 to 3 GHz), the compute in this table is
about 35,000 to 40,000 cycles (basis 19,000, post 6,000 plus allocations, rotations 10,000 to 13,000 on the one-group
path); the rest is waiting for the basis, which per query comes from the last-level cache (a query's 16.8 MB of bases
does not fit the 2 MB L2). At n = 128 a query's bases fit in L2 and the 4-row sums are 40% of the kernel's port-0
time.

## 2. Which outputs each reader takes: why the product cannot be partial

The levels alone would allow a partial product: deterministic level l reads `wq[c]` only at the coordinates with
fields at that level (`group_tab`, built per level when first reached), at most 8 × 16 = 128 coordinates per level at
16 bytes, so at most 256 of n over both levels (half the coordinates at n = 512); a randomised level reads the rotated
weights of the blocks rising there, and each of those mixes every coordinate of its block. But opening any cluster of
the group reads every coordinate, before any level:

- `cluster_q2w`: α = Σ_j (m · qdt2_j) / btot over all four blocks, so over all n coordinates (`ab`); `dwa[l]` for
  every deterministic level from `cq[l]` and `m · qvd2_l` (all coordinates); `nd[j]` for every block from `a2[j]`
  (all positions); `ewj` from `l1j` (all rotated weights); `ewdet` from `l1det`. These are the certified intervals'
  half-width terms of every level, including those of blocks not yet coded.
- `NuCut::new` (norm-sorted indexes): |qv|² and qv · m over all coordinates.
- Verify mode (`run_cluster`, the reference intervals): the whole `wq`.

So every output of the basis product is read whenever a group is opened, and a partial product ("idea 4" of
`reports/annideas/REPORT.md`, `NEXT.md`) needs those half-width terms replaced by fitted scales for the uncoded
part: that changes the certified intervals, so it is out of bounds here. It is not implemented.

The product's value depends only on the exact integers S_c (|S_c| < 2^42, converted exactly, then one rounding
`(vsc·S)·2^-24`). Any exact integer schedule therefore gives the same bits: another order of the 32-bit sums (they
wrap modulo 2^32 and the true sum fits), another register width, u8 query digits with `vpdpbusd`, AMX. This makes
kernel changes safe; none of them reads fewer than the n² bytes.

## 3. The candidates

| Candidate | Saves per group (exact counts) | Exactly equivalent? | Complexity |
|---|---|---|---|
| Basis rows for the levels reached only (lazily per level, or the deterministic levels' coordinates) | up to 1 − 256/n of the basis (50% at 512) | **No**: section 2; it needs new half-width terms for the uncoded coordinates | (not done) |
| Basis kernel: one transpose-and-add tree for 16 row sums, the scaling in vectors (R = 8 rows) | the 4-row sums' port-0 cycles 5.5n → about 1.5n: 500 of 1,700 (n = 128), 1,500 of 11,300 (384), 2,000 of 19,200 (512), 3,000 of 41,100 (768), 8,000 of 273,000 (2048); only while the bases are in cache | Yes (integer sums; same S, same two f64 products) | medium: AVX-512 and AVX2 kernels |
| Basis kernel: u8 query digits with `vpdpbusd` (no int8 → int16 widening) | the widening is n²/32 port-5 operations; the multiply-adds stay n²/16 on port 0, so nothing on this core | Yes | high; AVX2 has no VNNI |
| AMX-INT8 for the product | the compute, not the n² bytes | Yes | high: a third instruction set, OS permission |
| Prefetch depth | at most the gap between the kernel and the plain read of the basis | Yes | low, but only a measurement can set it |
| Post-processing in two passes, independent serial sums interleaved (`post_fused` in `run2.rs`) | about 12n → 7n cycles (10n of the library's are serial sums; `l1det`'s n terms in order stay serial): 640 (128), 1,900 (384), 2,600 (512), 3,800 (768), 10,000 (2048) | Yes: each sum keeps its terms and order (checked bit for bit) | low |
| A group's rotations read once for several groups, each product with `matvec64`'s arithmetic (`rot_together` in `run2.rs`) | on `get_gq2`'s path, the rotations' 2.4 n² bytes per group after the first: 41 KB, 348 KB, 627 KB, 1.41 MB, 10.0 MB; below n ≈ 768 they come from L2, above from the last-level cache | Yes (checked bit for bit against `group_q2r`) | medium: the probe's groups are known before it runs (section 6) |
| Rotations deferred until a randomised level reads them | the Σ nb_j² multiply-adds of a group none of whose opened clusters reaches a randomised level | Yes, if each group keeps the method it has today (`matvec64` on its own, `dgemm` in a batch) | medium-high: `ewj` would be computed at the first randomised level, not in `cluster_q2w` |
| The next group's basis prefetched into L2 during this group's post-processing and rotations | up to the post-processing and rotation time, hidden behind the stream | Yes | medium; only where the next group is known (`setup_groups`); needs a measurement to tune |
| Work shared between the groups of one query | nothing: each group's dual basis is fitted on its own (`fit.py --basis dual`, `DESIGN.md` 2), with no common factor, and an exact integer product does not factor | — | — |
| Several queries per basis read (a batched product) | the basis bytes divided by the batch | Yes (integer) | an API for batches; not a latency gain |
| `knn2_fast` sets up the groups of every probed cluster with enough visible rows before the loop, and some are never opened (their cluster's range leaves too few candidates) | the whole setup of each unused group | Values yes if the late ones are rotated by `dgemm` one at a time (matrixmultiply computes each column alike whatever the batch; to be checked by a test); the `groups` counter falls, as intended | medium; the saving depends on how many go unused, which only a run counts |

## 4. The paths and where each candidate applies

- One group alone (`get_gq2` → `group_q2`, rotations by `matvec64`, all four rotations read for this group): exact
  k-NN's seed (`seed2` → `probe`, the suite's indexes have no norm key), `Refiner`, the norm-key paths, and any group
  a later step needs that was not set up before.
- Several together (`setup_groups`, rotations by one `dgemm` per block): `run_planned` (threshold queries and exact
  k-NN's sweep, for the groups of clusters on the index path that the seed did not set up), `knn2_fast`.

`probe` takes clusters in centre order until 2,000 visible items: unfiltered at 1M it reads one or two clusters, but
under a 10% filter about 10, under 1% up to its 64 clusters (unless the plan scans first), and each new group reads
the rotations again. That is the filtered case, where robustness matters. None of the candidates changes which
clusters or groups are touched: visible sets decide that as before.

## 5. What is exact and what is not, in one place

Exact (same bits; the harness compares each with the library on every query it times, and
`run2::tests::gsetup_candidates_give_the_librarys_values` checks them on synth4k with two and six deterministic levels,
for stored items, a near-item and a random direction, on both instruction sets):
the fused post-processing, the rotations together with `matvec64`'s arithmetic, and, by the integer argument of
section 2, any exact schedule of the basis product. Not exact: any partial or approximate basis product, or bounds for
the uncoded part fitted instead of computed.

One thing outside the question that the same reading turned up: the exact product assumes small query components.
`prep2` stores the high 13-bit half in an `i16`, which wraps for |q_i| ≥ 16, and the 32-bit sums are guaranteed only
while max|q_i| · n < about 8,256 (n ≤ 2048 is asserted for unit queries). Unit queries are far inside both; the Python
`threshold()` takes q as given, so a caller passing a long vector with a large component would get a wrong product
without an error. A check in `prep2` (or a third limb) would close it.

## 6. Recommendation

1. **Below n = 768: stop here.** The exact savings (post-processing, the 4-row sums, the one-group rotations) are
   about 5 to 15% of a group's setup at n = 384 to 512, under 2% of an unfiltered approximate query; at n = 128
   a larger share of a small cost (4 µs). The irreducible part is the n² int8 stream, which every opened cluster
   needs. Nothing in the library changed.
2. **Run the timing** (section 7) to confirm the split before deciding otherwise. If, at n = 384 to 512,
   (post_lib − post_fused) + (rot_own − rot_together) is above about a fifth of one_total, the two candidates are
   worth moving into the library as they stand (both are written and checked).
3. **For n ≥ 768 (the 768-dimensional BioCLIP-2 indexes, or larger):** set up the groups `probe` will open in one
   call before it runs (they follow from the visible counts alone: clusters in centre order until 2,000 visible items),
   with each rotation row read once for all of them and each product as `matvec64` computes it (`rot_together`). Same
   groups, same values, same counters; the rotations' 2.4 n² bytes are read once per query instead of once per probed
   group. Then consider deferring rotations to the first randomised level.
4. **The planner's constant** `COSTS.grp_us` is a flat 93 µs, 3 to 23 times the measured 4 to 28 µs and not scaled
   by n; a form a + b·n² fitted from the timing would price groups better. It changes plans, not values, so it is
   the owner's decision.

## 7. The timing to run (when the machine is quiet)

`vexact --v2 --index DIR gsetup` (`rust/src/run2.rs`, `gsetup_cost`; the binary's own module, so the library's
source fingerprint and every saved index are unchanged) times, per group, on one pinned thread, for `--nq` item and
text queries, every group in turn as a query that touches all of them:

| Column | What |
|---|---|
| floor | a plain read of every group's int8 basis (no exact product reads less) |
| one_total, one_basis, one_rest | each group set up alone (`group_q2r` with rotations: `get_gq2`'s path), and its split |
| post_inplace, rot_inplace | the same without rotations (its post-processing in place), and the difference (its own rotations) |
| all_total, all_basis, all_post, all_rot | all groups together (`setup_groups`, rotations by `dgemm`) |
| post_lib, post_fused | the post-processing alone on each group's qv: as the library writes it, and fused |
| rot_own, rot_together | the rotations alone: group by group (`matvec64` per group), and together with the same arithmetic (8 groups per read of a rotation row) |

Each candidate is compared bit for bit with `group_q2r`'s output before its time is kept (a difference stops the
run). `--cold` reads 64 MB before each part, so each starts with its bases out of the private caches. Output: one line
per class and `gsetup.csv` / `gsetup_cold.csv` (microseconds per group, medians over the queries) in `--results`.

The suite's saved indexes were built by another library version (their key's library fingerprint, 393641e1…, is not
this build's, 2c064d14…), and an open by the CLI may write cache files into the index directory, so the script
re-encodes each from its fit into a scratch copy (fit files copied, `Xs.npy` linked, nothing under
`/home/user/suite-data` written) and times that:

```
S=/path/to/scratch CORE=3 NQ=100 bench/suite/notes/visit-order/scripts/groups_setup_time.sh
# n = 768 too (64 synthetic groups, 98,000 vectors, about 0.6 GB while it runs):
S=/path/to/scratch CORE=3 NQ=100 SYNTH=768 DATASETS= bench/suite/notes/visit-order/scripts/groups_setup_time.sh
# n = 2048 with 16 groups (about 0.6 GB of vectors):
S=/path/to/scratch CORE=3 NQ=50 SYNTH=2048 SYN_G=16 DATASETS= bench/suite/notes/visit-order/scripts/groups_setup_time.sh
```

`DRY=1` prints the commands; `KEEP=1` keeps the scratch indexes. Each suite index is re-encoded on every CPU first
(LAION 1M takes the longest), then timed on CPU `CORE` alone, warm and cold.
