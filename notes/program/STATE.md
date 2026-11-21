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
