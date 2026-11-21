# Referee report 3: `approximate/DIAL.md` (the levels' dial as a function of the stop's)

21 November 2026. I read DIAL.md, chapter 5 §5.8 to §5.9a, referee 2, `dial_laws.py`, `dial_replay.py` and the reader at
`/home/joe/code/vexact-sampled` (`research/sampled-width`, head `0a4a00979`, `rust/src` unmodified in the working tree).
I did not read any NOTEBOOK.md. My scripts are `ref3_replay.py` (the theorist's replay with its rules' code copied,
rows read as slices, extra rules and extra columns), `ref3_indep.py` (two numbers computed a second way) and
`ref3_timing.py` (time to recall from the timing session's files); outputs `ref3_*_{yahoo,laion,inat}.txt` and
`ref3_timing.txt`. Every job ran through `run-limited.sh 2 1 1`, under 20 s and under 60 MB resident.

## Verdicts in one place

| Claim | Verdict |
|---|---|
| §1 the rules as built (definitions, file and line) | no error found; two small additions (item 1) |
| §1.1 "unfiltered δ and ρ mostly sit on their priors" | correct for yahoo and LAION; **wrong for iNaturalist**, where 3 to 10 clusters a query are read directly |
| (2.1) and the stop's frame, u_c(t̂) > ψ | no error found as algebra; "ψ is a quantile of u_home" is right to a factor 1.5 to 2.5 only |
| (2.2) and β | correct at slope m = 1; with the measured slopes a height-dependent term is missing (repair below) |
| D1 | no error found (an identity) |
| D2, "equal risk per row declined is the cost optimum at κ = 1" | correct after a repair: cross terms change κ to c₁/(c₀ + c₁φ); three hypotheses to state |
| Rejection of equal risk per neighbour | sound, and does not rest on the replay |
| H1 to H3 and the 1/√2 | gap: H2 is not a measured law; the 1/√2 acts as a tuning constant (item 4) |
| s̄ (mean slack) | correct after repair: log-mean-exp, not the mean |
| The replay is causal | no error found |
| The row proxy for time | **wrong in kind on LAION** against the reader's own timings; misses two costs |
| "nothing fitted" | true in the letter only |
| The formula against ψ_ℓ = 3 and ψ − ū + 1 | not distinguishable on cost; **dominated** by a constant at its own mean dial |
| Recommendation | replace for timing by a constant offset, ψ_ℓ = max(0, ψ − c), c ≈ 1.5; see item 6 |

## 1. The definitions against the code

Every line number in DIAL.md §1 matches head `0a4a00979`: the stop key (`adaptive.rs` 786, used at 817 and 848), the
pooled moments (776 to 815), the prior (777), the window (87, 821 to 847), the bypass (826 to 832), the σ stand-in (834
to 845), `visit_z`/`visit_sampled` (901 to 906), `flush` (`q.rs` 3839 to 3863), σ = 0 (3875), the sample (3956, 3988,
4016, 4029, 4032 to 4053), the cap applied as est + min(d·h, cap) < t (`q.rs` 1308), ψ_ℓ (`adaptive.rs` 796), the
randomised levels at ψ times the median of 1/c (634 to 647, 645). No error found. Two additions:

- The σ a directly read window cluster contributes to the moments is the window's, which is the stand-in rad_c·s
  whenever σ_c cost more than a direct read (line 844). Those are exactly the clusters likely to be read directly, so
  δ and ρ can be in stand-in units. DIAL.md says "the values the window holds", which is right but hides this.
- The reader's code between `7ac09e9` (the sampled width as built) and `0a4a009` differs only by the knob
  (`git diff 7ac09e952 0a4a00979 -- rust/src`: comments and the `level_psi` plumbing), so the timing session's
  ψ_ℓ = 2 and 3 runs are of the as-built reader.

**δ and ρ on their priors.** Confirmed for yahoo: at `b87f151` 0.1 to 0.6 clusters a query are read directly at ψ 4
to 6, and at `0a4a009` 0.0 (`level-psi/ab-knn-yahoo-…-lp3.txt`). LAION: 0.4 to 3.0 direct of 9 to 30 visited, so the
moments rest on one to three early clusters. **iNaturalist: refuted.** 4.6 to 10.5 of 8 to 30 clusters are read
directly at `b87f151` and 2.8 to 5.9 at `0a4a009`, the first cluster among them, so δ and ρ are fed. The replay's
δ = 0, ρ = 1 is therefore a departure from the reader on iNaturalist, the dataset where the formula fails; its
stop-only recall there (0.959 at ψ = 4) is below the reader's (0.973). The PI's relay agrees (7.5 of 17.8 direct at
`d999606`).

A bookkeeping slip in §2.3: the reader's "past the deterministic levels" (15,304 on LAION at ψ = 5) does not include the
samples (`sample` removes them from the candidates and counts its runs on a separate `Stats`), but it does include
the first cluster, which is read before k are held, is not sampled and passes about 2,300 rows at certified widths
(`lp2` at ψ = 2: 2,276 past with 1.1 clusters read). The like-for-like figure is 15,304 − 2,342 ≈ 12,960 against the
replay's 12,591, closer than DIAL.md's comparison.

## 2. The frame

**(2.1) for the stop.** I re-derived it: with â₀ = μ_c + δσ_c and Y₀ = ψρσ_c, the row's height x cancels and the drop
condition is u_c(t̂) > ψ. No error found. It is a statement about a cluster in the window at the moment of the test.
A neighbour is missed when its cluster is unread when the query stops, which differs in two ways: its cluster can be
outside the window (missed with u_home < ψ), and a cluster with u_home > ψ is still read whenever another window
cluster keeps the query going. Measured (`ref3_indep_*.txt` against the replay's stop): share of neighbours with
u_home(τ_k) > ψ against the causal stop's miss, ψ = 4 / 5: yahoo 0.301 / 0.097 against 0.209 / 0.077; iNaturalist
0.104 / 0.012 against 0.041 / 0.008; LAION 0.139 / 0.019 against 0.121 / 0.044 (the dump's 30 clusters). So "ψ is a
quantile of the law of u_home" is right in kind and off by a factor 1.2 to 2.5 in the tail, which is 0.1 to 0.7 of a
dial unit at θ = 0.7. It also uses the final threshold, which the stop never sees.

**(2.2) for a level.** With e = m_ℓ + τ(x + √f·g), τ = σ_c√(1 − f) and ζ > 0 I get the drop condition
g > ψ_ℓ + (x − ζ)β with β = (1 − √(1 − f))/√(f(1 − f)). Correct. But the law assumes slope 1. With slope m (e/τ = m·x +
√f·g, measured m 0.92 to 1.24, §5.9a.2) the condition is

    g > ψ_ℓ + (x − ζ)·β − (m − 1)·x/√f,

so at m = 1.2, x ≈ 3, f ≈ 0.8 the dial loses 0.67 of a unit to the height. "ψ_ℓ is a quantile of the scatter after ζ
has paid for the height" holds only at m = 1; with m ≠ 1 the dial absorbs a term that grows with height, which is
referee 2's widening scatter in another form. This alone accounts for about a third to a half of iNaturalist's 1.4 to
2 units of shortfall (§4 of DIAL.md); part of the 1.2 belongs to the sample-versus-whole frame (referee 2 §7).

## 3. The criterion

I derived the first-order condition myself. Minimise T(ψ, ψ_ℓ) with M_stop(ψ) + M_lev(ψ, ψ_ℓ) fixed. With D2's prices
and no cross terms, (∂T/∂ψ)/(−∂M_stop/∂ψ) = (∂T/∂ψ_ℓ)/(−∂M_lev/∂ψ_ℓ) gives c₀/π₀ = c₁/π_ℓ, which is D2. So **equal risk
per row declined (π_ℓ = π₀) is the cost optimum at κ = 1**, as claimed. It needs, beyond D2's stated form of T:

1. One price a row in each class (c₀ an average over a new cluster's rows, including the rows the sorted range drops
   at almost no cost; c₁ a row past the deterministic levels).
2. Disjoint misses: true, since the levels act only in read clusters.
3. **No cross terms, which fails at first order.** ψ also opens clusters whose rows pass the levels at about the rate
   φ, and whose neighbours the levels drop at a rate m_new. Keeping these, ∂T/∂ψ = (c₀ + c₁φ)N′ and the neighbours
   bought by ψ are π₀N′(1 − m_new), so the condition becomes π_ℓ/c₁ = π₀(1 − m_new)/(c₀ + c₁φ): **κ_eff = c₁/(c₀ + c₁φ)**.
   With c₁/c₀ = 3 and φ = 0.1 to 0.2 (LAION) the correction is a factor 1.3 to 1.6, 0.2 to 0.3 of a dial unit wider.
   The effect of ψ_ℓ on t̂ (and so on the stop) is second order and DIAL.md names it.

**Rejection of equal risk per neighbour.** Sound, and it does not need the replay. Reading 1 is optimal only if
κ·(θ_y/θ_u)·R′/N′ = 1. In the replay R′/N′ ≈ (d ln R/dψ_ℓ)/(d ln N/dψ)·φ ≈ 0.65·φ, about 0.012 on yahoo at ψ = 5,
so reading 1 needs κ near 80. The reader's same-run timings put κ at 2 to 7 on LAION (lp3 minus lp2 at ψ = 4.5 to 6:
128 to 839 µs for 1,584 to 3,421 more rows past the levels, 0.08 to 0.25 µs a row, against 0.033 to 0.039 µs a row
touched). The supporting replay sentence ("the rule nearest reading 1 lies between med−1 and med+0") is weaker than the
argument: those rules are reading 1 only through H2, which fails on iNaturalist.

## 4. The algebra from the criterion to the formula

I re-derived the closing step with two scales. Under H1, D1, D2 and H3:

    ψ_ℓ = ȳ + (θ_y/θ_u)·(ψ − ū) + θ_y·ln(θ_u/(θ_y·κ·φ)),

which is (2.4) when θ_y = θ_u = θ and ȳ = −s̄. The algebra is right. Where each term comes from:

| Term | Status |
|---|---|
| ψ − ū | H1 (the stop's miss exponential above the median of u_home). Assumed; it fits yahoo (½e^(−1.37/0.75) = 0.080 against 0.086 at ψ = 5) and misses iNaturalist by 2 to 4 times |
| β | derived exactly from slope 1 and τ = σ√(1 − f); wrong at m ≠ 1 (item 2) |
| s̄ | the slack's law replaced by its mean (assumed), from the held rows' slacks in their own clusters (a proxy for the neighbours of the cluster in hand, assumed) |
| φ̂ | H3, R′/N′ = R/N, assumed; the replay's elasticities give R′/N′ ≈ 0.5 to 0.9·φ. The floor at the sampled share is a rule added after the first form failed (`fb4`, `fb16`), not derived |
| κ = 1 | by fiat; the reader's same-run timings suggest 2 to 7 on LAION |
| 1/√2 | H2, assumed |

**The 1/√2.** It is the mean excess of a unit-variance Laplace tail, used for both laws. The measured scales are
θ_u 0.6 to 0.87 (the reader's stop-only recall over ψ 4 to 6: yahoo 0.87, LAION 0.86, iNaturalist 0.61) and, for the
levels, 0.84 to 1.56 by referee 2's per-unit ratios (1.9 to 3.3), 0.9 to 1.0 by the reader's own lp2 against lp3 misses
at ψ = 9 (ratios 2.7 to 3.0) and 0.56 to 1.0 in the causal replay's c2 against c3 drops. So θ_y/θ_u is about 1.1 to
1.6, not 1. With the measured rates (θ_u = 0.75, θ_y = 1.0; rule `fbm` in `ref3_replay.py`) the slope in ψ is 1.33 and
the log term's coefficient 1.0. The dial rises by 0.7 to 1.2 units (yahoo 4.04 against 3.21 at ψ = 5; LAION 4.07
against 3.41; iNaturalist 5.04 against 3.89), and the proxy gets worse on yahoo and LAION (0.97 / 0.94 and
0.93 / 0.89 of as built at c₁ = 3, against 0.93 / 0.90 and 0.86 / 0.87) and no better on iNaturalist. "Nothing fitted"
is then false, since θ_u and θ_y are per-dataset measurements.

Said plainly: **the 1/√2 is not fitted by regression, but it is a tuning constant in effect.** Its value is not the
measured one, the measured one makes the rule worse, and the rule's form (the floor on φ̂, the slack, θ) was chosen
among 18 variants on the same 300 queries it is checked on. What the formula delivers is an offset ψ − ψ_ℓ of about
1.6 to 1.8 on yahoo and LAION and 1.1 on iNaturalist, and θ is one of the knobs that sets it.

**s̄, repaired.** Under H2's exponential tail the miss averaged over neighbours of slack σ_i is
½e^(−ψ_ℓ/θ)·mean_i e^(−σ_i/θ), so the shift is

    s̄_eff = −θ·ln mean_i exp(−β·(s_i − t̂)/(θ·σ_home(i))),

not the mean. The held set always contains a row of slack 0, so s̄_eff ≤ θ·ln k (1.63 at k = 10). The mean slack has
no such bound: as f → 1, β ~ 1/√(1 − f) → ∞ and the formula drives ψ_ℓ to its floor 0, which drops a neighbour at t̂
with probability one half. DIAL.md's Jensen remark (a factor 1.2 to 1.3) is the size of this at the recorded f; the
limit is the reason to repair it.

## 5. The replay

**Causal: no error found.** The running threshold is the k-th of the rows held at that moment, taken again after the
sample (`dial_replay.py` lines 89, 103); ū, s̄ and φ̂ use the held rows, rows already passed and the cluster in hand's
sample; the stop looks at the next 8 dumped clusters' μ_c and σ_c, which the reader's window has. The true neighbours
(`order.npy`) are used only to score. This answers the PI's question: the replay uses the running threshold at the time
each cluster is read, so the timing session's warning about final-threshold replays applies to DIAL.md §2.2 (the
laws, and the checks of H1 and H2 against them) and not to §2.3 or §4. Its levels match the reader within about two
points of recall (constant ψ_ℓ = 3 at ψ = 5: replay 0.922 / 0.949 / 0.990, reader 0.900 / 0.945 / 0.989 on yahoo /
LAION / iNaturalist). Its other limits are as DIAL.md lists them, plus δ and ρ on iNaturalist (item 1).

**Re-run.** `ref3_replay.py` reproduces DIAL.md §4's table to the digit on yahoo (formula at ψ = 5: 0.921 / 538 / 0.02 /
3.21 (0.66); as built 1,331; proxy 0.93 / 0.90 and 0.86 / 0.82) and the LAION and iNaturalist rows I compared. The
φ̂ figures I could not reproduce: the mean φ̂ over sampled clusters at the formula's dial is 0.047 (yahoo, ψ = 5),
0.115 (LAION, ψ = 5), 0.040 and 0.042 (iNaturalist, ψ = 4 and 5) against DIAL.md's 0.031, 0.077, 0.025 and 0.03, so
the misfit factors r/φ̂ are 0.55, 2.2, 4.3 and 12, not 0.8, 3, 7 and 17. The conclusion stands; the r values rest on 2
to 13 dropped neighbours in 100 queries.

**Independent numbers** (`ref3_indep.py`, its own loop, no code from the theorist's scripts). Stop-only recall with whole
clusters at ψ = 4 / 5 / 6: yahoo 0.791 / 0.923 / 0.973, LAION 0.879 / 0.956 / 0.961, iNaturalist 0.959 / 0.992 / 0.997:
equal to DIAL.md's certified column. Share of neighbours with u_home > 4 / 5 / 6 at the true threshold: yahoo 0.301 /
0.097 / 0.024, LAION 0.139 / 0.019 / 0.000, median 3.63 and 3.05: equal to §2.2. Two caveats on §2.2's version: t is the
least score among the neighbours found in the dump, which is above τ_k for the 2 to 26% of queries with a neighbour
outside it; on complete queries yahoo's share above 5 is 0.082. Clusters under 128 rows hold none of these neighbours,
so their exclusion does not matter.

**The row proxy.** Three faults, the third decisive.

1. It omits the first cluster's rows past the certified levels (about 2,180 / 2,340 a query on yahoo / LAION; on
   iNaturalist the first cluster is read directly). Priced at c₁ they compress every gain: the formula goes from
   0.93 / 0.90 to 0.95 / 0.91 (yahoo, c₁ = 3) and from 0.86 / 0.82 to 0.92 / 0.86 (c₁ = 6); "45 to 64% fewer rows"
   becomes 23% fewer on yahoo at ψ = 5 counting every row past the levels.
2. It prices a sampled row at 3 and a sampled cluster at nothing, where the sample opens a second, float-level copy
   of the cluster when the integer kernel is in use (`q.rs` 4025). LAION's phase profile puts the as-built extra in
   RUNCL, REPACK, DECODE and BOUNDS (`sampled-width/phases-laion.txt`).
3. **It ranks the rules differently from the reader on LAION.** From the timing session's files (`ref3_timing.txt`;
   each build as a ratio to the derived-width build timed in the same run), time to recall 0.90 / 0.95 / 0.98: as
   built 1.41 / 1.56 / 1.64 (yahoo), 1.41 / 1.64 / 1.72 (LAION), 0.85 / 0.68 / 0.56 (iNaturalist); constant ψ_ℓ = 3
   0.93 / 1.05 / 1.18, 1.44 / 1.77 / 1.97, 0.82 / 0.59 / 0.55. So against as built, ψ_ℓ = 3 is 0.66 to 0.72 on yahoo
   (the proxy said 0.77 to 0.91), 1.02 to 1.15 on LAION (the proxy said 0.66 to 0.84) and 0.87 to 0.98 on iNaturalist
   (the proxy 0.95 to 0.98). The two runs drifted (the derived build took 370 µs in one and 546 µs in the other on
   yahoo at ψ = 5), so this comparison is between runs and could be off by 10 to 20%; it is still the opposite sign on
   LAION.

