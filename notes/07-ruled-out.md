# 7. What was tried and ruled out

This chapter records the proposals that the theory program of 21 November 2026 tested and did not adopt, so that
nobody repeats one of them. Each section gives the question, what was tried, what was found, the verdict and the bar it
was held to, what would reopen it, and where the record is. Several tracks proved or measured something true on the way
to a negative verdict, and those results are stated with their tags because they outlive the proposal. Nothing here is
new: every number is carried from the file named beside it with that file's tag, and where a referee corrected a
statement the corrected form is given and the earlier one is marked [refuted] or withdrawn. Paths are relative to
`notes/program/` unless they say otherwise; paths that begin `rust/` are from the repository root.

| | Proposal | Verdict |
|---|---|---|
| 7.1 | Block norms for out-of-distribution queries | Dropped: from four blocks up the norms cost more bytes than they save; the basis is the lever |
| 7.2 | Nested lattice and trellis codes as a drop-in | Closed: 8.2% of the last level's width against a 10% bar; the nesting loss is the result |
| 7.3 | A stop on expected misses (a tail-fitted stop) | Dropped: no tail makes the setting a recall, and none beats the built stop beyond noise on yahoo or LAION; the per-query spread of A3 is kept as correct and nearly neutral |
| 7.4 | Alternatives to k-means for the partition | None adopted on the 200k sample: the coding schemes gain less than twice the seed spread, balanced k-means trades decisions for layout, the neighbour-graph partition was not run |
| 7.5 | Structured rotations in place of dense Haar | Sign–Hadamard rounds fail the guarantee at every block from 64 up, butterflies at 1,024; Kac walks and Kronecker forms have no proof of it; subgroup forms keep it and trade scalars |
| 7.6 | A shared dither in place of a rotation | The law is proved; the width is 2.0 to 2.65 times today's when nested from one bit |
| 7.7 | Confidence: statements withdrawn | A4 is R8 widened; R9 for every per-item rule is false; coding every block at 56 B withdrawn; code-size conditions only sufficient |
| 7.8 | Stage 0: a numerical attack on the floor | Nothing refuted; a code came within a factor 1.0026 of the floor |

## 7.0 Local symbols

Chapter 0's symbols hold, with these additions, each local to the section named.

- **7.2.** "tan" is the root of the mean of tan²φ over items for one block and one rise. A **rise** is the bits a block
  gains at one level; a schedule such as 2-3-4-7 lists a block's cumulative bits per position at its rises.
- **7.3.** v_c is the number of visible rows of cluster c, and ζ_c(t) = (t − μ_c)/σ_c. G is here a shape (the tail of
  the standardised within-cluster scores), not a number of groups. M = Σ over unread c of v_c·G(ζ_c(t̂)) is the
  expected number of unread rows above t̂ under the shape G, M̂ the same sum under an estimated shape Ĝ, X(t̂) the true
  number, and Miss the number of true neighbours not returned. r is a target recall, and κ_σ = M_σ/M̂_σ at a stopping
  time σ (unrelated to chapter 0's κ). ρ_ℓ(q, c) is the per-query spread of level ℓ for query q and cluster c, the
  per-query form of chapter 0's ρ_{g,ℓ}. In the per-query spread (A3) the notation is chapter 5's (5.6) with v_i and Q_i
  written ν_i and π_i: ν_i is coordinate i's coding variance, a_i = √(ν_iπ_i), w = ‖DΔ‖, h the certified half-width's
  code term, κ the row's own scale, and E_ω[a] the mean of the a_i under the weights ω_i = D_i²Δ_i² (none of these is
  chapter 0's a_j, ω or κ_j).
- **7.4.** S1d is the mean idealised half-width of a held-out row at the 16 B level, S1b the bytes a row needs at the
  target width w* = 0.06. w is a target mean half-width, λ a price, and φ(u) the function of P1.1 (not an angle). The
  **seed spread** is the largest gap among three build seeds of the built rule.
- **7.5, 7.6.** n_b is a block's size, y its item block, z = R·y, E = ŷ − y the error vector, A = ‖y‖·tan φ and Δ_⊥ the
  part of Δ orthogonal to y. "k rounds" is H·D_k ⋯ H·D₁, H the normalised Walsh–Hadamard matrix and D_i diagonals of
  uniform signs. In 7.5, M is a bound on a conditional density against uniform. A block's crest factor is its largest
  coordinate over its root mean square. In 7.6, s is an item's step, d_p the depth at which position p is coded,
  Δ_p the query's weight on position p and V_p uniform on (−1, 1).
- **7.7.** A bare d is a block's degrees of freedom, as in the sources (chapter 0's d_j). q = q_d(δ) is the
  one-coordinate sphere quantile and k_φ = q²·tan²φ (chapter 3 writes ζ and ξ²). s is the query's latitude
  Δ·y/(‖Δ‖‖y‖) and Ê its plain estimate from the message. R is a rate in bits per dimension, M the number of codewords,
  w* = q/√(4^R − 1 + q²), and 𝓛 the class of certificates whose estimate is linear in Δ and whose half-width is a
  multiple of ‖Δ‖.

