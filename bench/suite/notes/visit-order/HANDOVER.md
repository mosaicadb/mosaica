# Handover: approximate k-NN order, read planning and scaling (vexact / LIMINAL)

Repository `mosaicadb/mosaica`, branch `ccr-b37f5194-u55ji1` (head after this file is committed). The library is
the Rust crate under `rust/` (index format v2: `rust/src/v2/`). The benchmark harness is `bench/suite/`; the Python
bindings are `bench/annb/vexact-py/`, built by `bench/annb/install.sh` at the commit in its `PIN=` line. Read
`bench/suite/notes/visit-order/PLAN.md` (history, every decision with its numbers) before changing anything.

## Where things stand

**Pinned build: `d999606`** (`bench/annb/install.sh`). The approximate k-NN (`rust/src/v2/adaptive.rs`, `knn`):

- **Order.** Clusters are visited in the order of mu_c + lambda * sigma_c(q): mu_c the centre score, sigma_c(q) the
  spread of the cluster's scores along q (16 stored int8 directions plus an isotropic rest, `order.rs`),
  lambda = delta + 3 rho. delta and rho are the query's estimated shift and scale of the visible rows' scores, in
  each cluster's standardised units, from the rows scored in full while fewer than k are held (prior 0 and 1;
  frozen once k are held). With no filter or a benign one they stay near 0 and 1 (the order is mu + 3 sigma);
  when a filter keeps each cluster's low tail they move to about -2 and 0.4. A stream keyed by the free stand-in
  mu + lambda * rad_c * s feeds a window of 8 clusters whose sigma is computed.
- **Stop.** When the window's largest mu + (delta + psi rho) sigma is below the running k-th best. psi is the
  user's knob; psi = infinity is the exact query (`knn2`).