"Row proxy for time" is fair in kind for the randomised-level work (56% of the extra on yahoo): that work scales with
rows past the deterministic levels, which is what c₁ prices. It is not fair as a ranking of rules: it misses a fixed
cost a sampled cluster, and on LAION, where the dial's leverage is smallest, it predicts a gain the reader does not
show. DIAL.md's "recovers a third to a half of the gap" is from the proxy; on yahoo the reader's constant ψ_ℓ = 3
recovers all of it at recall 0.9, 90% at 0.95 and 70% at 0.98, and on LAION none.

**The reader's owner's figures** (relayed by the PI; the same runs as `ref3_timing.txt`, which they match: ψ_ℓ = 3
over `1fc22a5` at recall 0.9 / 0.95 / 0.99, iNaturalist 0.84 / 0.59 / 0.56, yahoo 0.93 / 1.06 / 1.31, LAION 1.44 /
1.77 / –) add one fact: LAION's remaining cost is the sample itself, about 800 µs a query of scattered stored-vector
reads and per-level gathers. What that does to DIAL.md's proxy conclusions:

- The proxy prices the sample at 3 row units a sampled row, about 5,000 units a query on LAION at ψ = 5 against
  about 61,000 touched, so 8% of the total. In time it is about 800 of 2,400 to 2,600 µs, about a third. A cost that
  no dial moves is thus underpriced four times, which inflates every rule's relative gain on LAION. Priced as timed,
  the dial acts on at most the rest, and on LAION the rest is where narrowing the levels buys little: cutting rows
  past the levels from 15,304 to 5,699 (ψ_ℓ = 3) did not show up as time.