## 7.1 Block norms for out-of-distribution queries

**Question.** Text queries do not follow the profile the deterministic weights were fitted on. Would storing J norms
of the coding error over blocks of coordinates, in place of one, narrow their half-width enough to pay for the norms?

**What was tried.** J = 2, 4, 8 and 16 equal-variance blocks, against one norm, one norm with text-fitted weights, a
basis and weights fitted on a mixed query sample, and the best diagonal weighting per (query, item) pair. The 200k
BioCLIP-2 sample (K = 200, G = 12), with 369 held-out text prompts, 361 species prompts and 100 held-out items scored
against all 200,000 items, in an idealised setting and a model quantiser at 4 to 32 B.

**What was found** [measured; `stage0-block-norms/REPORT.md`, "Verdict" and "The numbers that decide it"]. Block norms
cut the text half-width by 10% or more only with J ≥ 8, and in the idealised setting only at shallow levels (ratios
.90 / .94 / .97 at J = 4 and .84 / .90 / .95 at J = 16, at 0 / 32 / 256 exact coordinates). From J = 4 they cost more
bytes than they save at an equal undecided share; J = 2 pays for held-out text alone, by 0.7 to 1.8 B per item. The gain
is not specific to text, since items and species prompts gain 3 to 11% too. The mixed basis cuts the text half-width by
21 to 33% and saves 11 to 17 B with nothing more stored, and costs item queries 6 to 19%. There were 0 violations over
830 queries × 200,000 items × every bound and level.

**A result kept.** The best diagonal weighting for each pair sits at about 0.6 of the single norm for every kind of
query, items included [measured; same source, "What it shows"]. So the looseness of weighted Cauchy–Schwarz is the
chance alignment of the query's weights and the item's error within a pair, not a mismatch between query and profile,
and blocks cut by position recover only about a fifth of it at J = 16.

**Verdict and bar.** Dropped (`LOG-2026-11-21.md`, "Tracks"). The bar was bytes saved at an equal undecided share against the
bytes of the norms, which J ≥ 4 fails. For queries the fit did not see, the lever is the basis, which no change to the
norms reaches.

**What would reopen it.** The report's leads: J = 2 at the first one or two levels only, or a basis per query family.
The built code, a second dataset, and prompts that do not share templates with the training prompts (which favour the
mixed basis) were not tested.

**Record.** `stage0-block-norms/REPORT.md`; `stage0-block-norms/work/report_measure.txt` and `report_quant.txt`.

## 7.2 Better codes for the randomised levels

**Question.** Can nested lattice or trellis codes, decoded by today's one 16-entry lookup per nibble, narrow the last
randomised level by 10%?

**What was tried.** Product codebooks per nibble, additive stream codes, E8 and D4 in their Hamming and parity forms, a
sliding-window trellis, residual stages, a trellis-coded last plane, re-quantising each rise, and a phased trellis whose
nibbles alternate between two rises, searched by one exact Viterbi pass; on uniform directions, which is exact for a
rotated block (`codes/NOTEBOOK.md` entries 2 to 11).

**What was found.**

- *The cap bound.* For any M points and y uniform on the sphere, Pr[φ ≤ θ] ≤ min(1, M·Ω(θ)), Ω the normalised cap
  measure [proved, unrefereed; `codes/REPORT.md`, "Where the built code stands"]. The built code sits 1.31 to 1.77 times
  above it in tan; where the variance sits, the block of 256 at 1 and 2 bits, the gap is 1.31 to 1.42 and none of it is
  nesting [measured, `codes/built.py`; same section].
- *One rate.* Codes with today's decode gain 13 to 16% of width at 2 bits, 12% at 4 bits and 15 to 17% below one bit;
  E8 gains 7 to 10% and D4 4 to 6% [measured; `codes/REPORT.md` "Verdict"; `codes/NOTEBOOK.md` entry 8].
- *Nested.* Residual stages lose, paying the first rise's gain back at depth (1.945 times built tan at the 7-bit rise of
  2-3-4-7). A trellis-coded last plane gains nothing (0.989 to 1.009). E8 nested loses 1 to 10% against each rise alone
  (2.9% and 7.7% at equal emphasis on the two rises) and ends level with the built code [measured; `codes/NOTEBOOK.md`
  entry 7].
- *Stored scalars.* Under the phased trellis κ and tan φ keep their meaning and the 15-value tan table and 4-bit κ grid
  fit as well as today [measured, 500 items; `codes/NOTEBOOK.md` entry 9].

**The result kept: the nesting loss.** Nested under a 1-bit prefix as good as the built one, a table-additive trellis
keeps 6 to 8% of the 13 to 16% it gains at one rate, and E8 keeps nothing. The frontier between rises is steep: each
further 1% at the 2-bit rise costs about 3% at the 1-bit rise [measured; `codes/REPORT.md`, "Verdict" item 2]. No
published figure for this loss was found.

