# LIMINAL research notes

One set of notes for the mathematics of LIMINAL: what is proved, what is measured, what is open. The reports elsewhere
on this branch stay as the record of how each result was reached; these notes are the place to read the result.

## Chapters

| File | Subject |
|---|---|
| `00-notation.md` | The setting and the symbols every chapter uses |
| `01-certificates.md` | Deterministic certificates: weighted Cauchy–Schwarz, the dual basis, the cluster-level steps, the codes; the integer kernel as built, and the open defects of the build |
| `02-floors.md` | Lower bounds on certified width per bit, the water-filling floor, and instance-optimality of prefix reading |
| `03-randomised.md` | The randomised levels: the error law, a rounded rotation on the query side, the guarantee, the constants, and a lower bound for the confidence constant |
| `04-cost-laws.md` | What a query must read: shells, exponents, hardness, scaling with N; and how a row's bytes are spent, with the price of stored scalars |
| `05-approximate.md` | Approximate k-NN: the cluster order, the stop, statistical widths and the measured level width, hierarchies and the tree model, visible sets |
| `06-open-problems.md` | Conjectures, named gaps, the Editor's queries, and the work in progress or owed |
| `07-ruled-out.md` | What the program tried and did not adopt, with the bar each proposal was held to |
| `LEDGER.md` | Every claim in one table: status, chapter, original source |
| `program/STATE.md` | The program's current state: what entered the chapters, what is owed, what needs the machine |
| `program/LOG-2026-11-21.md` | The program's running log of 21 November 2026, frozen; the chapters cite its measurements |
| `program/<track>/` | Each track's notebook, claims and referee reports; not part of the notes until refereed |

## Status tags

Every statement carries one tag and its source (file and section).

- **[proved, refereed]**: a proof that an independent referee has checked.
- **[proved, unrefereed]**: a complete proof nobody else has checked.
- **[sketch: gap named]**: an argument with a stated hole.
- **[conjecture]**, **[refuted]**.
- **[measured]**: counted or timed on real data; say the dataset, the size and the script.
- **[computed]**: a number from floating-point evaluation of a proved inequality.
- **[model]**: derived under a stated model of the data.
- **[cited]**: a published result, source read; **[cited, unchecked]**: quoted from memory or a summary.

A tag is never upgraded in transcription. A result enters a chapter as [proved, refereed] only after a falsification
attempt and a referee's report, both kept under `program/`. A statement or proof that a referee writes itself, as a
restatement or a repair, is [proved, unrefereed] until someone other than its writer has read it, even though the
referee read the text it replaces.

## Rules for writing here

- Use the symbols of `00-notation.md`. A chapter may add local symbols if it declares them at its head.
- A proved result is given with its full proof, so a chapter can be read without its sources.
- Mathematics is written in plain Unicode in Markdown, as the reports do.
- Where two sources disagree, or a proof looks wrong, keep the source's status and record the point under
  "Editor's queries" at the end of the chapter. Do not repair a proof silently.
- British spelling. One point per sentence, with the relation between sentences stated.

## The program (started 21 November 2026)

The tracks and checks below ran in the first pass, each in its own directory under `program/`. Each line says where its
outcome now stands in the chapters; `program/STATE.md` gives the statuses and what is owed.

1. **Floors** (`floors/`): one floor for the head and the tail together. The water-filling floor settles Conjecture
   2.14 and is in chapter 2, §2.9a. The extensions and the transport claims have a referee's report and have not
   entered the chapters.
2. **Confidence, lower bounds** (`confidence/CLAIMS.md`): what the confidence constant must cost. R9 corrected and the
   union over N items is in chapter 3, §3.6a; the lower bound in a restricted class and the confidence tax are in
   §3.7a; the withdrawn statements are in chapter 7, §7.7.
3. **Confidence, design** (`confidence/DESIGN-CLAIMS.md`): the hand-over from deterministic to randomised levels, the
   rate–margin law and the price of a stored scalar. These are in chapter 4, §4.9, and coding every block at 56 B,
   which was withdrawn, is in chapter 7, §7.7.
4. **Codes** (`codes/`): table-decodable codes for the randomised levels. The track closed them as a drop-in, and its
   result, the nesting loss, is in chapter 7, §7.2.
5. **Approximate** (`approximate/`): the stop on expected misses, the per-query spread, the level width and the tree
   model. They are in chapter 5, §5.8a, §5.9a and §5.10.1a, and the dropped stop is in chapter 7, §7.3.
6. **Partition** (`partition/`): whether k-means is the right way to group rows. No alternative cleared its bar
   (chapter 7, §7.4), and the defect it found in the grouping is chapter 1, §1.8a, defect 8.
7. **Rotations** (`rotations/`): structured rotations, a shared dither, and a rounded rotation on the query side. The
   rounded rotation and what a seed promises are in chapter 3, §3.2a and the end of §3.4; the structured rotations and
   the dither are in chapter 7, §7.5 and §7.6.
8. **Referees of the kernel and of the sorted range** (`kernel/`): the cluster cut, the sorted range and the integer
   kernel read against the code. Their restatements are in chapter 1, §1.2 and §1.8, and the defects they found are in
   §1.8a.
9. **Stage 0 checks** (`stage0-floor-check/`, `stage0-block-norms/`): a numerical attack on the floor, which refuted
   nothing (chapter 7, §7.8), and block norms on out-of-distribution queries, which were dropped (chapter 7, §7.1).
10. **Library** (`library/`): prior art, and every citation given from memory. Its corrections are in chapter 6, §6.4,
    and in the chapters' sections on prior work.