- The randomised-level work is in the proxy only through c₁ on rows past the deterministic levels, at a price (3 or
  6) the replay never measured; same-run lp2 against lp3 gives 2 to 7 on LAION, and the first cluster's 2,100 to
  2,300 rows a query carry it too and are left out (fault 1).
- So: "the formula passes 31 to 64% fewer rows" stands as a row count; "a proxy for time at equal recall of 0.76 to
  0.93" does not stand as a statement about time on LAION, and on yahoo it understates what a constant dial
  achieves; "the optimum is flat" stands; "on iNaturalist it only ties as built" agrees with the reader. The replay is
  usable to rank rules by rows and drops at equal mean dial (where the formula loses to a constant), not by time.
- The owner's ψ_ℓ = 2 figure (LAION recall 0.955 at ψ = 6 against 0.973 at ψ_ℓ = 3) is the recall ceiling of item 6
  in another form: a constant dial too low costs recall that more ψ does not buy back.

## 6. The alternatives

At 100 queries a dropped neighbour is 0.001 of recall, about 1.5% of proxy cost at recall 0.9 to 0.95, so rules within
about 5% are not distinguishable. In the replay (`ref3_replay_*.txt`, c₁ = 3, recall 0.90 / 0.95):

| rule | yahoo | LAION | iNaturalist |
|---|---|---|---|
| formula (fbf1) | 0.93 / 0.90 | 0.86 / 0.87 | 1.00 / 1.03 |
| ψ_ℓ = 3 | 0.91 / 0.87 | 0.84 / 0.80 | 0.98 / 0.97 |
| ψ − ū + 1 | 0.92 / 0.87 | 0.83 / 0.83 | 1.04 / 1.09 |
| ψ − 1.5 | 0.91 / 0.90 | 0.83 / 0.85 | 0.99 / 0.98 |
| ψ − 2 | 0.91 / 0.88 | 0.83 / 0.81 | 1.04 / 1.07 |