**Verdict and bar.** Closed as a change to the index. With the schedule, the nesting and the decode kept, the best
package narrows the last level by 8.2% (6.6% with a 16-state encoder), below the 10% bar [model: an estimate from
measured tan ratios and an older build's variance shares; `codes/REPORT.md`, "Verdict"; `codes/NOTEBOOK.md` entry 10].
As built that is about 7% fewer full-vector reads, 3% of bytes and 1% of time at large answers [model: an estimate;
`codes/REPORT.md`, "What it would be worth, as built"].

**The 24 encoder scales, and what they cost.** The report keeps "a free change: 24 encoder scales in place of six",
which narrows the 6- and 7-bit rises by 6 to 8% [measured, `codes/exp_encoder.py`, 2,000 blocks; `codes/REPORT.md`
"Verdict" item 1]. This chapter does not call the change free, because the log lists its costs
(`LOG-2026-11-21.md`, "Needs the machine"). The scales are the encoder's trial values (`LAMS` in `rust/src/v2/mod.rs`),
not stored, so nothing stored or decoded changes. Trying 24 scales in place of six makes the encode loop over scales four
times longer; this is a count, since nothing was timed (`codes/NOTEBOOK.md` entry 8). The stored κ grid under the wider
range of scales, 1.6 to 5.0 in place of 2.0 to 4.2, was not checked. The change is worth about one point of the last
level's width, because the blocks of 96 and 128 that it helps carry 8% and 14% of that level's variance [model: an
estimate; `LOG-2026-11-21.md`, "Needs the machine"; `codes/NOTEBOOK.md` entry 10]. It is kept as a low-priority native
measurement.

**The rule for a redesigned schedule.** A block whose last rise is its only rise can use the one-rate codes, and the
16-word codebook below one bit is the cheapest of them [measured at one rate; `codes/REPORT.md`, "Verdict" item 3;
`codes/NOTEBOOK.md` entry 4].

**What would reopen it.** A schedule in which the variance-carrying blocks rise once; a lower bound on the nesting loss
of a table-additive code; or the runs not done (Barnes–Wall and Leech, a longer sliding window, nested D4, a phased
trellis for 1-3-4, the rate design with the new tan² table; `codes/NOTEBOOK.md` entry 11).

**Record.** `codes/REPORT.md`; `codes/NOTEBOOK.md` entries 2 to 11 and the `out_*.txt` files.

## 7.3 A stop on expected misses

**Question.** Can approximate search stop when M̂, the expected number of true neighbours left in the unread clusters
under a tail fitted from the rows read, falls below (1 − r)·k, so that r is a recall?

**What was tried.** The stop's guarantee with a known tail; a lower bound for any stop fed by the pooled sample; the
estimator's error at the decision; the measured shape of standardised scores; and replays of five tails against the
built stop and a fixed cluster count on yahoo, LAION text and glove, five visible sets, k = 10, 100 queries
(`approximate/CLAIMS.md` A2).

**What was found.**

- *With a known tail* [proved, unrefereed; the referee found no error in items 1 to 3; A2.1]. In model M3 with exact
  visible statistics and exact reads, Miss ≤ X(t̂), E[X(t̂) | everything read] = M, and E[recall | everything read up to
  σ] ≥ 1 − M_σ/k at any stopping time σ. It bounds a mean, not the share of queries below r: in the model at r = 0.9,
  7% (normal) and 23% (Student-t, 5 degrees) of queries fall below 0.9 [model, simulated by the referee;
  `approximate/referee/ref1_a2.py` part A].
- *With an estimated tail.* The first version bounded G/Ĝ at every unread cluster; that is true and nearly empty
  (median 10¹⁹⁷ in the referee's run), and it is replaced. The corrected form uses κ_σ = M_σ/M̂_σ [proved,
  unrefereed; A2.1, the repair the referee's], whose median is 27 and 90th percentile 300 in the same run [model,
  simulated by the referee].
- *No shape, no guarantee* [proved, unrefereed, one-shot; A2.2]. A rule that stops surely under some shape and keeps
  E[Miss] ≤ (1 − r)·k under every shape needs at least (3/32)·W/((1 − r)·k) pooled rows, W the unread rows. Two steps
  of the first proof (a shape chosen after the sample, and a false independence) are [refuted] and replaced by the
  referee's repairs; the constant is unchanged.
  The sequential form is [sketch: gap named]. "Nothing between all shapes and a parametric family" is withdrawn: a
  Cantelli stop is valid for every standardised shape and useless in practice, and the classes between are open.
- *The estimate at the decision* [measured; A2.3]. Computed from measured counts, with 1,300 to 4,000 pooled rows the prediction is
  off by a factor 2.2 to 4.0, because the decision reads the tail beyond where the query holds more than a handful of
  rows. The tail is near exponential, with a decay rate of 1.97 to 2.77 where the normal has 4.7 [measured; A2.4].
- *The premise fails* [measured; A2.5]. One shape for all queries over-predicts at τ_k by 2.2 to 2.5 times; the clusters
  read first over-predict the rest by 1.7 to 3.1 times; and the stop selects its own low estimates.
- *The replays* [measured; A2.6]. No stop on expected misses reads fewer rows than the built stop at equal mean recall on
  yahoo or LAION beyond noise (0.87 to 3.01 times its rows). At a nominal recall of 0.9 the delivered mean recall runs
  from 0.65 to 0.99 by tail and dataset.
- *A correction from the real reader.* A2.6 item 4 found the built stop losing to a fixed cluster count on LAION and
  glove. On the library's own stream the pooled stop wins on LAION (the fixed count reads 1.05 to 1.11 times its rows),
  so the LAION statement does not hold for the pooled stop; the glove statement does [measured, by the visit-order
  session; `LOG-2026-11-21.md`, "The fixed-cluster-count replay"]. Chapter 5, section 5.8a, carries the stop and this
  replay.

**A related result kept: the per-query spread.** For a fixed offset, ρ_ℓ(q, c)² = E[(Δ·e)² | κ]/E[h² | κ] = E_ω[a]/‖a‖₁
exactly, and averaged over offsets it returns chapter 5's (5.6) [proved, unrefereed; the referee found no error; A3.1].
On LAION's text queries it removes 8.5 of the 13.5 points by which the derived width exceeds the measured one at 16 B,
and in distribution it changes widths by under 1% [computed, with a stand-in allocation; A3.3]. On one kind of query it
buys nothing a retuned ψ does not, and the program records it as correct and nearly neutral (`LOG-2026-11-21.md`,
"Tracks"). The out-of-distribution width is now carried by A4's per-cluster sample, which was unrefereed when A3 and A4
were revised (A4.9) and has since had a referee's report (`approximate/referee/referee-2.md`; chapter 5, section 5.9a).

**Verdict and bar.** The stop on expected misses is dropped (`LOG-2026-11-21.md`, "Tracks"). It was held to rows at equal
mean recall against the built stop, and to a setting that delivers its recall, and it met neither. The per-query spread
is a different rule and is not dropped: it is kept as correct and nearly neutral.

**What would reopen it.** A calibration set with truth valid for the session's visible set, under which M̂ could be
thresholded by conformal risk control or CaST (A2.7); a proved class of tails that excludes A2.2's construction; or the
harness test of `approximate/REPLAY-SPEC.md` overturning A2.6's ranking, which A2.8 predicts it will not [conjecture].

**Record.** `approximate/CLAIMS.md` A2, A3 and "Changes after referee 1"; `approximate/referee/referee-1.md`.

## 7.4 Alternatives to k-means for the partition

**Question.** LIMINAL groups rows by spherical k-means. Is that the right objective for what a cluster is used for, and
does an alternative beat it?

**What was tried.** The objective each use implies, scaling with the number of cells, assignment of cells and rows to a
basis by coding cost, seven alternatives (`partition/CLAIMS.md` P1 to P4), a prior-art search (`library/partition.md`),
and an experiment on the 200k BioCLIP-2 sample (n = 768, K = 200, G = 12; 49,978 held-out rows for coding, 830 queries
for the decisions) with drop rules fixed in advance (`partition/EXPERIMENT-SPEC.md` section 5).

**The objectives.** The coding cost of a Gaussian cell at a price λ, bits plus λ times the mean half-width, is at least
D_λ = Σᵢ φ(λ²γᵢ), with φ(u) = √u for u ≤ 1 and 1 + ½ ln u above; D_λ/λ tends to F as λ → 0, and D_λ is the
log-determinant up to a constant at high price [proved, unrefereed; P1.1, P1.2; the referee found no error]. P1.1
rests on the full floor, which chapter 2, section 2.9a proves as Theorem 2.24.
k-means minimises a trace, which is linear in the spectrum where the coding cost is concave. The two disagree when one
cell is flat and the other small, and in a two-segment example the partition k-means prefers costs 2.00 bits a row more
at w = 0.05 [model; P1.3, confirmed by the referee]. The version the referee saw said that within a group k-means in the
group's metric is coding-optimal, so that the mismatch lies only in which basis a cell uses. That is [refuted]
(`partition/referee/referee-1.md`, P1.4): the group's metric is fitted to a scatter that moves when rows move, the
zero-read cost is concave in it, and the referee's counter-example has two partitions of one group that are both fixed
points and differ by 10%. The corrected statement is that the mismatch appears wherever a covariance is formed: which
rows share a cell, which cells share a basis, and which basis a row is coded in [model; P1.4, S.1]. The sorted range
counts the low tail of the keys, whose sum spherical k-means ascends, and the approximate order is served by a trace in
the queries' metric, as identities [proved, unrefereed; P1.6, P1.7]. The built fit stops short of a fixed point, with 4.4% of rows
away from their best centre on this sample [measured; `partition/experiment/RESULTS.md` item 1].

**The experiment** [measured; `partition/experiment/RESULTS.md`, "Summary table" and "Drop rules: which fired"]. The seed
spread is 1.5% of S1d and 3.0% of S1b, so a coding difference must exceed about 3% or 6% to count. The built partition
had 4.4% of rows away from their best centre, so the baseline is the built rule run to a fixed point.

| Alternative | Its figure | Bar | Outcome |
|---|---|---|---|
| Built rule to a fixed point (P1.6) | mean key +0.11%; coding and decisions within twice the seed spread | reported | baseline; worth little at K = 200 |
| Cells to bases by coding cost, B0 and B16 (P3.1) | S1b 0.5% and 1.4% lower; 2.2% and 2.5% with single moves | ≥ 1% and ≥ 5.9% | dropped, every variant |
| Rows to the cell of narrowest 16 B interval, C16 (P3.5) | S1d 1.6%, S1b 2.4% lower | S1b ≥ 3% | dropped |
| The same at zero read, C0 | S1b 1.6% lower | as C16 | not distinguished from C16 |
| Balanced k-means, D (P4.1) | S1b +0.23%; undecided rows +1.3 to 2.4% for items; order reads 53% more rows for items | S1b ≤ +1%, undecided ≤ +3% | not dropped; layout gains, decisions pay |
| Neighbour-graph partition (P4.2) | 69.8% of a row's 10 nearest rows in its own cell | ruled out above 80% | not ruled out; not run |

The level model is the floor's idealisation, not the built code's bytes, so the figures are ratios between partitions.

**What the grouping is worth** [measured; RESULTS item 3]. Against one basis for all rows the built grouping is worth 16%
of S1d and 22% of the bytes at w* = 0.06, and a random grouping of the same sizes keeps a quarter of that. A basis per
cell is a further 10% narrower at 16 times the per-row state. That is the room B could reach, and B reached under 2.5%.

**The number of bases is a trade.** The earlier statement that the theory "sides with fewer bases" is withdrawn. On
BioCLIP-2 at 1M the bytes per item rose 3 to 7% from 64 groups to 31, and 6 to 10% per halving from 62 groups to 7;
with full vectors taken out the code bytes rose 2 to 13% per halving [measured in `rust/REPORT_CLUSTERSIZE.md`, the split
the theorist's arithmetic; P2.5, after the referee's P4 item 3]. So fewer bases cost bytes read and save setup time and
storage, 37.7 B a row at G = 64 [computed from the rules; P2.5]. The slope is about twice the 2.2 to 3.7% per doubling
that `hierarchy/layered.md` 8.8 measured on yahoo and LAION and the earlier version used; that file is in another
worktree and no referee has checked the figure.

**Prior art** (`library/partition.md`, verdicts 1.6, 3.6 and 6.1). The log-determinant coding length as a segmentation
objective (Ma, Derksen, Hong and Wright 2007) is established [cited]. The entropy-constrained encoder rule (Chou,
Lookabaugh and Gray 1989) is established too, though the librarian read it in Gray and Neuhoff's survey and not in the
paper [cited, unchecked]. That a coding-optimal partition beats k-means is only empirical (Archer and Leen, 1.1 to
1.8 dB on image blocks), and neighbour-graph partitions read up to 2.3 times fewer candidates at 16 to 256 cells
(Dong, Indyk, Razenshteyn and Wagner) [cited]. Nothing was found that chooses a partition for the width of a certified
inner-product bound.

A defect found on the way, groups with fewer rows than the dimension, is defect 8 of chapter 1, section 1.8a.

**Verdict and bar.** No alternative is adopted. The coding schemes B and C were dropped by rules fixed in advance: B
gained at most 2.5% of S1b against a bar of twice the seed spread (5.9%), and C16 gained 2.4% against its bar of 3%.
Balanced k-means was not dropped, but it buys layout with
decisions and the approximate order. The neighbour-graph partition was not run. The mismatch of objectives is real and
present at every step where a covariance is formed, but on this sample no scheme that descends it gains more than twice
the seed spread.

**What would reopen it.** A gain above twice the seed spread on another corpus or at much larger K; a paired noise
estimate for B, which changes only the grouping of fixed cells; an encode of a winning partition, to see whether an
idealised gain reaches the built code's bytes; or a neighbour-graph partition at thousands of cells for the approximate
order alone.

**Record.** `partition/CLAIMS.md` (with "What failed or was withdrawn"); `partition/referee/referee-1.md`;
`partition/EXPERIMENT-SPEC.md`; `partition/experiment/RESULTS.md`; `library/partition.md`.

## 7.5 Structured rotations in place of dense Haar rotations

**Question.** The randomised levels rotate each block by a dense Haar matrix, 9.9 MB in f64 at n = 2048 and about 1 ms
per group setup. Can the certified bound be re-derived for a cheaper rotation?

**What was tried.** Sign–Hadamard rounds, butterflies, Kac walks, Kronecker, unitary and symplectic forms, and
sub-blocks: the condition the error law needs, failing inputs for the built code, a count of randomness, and the law
under Haar measure on subgroups (`rotations/CLAIMS.md` A to H; `library/rotations.md`). Tags below are the claims' own,
[proved, unrefereed] unless stated; verdicts are from `rotations/referee/referee-1.md`.

**What was found.**

- *The exact condition* (A1). The law holds for every codebook exactly when every orthonormal pair is sent to a uniform
  point of V₂(ℝⁿ); a conditional density at most M times uniform on that law costs c² → c² + 2·ln M. The referee found
  no error. Pair-uniformity is strictly weaker than Haar (H2).
- *A finitely supported rotation leaves nothing random* (A2). Reading A2 as the reason to reject sign–Hadamard products is
  [refuted]: A2 applies word for word to today's dense matrix, a function of a 64-bit seed. The reasons are the B claims
  and C1.
- *Sign–Hadamard rounds fail on the built code.* One round fails outright, 16 sd at n_b = 256 with probability ½ per side
  (B1; rebuilt by the referee). For more rounds, inputs constant on a subgroup of coordinates exceed the two-sided budget
  2^(−60.8). The referee's recomputed rates, with exact mixture weights [computed]: 2^(−19.1), 2^(−41.1), 2^(−58.5) for
  two, three and four rounds at n_b = 256; 2^(−20.6), 2^(−57.0) for two and three at 512; 2^(−31.9) for two at 1,024.
  They agree with B4's table within 0.1 and survive a pessimistic model of tan φ's rounding.
- *Small blocks, corrected.* "At blocks of 64 and below the construction does not reach the constant" is [refuted]
  (referee, "Addendum to B4"). Against each block's own constant the construction fails one round at n_b = 64 with
  probability 2.8·10⁻⁴ and two rounds at 2^(−13.0), and gives 2^(−11.6) to 2^(−46) for one to four rounds at 128, all
  above budget [computed by the referee]. So sign–Hadamard rounds fail the guarantee at blocks of 64 and 128 as well as
  at 256 and above: at every power-of-two block from 64 up, for one to four rounds, by this family of inputs alone. The
  original sentence may stand only for n_b ≤ 32; at 16 the construction reaches 1.9 to 2.2 sd against a constant of
  about 3.9.
- *Incoherence does not rescue them* (B5) [sketch: gap named]. The referee adds that inputs with every coordinate at
  most 4/√n_b times the norm fail two and three rounds at 256, so an incoherence assumption with that constant is
  [refuted] there, not merely unproved.
- *The randomness count* (C1). A half-width of the form h(A, ‖Δ_⊥‖) that never fails is at least Cauchy–Schwarz, so k
  rounds need kn_b − k + 1 ≥ 62 bits to beat it at all. The recommendation that followed in C2, a seed of 128 bits for
  N ≥ 2³³, is [refuted]: the guarantee is ε at the answer level, many atoms at every N.
- *Butterflies* (D2). For y = e₀, Δ = e₁ and any codebook the error is fixed by z up to sign: as corrected by the referee,
  it is |Δ·E| that is a function of z alone, and its sign is a fair coin given z [proved, unrefereed after the repair].
  Its rms is 3.7 to 4.1 sd at n_b = 256 and it exceeds 7.8 sd on 20 to 37% of draws at 1,024 [computed, D4]. No draw
  exceeded the certified constant at n_b ≤ 256, so there the record shows a wrong law, and a failed bound only at
  1,024 (referee, "D1 to D4").
- *Kac walks* (D1, D3). A random-pair walk needs at least 21.4·n_b steps to be within 2.5·10⁻¹⁹ of uniform [proved,
  unrefereed], and no density-ratio bound is known [sketch: gap named]. No failure was seen at n_b = 64, so the case is
  the floor and the missing theorem, not a counter-example.
- *Subgroups* (H1). Under Haar measure on a closed subgroup the error given z is invariant under y's stabiliser, so a
  rotation stored in fewer numbers than n_b² by some factor needs about that factor more scalars and sums per item and
  block, or leaves deterministic terms. The Kronecker row is [refuted] for
  unequal factors: for O(b) ⊗ O(k) with b > k the stabiliser contains O(b − k), so only b = k leaves it finite. Neither
  case gives a sphere, so neither has an exact law of today's form.
- *Unitary and symplectic forms* (H4) keep the law in n_b − 2 or n_b − 4 dimensions and halve or quarter the stored
  matrix, at the price of a second stored scalar, and of a second sum unless a deterministic term is carried; that term
  is 10% of c·sd on average and 21% at the ninth decile [computed at n_b = 64 for the unitary form only; nothing was
  measured for the symplectic].
- *Sub-blocks* (F1 to F5). Independent Haar rotations on sub-blocks keep the law. With own scalars nothing is lost in
  width, but the scalars per item multiply; with one κ per block a radial term widens the bound 12 to 20% with two
  sub-blocks [computed; reproduced by the referee].

A positive result came on the way: a rounded copy of the same rotation on the query side, the build unchanged, costs a
deterministic allowance [proved, refereed; E1; chapter 3, section 3.2a, Proposition 3.25]. In f32 the allowance is a few
thousandths of a percent of the random term at the rms tan φ [computed, reproduced by the referee; E2].

**Verdict and bar.** The bar was the guarantee "for any data, over the build's rotation" at today's constant.
Sign–Hadamard rounds fail it at every block from 64 up, and butterflies at 1,024. Kac walks have neither a failure nor a
theorem, only D1's floor, and Kronecker forms give no law of today's form; neither meets the bar. Unitary, symplectic
and sub-block forms keep it and trade matrix bytes for per-item scalars and sums at H1's rate; none was adopted.

**What would reopen it.** An ensemble with o(n²) parameters that is uniform on pairs, which H6 conjectures does not exist;
a density-ratio bound for a Givens schedule of about C·n·(ln n + ln(1/δ)) angles (D3); or a tail bound for the built
codebook under structured rotations that covers B3's inputs.

**Record.** `rotations/CLAIMS.md` A to F and H; `rotations/referee/referee-1.md`; `library/rotations.md`.

## 7.6 A shared dither in place of a rotation

**Question.** Can a subtractive dither shared by every item, with no rotation, give the randomised levels an exact error
law and save the rotation and the per-item κ and tan φ?

**What was tried.** A nested dithered scalar code in the group's basis: the seed gives each position a dither uniform over
one cell of the coarsest grid it uses, and each item has a step s (`rotations/CLAIMS.md` G1 to G10).

**What was found.** Tags are the claims'; the referee's verdicts are in `rotations/referee/referee-1.md`, "G1 to G10".

- *The law* (G1) [proved, unrefereed; the referee found no error]. Without overload, the errors of the coded positions are
  independent and uniform, and depend on the item through s alone: Δ·E = s·Σ_p Δ_p·2^(−d_p)·V_p with V_p uniform on
  (−1, 1).
- *The constant* (G5, G5a). The sub-Gaussian value, 9.256 at N = 10⁹, is valid for every weight vector [proved,
  unrefereed]; a cited Rademacher bound gives 9.040 [cited, unchecked]; the worst case is conjectured to be 8.896
  [conjecture]. These are 4.8%, 2.4% and 0.7% above today's 8.830 [computed; reproduced by the referee].
- *Scalars* (G6). One step and one norm per block, 8 bytes for four blocks against 15 to 20.5 bytes now [proved,
  unrefereed, as a count].
- *The width* (G7) [computed, `rotations/g_dither.py`; the referee checked the coefficients by hand and did not rebuild
  the crest factors]. The dither's spread is set by a block's largest coordinate, so it scales with the crest factor
  (about 3 at n_b = 256). Nested from one bit the code's spread is 2.0 to 2.65 times today's at every depth, at blocks
  of 64 to 768, which is 1.0 to 1.4 bits per position lost; the half-width ratio is this times 1.01 to 1.12 for the
  constants. Clipping does not rescue it, and at sub-bit rates the dither is no better than leaving
  the block uncoded. A flattening transform keeps the law but cannot go below a Gaussian crest factor (G8).
- *With the rescaled code* (G9) the stored κ depends on the error, so G5 does not apply and the two do not combine. The
  referee repairs one word: the two sums are uncorrelated only when every coded position has one depth; the conclusion
  stands.

**Verdict and bar.** The law is proved, and the librarian found no prior statement of it. Against today's width the
dithered code is 2.0 to 2.65 times wider when nested from one bit, and the first randomised level is below one bit per
position, where the dither is no better than leaving the block uncoded; so it is not adopted.

**What would reopen it.** A dither over a lattice cell in 8 to 24 dimensions with its range set by group norms, which
would halve the crest factor [conjecture, G10], if such a code is table-decodable (7.2 closed E8 as a drop-in); or a
schedule whose blocks are first coded at two bits or more, where the ratio is 1.3 to 1.8.

**Record.** `rotations/CLAIMS.md` G1 to G10; `rotations/referee/referee-1.md`; `library/rotations.md` section 6.

## 7.7 Confidence: statements withdrawn

The confidence track asked what the confidence constant must cost and where randomised levels should take over. Its
proved results go to chapter 3; this section records what it withdrew.

**A4 as a new, narrower interval.** A4 centres at Ê/(1 + k_φ) with half-width q·tan φ/√(1 + k_φ), and was presented as a
new interval narrower than the plain rule. The referee showed it is R8's interval with Ê² dropped under the root, so it
contains R8's for every outcome: it is R8 with its half-width frozen at its value at Ê = 0
(`confidence/referee/referee-lower-bounds.md` section 2). What is new is that A4 lies in 𝓛 and meets Theorem B's lower
bound, inside the bit budget after the referee's repair (A4′) [proved, unrefereed; `confidence/CLAIMS.md`, "Changes
after the referee"]. Chapter 3, section 3.7a, gives the bound as Theorem 3.43 and A4 as Proposition 3.46. "Narrower"
holds of half-widths only, since A4's upper end is above the plain rule's in 13 to 22% of draws at s = 0 [model,
simulated by the referee at n ≤ 13]. One event serves the plain rule, A4 and capped R8 in one block, so their minimum
may be used [proved, unrefereed; A5, the referee's; chapter 3, Proposition 3.48].

**R9 "for any per-item rule".** That an answer exact with probability 1 − ε forces every per-item rule's mean failure
probability below (ε/N)(1 + o(1)) is [refuted] (`confidence/CLAIMS.md` C2.0): a rule that excludes every item when the
rotated query falls in a set of probability ε is per-item and has mean failure ε. What holds is Corollary Q: for every
per-item rule and i.i.d. corpus of threshold items, Pr_ω[p(ω) ≥ x/N] ≤ ε/(1 − e^(−x)) [proved, unrefereed; the
referee's statement]. This quantile form is chapter 3's Corollary 3.30, in section 3.6a.

**Coding every block at the 56-byte level.** The recommendation to code every block at the first randomised level is
withdrawn once the stored scalars are charged: in time coding three or four blocks there would pay only below 0.3 to
1.1 ns per block-rise, against a decode of 1.4 to 2.8 ns, with measured costs [model; `confidence/DESIGN-CLAIMS.md`
C5.2, C5.6]. The earlier native trial was 8 to 24% slower, though that trial changed every level's rates, so the figure
is confounded (`confidence/referee/referee-design.md` section 4.1). The model also over-predicted the gain (6 to 26%
fewer entrants to 96 B against 2% measured). The referee
restates what replaced it: in time at most one coded block, robust over 1.4 to 10 ns; one against none undecided; in
stored bytes a near-tie, three blocks winning by 4 to 6% if |y| is not charged [model;
`confidence/referee/referee-design.md` section 4.2]. The candidate left is one wider block at the same 24 B (C5.7, V1).
This is in chapter 4, section 4.9 (4.9.5 and 4.9.6).

**Code-size conditions called necessary.** For one block, one level, the plain rule and a random shared code, the union
over N is attained to 1% under a sufficient condition: 50 to 51 bits per block, or 30 to 41 bits by the referee's
refinement, at N = 10⁹ [proved, unrefereed; computed; `confidence/CLAIMS.md`, C2 summary]. "R9 needs M ≫ N·ln²(N/δ)" and
"it fails with fewer codewords than items" are withdrawn as necessary conditions. The refined bound contradicts them at
d = 95: at 29 bits, where M = 5.4·10⁸ is fewer than N = 10⁹, the union is attained within 2% [computed;
`confidence/CLAIMS.md` C2.4; `confidence/referee/referee-lower-bounds.md` section 6, Claim E]. The brief's target, "the
least half-width is q·(4^R − 1)^(−½)", is likewise withdrawn: it is an upper bound (chapter 3, Theorem 3.38) and the
k_φ → 0 limit of w* [proved, unrefereed; `confidence/CLAIMS.md`, Corollary B4 and "What the track establishes"].

**What would reopen these.** A lower bound outside 𝓛, for the built scalar code, or for several blocks and levels (gaps
G1, G3); and for the 56-byte level, the native test of C5.7 with block-rises counted.

**Record.** `confidence/CLAIMS.md`, "Changes after the referee"; `confidence/referee/referee-lower-bounds.md`;
`confidence/DESIGN-CLAIMS.md` C5; `confidence/referee/referee-design.md` sections 4 and 6.

## 7.8 Stage 0: the numerical attack on the floor

**Question.** Before the floor of Conjecture 2.14 was proved, could a real code be found below it?

**What was tried.** Exact mean half-widths of whole codes and single cells against the water-filling floor W (F1), the
floor with κ_k on a head (F2), and the one-dimensional lemma and per-cell inequality the proof would use. The codes were
entropy-coded scalar quantisers, a quantised head with a norm-coded tail, shell codes, orthant-by-shell cells, 2-D
lattices and D4, on flat, power-law and measured spectra (`stage0-floor-check/RESULTS.md`).

**What was found** [computed; `stage0-floor-check/RESULTS.md`, "Verdicts" and appendix]. Nothing was refuted. The closest
approach to F1 is a one-coordinate quantised head with a norm code on a flat tail of 2,000 eigenvalues, at 1.0026 of W;
against F2 with S the head the same code reaches 1.0023, and that ratio falls roughly in inverse proportion to the
tail's length. Three early ratios below 1 were a bug in the tail grid, fixed and rerun.

**Verdict.** Nothing to rule out. The floor survived, and chapter 2, section 2.9a now proves it under the Gaussian model
(Theorem 2.24). The attack shows it is close to tight on codes with a short head and a long norm-coded tail [computed;
same source].

**What would reopen it.** The families not run: spherical-cap codes, lattices beyond D4, cells that couple head and tail,
and F2 for heads of two and three coordinates at long tails.

**Record.** `stage0-floor-check/RESULTS.md` and its logs; chapter 2, section 2.9a.

## Editor's queries

1. The brief asked for "the recomputed rates in E7". The rotations claims have no E7; 7.5 gives the referee's recomputed
   B4 rates.
2. `codes/REPORT.md` and `codes/NOTEBOOK.md` entry 10 tag their worth figures [estimate], which is outside the tag scheme
   of `README.md`; 7.2 carries them as [model] with the word "estimate" kept, as chapter 4 does.
3. The sources disagree on whether the 24 encoder scales are free. `codes/REPORT.md` ("Verdict" item 1) and the
   "Tracks" table of `LOG-2026-11-21.md` call them free; the same log's "Needs the machine" lists a fourfold encode
   loop and an unchecked κ grid. 7.2 follows the second and does not use the word. The log's "one point" is consistent
   with `codes/NOTEBOOK.md` entry 10's shares.
4. `confidence/DESIGN-CLAIMS.md` C5.3 item 1, and `LOG-2026-11-21.md` "Needs the machine", still read "the optimum codes
   one block"; 7.7 gives the referee's restatement (`confidence/referee/referee-design.md` sections 4.2 and 7), which the
   design file has not taken in.
5. 7.6's "1.3 to 1.8" for blocks first coded at two bits is G7's table (1.30 to 1.77); the claims' prose says the PI's
   1.3 to 1.45 holds there.
