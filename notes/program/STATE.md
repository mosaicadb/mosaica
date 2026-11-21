# Theory program: state

Kept by the PI. Updated 21 November 2026, 07:20, after a machine shutdown. Statuses use the tags of `../README.md`.
Nothing under `program/` has entered the chapters; a result enters only after a falsification attempt and a referee.

## Tracks

| Track | State | Where |
|---|---|---|
| Floors | **F1** (the water-filling floor, which is Conjecture 2.14) is [proved, unrefereed]: one referee read it and its lemmas line by line and found no error, with three numerical attacks; its report is partial. **F2** (Theorem 1's constant on the head, the tail in full) is not yet refereed. Extensions and the transport claims have a referee's report with repairs not yet applied. | `floors/CLAIMS.md`, `floors/referee/` |
| Confidence, lower bounds | Three runs lost to timeouts; about 2,100 words of C1 saved; resumed. Nothing to report yet. | `confidence/CLAIMS.md` |
| Confidence, design | Done, unrefereed, all [model] or [computed]: the hand-over, the hybrid optimum, the rate–margin law (fails for in-distribution queries at large answers). | `confidence/DESIGN-CLAIMS.md` |
| Codes | Closed as a drop-in (8.2% of the last level's width against a 10% bar). Kept: 24 encoder scales (free), the nesting loss as a result, one-rate codes for a redesigned schedule. | `codes/REPORT.md` |
| Approximate | The tree model is a generalised random energy model with a step profile; a leaf floor at a fixed level and a complete floor for the k best; claims revised after referee 1, not re-refereed. The stop on expected misses is dropped. The per-query spread is correct and nearly neutral. | `approximate/CLAIMS.md`, `approximate/referee/referee-1.md` |
| Partition (new, 21 November 08:00) | The owner's question: is k-means the right thing to group by? A theorist is deriving the objective each use of a cluster implies (the floor makes coding cost a log-determinant quantity, where k-means minimises a trace), the scaling of cluster size and group count, assignment of clusters to bases by coding cost, and the alternatives; a librarian is checking prior art. An experiment spec for the 200k sample follows. No earlier clustering-mathematics work was found saved on any branch. | `partition/`, `library/partition.md` |
| Confidence, lower bounds: referee | Report in. No displayed inequality wrong; several overstatements. A4 is R8 widened, not a new interval, and is not better than the plain rule one-sidedly; the converse holds in a restricted class (one block, linear estimate, half-width proportional to the offset's norm); R9 as worded is false and should be restated in quantile form; the "50 bits" is slack (30 to 41). The theorist is revising. | `confidence/referee/referee-lower-bounds.md` |
| Stage 0: floor attack | Numerics cut off by the shutdown; report owed from the logs. | `stage0-floor-check/` |
| Stage 0: block norms | Dropped. | `stage0-block-norms/REPORT.md` |
| Library | Three reports. | `library/` |

## Owed

- To the floors theorist, in one round once the core referee finishes: the extensions referee's repairs
  (`floors/referee/referee-extensions.md`); the novelty check (`library/floors-novelty.md`: L1 is the one-dimensional
  case of Bolley, Gentil and Guillin 2018, Theorem 2.1; B2 is the classical MM* argument, Aubrun, Szarek and Ye,
  Appendix D; cite Steiner, Dempfle, Ingber and Weissman, arXiv:1404.5173, and Balasubramanian, Bhandari and
  Kasiviswanathan, arXiv:2610.24890; word F1 as the first proof of the converse at finite n, for covariances that need
  not commute, in mean-width form); "tight" is in probability, not in mean width; B4c's first table is a consistency
  check, not a prediction.
- A referee for F2 if the core referee does not finish it, and one for the confidence design claims.
- Into the chapters after refereeing: F1 and F2 into chapter 2 (closing Conjecture 2.14); the prior-art corrections
  already in `../06-open-problems.md` 6.4; the leaf floors into chapter 5.
- Seven tags outside the scheme in `../LEDGER.md` to map.

## Needs the machine (not before the timing session says it is free)

The visit-order session (tessera-1f) times until about 10:05 on 21 November; keep cores 10 and 11 clear until then.
After that it runs the fixed-cluster-count replay on cores 0-7 at nice 19 and will send the numbers; cores 8-11 are
then free for this program's native and timing work.

- The 24-scale encoder, measured natively. Low priority: it is worth about one point of the last level's width, makes
  the encode loop four times longer, and the stored κ grid under a wider range of scales has not been checked. The
  scales are the encoder's trial values (`LAMS`), not stored; the owner's point that every stored scalar read has a
  cost applies to κ and tan φ, and is being added to the confidence design model (C5).
- The rate–margin test on the 50k verify dump (`confidence/c4_dump_test.py`).
- A fit at text share 0.2 against the shipped mixed fit (floors B4d).
- The 56-byte level. Coding every block there is withdrawn: with the stored scalars charged (confidence C5) the optimum
  codes one block, as the index does. What is left to test natively (C5.7): one wider block at the same 24 B, two
  blocks, and the `--design-exact` layout again with phases recorded and the number of block-rises decoded per query
  printed, so that the cost of a block-rise is measured and not inferred. The model over-predicts gains (6 to 26%
  fewer entrants where the earlier trial measured 2%), so expect a few percent at most.
- The replay of the built stop against a fixed cluster count: the visit-order session will put it to the owner.

## Decisions that are the owner's

- Whether to referee the built integer kernel against the refereed design, and the sorted-range proof (chapter 1,
  queries 13 and 14).
- A basis per query family, or a lower text share in the single fit.
- Whether to redesign the first randomised level.

## Rules for agents on this machine

One thread, nice 10, pinned to the core the brief names, under 100 MB resident per job, no command that walks the
disk, no pass over a large data file, no builds, no git changes. Four datasets of another session's timings were
re-run on 21 November because agents of this program broke looser versions of these rules.

## Notes to fix at the next pass

- Chapter 5, section 5.13 calls spilled copies "a design never run"; `bench/REPORT.md` records a SOAR second
  assignment that was run (12 to 15% fewer items for the same recall, no faster, because the copies are scattered
  rows). Reported by the session that added the SOAR and SPANN references.
- Chapter 2, Editor's query 18: whether κ_k − k is least at k = 1 for every k (computed to 10⁷, not proved).
- Chapter 6, section 6.2's summary of chapter 2's query 8 is out of date.

## Update 09:15, 21 November

The machine is free on every core as of 09:10 (the visit-order session has finished timing and will send the
fixed-cluster-count replay numbers). Jobs now go through `program/run-limited.sh` (cores, threads, address-space cap).

Running:
- The partition experiment on the 200k sample (`partition/experiment/`), in the order of `partition/EXPERIMENT-SPEC.md`:
  first how far the built fit is from a fixed point, the noise between seeds and the price of the built grouping.
- The rate-margin test on the 50k verify dump (`confidence/dump-test/`): the specified criteria, then a term-by-term
  account of why in-distribution queries at large answers read about twice what the law predicts.

Done since the table above was written: chapter 2 carries the proved floor (section 2.9a), audited; the confidence
lower bounds and the partition claims are revised after their referees; neither has entered the chapters.

## The fixed-cluster-count replay on the real reader (from the visit-order session, 21 November 09:40)

The approximate track's idealised finding (A2.6: the built stop lost to a fixed cluster count on LAION and glove
unfiltered) was rerun on the library's own stream by the visit-order session. Fixed count's rows over the stop's rows
at recall 0.9 / 0.95 / 0.98 / 0.99, 100 queries, k = 10; below 1 means the fixed count reads fewer. [measured, by that
session; logs in /home/joe/code/vexact-data/sc-study/out-fixed2/]

- Pooled stop (b87f1513e), unfiltered. The stop wins on LAION (1.07 / 1.05 / 1.11 / 1.06), yahoo, landmark and BioCLIP
  (1.42 to 1.87). The fixed count wins on arxiv (0.82 / 0.83 / 0.75 / 0.73), iNaturalist, glove (0.82 to 0.89) and SIFT.
- The older stop (f528836) loses to a fixed count on LAION (0.60 to 0.82), which is what the idealised replay saw.
- Under filters the pooled stop wins nearly everywhere (1% sets 1.03 to 1.4; BioCLIP 1.2 to 2.4), except iNaturalist
  region-25, SIFT at 25% and arxiv random-25 at high recall.

Reading, theirs: the per-query stop costs 5 to 25% against a fixed count on four datasets unfiltered and wins by more
elsewhere and under filters; there is room to improve the stop at high recall on those four, and no case for a fixed
count. A2.6's LAION statement does not hold for the pooled stop; its glove statement does.

## Approximate mode: the level width (21 November, from the visit-order session's dumps; all [measured, by that session])

Files: /home/joe/code/vexact-data/suite/rho-dump/; scripts under bench/suite/notes/visit-order/hierarchy/layered/ in
/home/joe/code/vexact-visitorder (rho_check.py, psi_eff.py, kappa_check.py, pair_check.py, rand_sample.py).

- Rule 1's derived rho is 1.29 too narrow on iNaturalist at 12 groups and 2.04 at 1 group; right within 6% on yahoo
  and LAION. Cause as predicted here: few groups for the dimension.
- The width that works is measured per cluster: the sd of (score − midpoint) over 32 to 64 sampled rows. The PI's
  forms 1 (variance bookkeeping) and 2 (pooled increments) fail.
- The PI's Gaussian account of why an oracle spread keeps only 0.8 of the neighbours was wrong on the numbers, and so
  was the mixture reading. What holds: a neighbour's error in units of the level's spread tracks its score's height in
  units of the cluster's spread (slope 0.92 to 1.24 through the origin, correlation 0.6 to 0.7).
- The rule built on that: keep a row when a ≥ t̂ − (ζ_c + ψ0·√f)·tau, every term from the per-cluster sample. At
  ψ0 = 2 / 3 it keeps 0.92 to 0.98 / 0.97 to 0.99 of the true ten on all five indexes, at rows within 7% of the most
  efficient rule (κ = 1 − f). It is a tendency, not a law; iNaturalist's slope is 1.2.
- A4.4's identity holds on the real codes: a sample read at the first randomised level where every block is coded
  estimates tau, μ_c and σ_c with bias of 2 to 6% (low on LAION) and modest extra noise, at about 10 KB against 256 KB
  of stored vectors on iNaturalist. It needs the group's rotations set up, so it pays only where they are cheap or
  already done.

Owed: the theorist's checked write-up (A4) to that session; a derivation of the slope; what breaks the rule under
filters and for clusters smaller than the sample.