The formula is never the best and never distinguishable from the best. More telling, **at equal mean dial a constant
dominates it**, with fewer rows past the levels and fewer neighbours dropped: yahoo ψ = 5, ψ_ℓ = 3 passes 421 and drops
0.01 where the formula (mean 3.21) passes 538 and drops 0.02; LAION ψ = 5, 3,310 and 0.07 against 4,501 and 0.11;
LAION ψ = 4.5, 2,830 and 0.07 against 2,957 and 0.13; iNaturalist ψ = 4, 302 and 0.02 against 335 and 0.07; iNaturalist
ψ = 6, ψ − 2 passes 616 and drops 0.02 where the formula (mean 4.80) passes 900 and drops 0.04. Its spread of 0.6 to
0.8 between clusters costs more than its adaptation gains, as DIAL.md's own remark on convexity predicts.

**A constant fails at high recall.** The reader's same-run lp2 and lp3 runs reach only 0.989 / 0.995 (yahoo), 0.970 /
0.989 (LAION) and 0.991 / 0.996 (iNaturalist) at ψ = 9, where as built and the derived build reach 0.998 to 0.999. In
the replay ψ_ℓ = 3's drops on yahoo grow from 0.01 at ψ = 5 to 0.03 at ψ = 6.5 while the stop's misses fall, so r
rises from 0.013 to 0.14. A constant is not one knob: it caps recall. The 30-cluster dumps hide this on LAION. The
dial must rise with ψ, which the formula, ψ − ū + c and ψ − c all do. Its slope in the replay is 0.72 to 0.85 per unit
of ψ, not 1 as §4 says, because φ̂ and ū feed back.