- **Reads.** Each cluster is read directly or on the index path, priced by 13 measured constants timed at index
  open (`Measured`, cached in `compute-constants.txt` keyed by the library's fingerprint). Clusters with at most 16
  visible rows go straight into the batched direct read (no sigma). A group's setup is bought ski-rental style: once
  the savings its directly read clusters forwent reach the setup's cost.
- **Approximate threshold** (`threshold` in adaptive.rs) is unchanged by the order work: its own order at its
  threshold, the same measured constants, and a sparse hand-over to the exact query by the planner's rule.

**Why mu + 3 sigma** (theory, `PLAN.md` and the stress results): the recall gained per row read is the per-row
chance Phi-bar((t* - mu)/sigma) at the final k-th best t*; cluster size cancels. mu + c sigma is its threshold-free
form, and rows read are flat in c over about 2.5 to 4 (stress-tested over k = 1 to 1000, score-correlated filters,
random/region mixes, out-of-distribution queries, outliers, merged clusters, heavy tails, two held-out sets:
`results/order-stress/`, `results/order-self-correcting/`). Bruch, Krishnan and Nardini (NeurIPS 2025, arXiv
2405.12207) use the same order at the same multiplier, with no filters, stopping rule or exact fallback.

**Held back:** the group-count rule `9916e61` (G = round(N / (0.5 n0^2)), no cap of 64), reverted by `91615b4`
because it gives glove 237 groups, untested.

## What to do, in order

Owner's principles, verbatim: "We also need to preserve the things that actually make this approach special -
particularly the fast checking using the dual basis and robustness under filtering. No changes that compromise our
strengths." "I'd generally want to aim for elegance/simplicity here as well. Ideally we want to find an optimal
algorithm/process - not just hyper optimise through a million tweaks." Constants measured on the machine are fine
if each has a physical meaning and the planner is robust; calibrated constants are not unless robust across
dimensions, datasets and filter patterns. AVX2 and AVX-512 must both work. **Glove (100-d) is an adversarial
dataset for this library, not an exemplar**: progressive reading pays least at low dimension. Use it (and SIFT) as
a guardrail (no change may make it much worse), never to choose a design. Decide on 384-d and above: yahoo (384),
LAION (512, text queries against images, out of distribution), iNaturalist (2048), arxiv-nomic, landmark-dino and
bioclip (768).

1. **Benchmark `d999606`** against the previous pins `7536990` (library `00965ba`) and `f528836`. k-NN and
   threshold; filters none, random-1, random-25, region-1, region-25. Report time to recall as a ratio to the
   legacy `spread` arm and to the previous builds. Known from quick local checks (50 queries, noisy): LAION better
   than both earlier builds at high recall (no filter at 0.99: about 1.0 of spread against 2.2 for 7536990);
   region-1 level with or ahead of 7536990; glove random-1 at recall 0.9-0.95 behind 7536990 (guardrail item).
   iNaturalist was 1.1-1.6x slower unfiltered with `f528836`; check whether the new order fixes it.
2. **Group count.** Re-apply `9916e61` (revert `91615b4`'s rust/ part). Evidence for about 8 groups at 1M and
   n = 384-512: `hierarchy/layered.md` 8.8 (bytes), 8.10 (timing: 8 groups never slower than 64; exact knn2 -19%
   on yahoo, approximate -10 to -26% unfiltered), and `rust/REPORT_CLUSTERSIZE.md` (768-d, exact threshold: fewer
   groups faster or equal in all 24 cells). Refit (`rust/tools/fit.py`, rule in `rust/tools/rules.py` and
   `fit.rs Groups::count`, kept in agreement by a test) and time exact and approximate at the rule's G against 64
   on the representative sets. Glove guardrail: at 237 groups it must not be more than about 10% slower; if it is,
   derive the group count from how much tighter bases help on the data (measurable at build), do not special-case.
3. **Rule 1: a statistical width at the levels** (`hierarchy/layered.md` sections 0, 3, 8.6). In the approximate
   reader, pass each level's half-width factor (`Prep2::dial`) as psi * rho_l with rho_l = |a|_2 / |a|_1 of the
   level's per-coordinate terms, derived from the build's weights (no calibration). Exact queries keep dial 1.
   Counted: 4-10x fewer bytes unfiltered, 2-6.5x at 25% visible; recall at a given psi drops (LAION text 6.5 points
   at psi 4), so the default psi moves up about half a unit. Risk: the level kernel's per-row cost; expect gains at
   384-d and above, little on glove. Build it as its own commit and time it.
4. **Scale (100M to 1B+).** `hierarchy/layered.md` 8.7 to 8.10: one hierarchy of fixed 2,000-row leaves (storage
   and approximate reads), bases of about kappa n^2 rows (kappa 0.5-1), and routing (sqrt(N) parents plus a 32-byte
   centre sketch from about 16M rows) as the only part that grows. Nothing is built. First measure rows and
   clusters to recall on slices of Deep1B, BigANN or LAION-100M at the build rule's cluster size and at 2,000 rows
   (the extrapolation is from a 16-fold range of N), and time exact queries on 2,000-row leaves.
5. Lower priority: the remaining glove gaps (guardrail only); LAION's hybrid-sketch anomaly (the hybrid sketch
   loses only with text queries; not needed, the self-correcting order fixes the held-out LLM set it was for).

## How to measure

- Paired, interleaved timing, never across sessions: `bench/suite/notes/visit-order/scripts/ab_orders.py DATASET
  spread adaptive --psi 2,2.5,...,7,inf --filters ... --reps 5 --core C --nq N`, needs an index kept by the suite
  (`run.py ... --keep-work`), prep under `$SUITE_DATA`. To compare builds, build each wheel (install.sh with a REV)
  and run them back to back in one session. `ab_curves.py` interpolates time to recall; exclude psi = inf from
  the curve. `profile_phases.py --adaptive` gives per-phase time and counts.
- Rows-only replays that need no timing: the ignored tests `study_order_stress`, `study_probe` and the
  `study_layered_*` tests in `rust/src/v2/e2e.rs` (run with `cargo test --release -- --ignored NAME`, settings in
  their doc comments). Decide order questions in rows first; time only what survives.
- Pin builds to cores other than the timing core. Any library edit changes the fingerprint, so indexes and
  measured constants are rebuilt or re-timed on open.
- Record each round's results under `bench/suite/notes/visit-order/results/<name>/` and a Progress entry in
  PLAN.md, as the earlier rounds did.
