# Ledger of claims

Every claim of chapters 1 to 5, in chapter order. A numbered result keeps its number in the chapter (1.13 is
Proposition 1.13). A claim the chapter states without a number gets the chapter's number and a letter sequence
(1.a, 1.b, …) in the order it appears. The status is the tag the chapter gives; where a row carries more than one tag,
the first is the claim's main tag and the others are named in the claim. Tags outside the scheme of `README.md` are
copied as the chapter writes them and listed after the table. The source column names the original file and section
as the chapter cites them; paths are those of the chapter.

| Id | Claim | Status | Section | Source |
|---|---|---|---|---|
| 1.1 | q·x = q·m + α·m·r + Δ·r for every α, with Δ = q − αm | [proved, unrefereed] | 1.1 | `FINDINGS.md` §4 item 2; `agent2/REPORT.md` "Fixes"; `reports/surge/integer-kernel.txt` §0 |
| 1.a | Bounding Δ·r in place of q·e tightened bounds about threefold on BioCLIP-2 | [measured] | 1.1 | `FINDINGS.md` §4 |
| 1.2 | The key â gives m·r = ‖m‖â − ‖m‖² and ‖r‖² = 1 − 2‖m‖â + ‖m‖² | [proved, unrefereed] | 1.1 | `quant/REPORT.md` "The code as kept" item 3; `entropy/REPORT.md` §4; `reports/surge/level0/REPORT.txt` §1 |
| 1.b | Taking the item scale and residual norm from the key cut per-item bytes 1–8% | [measured] | 1.1 | `quant/REPORT.md` (`run2.py`) |
| 1.3a | With α = q·m/‖m‖², Δ ⟂ m and ‖Δ‖ = √(1 − (q·m̂)²) ≤ 1 (= 3.11 (i)) | [proved, refereed] | 1.1 | `reports/surge/level0/REPORT.txt` §1 Theorem B(i); `reports/surge/referee.txt` §3 |
| 1.3b | α* = ⟨Dq, Dm⟩/‖Dm‖² minimises ‖D(q − αm)‖ | [proved, unrefereed] | 1.1 | `agent2/REPORT.md` "Fixes"; `combined/REPORT.md` item 3 |
| 1.c | α* at every deterministic level cut bytes 9/7/4/2% (item) and 7/6/4/2% (text) | [measured] | 1.1 | `combined/REPORT.md` (`core.py`) |
| 1.d | Sharing tables with α = 0 read 40–46% more bytes | [measured] | 1.1 | `rust/REPORT2.md` via `integer-kernel.txt` §3 |
| 1.e | ‖Δ‖ over true neighbours 0.80–0.81 (GloVe), 0.93 (NYTimes) at k = 10 | [measured] | 1.1 | `reports/surge/level0/REPORT.txt` §1 |
| 1.4 | Cluster cut: β̲_c − ρ_⊥√x₂ ≤ q·x ≤ β̄_c + ρ_⊥√x₂ | [proved, unrefereed] | 1.2 | `SYSTEM.md` §2.2; `DESIGN.md` §3; `agent3b/REPORT.md` §2 C2 |
| 1.5 | The rows the annulus and table bounds leave undecided form one contiguous range of the sort by â (= 4.10) | [proved, unrefereed] | 1.2 | `agent3b/REPORT.md` §2 C1; `combined/REPORT.md` item 2; `DESIGN.md` §3 (proof written by the editor) |
| 1.6 | Bound monotone in ν_x, so a ν-sorted cluster decides a prefix | [proved, unrefereed] | 1.2 | `groups/REPORT.md` §3.3; `DESIGN.md` §9 item 2; `NuCut` in `rust/src/v2/q.rs` |
| 1.7 | Tightest interval from two angles to a representative is [cos(φ_q + φ_x), cos(φ_q − φ_x)], n ≥ 3 (= 2.7) | [proved, unrefereed] | 1.2 | `groups/REPORT.md` §2.1 |
| 1.f | Candidates at a 1% answer: 22% (in-distribution), 95% (OOD) | [measured] | 1.2 | `SYSTEM.md` §2.2 |
| 1.g | Items touched after cut and range, by answer size, same in every basis | [measured] | 1.2 | `dualproto/REPORT.md` §3 |
| 1.h | Undecided rows under range alone, with table, and with an oracle | [measured] | 1.2 | `groups/REPORT.md` §3.1 (`measure.py`) |
| 1.i | Rows touched depend on the anchor-to-cluster ratio | [measured] | 1.2 | `review/REPORT.md` §1.3 (`e1_anchors.py`) |
| 1.j | 16–64 anchors is the regime comparable to 10⁹ rows | [conjecture] | 1.2 | `review/REPORT.md` §1.3 |
| 1.k | Sorting by the norm of 1.6 leaves 61–97% (text), 13–84% (item) | [measured] | 1.2 | `groups/REPORT.md` §3.3 |
| 1.8 | Weighted Cauchy–Schwarz: \|Δ·e\| ≤ ‖DΔ‖·‖D⁻¹e‖ | [proved, unrefereed] | 1.3 | `FINDINGS.md` §4 item 5; `SYSTEM.md` §2.1 |
| 1.9 | Quantised basis: u·r = (Vᵀu)·z̃ + u·ε, \|u·ε\| ≤ e_inv‖u‖‖r‖ | [proved, unrefereed] | 1.3 | `review/REPORT.md` §2 item 4; `SYSTEM.md` §2.1 |
| 1.10 | The certificate as built; floating-point allowances proved nowhere | [sketch: gap named] | 1.3 | `reports/surge/integer-kernel.txt` §0 (A0); `rust/src/v2/q.rs` |
| 1.l | Verify mode: 0 violations in 731M/605M/512M checks; 0 of 10.6M integer decisions not implied | [measured] | 1.3 | `dualproto/REPORT.md` §2; `rust/REPORT_BOUNDS.md` |
| 1.m | Actual \|Δ·e\| 29× (text) and 21× (item) below the bound at 96 B | [measured] | 1.3 | `agent2/REPORT.md` |
| 1.n | Zero-read interval spans ±6.1 sd (text), ±4.5 sd (item) | [measured] | 1.3 | `groups/REPORT.md` §3.5 (`margin.py`) |
| 1.o | Typical errors 5–10% of the bound; worst pairs are near-duplicates | [measured] | 1.3 | `FINDINGS.md` §5 |
| 1.11 | One stored norm fixes the weights: other weights give a bound at least ν‖WΔ‖ | [proved, unrefereed] | 1.3 | `reports/surge/ood/REPORT.txt` item 6 |
| 1.12 | Two stored norms: \|Δ·e\|² ≤ (a + kν²)·Σ Δ_i²/(1 + kW_i⁻²) | [sketch: gap named] | 1.3 | `ood/REPORT.txt` item 6; `quant/REPORT.md` "The code as kept" item 4 |
| 1.p | Two-norm gain 2–10% (stand-ins, item profile), 0–2% (H25), 0.3% (box ∩ ellipsoid) | [measured] | 1.3 | `ood/twonorm.py`; `quant/REPORT.md` |
| 1.q | Storing two norms worse overall once bytes are charged | [measured] | 1.3 | `review/REPORT.md` §2 item 1 (`e4_detprofile.py`) |
| 1.r | Offset orthogonal to the code; its gain "a few per cent" is [conjecture] | [sketch: gap named] | 1.3 | `FINDINGS.md` §4 item 5; `ood/REPORT.txt` item 5 |
| 1.s | A stored exact scalar along a learnt g saves 10–25% of text bytes | [conjecture] | 1.3 | `ood/REPORT.txt` item 2 |
| 1.t | Box ∩ ball, sign and Hölder bounds no better than weighted Cauchy–Schwarz | [conjecture] | 1.3 | `FINDINGS.md` §7 |
| 1.u | Deterministic width ≈ Σa_i, randomised ≈ c√(Σa_i²); gain ≈ √(coded coordinates)/c | [model] | 1.3 | `FINDINGS.md` §5 |
| 1.13 | tr(MΣ_Q)·tr(M⁻¹Σ_R) ≥ (Σ√γ_i)², equality at the geometric mean Σ_Q⁻¹ # Σ_R; published as Bhatia–Jain–Lim 2019, Theorem 2, eq. (16) [cited] | [proved, unrefereed] | 1.4 | `groups/REPORT.md` §2.4 Proposition 3; `program/floors/referee/referee-extensions.md` B1 |
| 1.14 | The zero-bit M-ball is within AM/GM of √γ_i of the best zero-bit certificate (uses 2.3 [proved, refereed], κ_k ≥ k [computed]) | [proved, unrefereed] | 1.4 | `groups/REPORT.md` §2.4; `proofcheck/constants.py` |
| 1.v | Dual basis in exact coordinates cuts coordinates read (text and item) | [measured] | 1.4 | `groups/REPORT.md` §3.3 |
| 1.w | Native dual basis cuts bytes; mixed fit 20–22% more on OOD; 0 wrong | [measured] | 1.4 | `dualproto/REPORT.md` §§2–3; `rust/results/combined/s1/bytes_dual_vs_dualmix.log` |
| 1.x | Dead-list entry overturned: dual pair narrows half-width 17–32%; why the per-group fit fails is [conjecture] | [measured] | 1.4 | `dualproto/REPORT.md` §6 (`tools/clipcheck.py`) |
| 1.y | Zero-bit slack: plain ball barely beats PCA; block bound 0.250 → 0.213 / 0.190 | [measured] | 1.4 | `dualproto/REPORT.md` §6; `groups/REPORT.md` §3.3 (`tables/bases.txt`) |
| 1.z | Grouping in the M⁻¹ metric changes coordinates read by at most 0.7 per item | [measured] | 1.4 | `groups/REPORT.md` §3.4 (`prep_m.py`) |
| 1.15 | A K-way grouping is worth at most log₂K bits per item (= 2.8) | [proved, unrefereed] | 1.5 | `groups/REPORT.md` §2.2 Proposition 1 |
| 1.aa | B_s summary bits over N_g independent items worth B_s/N_g each (cites 2.4) | [proved, refereed] | 1.5 | `proofcheck/CONSOLIDATED_PROOFS.md` Corollary 2; `proofcheck/REFEREE.md` |
| 1.16 | Query-chosen summaries gain at most ν√(2λ_max(Σ_Q) ln A) (= 2.9) | [proved, unrefereed] | 1.5 | `groups/REPORT.md` §2.3 Proposition 2 |
| 1.17 | Cluster extremes ask more precision than per-member decisions | [model] | 1.5 | `groups/REPORT.md` §2.5 |
| 1.ab | Group summaries leave 89–100% (text) undecided; finer groups do not pay; score sd and slack figures | [measured] | 1.5 | `groups/REPORT.md` §§3.1–3.2 |
| 1.ac | Fixed-rate tree code costs 1.5–2.0% more bytes and decodes 10–17× faster (times [model]) | [measured] | 1.6 | `quant/REPORT.md` "Bytes against time"; `review/REPORT.md` §1.1 |
| 1.ad | Encoder on real residuals: signs plus 1-bit stages; clip shares; tails; kurtosis 3.08 | [measured] | 1.6 | `dualproto/REPORT.md` §6 (`VEXACT_CLIPSTATS`) |
| 1.ae | A rounding-direction bug broke the scale bound in 542 cases | [measured] | 1.6 | `quant/REPORT.md` |
| 1.af | At 56 B mean half-width 0.080, 0.062 from uncoded blocks; random term whole from 96 B | [measured] | 1.6 | `rust/REPORT_BOUNDS.md` Part 2 (`study/bounds_study.py`) |
| 1.ag | Halving optimal; tan²φ 0.133 at 2 bits; nesting costs 0.179 vs 0.137 | [computed] | 1.7 | `rust/REPORT_BOUNDS.md`; `study/quant_study.py` |
| 1.ah | Deterministic levels at 1.17–1.37× the conjectured and 1.42–1.86× the proved floor; E8 7% is [conjecture] | [computed] | 1.7 | `quant/REPORT.md` (`floor2.py`) |
| 1.ai | Referee's narrowed comparison: escape code 1.36–1.42×, scalar code 1.5–1.7× and 2.7–9×; corrected ratios | [computed] | 1.7 | `proofcheck/REFEREE.md` "Headline claim" |
| 1.aj | Negentropy J = 80 bits; usable savings 1.9–10.4% (image), under 1% (text); 1.3–1.6× from closing the gap is [conjecture] | [measured] | 1.7 | `entropy/REPORT.md` |
| 1.18 | The accumulators: \|σΘ + B_ℓ − ŝ_ℓ\| ≤ e_ℓ | [proved, refereed] | 1.8 | `reports/surge/integer-kernel.txt` §1.2 |
| 1.19 | Integer IN/OUT decisions imply the real-valued certificate's | [proved, refereed] | 1.8 | `integer-kernel.txt` §1.3 (its Theorem D); `reports/surge/referee.txt` §6 |
| 1.20 | 256d + g ≥ 0 iff d + ⌊g/256⌋ ≥ 0 | [proved, refereed] | 1.8 | `integer-kernel.txt` §1.4 |
| 1.21 | The kernel's saturating additions never saturate | [refuted] | 1.8 | `integer-kernel.txt` range paragraph; `referee.txt` §6 |
| 1.21′ | The built kernel's clamps and saturation order withhold decisions only | [proved, unrefereed] | 1.8 | comment at `rust/src/v2/kern.rs` lines 386–404 |
| 1.22 | Centre tables by linearity: same real table, e_ℓ ≤ 3Uσ | [proved, unrefereed] | 1.8 | `integer-kernel.txt` §3 |
| 1.ak | Full-vector step: float32 rounding bound stated in code only | [sketch: gap named] | 1.8 | `SYSTEM.md` §§1, 2.2; `full_err_qn` in `rust/src/v2/q.rs` |
| 1.al | End to end: 0 wrong at 50k; byte-identical to the f16 scan at 1M | [measured] | 1.8 | `rust/REPORT_BOUNDS.md` (`check.sh`); `SYSTEM.md` §3 |
| 1.am | Prior work read in full: VA-file, IQ-tree, BOND, LEMP, ANSMET, PDX, Panorama, Extended RaBitQ, QUIP, LeanVec, RoarGraph, ScaNN, iDistance, M-tree, list of clusters, Schubert, GNAT, LAESA, Tribase | [cited] | 1.9 | `lit/REPORT.md` |
| 1.an | Prior work from abstracts or summaries: VA+-file, FEXIPRO, L2AP, Fukunaga–Narendra, TRIM, CLIP | [cited, unchecked] | 1.9 | `lit/REPORT.md` |
| 2.0 | The floor Σ√min(γ_i, θ) with B counting code bits only | [refuted] | 2.1 | `FINDINGS.md` §5; `agent1/REPORT.md` §2; `PROOFS.md` Agent 1 |
| 2.1 | With exact real scalars no positive floor holds; with ‖e‖ and m·e the cell has measure zero | [proved, refereed] | 2.1 | `PROOFS.md` Agent 1; `proofcheck/REFEREE.md` verdicts |
| 2.1R | For non-Gaussian r the proved bound falls by exp(−J/k) | [proved, refereed] | 2.1 | `PROOFS.md`; `REFEREE.md`; `CONSOLIDATED_PROOFS.md` §1 remarks |
| 2.2 | Volume floor: E h ≥ E‖g_k‖·ω_k^(−1/k)·exp((H_S − H(c))/k) | [proved, refereed] | 2.2 | `CONSOLIDATED_PROOFS.md` §1 (Theorem 1); `REFEREE.md` |
| 2.3 | Gaussian residuals: E h ≥ κ_k(γ₁⋯γ_k)^(1/2k)·2^(−B/k) | [proved, refereed] | 2.2 | `CONSOLIDATED_PROOFS.md` §1 (Corollary 1) |
| 2.a | κ_k ≥ k for k ≤ 200,000 | [computed] | 2.2 | `proofcheck/constants.py`; `agent1/theory.py` |
| 2.b | 1-D high-rate quantiser attains the bound (1.0001); no tested code fell below it | [computed] | 2.2 | `proofcheck/toyfloor.py`; `proofcheck/misc.py` (a) |
| 2.c | Worst achieved/floor 1.13 on 3,000 non-commuting pairs | [computed] | 2.2 | `REFEREE.md` "Fixes to the floor" |
| 2.4 | Shared information: B replaced by B + B_s/N_g for independent items | [proved, refereed] | 2.2 | `CONSOLIDATED_PROOFS.md` §1 (Corollary 2); `REFEREE.md` |
| 2.5 | Concentration floor: E h ≥ Σ_T√γ_i − (√(2H(c)) + 1/√2)√(Σ_Tγ_i) | [proved, refereed] | 2.3 | `CONSOLIDATED_PROOFS.md` §2 (Theorem 2); `REFEREE.md` |
| 2.d | Per-cell inequality checked on balls in 5, 50, 500 dimensions | [computed] | 2.3 | `proofcheck/toyfloor.py` (4) |
| 2.6 | Combined floor (head with 1/√e and B + 1, plus concentration tail) | [proved, refereed] | 2.4 | `CONSOLIDATED_PROOFS.md` §3 (Theorem 3); `PROOFS.md` |
| 2.e | Best proved / conjectured: 0.70 at 0 B to 1.00 at 384 B (group 12) | [computed] | 2.5 | `agent1/theory.py`, `theory.log` |
| 2.f | Power-law spectra: best proved bound 1.001–1.027 of the conjectured | [computed] | 2.5 | `agent1/theory.log` |
| 2.7 | One representative: interval [cos(φ_q + φ_x), cos(φ_q − φ_x)] (= 1.7) | [proved, unrefereed] | 2.6 | `groups/REPORT.md` §2.1 |
| 2.g | The member factor falls as K^(−0.053) | [measured] | 2.6 | `groups/REPORT.md` §3.2 |
| 2.8 | A grouping is worth at most log₂K bits per item (= 1.15) | [proved, unrefereed] | 2.6 | `groups/REPORT.md` §2.2 |
| 2.h | 16× groups saves 4.5 of 45 coordinates and costs 11.4 in centres; summaries leave 89–100% | [measured] | 2.6 | `groups/REPORT.md` §1, tables 3.1–3.2 |
| 2.9 | Query-chosen summaries (= 1.16) | [proved, unrefereed] | 2.6 | `groups/REPORT.md` §2.3 |
| 2.i | Correction ≈ slack·√(2 ln A/D) for offsets over D directions | [model] | 2.6 | `groups/REPORT.md` §2.3 |
| 2.j | λ_max(Σ_Q) is 18% of the trace for text | [measured] | 2.6 | `groups/scripts/offsets.py` |
| 2.k | 64 anchors of 2,000 lower undecided clusters only 25.8% → 22.1% | [measured] | 2.6 | `agent3b/pair.py` |
| 2.10 | Summaries deciding cluster maxima need 1 − h₂(δ) bits per such item (= 4.3) | [proved, unrefereed] | 2.6 | `agent3b/REPORT.md` §2 |
| 2.l | 91% of items have e_i > 0.05, 66% > 0.1 | [measured] | 2.6 | `agent3b/REPORT.md` §2 |
| 2.11 | A fixed-stream reader must read ℓ* levels; the in-order reader reads exactly ℓ* | [proved, refereed] | 2.7 | `CONSOLIDATED_PROOFS.md` §4 (Theorem 4); `REFEREE.md` "Theorem D" |
| 2.12 | Byte-granular readers: level reader within max C_ℓ/(C_{ℓ−1} + 1) | [proved, refereed] | 2.7 | `CONSOLIDATED_PROOFS.md` §4; `REFEREE.md` |
| 2.m | Exact box ∩ ellipsoid 3/7/14% narrower than weighted Cauchy–Schwarz | [measured] | 2.7 | `agent3a/exp_c.py` E3 |
| 2.n | The same range lowers bytes read by about 0.3% | [measured] | 2.7 | `quant/REPORT.md` |
| 2.13 | Entropy bound: E[bits read] ≥ H(S \| q) − h₂(ε) − εN (= 4.1) | [proved, unrefereed] | 2.8 | `agent3b/REPORT.md` §1 |
| 2.o | Bound is 0.00028–0.0046% of the dataset on the 200k subsets | [computed] | 2.8 | `agent3b/stats.py`, `stats.log`, `stats1024.log` |
| 2.p | Baseline code reads 0.564–5.336% at n = 768 | [measured] | 2.8 | `agent3b/base.py` |
| 2.q | An unbounded index over finitely many queries attains the bound (4.c tags it [model]) | [sketch: gap named] | 2.8 | `agent3b/REPORT.md` §1 |
| 2.r | Synthetic Gaussian residuals: certificate 1.36–7.4× the best proved bound | [measured] | 2.9 | `agent1/theory.py`, `theory.log` |
| 2.s | Agent 3a's model ratios, corrected to count scalars: 1.49–2.11 | [model] | 2.9 | `agent3a/exp_c.py` E4; `REFEREE.md` "Most important error" |
| 2.t | Code track on real residuals: before → after against both floors; E8 7% is [conjecture] | [computed] | 2.9 | `quant/floor2.py`; `quant/logs/floor_*.log` |
| 2.u | Per-item floors on bytes read; baseline 3.9–10× above | [model] | 2.9 | `agent3b/stats.py`; `agent3b/REPORT.md` |
| 2.19 | H(μ) ≤ ln(√(2πe)·C(μ)) for a law on ℝ: Bolley–Gentil–Guillin 2018, Theorem 2.1 at n = 1; proof kept, text read by the librarian only | [cited] | 2.9a | `program/floors/CLAIMS.md` N.1; `program/library/floors-novelty.md` §1; `referee-core.md` stage 1; `stage0-floor-check/RESULTS.md` §1 (a consequence) |
| 2.20 | H(ν) ≤ Σᵢ ln(√(2πe)·Cᵢ(ν)) along the triangular coupling; that it implies the published n-dimensional form is [proved, unrefereed] | [proved, refereed] | 2.9a | `program/floors/CLAIMS.md` N.2; `referee-core.md` stage 1; `stage0-floor-check/RESULTS.md` §2 (the per-cell bound, which rests on it) |
| 2.21 | The water-filling programme: W(H) = Σ√min(γ_i, θ) | [proved, refereed] | 2.9a | `program/floors/CLAIMS.md` N.3; `referee-core.md` stages 1, 3 |
| 2.22 | Convexity and bounds for Ψ, W and V_S (items 1–5); item 6, the sum bound, and the condition γ₁ > γ₂ in item 4 are [proved, unrefereed] | [proved, refereed] | 2.9a | `program/floors/CLAIMS.md` N.4; `referee-core.md` stages 2, 3 |
| 2.23 | κ_k ≥ k for every k; least value of κ_k − k is √e − 1 at k = 1, [computed] for k ≤ 5,000 and a grid to 10⁷ only | [proved, refereed] | 2.9a | `program/floors/CLAIMS.md` N.5; `referee-core.md` stages 2, 3; `referee/core_f2_misc.log` |
| 2.24 | Full floor: E h ≥ W(I(z; c)) ≥ Σ√min(γ_i, θ) for Gaussian r and Δ, any n, non-commuting covariances (= 2.14); the case I = ∞ is [proved, unrefereed] | [proved, refereed] | 2.9a | `program/floors/CLAIMS.md` N.6; `referee-core.md` stage 1; `RESULTS.md` |
| 2.25 | E h ≥ V_S(I(z; c)) for every head set S; k ≥ 1, empty T and I = ∞ are [proved, unrefereed] | [proved, refereed] | 2.9a | `program/floors/CLAIMS.md` N.7; `referee-core.md` stage 3; `RESULTS.md` |
| 2.26 | With the water-filling head, E h ≥ κ_k√θ + Σ_{i>k}√γ_i; ties and k = n₊ in the proof are [proved, unrefereed] | [proved, refereed] | 2.9a | `program/floors/CLAIMS.md` N.7; `referee-core.md` stage 3; `RESULTS.md` §3 (head sets near the water level) |
| 2.x | Tightness of 2.24: computed codes within 1.00055 and 1.0026 of W; Ingber–Courtade–Weissman's scheme tight in probability only [cited] | [computed] | 2.9a | `program/floors/CLAIMS.md` N.6; `referee/core_f2_misc.log`; `RESULTS.md`; `referee-extensions.md` P1 |
| 2.y | The water-filling head is not always the best (3.649 against 4.091 for γ = (4, 1, 1)); best head gains 1–2% over W on the measured spectra | [computed] | 2.9a | `program/floors/CLAIMS.md` N.7; `floors_eval.py`; `check_after_referees.log` line 2 |
| 2.z | A seed shared by encoder and certificate: the floors hold at I(z; c \| ω) | [proved, unrefereed] | 2.9a | `program/floors/CLAIMS.md` N.7 |
| 2.aa | Corollary 2.3 and Theorems 2.5 and 2.6 follow from 2.24–2.25; Theorem 2.2 does not | [proved, unrefereed] | 2.9a, 2.5 | `program/floors/CLAIMS.md` N.7; `referee-core.md` "Summary for the PI"; `check_after_referees.log` line 1 |
| 2.ab | The formula was stated before (Wu–Wang–Flierl, no converse; isotropic converses by Ingber–Courtade–Weissman and Steiner–Dempfle–Ingber–Weissman); 2.24 is the first finite-n, non-commuting, mean-width converse as far as the search goes | [cited] | 2.9a, 2.11 | `program/library/floors-novelty.md` §3; `referee-extensions.md` P1–P2 |
| 2.ac | Numerical checks by the author, the core referee and the falsifier: no code below the floors | [computed] | 2.9a | `check_schemes.py`; `check_chain1d.py`; `referee/core_*.py`; `RESULTS.md` |
| 2.14 | Full floor E h ≥ Σ√min(γ_i, θ); settled by 2.24 (= 2.24) | [proved, refereed] | 2.10 | `CONSOLIDATED_PROOFS.md` §6; `program/floors/CLAIMS.md` N.6 |
| 2.15 | Gaussian residuals are the worst case for the true optimum | [conjecture] | 2.10 | `CONSOLIDATED_PROOFS.md` §6 |
| 2.16 | Theorem 2.2 holds for non-Gaussian offsets with covariance Σ_Q | [conjecture] | 2.10 | `CONSOLIDATED_PROOFS.md` §6 |
| 2.17 | √k unavoidable for a class unknown at build | [conjecture] | 2.10 | `CONSOLIDATED_PROOFS.md` §6 |
| 2.17′ | For a uniformly random W, E‖P_W(x − x̂)‖² = (k/n)‖x − x̂‖² | [proved, refereed] | 2.10 | `REFEREE.md` "T4" remark (Editor's query 15) |
| 2.18 | Pr[\|U\| ≥ s] ≤ (1 − s²)^(d_j/2) for s ≤ 1/√2; grid check [computed] | [conjecture] | 2.10 | `CONSOLIDATED_PROOFS.md` §6; `proofcheck/constants.py` |
| 2.v | Ordentlich–Polyanskiy; Indyk–Wagner | [cited] | 2.11 | `lit/REPORT.md` item 4 |
| 2.w | Shannon lower bound; Alon–Klartag; Okajima–Yoshida; Xiao–Cao; Seidl–Kriegel; Urysohn, Gaussian isoperimetry, concentration, Fano, prefix codes | [cited, unchecked] | 2.11 | `lit/REPORT.md` items 4–5 |
| 3.1 | Error law: E ⟂ y, ‖E‖ = ‖y‖tan φ, direction uniform on y^⊥, for any codebook | [proved, refereed] | 3.2 | `CONSOLIDATED_PROOFS.md` §5 (Lemma 8); `referee.txt` §2 |
| 3.a | The law holds on nytimes and glove (sd 0.9999–1.0000, sphere tails); kurtosis 2.988 at 50k | [measured] | 3.2 | `reports/surge/level0/lawcheck.py`; `reports/bounds3/scale_check.py` |
| 3.b | Exact one-sided tail ½·I_{1−s²}((d − 1)/2, ½); d = n − 1 for one block | [proved, refereed] | 3.3 | `level0/REPORT.txt` §1; `referee.txt` §2 |
| 3.2 | Pr[\|u₁\| ≥ s] ≤ 2(1 − s²)^(m/2) for s ≤ 1/√2 | [proved, refereed] | 3.3 | `CONSOLIDATED_PROOFS.md` §5 (Proposition 10); `REFEREE.md` "Exact tail" |
| 3.c | Exact tail multiplies c by 0.54–0.94; closed bound vs numerical constants | [computed] | 3.3 | `proofcheck/constants.py`; `referee.txt` §2 |
| 3.3 | E exp(λu₁) ≤ exp(λ²/(2m)); sub-Gaussian bound over blocks | [proved, refereed] | 3.3 | `CONSOLIDATED_PROOFS.md` §5 (Proposition 11; Editor's query 1) |
| 3.4 | Sub-bit sketch: tan²φ/(πn_j/(2k′) − 1) → 1 | [proved, refereed] | 3.3 | `CONSOLIDATED_PROOFS.md` §5 (Proposition 13); `REFEREE.md` |
| 3.d | E tan²φ infinite for k′ ≤ 2; excess 78/11/2%; sketch matched 2M synthetic pairs [measured] | [computed] | 3.3 | `proofcheck/misc.py` (b); `agent3a/exp_d.py` |
| 3.5 | Class-uniform bound over a subspace W fixed independently of ω | [proved, refereed] | 3.3 | `CONSOLIDATED_PROOFS.md` §5 (Theorem 9); `REFEREE.md` "T4" |
| 3.e | Real offsets full-dimensional; subspace class width 0.88–0.99 of plain CS | [measured] | 3.3 | `agent3a/REPORT.md` |
| 3.6 | Adaptive queries at d = 0: Pr[some wrong] ≤ n_q·ε | [proved, refereed] | 3.4 | `CONSOLIDATED_PROOFS.md` §5 (Theorem 5); `REFEREE.md` "T1" |
| 3.f | d > 0 is not adaptively safe | [proved, unrefereed] | 3.4 | `agent3a/REPORT.md`; `REFEREE.md` |
| 3.7 | Leakage: 2^(B_z)·n_q·ε | [proved, refereed] | 3.4 | `CONSOLIDATED_PROOFS.md` §5 (Theorem 6); `REFEREE.md` "T2" |
| 3.g | The leakage bound is close to tight (3.0–15.2 sd predicted) | [measured] | 3.4 | `agent3a/exp_b.py` |
| 3.8 | Query-time randomness does not help: error ≥ ½ on some corpus | [proved, refereed] | 3.4 | `CONSOLIDATED_PROOFS.md` §5 (Theorem 7); `REFEREE.md` "T3" |
| 3.h | Public seed: no guarantee below the deterministic certificate against a party that reads it | [proved, refereed] | 3.4 | `CONSOLIDATED_PROOFS.md` §5; `REFEREE.md`; `referee.txt` §1 |
| 3.9 | Union over a seed-free set 𝒰 of undecided pairs | [proved, refereed] | 3.4 | `CONSOLIDATED_PROOFS.md` §5 (Proposition 12); `REFEREE.md` "Other checks" |
| 3.10 | Exact k-NN fails with probability at most k·L·δ, for the stated algorithm class | [proved, refereed] | 3.5 | `level0/REPORT.txt` §1 Theorem A; `referee.txt` §1 |
| 3.i | Monte Carlo confirms 3.10; the width without e_m is unsound (810 wrong) | [measured] | 3.5 | `reports/surge/referee_work/thmA_mc.py`; `referee.txt` §2 |
| 3.j | The union over k is tight | [proved by example; measured] | 3.5 | `reports/surge/bounds2.txt` §2 (`simcap.py`) |
| 3.k | Drop events at different levels are disjoint; any fixed split of ε is sound | [proved, unrefereed] | 3.5 | `bounds2.txt` §2 |
| 3.l | Gaps between neighbours give a constant | [refuted as a constant] | 3.5 | `bounds2.txt` §2 |
| 3.m | Items' errors are independent under one rotation | [refuted] | 3.5 | `bounds2.txt` §2 (`pairdep.py`) |
| 3.n | Theorem 3.11's upper end needs only the upper event (valid only with the cap at b; Editor's query 12) | [proved, unrefereed] | 3.5 | `bounds2.txt` §2 |
| 3.o | One-sided tail plus uneven split worth about 1% of bytes on glove | [model] | 3.5 | level-0 counts, `bounds2.txt` §2 |
| 3.p | Constants for k-NN | [computed] | 3.5 | `level0/consts.py`; `referee.txt` §2; `bounds2/consts2.py` |
| 3.11 | Centre-corrected, self-normalised interval (i), (ii) as repaired, (iii) | [proved, refereed] | 3.5 | `level0/REPORT.txt` §1 Theorem B; `referee.txt` §3 |
| 3.q | Backstop q·x* < t̂ + (√d − c)‖Δ‖sd: holds for the plain width; [refuted] for the self-normalised width | [proved, refereed] | 3.5 | `referee.txt` §3 |
| 3.r | Theorem 3.11 cuts glove survivors 1.2–2.2× at the same constant | [measured] | 3.5 | `level0/lvl0c.py` |
| 3.12 | Theorem B for several blocks (R8) | [proved, refereed] | 3.5 | `reports/bounds3/REPORT.txt` §2 R8; `bounds3/referee/REFEREE.txt` item 10 |
| 3.s | ξ² is 0.092/0.079/0.012/0.0008 at the four levels; saves 0.6% of bytes | [measured] | 3.5 | `bounds3/REPORT.txt` |
| 3.13 | T1: Pr[R ≠ S] ≤ L(\|S∩U₀\|δ(c_ex) + \|U₀∖S\|δ(c_in)) (Editor's query 4) | [proved, refereed] | 3.6 | `bounds2.txt` T1; `bounds3/referee/REFEREE.txt` item 8; `referee.txt` §4 |
| 3.t | Verifying admitted items costs 0.89–1.17 reads per match | [measured] | 3.6 | `thr768.py` |
| 3.14 | T2: self-bounding exclusion at c_x(𝒩) | [proved, unrefereed] | 3.6 | `bounds2.txt` T2 |
| 3.15 | T3: cap grid, ≤ 2.1ε₀ (increasing caps) or 3.1ε₀ | [proved, unrefereed] | 3.6 | `bounds2.txt` T3 |
| 3.u | Toy: the factor 2.1 cannot be dropped | [measured] | 3.6 | `simcap.py` |
| 3.16 | T4: band rule, ≤ 6ε₀ for the plain width; [sketch: gap named] for its use | [proved, unrefereed] | 3.6 | `bounds2.txt` T4 (Editor's query 5) |
| 3.17 | T5: E[wrong] ≤ K·δ(c)·E[undecided] | [proved, unrefereed] | 3.6 | `bounds2.txt` T5 |
| 3.v | The rule δ(c) = ε/(K·L·D_ℓ) is sound up to a small factor; toy [measured]; floor c ≈ 7.7–7.8 [model] | [conjecture] | 3.6 | `bounds2.txt`; `simband.py` |
| 3.w | Tolerance d: unions unchanged; d worth d/(2sd_ℓ) units of c | [proved, unrefereed] | 3.6 | `bounds2.txt` |
| 3.x | Expected errors ≤ ε has the same constant as Pr[any error] ≤ ε | [proved, unrefereed] | 3.6 | `bounds2.txt` |
| 3.y | Filters: all-items constant makes masked results equal visible-set results | [proved, unrefereed] | 3.6 | `bounds2.txt` |
| 3.z | Constants for thresholds | [computed] | 3.6 | `bounds2/consts3.py` |
| 3.aa | Measured effect on thresholds; exact reads ∝ \|S\|^0.8 is [measured and argued] | [measured] | 3.6 | `thr768.py` |
| 3.ab | Chernoff with exact Λ over shares gives 9.086 (implementation unrefereed) | [computed] | 3.7 | `rust/REPORT_BOUNDS.md` Part 2 |
| 3.18 | R1: T(a; x) non-decreasing in every a_j | [proved, refereed] | 3.7 | `bounds3/REPORT.txt` R1; referee item 5 |
| 3.19 | R2: any h ≥ H_δ(ā) from rounded-up scales is valid | [proved, refereed] | 3.7 | R2; referee item 3 |
| 3.20 | R3: h = c*·sd valid; values by a checked certificate [computed] | [proved, refereed] | 3.7 | R3; referee item 6 |
| 3.ac | Four separate blocks cost almost nothing against one rotation (8.7659 vs 8.7653) | [computed] | 3.7 | `cert2.py`; `bounds3/REPORT.txt` |
| 3.21 | R4: closed form by convex order against one sphere | [proved, refereed] | 3.7 | R4; referee item 7 |
| 3.ad | "Four blocks are no worse than one sphere in d_Σ dimensions" | [refuted] | 3.7 | `bounds3/REPORT.txt` |
| 3.22 | R5: per-block constants; concavity of F is [conjecture] | [proved, refereed] | 3.7 | R5; referee item 9; `plane_cert.py` |
| 3.ae | Pointwise rule from the fourth cumulant alone | [refuted] | 3.7 | `bounds3/REPORT.txt` |
| 3.23 | R6: (i) one-sided thresholds and (iii) changed k-NN counts sound; (ii) `knn2d` [refuted] | [proved, refereed] | 3.7 | R6; referee items 1, 8 |
| 3.24 | R7: overlap of two levels' failures bounded by Pr[Z ≥ 2] | [proved, refereed] | 3.7 | R7; referee item 10 |
| 3.af | Level correlations 0.22–0.47 [measured]; overlaps e^−31 to 7·10⁻⁷ [computed]; Brownian count [sketch: gap named]; conditioning [refuted as a saving] | [measured] | 3.7 | `level_corr.py`; `overlap01.py` |
| 3.ag | Constants by split weight and by L | [computed] | 3.7 | `bounds3/REPORT.txt` |
| 3.R9 | No per-item rule beats the item's own quantile with N items | [sketch: gap named] | 3.7 | `bounds3/REPORT.txt` R9 (unchecked by the referee) |
| 3.R10 | Block-wise splitting, Bernstein and Jensen routes | [refuted] | 3.7 | `bounds3/REPORT.txt` R10 |
| 3.ah | Constants table | [computed] | 3.7 | `bounds3/consts.py`; `cert2.py` |
| 3.ai | Replay on the 50k dump: rows A to R; 10.8% of exact reads per unit of c | [measured] | 3.8 | `bounds3/final_sim.py` |
| 3.aj | The stored scale was 2.45% too large; refit of the tan φ table | [measured] | 3.8 | `scale_check.py`; `rust/REPORT_CONST.md` step 4 |
| 3.ak | Native index steps 1–4; 0 wrong; largest error 5.47 sd | [measured] | 3.8 | `rust/results/const/`; `rust/REPORT_CONST.md` |
| 3.al | N from the index's size is sound under the referee's condition | [proved, refereed] | 3.9 | `bounds3/referee/REFEREE.txt` item 2 |
| 3.am | PQR-Search | [cited] | 3.10 | `reports/surge2/literature.txt` |
| 3.an | ADSampling; RaBitQ | [cited, unchecked] | 3.10 | `reports/surge2/literature.txt` |
| 4.a | Baseline share read at 1M by threshold and d | [measured] | 4.1 | `FINDINGS.md` §1 |
| 4.1 | Entropy bound: N·h₂(p) − h₂(ε) − εN bits (= 2.13) | [proved, unrefereed] | 4.1 | `agent3b/REPORT.md` §1 |
| 4.b | The baseline is 1,200–1,900× above the bound (baseline unnamed; Editor's query 16) | [measured] | 4.1 | `agent3b/REPORT.md` §1 |
| 4.c | Theorem 4.1 is attained by an unbounded index (2.q tags it [sketch: gap named]) | [model] | 4.1 | `agent3b/REPORT.md` §1 |
| 4.d | Per-query fixed reads are as large as what the support table saves | [measured] | 4.1 | `agent3b/REPORT.md` headline |
| 4.e | Pătraşcu–Thorup Ω(n/log(Sn/N)) probes; Rubinstein; Afshani–Cheng; ALRW hashing; Pestov; Chávez et al.; Weber–Schek–Blott; cover trees | [cited] | 4.2 | `sublinear/REPORT.md` §3, refs. 1, 7, 10, 12–15, 20 |
| 4.f | Earlier cell-probe bounds; Williams; ARW; Chen; Alman–Williams; the hashing model; navigating nets; Matoušek; Chan | [cited, unchecked] | 4.2 | `sublinear/REPORT.md` §3, refs. 5, 6, 8, 9, 11, 16–19 |
| 4.2 | A sublinear threshold index with polynomial build solves OV subquadratically | [proved, unrefereed] | 4.2 | `sublinear/REPORT.md` §3.3 Proposition 1 |
| 4.g | Intrinsic dimension 279 (text), 92 (item); C_exp 4,050 and N/k; guarantees vacuous | [measured] | 4.2 | `sublinear/REPORT.md` §3.4 (`tail.py`) |
| 4.h | Item exponents 0.49–0.64, text 0.91–0.93 | [measured] | 4.2 | `sublinear/REPORT.md` §3.4 |
| 4.3 | Summaries cannot be small (given a measured property) (= 2.10) | [proved, unrefereed] | 4.2 | `agent3b/REPORT.md` §2 |
| 4.i | 91% of rows have e_i > 0.05 (= 2.l) | [measured] | 4.2 | `agent3b/REPORT.md` §2 |
| 4.4 | Shell identity for staged reads | [proved, unrefereed] | 4.3 | `sublinear/REPORT.md` §4 Proposition 2 |
| 4.5 | Exponent of a shell in terms of hazard rates | [proved, unrefereed] | 4.3 | `sublinear/REPORT.md` §4 Proposition 3 |
| 4.6 | Three tails: 0, w/(w + s* − t_N), w/z_N to leading order | [proved, unrefereed] | 4.3 | `sublinear/REPORT.md` §4 |
| 4.7 | Every fixed-slack level is asymptotically linear at fixed k | [proved, unrefereed] | 4.3 | `sublinear/REPORT.md` §4 Proposition 4 |
| 4.j | On- and off-manifold limits (C₁^(n_int); 1 − ν/n_int) | [model] | 4.3 | `sublinear/REPORT.md` §4 |
| 4.8 | Exact k-NN reads at least the threshold query at τ_k (interval model) | [proved, unrefereed] | 4.3 | `sublinear/REPORT.md` §4 Proposition 5 |
| 4.k | Seed overshoot 5–22% of an oracle's bytes | [measured] | 4.3 | `knn/REPORT.md` via `sublinear/REPORT.md` §4 |
| 4.9 | Shared structure does not rescue linearity | [sketch: gap named] | 4.3 | `sublinear/REPORT.md` §4 "What follows" |
| 4.10 | The undecided rows of a cluster form one range (= 1.5) | [proved, unrefereed] | 4.4 | `agent3b/REPORT.md` §2 C1 (tag assigned by the editor) |
| 4.l | First pass: shares undecided and exponents | [measured] | 4.4 | `sublinear/REPORT.md` §5.2 (`zero.py`) |
| 4.m | Per-row read: coordinates grow as N^0.79–0.87 (text), N^0.49–0.60 (item) | [measured] | 4.4 | `sublinear/REPORT.md` §5.3 (`ladder.py`) |
| 4.n | Score tail: shells by width and p | [measured] | 4.4 | `sublinear/REPORT.md` §5.4 (`tail.py`) |
| 4.o | Intrinsic dimension 15.7–21.6 (item), 52–89 (text) | [measured] | 4.4 | `sublinear/REPORT.md` §5.4 |
| 4.p | Only cached cluster extremes change the exponent much (0.12–0.49) | [measured] | 4.4 | `sublinear/REPORT.md` §1, §5.2, §9 |
| 4.q | Other levers change exponents by hundredths; batching and caches constant only [model] | [measured] | 4.4 | `sublinear/REPORT.md` §§5.2, 5.3, 5.6, 6 |
| 4.r | 200k subsets: range and table cuts; byte concentration near t; perfect summary | [measured] | 4.4 | `agent3b/REPORT.md` headline, §§1–2 |
| 4.s | Extrapolation to 10⁹ | [conjecture] | 4.4 | `sublinear/REPORT.md` §5.7 (`extrap.py`) |
| 4.t | Timed cost at 1M; fixed cost 6–7 ms | [measured] | 4.4 | `SYSTEM.md` §4, §4.2 |
| 4.u | Visible sets: work sublinear in \|V\| in the same weak sense | [measured] | 4.5 | `sublinear/REPORT.md` §5.5 |
| 4.v | Zero-read slack about √n_eff within-cluster sd (Editor's query 17) | [model] | 4.6 | `sublinear/REPORT.md` §7 |
| 4.w | Fewer dimensions leave less room in the tail | [measured on a proxy] | 4.6 | `sublinear/REPORT.md` §7 (`dims.py`) |
| 4.x | At n = 1,024 range and table read 0.159–0.419% | [measured] | 4.6 | `agent3b/REPORT.md` headline |
| 4.y | Approximate reads at 1M: 23–64 MB a query | [measured] | 4.7 | `layered.md` §1 (replay B) |
| 4.z | Approximate law rows ∝ n̄^u·N^v; share depends on K; visible set as subsample | [measured] | 4.7 | `layered.md` §2 (`layered/scaling.py`) |
| 4.aa | MB per query at 100M and 1B | [model] | 4.7 | `layered.md` §8.1 (`layered/extrapolate.py`) |
| 4.ab | Clusters of fixed size worth about 0.04 of the text exponent | [measured, share inferred] | 4.8 | `sublinear/REPORT.md` |
| 4.ac | Cluster size against time at 1M; linear-in-N time model [model] | [measured] | 4.8 | `rust/REPORT_CLUSTERSIZE.md` |
| 4.ad | Rows touched rise with cluster size at 200k | [measured] | 4.8 | `agent3b/REPORT.md` §2 |
| 4.ae | 2,000-row leaves: 4–9× fewer rows at 1M [measured]; gain at 100M and 1B | [model] | 4.8 | `layered.md` §0 |
| 4.af | N^v is the best exponent of a centre-keyed hierarchy on these data (Editor's query 14) | [measured] | Open | `layered.md` §0, §2 |
| 5.1 | E[Rec(V) \| I] = (1/k)Σ_V g_c for static policies | [proved, unrefereed] | 5.2 | `theory.md` §2.1 |
| 5.2 | Static recall is a 0/1 knapsack; density prefix bounds; Dantzig [cited, unchecked] | [proved, unrefereed] | 5.2 | `theory.md` §2.2 |
| 5.3 | Smith's rule minimises the area under the recall curve | [cited, unchecked] | 5.2 | `theory.md` §2.3 |
| 5.4 | A global threshold on g_c/w_c is optimal for mean recall at mean cost | [proved, unrefereed] | 5.2 | `theory.md` §2.4 |
| 5.a | Quake, LAET, DARTH are instances; min-sum set cover guarantees | [cited, unchecked] | 5.2 | `theory.md` §§2.4–2.5 |
| 5.5 | Weitzman's reservation index gives a cluster order μ_c + ψ_cσ_c | [cited, unchecked] | 5.2 | `theory.md` §2.5 |
| 5.6 | Exact Bayes index p_c = E[F̄_c(τ_k)], Σ n_c p_c = k (Editor's query 3) | [proved, unrefereed] | 5.3 | `theory.md` §3.1 |
| 5.b | Plug-in and smoothed approximations of (5.1) | [proved, unrefereed] | 5.3 | `theory.md` §3.1 |
| 5.7 | Under M3 the order is by ζ_c for every shape | [proved, unrefereed] | 5.3 | `theory.md` §3.2 |
| 5.8 | Cantelli count bound; Cantelli [cited, unchecked] | [proved, unrefereed] | 5.3 | `theory.md` §3.3 |
| 5.9 | No model, no guarantee from first and second moments | [proved, unrefereed] | 5.3 | `theory.md` §3.3 |
| 5.10 | Linear rules are tangents of the ζ-fan | [proved, unrefereed] | 5.3 | `theory.md` §3.4 |
| 5.11 | Adaptive beats static (0.85 vs 0.70) on an example | [proved, unrefereed] | 5.4 | `theory.md` §4.1 |
| 5.12 | Ceiling on adaptive policies; the gap's size is [sketch: gap named]; simulation [model] | [proved, unrefereed] | 5.4 | `theory.md` §4.2; `theory/sim.py` |
| 5.13 | Margin identity and e^(−2δ) guarantee; sensitivity consequences | [proved, unrefereed] | 5.5 | `theory.md` §8, §5 |
| 5.c | H's α_H as shrinkage of σ̂_c; simulation [model] | [sketch: gap named] | 5.5 | `theory.md` §5, §11 |
| 5.14 | Invariance under q′ = bq + v | [proved, unrefereed] | 5.6 | `theory.md` §6.1 |
| 5.15 | O2 from O3 under (A1), (A2) | [proved, unrefereed] | 5.6 | `theory.md` §6.2 |
| 5.d | One ξ′ = 0.4 within 0.006 of the best on five datasets | [measured] | 5.6 | `theory.md` §6.2 (Editor's query 12) |
| 5.e | 1/n′ = qᵀΣ_c q/tr Σ_c; average is the participation ratio | [proved, unrefereed] | 5.6 | `theory.md` §7 |
| 5.f | ξ₀ = 0.5 and 0.1 correspond to n′ of 20–40 and ≈ n | [sketch: gap named] | 5.6 | `theory.md` §7 |
| 5.g | Replays: radius order fails on text; invariant orders recover; global stop beats fixed probes | [measured] | 5.7 | `README.md` Findings; `scripts/` |
| 5.h | Ablation, size factor, stress, self-correcting order, pooled estimate, heuristic stop | [measured] | 5.8 | `results/ablation/`, `results/order-rows/`, `region25-order/README.md`, `PLAN.md` |
| 5.i | Bruch, Krishnan and Nardini use the same order | [cited, unchecked] | 5.8 | arXiv 2405.12207, via `HANDOVER.md` |
| 5.j | Per-cluster visible centres: 0.63–1.02 of pooled rows | [measured] | 5.8 | `region25-order/README.md` |
| 5.k | Rule 1 is greedy recall per cost under a normal residual; one ψ within 0–17% | [model] | 5.9 | `hierarchy/layered.md` §4.1 |
| 5.16 | ρ_{g,ℓ}² = Σa_i²/(Σa_i)² | [model] | 5.9 | `hierarchy/layered.md` §4.2 |
| 5.l | Derived ρ within 2–5% (in distribution), 15% above on LAION text; BioCLIP-2 factor 0.4–0.5 | [measured] | 5.9 | `hierarchy/layered/explore.py`; `reports/annideas/REPORT.md` |
| 5.17 | Pass probability Φ((ψ − g√(1 − ϑ))/√ϑ) for a stage (Editor's query 6) | [model] | 5.9 | `hierarchy/layered.md` §4.3 |
| 5.m | The key explains 4–20%; a statistical width on it costs bytes | [measured] | 5.9 | `hierarchy/layered.md` §4.3 |
| 5.n | Statistical widths read 6–10× fewer MB unfiltered; recall and ψ effects | [measured] | 5.9 | `hierarchy/layered.md` §5 |
| 5.18 | Tree law in model T: beam size B_j; OV remark [cited, unchecked] | [proved, unrefereed] | 5.10.1 | `hierarchy/theory.md` §2.3 |
| 5.o | Tree law within 25% at groups, optimistic 1.5–13× below clusters; middle level 3.6–10× fewer centres | [measured] | 5.10.1 | `hierarchy/theory/hier.py` |
| 5.19 | σ_g(q)² = qᵀC_g q + mean_c σ_c(q)² | [proved, unrefereed] | 5.10.2 | `hierarchy/theory.md` (3.1) |
| 5.20 | Expand-or-scan values V_g(λ) give the optimal adaptive tree policy under (S1), (S2) | [proved, unrefereed] | 5.10.2 | `hierarchy/theory.md` §3.2 |
| 5.p | Branching-bandit index [cited, unchecked]; DP agreement on 300 instances | [model] | 5.10.2 | `hierarchy/theory.md` §3.2 |
| 5.21 | Never expand a sparse node | [proved, unrefereed] | 5.10.2 | `hierarchy/theory.md` §3.2 |
| 5.q | Total-variance key cuts parents expanded 15–25% | [measured] | 5.10.2 | `hierarchy/theory.md` |
| 5.r | Best ψ does not depend on the unit's size | [model, measured] | 5.10.3 | `hierarchy/theory.md` §3.4 |
| 5.s | Sub-clusters in rows 2.1–3.8×; in time only with contiguous layout; leaf design [model] | [measured] | 5.10.4 | `hierarchy/theory.md` §4.1; `subclusters.md`; `hierarchy/layered.md` §§8.6–8.7 |
| 5.t | Groups do not prune; savings around setup [model]; κ 0.7–1.1 | [measured] | 5.10.5 | `hierarchy/theory.md` §2.4; `groups-setup.md`; `hierarchy/layered.md` §0, §8.7 |
| 5.22 | Group-opening plan optimal over closed sets | [proved, unrefereed] | 5.10.6 | `hierarchy/policy.md` §2.1 |
| 5.23 | Largest-index order has every plan as a prefix; area optimality [cited, unchecked] | [proved, unrefereed] | 5.10.6 | `hierarchy/policy.md` §2.2 |
| 5.24 | Expanding a cluster with n_c^vis·c_item ≤ o_c never pays | [proved, unrefereed] | 5.10.6 | `hierarchy/policy.md` |
| 5.25 | Log-error δ in values loses at most e^(−2δ) | [proved, unrefereed] | 5.10.6 | `hierarchy/policy.md` |
| 5.26 | Lazy heap takes the eager order's actions | [proved, unrefereed] | 5.10.6 | `hierarchy/policy.md` |
| 5.u | Policy replay refuted the specification in three places | [measured] | 5.10.6 | `hierarchy/policy_replay.md`; `hierarchy/policy.md` §11.1 |
| 5.27 | Shares are free of κ_q at τ_k; first-order error at τ* | [proved, unrefereed] | 5.10.6 | `hierarchy/policy.md` §11.2 |
| 5.28 | The plan on calibrated shares maximises expected recall at its cost; replay [measured], synthetic [model] | [proved, unrefereed] | 5.10.6 | `hierarchy/policy.md`; `hierarchy/theory/policy_check2.py` |
| 5.v | Approximate threshold queries: three simplifications | [proved, unrefereed] | 5.11 | `hierarchy/theory.md` §6.1 |
| 5.w | Threshold orders, sub-clusters, the library's threshold query timings; the share fix checked in simulation only | [measured] | 5.11 | `hierarchy/theory.md` §6.2; `hierarchy/policy.md` §11.3; `PLAN.md` |
| 5.x | Random visible sets thin each cluster with the same table | [model] | 5.12 | `hierarchy/policy.md` §4.1 |
| 5.y | Random visible set as a subsample; correlated sets read 35–48% | [measured] | 5.12 | `hierarchy/layered.md` §0; `hierarchy/theory.md` §7.1 |
| 5.z | Shares cancel common miscalibration (by 5.27); drift measured | [proved, unrefereed] | 5.12 | `hierarchy/policy.md` §11.3 |
| 5.aa | A sub-centre as a 20-byte coded row would not rank sub-centres | [conjecture] | Open | `hierarchy/theory.md` §5.2 |

## Counts by main tag

| Tag | Rows |
|---|---|
| [proved, refereed] | 47 |
| [proved, unrefereed] | 70 |
| [sketch: gap named] | 9 |
| [conjecture] | 10 |
| [refuted] | 6 |
| [measured] | 86 |
| [computed] | 22 |
| [model] | 16 |
| [cited] | 6 |
| [cited, unchecked] | 8 |
| outside the scheme | 5 |
| total | 285 |

Rows are claims as the chapters group them, so a measured row often holds several figures, and a row's secondary tags
(a [conjecture] inside a [measured] row, for example) are not counted. Six results and one measurement appear in two
chapters (1.5 = 4.10, 1.7 = 2.7, 1.15 = 2.8, 1.16 = 2.9, 2.10 = 4.3, 2.13 = 4.1, and 2.l = 4.i), and each appearance
has its own row. Conjecture 2.14 and Theorem 2.24 are one statement within chapter 2, and each has its own row. The
letters 2.x to 2.ac follow 2.w although section 2.9a comes before section 2.10, so that no earlier letter changes.

## Tags outside the scheme

These are copied from the chapters, and the PI should map each to a tag of `README.md`: [proved by example; measured]
(3.j), [refuted as a constant] (3.l), [measured and argued] (inside 3.aa), [refuted as a saving] (inside 3.af),
[measured on a proxy] (4.w), [measured, share inferred] (4.ab), [model, measured] (5.r), [proved, unrefereed;
conditional on the OV conjecture] (4.2) and [proved, unrefereed; given a measured property] (4.3). Two further rows
use a tag outside its definition: 2.r is [measured] on synthetic data, and the grid check of 2.18 is [computed]
although it evaluates no proved inequality.

## Unrefereed results that other chapters or the built index rely on

Each of these rests on one proof that nobody else has checked, or on a sketch, and something later depends on it.

- **1.5 (= 4.10), the contiguous range.** The index's `sorted_range2` relies on it to decide rows from the key, and
  chapter 4's "range" exponents measure it. The full proof is the editor's; the sources give one line.
- **1.6, the ν-sort.** `NuCut` in `rust/src/v2/q.rs` implements it; the stored-basis statement is the editor's
  (chapter 1, Editor's query 13).
- **1.8 and 1.9, weighted Cauchy–Schwarz and the quantised basis.** Every deterministic decision of the index uses
  them, and chapter 5's rule 1 (5.16) takes 1.8's bound as its width. Both are one-line consequences of
  Cauchy–Schwarz.
- **1.10 and 1.ak, the floating-point allowances and the full-vector rounding bound.** The built certificate and the
  full-vector step use bounds that exist only in code comments; verify mode found no violation (1.l).
- **1.21′ and 1.22, the built kernel's saturation repair and the centre tables by linearity.** The integer kernel as
  built depends on both, and on the two departures from the refereed design in chapter 1's Editor's query 14.
- **1.15 (= 2.8) and 4.4–4.7.** Chapter 4's Claim 4.9 combines 2.8 with the refereed 2.4, and 4.7 is the basis of
  "every fixed-slack level is asymptotically linear".
- **2.13 (= 4.1), the entropy bound.** Chapter 4 uses it to set the metric; the source's proof is one sentence.
- **3.n, the one-sided upper end of Theorem 3.11.** The one-sided k-NN constants (3.p) rest on it, and chapter 3's
  Editor's query 12 doubts the argument for Ê > b.
- **5.2, 5.4 and 5.10.** The built order μ_c + 3σ_c(q) is justified in §5.8 by these results, and 5.14 by the stand-in
  key's invariance. Rule 1 as built (§5.9) rests on 5.16, which is a model and not a proof.

The constants the index uses (chapter 3 §3.9) rest on refereed results (3.18–3.21, 3.23 (i), 3.al) and on computed
values. Two computations behind them were not refereed line by line: the Chernoff fallback's implementation (3.ab) and
`plane_cert.py` (3.22).