**What the evidence supports.** A constant offset, ψ_ℓ = max(0, ψ − c), with c ≈ 1.5: it is never worse than the
formula in the replay, has no spread between clusters, has no recall ceiling, and puts ψ_ℓ at 3.5 at ψ = 5 and 4.5 at
ψ = 6, within the formula's range. It is one fitted number, from three datasets of 100 queries; c = 2 is as good on
yahoo and LAION and worse on iNaturalist at ψ = 4. To be called "one knob, nothing fitted" an offset rule needs c
derived. The formula supplies the right order (its mean offset is 1.1 to 1.8) and the right direction across datasets
(smaller on iNaturalist), but not a value better than the constant. For the formula itself to qualify it would need
the measured scales (then it is fitted per dataset, and worse), the slack repaired, a version with less spread
between clusters (for instance ū over more than k rows, or frozen once a query, untested), and a reader timing.

## 7. Scope

- **Filters.** ū uses the whole cluster's μ_c and σ_c and the pooled δ, ρ, which under 25% sets are well fed by
  direct reads, so u is in the visible rows' units; φ̂ counts visible rows. Lemma 5.31 licenses the whole-cluster
  height inside the level's width, where visibility is independent of the error given the score; it says nothing
  about H1 to H3 under a filter, and the formula adds nothing to the lemma's failures (filters on the code or on
  left-out directions). Not tested, as DIAL.md says; under 25% sets most reads are direct and the formula would act
  on few clusters.
