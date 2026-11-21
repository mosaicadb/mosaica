# LIMINAL research notes

One set of notes for the mathematics of LIMINAL: what is proved, what is measured, what is open. The reports elsewhere
on this branch stay as the record of how each result was reached; these notes are the place to read the result.

## Chapters

| File | Subject |
|---|---|
| `00-notation.md` | The setting and the symbols every chapter uses |
| `01-certificates.md` | Deterministic certificates: weighted Cauchy–Schwarz, the dual basis, the cluster-level steps, the codes |
| `02-floors.md` | Lower bounds on certified width per bit, and instance-optimality of prefix reading |
| `03-randomised.md` | The randomised levels: the error law, the guarantee, the constants |
| `04-cost-laws.md` | What a query must read: shells, exponents, hardness, scaling with N |
| `05-approximate.md` | Approximate k-NN: the cluster order, the stop, statistical widths, hierarchies, visible sets |
| `06-open-problems.md` | Conjectures, named gaps, and the items the program is working on |
| `LEDGER.md` | Every claim in one table: status, chapter, original source |
| `program/<track>/NOTEBOOK.md` | Each researcher's working notebook; not part of the notes until refereed |

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
attempt and a referee's report, both kept under `program/`.

## Rules for writing here

- Use the symbols of `00-notation.md`. A chapter may add local symbols if it declares them at its head.
- A proved result is given with its full proof, so a chapter can be read without its sources.
- Mathematics is written in plain Unicode in Markdown, as the reports do.
- Where two sources disagree, or a proof looks wrong, keep the source's status and record the point under
  "Editor's queries" at the end of the chapter. Do not repair a proof silently.
- British spelling. One point per sentence, with the relation between sentences stated.

## The program (started 21 November 2026)

Tracks, each with a notebook under `program/`:

1. **floors**: one floor that gives the head and the tail of the conjectured bound together; certificates as optimal
   transport.
2. **confidence**: a lower bound for randomised certificates (what the confidence constant must cost), the gap in R9,
   the hand-over from deterministic to randomised levels, a rate–margin law for bytes read.
3. **codes**: closing the gap between the randomised levels' code and the sphere's rate–distortion bound with
   table-decodable codes.
4. **approximate**: a random-energy model of the read, a stop on expected misses, a per-query spread at the levels.

Stage 0 checks (`program/stage0-*`, `program/library`): a numerical attack on the floor conjecture, block norms on
out-of-distribution queries, and a check of prior art and of every citation given from memory.