- **Before k are held.** Not covered, and it is not small: the first cluster passes about 2,100 to 2,300 rows past the
  certified levels on yahoo and LAION, which is 80% of all rows past the levels on yahoo at the formula's dial at
  ψ = 5. A rule for it (a sample once the cluster's first k rows are scored) is a lever the dial does not reach.
- **Clusters read whole** (direct, at most 16 rows, under 128 rows): untouched, correctly.
- **The randomised levels.** ψ_r = Φ̄⁻¹(½exp(−√2(ψ_ℓ + s̄))) is untested and leaves out the union over the four
  randomised levels; given that over half of yahoo's extra time is there, it deserves its own derivation, not a
  footnote.
- **"Can exceed ψ only in the first sampled clusters, and by under one unit".** Not shown: the floor gives
  θ·ln(n_c/64), 2.4 at 2,000 rows and 3.6 at 10,000, so in a large cluster with small ū the formula exceeds ψ by more.
  The certified cap bounds the harm.

## 8. Units and sanity

- **Units.** Under a common rescaling of every score every term is invariant: no error found. Under a mis-scale of
  σ_c alone (a sketch error) ū moves by the factor unless ρ absorbs it, and unfiltered ρ is on its prior on yahoo,
  so the formula is not invariant to that; neither is the built width (ζ and f). It tracks the stop's own mis-scale,
  which is arguably what is wanted.
- **Monotone in ψ.** At a fixed state, slope 1; along a query the feedback through φ̂ (wider dial, more rows past,
  larger φ̂) and ū damps it to 0.7 to 0.85 a unit, still positive. No error found in sign.
- **φ̂ → 0.** The log term diverges; the floor stops it at θ·ln(n_c/64), which grows with cluster size (item 7).
- **β's limits.** f → 0: β ~ √f/2 → 0, s̄ → 0, consistent. f → 1: β → ∞ and the mean-slack form sends ψ_ℓ to 0, a 50%
  miss at t̂; the log-mean-exp repair bounds the slack's shift by θ·ln k.

## Not checked

- The timing session's runs themselves: I used their files, read and not re-run; the comparison of as built with
  ψ_ℓ = 3 is between two runs normalised by a common build.
- The filtered dumps; the randomised levels' dial; any rule with less spread between clusters than the formula.
- §2.2's tilt study and the "within 20%" fit of H1 out to the 99.5th percentile.
- Whether `clu`'s σ_c is the window's σ_c or the stand-in for clusters the reader would not price.
