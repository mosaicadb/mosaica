# Ledger of claims

Every claim of chapters 1 to 5 and 7, in chapter order. A numbered result keeps its number in the chapter (1.13 is
Proposition 1.13). A claim the chapter states without a number gets the chapter's number and a letter sequence
(1.a, 1.b, …) in the order it appears; letters added after the ledger was first built continue each chapter's sequence
after its last letter, so no earlier id changes, and a row sits at its place in the chapter whatever its letter.
Chapter 7 has rows only for claims that are not already a row of chapters 1 to 5, and names the row it would repeat
where it touches one. The status is the tag the chapter gives; where a row carries more than one tag, the first is the
claim's main tag and the others are named in the claim. A bracket that begins with a tag of `README.md` and goes on only
with its source, its scope (which form, at what size) or who did the work keeps that tag here; one whose words change
or combine the kind of evidence (a simulation called measured, a model evaluation called computed, "model with measured
costs") is copied whole and listed after the table. The source column names the original file and section as the
chapter cites them; paths are those of the chapter.

| Id | Claim | Status | Section | Source |
|---|---|---|---|---|
| 1.1 | q·x = q·m + α·m·r + Δ·r for every α, with Δ = q − αm | [proved, unrefereed] | 1.1 | `FINDINGS.md` §4 item 2; `agent2/REPORT.md` "Fixes"; `reports/surge/integer-kernel.txt` §0 |
| 1.a | Bounding Δ·r in place of q·e tightened bounds about threefold on BioCLIP-2 | [measured] | 1.1 | `FINDINGS.md` §4 |
| 1.2 | The key â gives m·r = ‖m‖â − ‖m‖² and ‖r‖² = 1 − 2‖m‖â + ‖m‖² | [proved, unrefereed] | 1.1 | `quant/REPORT.md` "The code as kept" item 3; `entropy/REPORT.md` §4; `reports/surge/level0/REPORT.txt` §1 |
| 1.b | Taking the item scale and residual norm from the key cut per-item bytes 1–8% | [measured] | 1.1 | `quant/REPORT.md` (`run2.py`) |
| 1.3a | With α = q·m/‖m‖², Δ ⟂ m and ‖Δ‖ = √(1 − (q·m̂)²) ≤ 1 (= 3.11 (i)) | [proved, refereed] | 1.1 | `reports/surge/level0/REPORT.txt` §1 Theorem B(i); `reports/surge/referee.txt` §3; `program/kernel/referee-range.md` |
| 1.3b | For positive diagonal D with Dm ≠ 0, α* = ⟨Dq, Dm⟩/‖Dm‖² minimises ‖D(q − αm)‖ (the range referee's restatement) | [proved, unrefereed] | 1.1 | `agent2/REPORT.md` "Fixes"; `combined/REPORT.md` item 3; `program/kernel/referee-range.md` |
| 1.c | α* at every deterministic level cut bytes 9/7/4/2% (item) and 7/6/4/2% (text) | [measured] | 1.1 | `combined/REPORT.md` (`core.py`) |
| 1.d | Sharing tables with α = 0 read 40–46% more bytes | [measured] | 1.1 | `rust/REPORT2.md` via `integer-kernel.txt` §3 |
| 1.e | ‖Δ‖ over true neighbours 0.80–0.81 (GloVe), 0.93 (NYTimes) at k = 10 | [measured] | 1.1 | `reports/surge/level0/REPORT.txt` §1 |
| 1.4 | Cluster cut for any anchor p̂ and number β with ρ_⊥ ≥ ‖q − βp̂‖: β̲_c − ρ_⊥√x₂ ≤ q·x ≤ β̄_c + ρ_⊥√x₂ (the range referee's restatement) | [proved, unrefereed] | 1.2 | `SYSTEM.md` §2.2; `DESIGN.md` §3; `agent3b/REPORT.md` §2 C2; `program/kernel/referee-range.md` |
| 1.5a | Two bounds from the key: the annulus \|q·x − (q·m̂)â\| ≤ ‖q_m‖√(R² − â²), and table plus remainder | [proved, unrefereed] | 1.2 | `agent3b/REPORT.md` §2 C1; `program/kernel/referee-range.md` |
| 1.5b | Each bound is concave (upper) or convex (lower) in â, so {U ≥ t} and {L < t} are intervals; trigonometric forms | [proved, unrefereed] | 1.2 | `program/kernel/referee-range.md` |
| 1.5 | For the stored key widened by δ_â: out rows score below t, in rows at least t, the undecided rows one contiguous range inside the not-out range, the ends not fixed; (d), exact keys, is the earlier lemma (= 4.10 in its annulus case) | [proved, unrefereed] | 1.2 | `agent3b/REPORT.md` §2 C1; `combined/REPORT.md` item 2; `DESIGN.md` §3; `program/kernel/referee-range.md` |
| 1.ao | Float64 model of the cut and range: closed forms match the bound; undecided set contiguous at true keys; all 1,729 wrong decisions are rows with \|x·m̂\| > 1 + DA; out and in rows occur at either end | [model] | 1.2 | `referee-range.md` F2, F7 (`closed_form_check.py`, `range_attack.py`) |
| 1.ap | For the annulus bound alone, which end is out or in follows the angles ϑ and ϑ_t; the report gives the rule without proof | [sketch: gap named] | 1.2 | `referee-range.md` "Verdicts" |
| 1.6 | Bounds from the stored basis and ν_x ≥ ‖z̃_x‖, for any α or the best of several, are monotone in ν_x, so a ν-sorted cluster decides a prefix and at most one prefix is non-empty | [proved, unrefereed] | 1.2 | `groups/REPORT.md` §3.3; `DESIGN.md` §9 item 2; `NuCut` in `rust/src/v2/q.rs`; `program/kernel/referee-range.md` |
| 1.aq | Model of `NuCut`: 748 wrong decisions in 244,800 checks, all rows with \|x·m̂\| > 1 + DA | [model] | 1.2 | `referee-range.md` (`nucut_attack.py`) |
| 1.ar | The rounding of z̃ that e_inv leaves out is up to about 5·10⁻¹⁰ relative at n = 768 | [model] | 1.2 | `referee-range.md` F8 |
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
| 1.as | The f32 evaluation of m·r from the key is off by up to 2.7·10⁻⁷·‖m‖, covered by h_fix only while \|α\|·‖m‖ ≤ 23 | [model] | 1.3 | `program/kernel/referee-kernel.md` §3e |
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
| 1.18 | The accumulators: \|σΘ + B_ℓ − ŝ_ℓ\| ≤ e_ℓ; holds with the built signed B_ℓ by the same proof | [proved, refereed] | 1.8 | `reports/surge/integer-kernel.txt` §1.2; `program/kernel/referee-kernel.md` §2 |
| 1.at | The code's e_ℓ bounds each unit's table error on the path with own tables (float part 9 units against 48 allowed) | [proved, unrefereed] | 1.8 | `referee-kernel.md` §3d |
| 1.au | Model of the tables: (1) reached at most 0.976·e_ℓ, no entry clamped; e_ℓ 0.51–0.63 steps a unit with own tables | [model] | 1.8 | `referee-kernel.md` (`k4_tables.py`) |
| 1.18a | The f64 thresholds round outward: T_in's argument ≥ K_in/σ and T_out's ≤ K_out/σ, to first order in ū, for any tol and dial | [proved, unrefereed] | 1.8 | `referee-kernel.md` §3b |
| 1.av | Attack on the thresholds: 0 violations in 1,700,000 cases against exact rationals | [model] | 1.8 | `referee-kernel.md` (`k2_thresholds.py`) |
| 1.18b | The f64 margins: σ·Rout ≥ B_ℓ + e°_ℓ + ψ_ℓX; σ·Rin ≥ X + e_ℓ − B_ℓ when B_ℓ < 0; X covers the true weighted error norm | [proved, unrefereed] | 1.8 | `referee-kernel.md` §3c |
| 1.aw | Attack on the margins: 63 quotients short of an integer by ≤ 1.0·10⁻¹⁰ steps, all covered by `up`; Rin and Rout never short | [model] | 1.8 | `referee-kernel.md` (`k3_margins.py`) |
| 1.19 | As built: IN implies the certificate's lower end at ψ = 1 whatever the dial; OUT implies est + ψ_ℓ·hh < t, a certificate for ψ_ℓ ≥ 1 and a statistical exclusion below; the design's form was [proved, refereed] | [proved, unrefereed] | 1.8 | `integer-kernel.txt` §1.3 (its Theorem D); `reports/surge/referee.txt` §6; `referee-kernel.md` §2, §6 |
| 1.20 | 256d + g ≥ 0 iff d + ⌊g/256⌋ ≥ 0 | [proved, refereed] | 1.8 | `integer-kernel.txt` §1.4 |
| 1.21 | The design's saturating additions never saturate; the build takes the referee's second repair (see 1.21′) | [refuted] | 1.8 | `integer-kernel.txt` range paragraph; `referee.txt` §6 |
| 1.21′ | Saturation as built, at most 64 units: the saturating two-word compare equals the exact IN and OUT tests; the sentinel never decides; clamped thresholds only withhold | [proved, unrefereed] | 1.8 | `referee-kernel.md` §3a; comment at `rust/src/v2/kern.rs` lines 398–404 |
| 1.ax | The compare stays exact up to 127 units (the editor's reading of the ranges for U units) | [sketch: gap named] | 1.8 | `referee-kernel.md` §3a verdict |
| 1.ay | Attack on the compare: 0 mismatches at 64 and 127 units; at 128 units the OUT compare is wrong, at 160 both | [model] | 1.8 | `referee-kernel.md` (`k1_compare.py`) |
| 1.22 | Centre tables as built: linear in the weights; the f32 combine's per-part bound makes (1) hold; the design's e_ℓ ≤ 3Uσ does not hold for the built combine | [proved, unrefereed] | 1.8 | `integer-kernel.txt` §3; `referee-kernel.md` §3d |
| 1.az | Combined tables' e_ℓ a unit: median 0.6–0.84 steps, at most 1.8, in ordinary regimes; median 344–367, up to 1,750, when w_q ≈ α·w_m | [model] | 1.8 | `referee-kernel.md` (`k4_tables.py`) |
| 1.ba | Of 72,711 integer decisions placed near the boundary, none not implied by the real certificate | [model] | 1.8 | `referee-kernel.md` §§2–6 |
| 1.ak | Full-vector step: float32 rounding bound stated in code only | [sketch: gap named] | 1.8 | `SYSTEM.md` §§1, 2.2; `full_err_qn` in `rust/src/v2/q.rs` |
| 1.al | End to end: 0 wrong at 50k; byte-identical to the f16 scan at 1M | [measured] | 1.8 | `rust/REPORT_BOUNDS.md` (`check.sh`); `SYSTEM.md` §3 |
| 1.bb | Defect 1, the key clamp: rows with x·m̂ > 1 + DA break the range, `NuCut` and the certificate (1,729 wrong in 872,256 checks; a constructed row decided out at score t); widening by √x₂ − 1 gave 0 wrong | [model] | 1.8a | `referee-range.md` F1, F2, F6, F7; `referee-kernel.md` §3e (b), K3 (`range_attack.py`, `range_corner.py`, `nucut_attack.py`, `k6_key.py`) |
| 1.bc | Defect 2, the square root of a cancelling difference: ρ_⊥ and s1 fall below their exact values, and a matching row is skipped with its cluster | [model] | 1.8a | `referee-range.md` F3 (`range_corner.py` part B, `range_corner_c.py`) |
| 1.bd | Defect 3, no unit-count guard on the default path: a schedule of 128 or more units makes the OUT compare wrong | [model] | 1.8a | `referee-kernel.md` §3a, K1 (`k1_compare.py`) |
| 1.be | Defect 4, a NaN threshold sends T_in to TMIN and takes items IN (from reading the code; nothing run) | [model] | 1.8a | `referee-kernel.md` §3b, K2 |
| 1.bf | Defect 5, the m·r allowance is short for \|α\|·‖m‖ > 23 | [model] | 1.8a | `referee-kernel.md` §3e (a), K4 (`k6_key.py`) |
| 1.bg | Defect 6, the table cache keyed by level only; unreachable today (from reading the code) | [model] | 1.8a | `referee-kernel.md` §4 |
| 1.bh | Defect 7, e_inv does not bound the rounding of z̃; the size is absorbed by other margins unwritten | [model] | 1.8a | `referee-range.md` F8 |
| 1.bi | Defect 8, groups with fewer rows than n: a 617-row group needs 200 B at w* = 0.06 against a mean of 36 B; costs bytes, not soundness | [measured] | 1.8a | `program/partition/experiment/RESULTS.md` items 1, 2 (`diag_groups.py`) |
| 1.am | Prior work read in full: VA-file, IQ-tree, BOND, LEMP, ANSMET, PDX, Panorama, Extended RaBitQ, QUIP, LeanVec, RoarGraph, ScaNN's anisotropic loss, iDistance, M-tree, list of clusters, Schubert, GNAT, LAESA, Tribase | [cited] | 1.9 | `lit/REPORT.md` |
| 1.an | Prior work from abstracts or summaries: VA+-file, FEXIPRO, L2AP, ScaNN, SOAR, SPANN, SPFresh, Fukunaga–Narendra, TRIM, CLIP | [cited, unchecked] | 1.9 | `lit/REPORT.md` |
| 1.bj | No row lies beyond 1 + DA on eight built indexes; largest \|x·m̂\| 0.996, largest norm 1 + 4.0·10⁻⁴; the key-clamp repair is preventive | [measured] | 1.8a | `program/LOG-2026-11-21.md`, visit-order session |
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
| 2.23 | κ_k ≥ k for every k ≥ 1 | [proved, refereed] | 2.9a | `program/floors/CLAIMS.md` N.5; `referee-core.md` stage 2 |
| 2.ad | κ_k − k ≥ 0.6487, least value √e − 1 at k = 1, over every k ≤ 5,000 and a logarithmic grid to 10⁷ only; no proof covers every k | [computed] | 2.9a | `referee-core.md` stage 3; `referee/core_f2_misc.log` (Editor's query 18) |
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
| 3.25 | A rounded copy R̃_j of the rotation on the query side, the build encoding against R_j, costs the allowance ‖y_j‖·sec φ_j·(ρ_R,j + η_R,j)·‖Δ_j‖, same constant and δ | [proved, refereed] | 3.2a | `program/rotations/CLAIMS.md` E1; `rotations/referee/referee-1.md` "E1, E2" |
| 3.ao | The worst query's change of the estimate reached 0.52–0.58 of the allowance, never above it | [model] | 3.2a | `referee-1.md` (`ref_e.py`) |
| 3.ap | Encoding against R̃ breaks the exact law at a coarse grid (mean +0.046, 90 standard errors) | [model] | 3.2a | `referee-1.md` (`ref_e_build.py`) |
| 3.aq | A guarantee at f32 encoding against R̃ costs a factor E[2^Q] on δ (E[Q] ≈ 0.35 at depth 8, [model]) plus slacks not worked out | [sketch: gap named] | 3.2a | `referee-1.md` "Is 'the build must not encode against R̃' necessary?" |
| 3.ar | The codes from R·y and R̃·y differed for no item with f32 and for 1–62% with int16 | [model] | 3.2a | `referee-1.md` |
| 3.as | The referee's ten repairs to the use of 3.25 (the factor 1 + dk_j in the slot, the product against x_j, the kernel's f32 bound, the centre tables, and others) | [proved, unrefereed] | 3.2a | `referee-1.md` items 1–10 |
| 3.at | The slot form exceeds 3.25's form by √n_j/‖ō‖, 2.0 to 4.2; int16 through the slot costs 0.05–0.1 / 0.4–0.7 / 6–13% at depths 1 / 4 / 8 | [computed] | 3.2a | `ref_e.txt` |
| 3.au | E2: the allowance in f32 is at most 0.0024% of c·sd; int16 0.025–3.9%; int8 unusable from depth 4 | [computed] | 3.2a | `rotations/e_rounding.py`, `out_e.txt`; reproduced in part by the referee |
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
| 3.26 | The law holds for every codebook exactly when every orthonormal pair goes to a uniform point of V₂(ℝⁿ); a density ratio M costs a factor M (M^J over J blocks) | [proved, refereed] | 3.4 | `program/rotations/CLAIMS.md` A1; `referee-1.md` "A1" |
| 3.27 | A rotation with finitely many values is a function of z for almost every y, and some codebook then puts the error at its Cauchy–Schwarz size | [proved, refereed] | 3.4 | `rotations/CLAIMS.md` A2; `referee-1.md` "A2" |
| 3.av | 3.27 applies to the index's own seeded dense matrix, so it cannot be the reason to reject sign–Hadamard products | [proved, unrefereed] | 3.4 | `referee-1.md` "A2" |
| 3.28 | A rotation from B_s fair bits: δ < 2^(−B_s) forces no failure, and a half-width h(A, ‖Δ_⊥‖) that never fails is at least Cauchy–Schwarz | [proved, refereed] | 3.4 | `rotations/CLAIMS.md` C1; `referee-1.md` "C1" |
| 3.aw | With a 64-bit seed the per-event budget at N = 10⁹ is 4.6 atoms, and falls below one atom at N = 2³³ | [computed] | 3.4 | `rotations/CLAIMS.md` C2; arithmetic checked by the referee |
| 3.ax | The guarantee is ε at the answer level, 1.8·10¹⁰ atoms at every N, so a longer seed makes nothing provable; the advice to use one is withdrawn | [proved, unrefereed] | 3.4 | `referee-1.md` "C2" (a model of the generator as independent Haar draws) |
| 3.10 | Exact k-NN fails with probability at most k·L·δ, for the stated algorithm class | [proved, refereed] | 3.5 | `level0/REPORT.txt` §1 Theorem A; `referee.txt` §1 |
| 3.i | Monte Carlo confirms 3.10; the width without e_m is unsound (810 wrong) | [measured] | 3.5 | `reports/surge/referee_work/thmA_mc.py`; `referee.txt` §2 |
| 3.j | The union over k is tight | [model] | 3.5 | `reports/surge/bounds2.txt` §2 (`simcap.py`) |
| 3.k | Drop events at different levels are disjoint; any fixed split of ε is sound | [proved, unrefereed] | 3.5 | `bounds2.txt` §2 |
| 3.l | Gaps between neighbours give a constant | [refuted] | 3.5 | `bounds2.txt` §2 |
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
| 3.aa | Measured effect on thresholds; exact reads ∝ \|S\|^0.8, measured and argued, is [model] | [measured] | 3.6 | `thr768.py` |
| 3.29 | Given the seed, the failure events of i.i.d. items are independent with one probability p(ω); N·p̄ − (N²/2)·π₂ ≤ Pr[some item fails] ≤ N·p̄ | [proved, unrefereed] | 3.6a | `program/confidence/CLAIMS.md` C2.1; `confidence/referee/referee-lower-bounds.md` §6 |
| 3.ay | The latitude-corpus model agreed with simulation in 16 one-block cells and a two-block case (0.6020 against 0.6014) | [model] | 3.6a | `referee-lower-bounds.md` (`ref_sim_c.py`) |
| 3.30 | R9 in quantile form: for every per-item rule, Pr_ω[p(ω) ≥ x/N] ≤ ε/(1 − e^(−x)) | [proved, unrefereed] | 3.6a | `referee-lower-bounds.md` §6 (statement); `CLAIMS.md` C2.1 (proof) |
| 3.az | Mean form: p̄ ≤ ε/N + (N/2)·π₂, so the union is attained up to 1 − γ/2 when π₂ ≤ γ·p̄/N (the track's Corollary C1) | [proved, unrefereed] | 3.6a | `CLAIMS.md` C2.1 |
| 3.31 | Random shared codebook, equator corpus: π₂ ≤ 2e^(−λ) + λ²δ² + M·δ·I*(θ), with step (c) or the referee's (c′) | [proved, unrefereed] | 3.6a | `CLAIMS.md` C2.2; `referee-lower-bounds.md` §6 |
| 3.ba | One codeword's event was 0.83–0.96 of δ·I(θ) in four settings, never above | [model] | 3.6a | `c2_revision.py` |
| 3.32 | Any code: Pr[F_1 ∩ F_2 \| z_1, z_2] ≤ T_{d−1}(ζ√(2/(1 + χ))) | [proved, unrefereed] | 3.6a | `CLAIMS.md` C2.2; `referee-lower-bounds.md` §6 |
| 3.bb | The bound of 3.32 is below δ·10⁻¹¹ for χ ≤ χ₀ = 0.52 / 0.45 / 0.34 / 0.33 at d = 95 / 127 / 255 / 287 | [computed] | 3.6a | `c2_union.log` part 3; reproduced by the referee |
| 3.33 | One block, one level, the plain rule and a random shared codebook: some corpus fails with probability ≥ Nδ(1 − 0.01 − Nλ²δ/2), so δ ≤ (ε/N)(1 + 0.011) | [proved, unrefereed] | 3.6a | `CLAIMS.md` C2.3; `referee-lower-bounds.md` §6 |
| 3.bc | Codebook sizes sufficient at N = 10⁹: 50–51 bits by step (c), 30–41 bits by (c′); at d = 95 and 29 bits the union is attained within 2% | [computed] | 3.6a | `c2_union.log` part 2; `c2_revision.log` part 1; `ref_num2b.py` |
| 3.bd | The union needs M ≫ N·ln²(N/δ), and fails with fewer codewords than items | [refuted] | 3.6a | `referee-lower-bounds.md` §6 |
| 3.be | With one codeword the union over N is not attained (π₂ of order δ) (the track's Claim E) | [proved, unrefereed] | 3.6a | `CLAIMS.md` C2; `referee-lower-bounds.md` §6 |
| 3.bf | The excess Pr[F_2 \| F_1] − p̄ of a small random code depends on d and δ, not on M/N alone | [model] | 3.6a | `ref_sim_c.py` part D |
| 3.bg | 3.33 does not cover the factor N·L over levels, other rules, several blocks or the built code (G3); k-NN not at all | [sketch: gap named] | 3.6a | `CLAIMS.md` C2; `referee-lower-bounds.md` §6 |
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
| 3.af | Level correlations 0.22–0.47 [measured]; overlaps e^−31 to 7·10⁻⁷ [computed]; Brownian count [sketch: gap named]; conditioning as a saving [refuted] | [measured] | 3.7 | `level_corr.py`; `overlap01.py` |
| 3.ag | Constants by split weight and by L | [computed] | 3.7 | `bounds3/REPORT.txt` |
| 3.R9 | No per-item rule beats the item's own quantile with N items (R9 as worded; first tagged a sketch); replaced by Corollary 3.30, with Theorem 3.33 where the union is attained | [refuted] | 3.7, 3.6a | `bounds3/REPORT.txt` R9; `confidence/referee/referee-lower-bounds.md` §6 |
| 3.R10 | Block-wise splitting, Bernstein and Jensen routes | [refuted] | 3.7 | `bounds3/REPORT.txt` R10 |
| 3.ah | Constants table | [computed] | 3.7 | `bounds3/consts.py`; `cert2.py` |
| 3.34 | Cap measure: C_d·sin^dθ/d ≤ Ω_d(θ) ≤ C_d·sin^dθ/(d·cos θ) | [proved, refereed] | 3.7a | `program/confidence/CLAIMS.md` C1.1 (F1); `referee-lower-bounds.md` §8 |
| 3.35 | Haar rotation and nearest codeword: Ê − Δ·y = tan φ·‖Δ_⊥‖·U; ζ·tan φ·‖Δ‖ is valid (A1) | [proved, refereed] | 3.7a | `CLAIMS.md` C1.1; `referee-lower-bounds.md` §8 |
| 3.36 | cos φ stored rounded down on a geometric grid: h = [η_g + (1 + η_g)ζt̄]‖Δ‖ is valid (A2) | [proved, refereed] | 3.7a | `CLAIMS.md` C1.1; `referee-lower-bounds.md` §8 |
| 3.37 | M random codewords: Pr[φ > θ] ≤ exp(−M·Ω_d(θ)); tan θ₁ = tan θ_ν(1 + O(ln d/d)) (A3) | [proved, refereed] | 3.7a | `CLAIMS.md` C1.1; `referee-lower-bounds.md` §8 |
| 3.bh | A fixed covering code does the same without the d⁻² exception (Böröczky–Wintsche) | [cited, unchecked] | 3.7a | the librarian's summary |
| 3.bi | No codebook has sin φ below 2^(−ν)(1 − a/d) on more than about e^(−a)/√(2πd) of the sphere: θ_ν is the least angle | [proved, unrefereed] | 3.7a | `CLAIMS.md` C1.1 "The limit on tan φ"; `program/codes/REPORT.md` |
| 3.38 | Achievability: B = νd + O(log d) bits give E_ω h ≤ ζ_d(δ/2)·tan θ_ν·(1 + O(ln d/d)) + d⁻² (Theorem A) | [proved, refereed] | 3.7a | `CLAIMS.md` C1.1; `referee-lower-bounds.md` §8 |
| 3.39 | The algebra: max over v is √(e_∥² + ζ²e_⊥²); min over κ is W_ζ(φ)² (B0) | [proved, refereed] | 3.7a | `CLAIMS.md` C1.3; `referee-lower-bounds.md` §4 |
| 3.40 | Validity for one y and a family of queries: E_ω T_d((h₁ + v·e_∥)/(√(1 − v²)·e_⊥)) ≤ δ (B1) | [proved, refereed] | 3.7a | `CLAIMS.md` C1.3; `referee-lower-bounds.md` §4 |
| 3.41 | A grid of latitudes: Pr_ω[h₁ < cos ψ_m·√(e_∥² + ζ₀²e_⊥²)] ≤ m·δ/T_d(ζ₀) (B2) | [proved, refereed] | 3.7a | `CLAIMS.md` C1.3; `referee-lower-bounds.md` §4 |
| 3.bj | Cutting the half-width to 0.95 of the quantile multiplies the failure probability by 334 (d = 95) and 92 (d = 255) | [computed] | 3.7a | `c1_converse.py` part 2 |
| 3.42 | Cap bound: σ{y : φ ≤ θ} ≤ M·Ω_d(θ) for every ω, with φ := π for ŷ = 0 (B3) | [proved, unrefereed] | 3.7a | `CLAIMS.md` C1.3 |
| 3.43 | The converse in 𝓛, every d and δ: E h₁ ≥ cos ψ_m·W_{ζ₀}(θ)·(1 − mδ/T_d(ζ₀) − M·Ω_d(θ)) (Theorem B) | [proved, refereed] | 3.7a | `CLAIMS.md` C1.3; `referee-lower-bounds.md` §4 |
| 3.44 | First order: E h₁ ≥ ζ/√(4^ν − 1 + ζ²)·(1 − o(1)) when ln(1/δ)/ln d → ∞ (B4) | [proved, unrefereed] | 3.7a | `CLAIMS.md` C1.3; `referee-lower-bounds.md` §4 |
| 3.45 | Theorem 3.43 integrated over the quantile: E h₁ ≥ cos ψ_m·∫₀¹ W_{ζ(u)}(θ(u)) du (B6) | [proved, refereed] | 3.7a | `CLAIMS.md` C1.3; `referee-lower-bounds.md` §4 |
| 3.46 | R8 with its half-width frozen at Ê = 0, h₁ = W_ζ(φ), is one-sided δ-valid and meets 3.43 (A4) | [proved, unrefereed] | 3.7a | `CLAIMS.md` C1.3; `referee-lower-bounds.md` §2 |
| 3.bk | Over seven codes and 63 cells the one-sided rate of 3.46 never exceeded δ | [model] | 3.7a | `ref_sim_a4.py` |
| 3.bl | 3.46's upper end lies above the plain end in 13–22% of draws at v = 0, 50–98% at v = −0.3, over 99% at −0.9 | [model] | 3.7a | `referee-lower-bounds.md` §2 |
| 3.47 | 3.46 inside the bit budget: ĉ on a geometric grid and h₁ = W_ζ(φ̂) + η_g (A4′) | [proved, unrefereed] | 3.7a | `CLAIMS.md` C1.3; `referee-lower-bounds.md` §1(a) |
| 3.48 | One event serves the plain end, 3.46's end and capped R8 in one block (A5) | [proved, unrefereed] | 3.7a | `CLAIMS.md` C1.3; `referee-lower-bounds.md` §2 (Editor's query 20) |
| 3.bm | The smallest of the three ends was exceeded at rates 0.0097–0.0102 against δ = 0.01 | [model] | 3.7a | `c1_revision.log` part 4 |
| 3.bn | Without the cap, R8's upper end used one-sidedly fails up to 2δ for aligned queries; with it at most δ | [model] | 3.7a | `referee-lower-bounds.md` §2; reproduced by the theorist |
| 3.49 | One side against two in 𝓛: a two-sided scheme satisfies 3.43; one side costs at most one halving of δ (B5) | [proved, unrefereed] | 3.7a | `CLAIMS.md` C1.3; `referee-lower-bounds.md` §4 |
| 3.bo | The two tails of 3.46 sum to 1.0000·T_d(ζ) for ξ² ≥ 0.09, 1.035 at 0.018, 1.91 (referee 1.94) at 0.001 | [computed] | 3.7a | `c1_revision.log` part 3; `ref_num1.py` (Editor's query 18) |
| 3.bp | The optimum in 𝓛 for one block: w* = ζ/√(4^ν − 1 + ζ²) to first order | [proved, unrefereed] | 3.7a | combines 3.43, 3.44, 3.47, 3.37 |
| 3.bq | Under the averaged requirement the shrunk estimate's error is exactly sin φ·U′, so such a converse proves at most ζ·E sin φ (the track's N2) | [proved, unrefereed] | 3.7a | `CLAIMS.md` N2; `referee-lower-bounds.md` §3 |
| 3.br | The sin rule fails fixed queries: 1.8% at v = 0.3 to 78% at 0.99 for δ = 0.01 | [model] | 3.7a | `c1_revision.py` part 4; the referee's run |
| 3.bs | TurboQuant's 4^(−b) against 1/(4^b − 1) | [cited, unchecked] | 3.7a | the librarian's summary |
| 3.bt | At the index's block sizes the proved lower bound in 𝓛 is 4–6% below the proved upper bound and 1–5% below w*; plain exceeds w* by √(1 + ξ²) | [computed] | 3.7a | `c1_converse.py` part 3; `c1_revision.py` part 1; `ref_num1.py` |
| 3.bu | Simulation at d = 12, 512 codewords: one-sided rates of plain, sin, 3.46, R8 with and without cap, and their minimum, by latitude | [model] | 3.7a | `c1_revision.py` part 4 |
| 3.50 | The confidence tax: R_cert − R_sd = ½·log₂[(w² + ζ²(1 − w²))/(w² + 1/d)], tending to log₂ c for w ≪ 1/√d (given 3.47 and 3.43) | [proved, unrefereed] | 3.7a | `CLAIMS.md` C1.3; `referee-lower-bounds.md` §5 |
| 3.bv | Tax figures: limits 2.88 / 3.05 / 3.12 bits at d = 95 / 255 / 767 | [computed] | 3.7a | `c1_revision.log` part 2; reproduced in part by the referee |
| 3.bw | G1: no lower bound outside 𝓛; a trimmed Urysohn inequality is missing | [sketch: gap named] | 3.7a | `CLAIMS.md` C1; `referee-lower-bounds.md` §7 |
| 3.bx | The least mean half-width in the full model is R8's, ζ·tan θ_ν·√(1 + ξ² − v²)/(1 + ξ²) | [conjecture] | 3.7a | `CLAIMS.md` C1 |
| 3.by | G2: the first-order constant for δ fixed as d → ∞ | [sketch: gap named] | 3.7a | `CLAIMS.md` C1 |
| 3.bz | G3: no bound on Pr[χ > χ₀ \| F_1] at 10⁻¹¹ for the built scalar code | [sketch: gap named] | 3.7a | `CLAIMS.md` C2; `referee-lower-bounds.md` §6 |
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
| 4.2 | A sublinear threshold index with polynomial build solves OV subquadratically (a lower bound conditional on the OV conjecture) | [proved, unrefereed] | 4.2 | `sublinear/REPORT.md` §3.3 Proposition 1 |
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
| 4.10 | The undecided rows of a cluster form one range, exact key, annulus bound (= 1.5 (d) in its annulus case; 1.5 is the current statement) | [proved, unrefereed] | 4.4 | `agent3b/REPORT.md` §2 C1 (tag assigned by the editor) |
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
| 4.w | Fewer dimensions leave less room in the tail | [model] | 4.6 | `sublinear/REPORT.md` §7 (`dims.py`) |
| 4.x | At n = 1,024 range and table read 0.159–0.419% | [measured] | 4.6 | `agent3b/REPORT.md` headline |
| 4.y | Approximate reads at 1M: 23–64 MB a query | [measured] | 4.7 | `layered.md` §1 (replay B) |
| 4.z | Approximate law rows ∝ n̄^u·N^v; share depends on K; visible set as subsample | [measured] | 4.7 | `layered.md` §2 (`layered/scaling.py`) |
| 4.aa | MB per query at 100M and 1B | [model] | 4.7 | `layered.md` §8.1 (`layered/extrapolate.py`) |
| 4.ab | Clusters of fixed size worth about 0.04 of the text exponent | [model] | 4.8 | `sublinear/REPORT.md` |
| 4.ac | Cluster size against time at 1M; linear-in-N time model [model] | [measured] | 4.8 | `rust/REPORT_CLUSTERSIZE.md` |
| 4.ad | Rows touched rise with cluster size at 200k | [measured] | 4.8 | `agent3b/REPORT.md` §2 |
| 4.ae | 2,000-row leaves: 4–9× fewer rows at 1M [measured]; gain at 100M and 1B | [model] | 4.8 | `layered.md` §0 |
| 4.ag | An item entering the first randomised level reads 29.5 B: 24 B of code and 5.5 B of scalars | [measured] | 4.9 | `rust/results/system/bd_p2c.out`; `SYSTEM.md` §2.1, §5; DESIGN-CLAIMS C5.1; the referee §4.1 |
| 4.ah | The cap bound on tan²φ at rate R is t(R) = 1/(4^R − 1) | [proved, unrefereed] | 4.9.1 | `codes/REPORT.md` |
| 4.ai | Built curves: one-rate code ratio ρ_c 1.305 / 1.41 / 1.69 at 1 / 2 / 7 bits, stored scale 1.0245, deterministic factor 1.19 | [measured] | 4.9.1 | `codes/out_built.txt`; `reports/bounds3/scale_check.log`; `quant/REPORT.md`, via DESIGN-CLAIMS §0 |
| 4.aj | The layout programme's deterministic factor rising from 1 to 1.19, and 16 bits of scalars per coded block (the source's estimate) | [model] | 4.9.1 | DESIGN-CLAIMS C3b.2 |
| 4.ak | A rotated block beats its own zero-bit bound from B₀ = (n_T/2)·log₂(1 + c²/d_T) bits, 35.7 at c = 8 and n_T = 96; a sign per position needs 2.18 times more | [model] | 4.9.2 | DESIGN-CLAIMS C3a.1; the referee §2 |
| 4.al | A flat block beats the same bits spent deterministically iff d_T > c² and B > B₁ (77.6 bits at c = 8, n_T = 96); at high rate iff d_T(GM/AM)² > c² | [model] | 4.9.2 | DESIGN-CLAIMS C3a.2; the referee §2 |
| 4.am | With the nested code the first randomised level is 3% on the losing side of that margin; a one-rate code would win by 11% | [model] | 4.9.2 | the referee §2, E1 (correcting DESIGN-CLAIMS C3a.2) |
| 4.an | Over the frontier the hand-over is at most 16 B with ideal curves and between 32 and 40 B with built ones; group 6 gives the same | [model] | 4.9.2 | DESIGN-CLAIMS C3b.3; `c3b_g12_c8_*.log`; `c3_tables.log`; `c3b_g6_c8_*.log`; the referee §3 |
| 4.ao | The hand-over moves as c²: at c = 4 at most 4 B (ideal) and 8 B (built) | [model] | 4.9.2 | DESIGN-CLAIMS C3b.4 item 2 |
| 4.ap | The best layout found is at least 1.4 / 2.1 / 2.7 times narrower than W at 32 / 96 / 256 B (ideal), 1.2 / 1.5 / 1.9 times than 1.19·W at 56 / 96 / 256 B (built) | [model] | 4.9.2 | DESIGN-CLAIMS C3b.3, C3b.4 |
| 4.aq | In time, no coded block at the first level is between 3% wider and 11% narrower than one; crossover 1.7 ns, so one against none is undecided | [model] | 4.9.2 | DESIGN-CLAIMS C5.3 item 3; the referee §4.2 |
| 4.ar | Under Theorem 3.11 at Ê = 0, B₀ disappears and a flat block with d_T > c² wins from its first bit (interval [proved, refereed] in chapter 3) | [model] | 4.9.2 | the referee §4.4 (restating DESIGN-CLAIMS C5.4) |
| 4.as | The rate–margin law: an item's bytes depend on ũ_i alone, a query's bytes are a shell count in units of each item's scale, and each halving costs n_⋆(ũ) bits | [model] | 4.9.3 | DESIGN-CLAIMS C4.1 items 1–3; the referee §5 |
| 4.at | At an optimum n_⋆ = Σ_j n_j cos²φ_j, holding along the computed frontier within 4% from 24 to 256 B | [model] | 4.9.3 | DESIGN-CLAIMS C3b.1 item 4, C3b.3 item 6; the referee §3 |
| 4.au | Query type enters through ũ and the class's own w̄_ℓ; answer size only through ũ; the visible set untested | [measured] | 4.9.3 | RESULTS §2; the referee §5 (replacing DESIGN-CLAIMS C4.1 item 4) |
| 4.av | On the frontier an item at a tenth of its zero-bit bound needs at most about 56 B, at a hundredth about 215 B; not a power law; the deterministic floor alone needs about 96 B at a tenth | [model] | 4.9.3 | DESIGN-CLAIMS C4.1; the referee §3, §5 |
| 4.aw | Dump test: each level's normalised width is a property of the level; survival steps at 0.990–1.005 of w̄_ℓ; the law predicts each query's bytes (1.000–1.001 pooled, rms 0.049–0.051); T5 0.92–1.23 | [measured] | 4.9.3 | RESULTS §1, §2 (`c4_dump_test.py`, `c4_dump_test_replay.py`, `t2_criteria.py`, `t5.py`) |
| 4.ax | The estimate's error costs nothing net: 238.0 B against the index's 237.9 at item 50% | [measured] | 4.9.3 | RESULTS §3.1 |
| 4.ay | The model's inputs are wrong in two terms at item 50%: Gaussian margins ×1.68 and the scale η ×1.28; η_i falls with the answer size, refuting C4.2's suggested cause in direction | [measured] | 4.9.3 | RESULTS §3.2 (`step3.py`, `chain_table.py`); the referee §5 |
| 4.az | Carried to 1M, 131.7 × 1.68 × 1.24 = 274 B against 276.2 B measured (arithmetic on two factors) | [model] | 4.9.3 | RESULTS §5 (Editor's query 27) |
| 4.ba | The best package that keeps the decode is worth about 3% of bytes (an estimate in the source) | [model] | 4.9.3 | `codes/REPORT.md`; the referee §6 item 7 |
| 4.bb | Stored bytes per item by level, 6.5 to 276.3; one block-rise costs 1–1.5 B and each block 1 B of ‖y‖ once | [measured] | 4.9.4 | `bd_p2c.out`, via DESIGN-CLAIMS C5.1; the referee §4.1 |
| 4.bc | The scalars are about 1% of what an item reaching 256 B stores and 4–5% of what a query reads at item 50% | [model] | 4.9.4 | DESIGN-CLAIMS C5.1 |
| 4.bd | Profiled phases at 1M: the randomised scalars' decode is 4.5–12.4% of the wall | [measured] | 4.9.4 | `SYSTEM.md` §4.2 = `bd_p2c.out` |
| 4.be | About 1.4–2.8 ns per (item, coded block, rise) for the decode alone, uncertain by a factor 2 and biased high; 3–10 ns all-in, uncertain by 3.3 and confounded | [model] | 4.9.4 | DESIGN-CLAIMS C5.1; the referee §4.1 |
| 4.bf | The dump test counts 5.60 rises per item of N at item 50% (5.73 at c = 8) | [measured] | 4.9.4 | RESULTS §4 |
| 4.bg | τ_byte ≈ 0.133 ns, τ_lvl ≈ 3.2 ns, a full vector ≈ 104 ns; by 256 B an item pays 11 block-rises, 15–110 ns, against about 34 ns of code decode | [model] | 4.9.4 | DESIGN-CLAIMS C5.1, C5.5; the referee §4.1, §7 |
| 4.bh | A level that codes a second block pays 15% on its random term (constant 6.931 against 7.963) | [computed] | 4.9.4 | DESIGN-CLAIMS C5.1 |
| 4.bi | More coded blocks at 56 B save time only below 0.3–1.1 ns a block-rise (three or four) or 0.7–3.3 ns (two at free rates); bytes save 0.2–2.9 B | [model] | 4.9.5 | DESIGN-CLAIMS C5.2 (`c5a.py`) |
| 4.bj | The model predicts a 6–26% fall in entrants to 96 B where the native trial measured 2% | [model] | 4.9.5 | DESIGN-CLAIMS C5.2 item 4; the referee §4.1 |
| 4.bk | The likely cause is the model's block norms; the index's block 0 is about 1.5 times heavier | [model] | 4.9.5 | DESIGN-CLAIMS C5.2 item 4, C3 (c) item 4 |
| 4.bl | Charging scalars per coded block: in time at most one coded block at the first level; in bytes a near-tie, three blocks winning by 3.6–5.9% at 1–1.5 B | [model] | 4.9.5 | DESIGN-CLAIMS C5.3 (`c5b.py`); `referee/rd_c5b_s.py`; the referee §4.2 |
| 4.bm | Charged 2.5 B a coded block, the later levels still code three, four and two blocks | [model] | 4.9.5 | DESIGN-CLAIMS C5.3 item 4 |
| 4.bn | Under Theorem 3.12 three blocks are 24% narrower but save time only below 0.5–1.5 ns; two blocks at about 1 and ¾ bit could pay | [model] | 4.9.5 | the referee §4.4 (`rd_c5c_r8.py`) |
| 4.bo | The candidate, one wider block at the same 24 B: −19 to −20% (−14% with dump-like norms), −22 to −26% under 3.11 at Ê = 0 | [model] | 4.9.6 | DESIGN-CLAIMS C5.3 item 2, C5.7 V1 (`c5e.py`); the referee §4.4 |
| 4.bp | Any change to the 56-byte level can save at most 31.5 B per item of N, 13% of the randomised bytes, at item 50% | [measured] | 4.9.6 | RESULTS §3.3, §4 (`step3.py`, `step3_levels.py`) |
| 4.bq | The candidate's predicted time gain is about 1 ns per item, 1.3% of the query, against ±5% timing noise | [model] | 4.9.6 | DESIGN-CLAIMS C5.7 |
| 4.br | The native trial of three coded blocks at 56 B cut entrants to 96 B by 2% and was 8–24% slower at 1M | [measured] | 4.9.6 | `rust/REPORT_BOUNDS.md` Part 2, via DESIGN-CLAIMS C5.1 |
| 4.bs | Nested table-decodable codes keep 6–8%, the one-rate gains do not survive nesting, and no block at 56 B can use a one-rate sub-bit code | [measured] | 4.9.6 | `codes/REPORT.md`; the referee §6 item 1 |
| 4.bt | With the scalars charged: the ideal optimum codes one block at the first level; scalars are 5–25% of bytes but 8–52% of time; each level entered costs about 3.2 ns | [model] | 4.9.6 | DESIGN-CLAIMS C5.6; the referee §4.1, §7 |
| 4.bu | Going from 14 levels to 4 was faster at every cell for 3–11% more bytes | [measured] | 4.9.6 | `rust/REPORT_RAND.md`, as read in DESIGN-CLAIMS C5.5 |
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
| 5.ab | With the tail known, the stop on expected misses bounds mean recall; in the model at r = 0.9, 7% (normal) and 23% (Student-t) of queries fall below r [model] | [proved, unrefereed] | 5.8a | `program/approximate/CLAIMS.md` A2.1; `referee/ref1_a2.py` |
| 5.ac | A stop fed by the pooled sample and valid for every shape needs about a tenth as many pooled rows as rows unread (one-shot; sequential form a sketch) | [proved, unrefereed] | 5.8a | `CLAIMS.md` A2.2 |
| 5.ad | The within-cluster tail is near exponential (rate 1.97–2.77); an estimated tail is off 2–4× at the decision; no stop on expected misses read fewer rows than the built stop | [measured] | 5.8a | `CLAIMS.md` A2.3–A2.6 (`replay_np.py`) |
| 5.ae | Idealised replay: a fixed cluster count took 0.68–1.65 of the stop's rows at equal mean recall | [measured] | 5.8a | `CLAIMS.md` A2.6 items 1, 4; tables T7, T8 |
| 5.af | On the library's reader the pooled stop wins on LAION, yahoo, landmark and BioCLIP and under filters; the fixed count wins unfiltered on arxiv, iNaturalist, glove and SIFT | [measured] | 5.8a | `program/LOG-2026-11-21.md`; logs in `vexact-data/sc-study/out-fixed2/` |
| 5.k | Rule 1 is greedy recall per cost under a normal residual; one ψ within 0–17% | [model] | 5.9 | `hierarchy/layered.md` §4.1 |
| 5.16 | ρ_{g,ℓ}² = Σa_i²/(Σa_i)² | [model] | 5.9 | `hierarchy/layered.md` §4.2 |
| 5.l | Derived ρ within 2–5% (in distribution), 15% above on LAION text; BioCLIP-2 factor 0.4–0.5 | [measured] | 5.9 | `hierarchy/layered/explore.py`; `reports/annideas/REPORT.md` |
| 5.17 | Pass probability Φ((ψ − g√(1 − ϑ))/√ϑ) for a stage (Editor's query 6) | [model] | 5.9 | `hierarchy/layered.md` §4.3 |
| 5.m | The key explains 4–20%; a statistical width on it costs bytes | [measured] | 5.9 | `hierarchy/layered.md` §4.3 |
| 5.n | Statistical widths read 6–10× fewer MB unfiltered; recall and ψ effects | [measured] | 5.9 | `hierarchy/layered.md` §5 |
| 5.ag | The derived width is 1.29–2.04 times too narrow on iNaturalist and right within 6% on yahoo and LAION | [measured] | 5.9a.1 | `rho_check.py`; `program/LOG-2026-11-21.md` |
| 5.ah | The mean of the ratio and the ratio of the means agree within 3% in A3's model | [model] | 5.9a.1 | `referee/ref2_a41.txt` |
| 5.29 | (Δ·e_vec)/h = Δ̂·ê, a cosine; the derived ρ² replaces the cluster's error second moment by the profile's diagonal (A4.1) | [proved, refereed] | 5.9a.1 | `CLAIMS.md` A4.1; `referee-2.md` §1 |
| 5.ai | When a cluster's own error directions carry the query's weight, actual over derived rises from 1.00 to 2.1–2.7 at 32 clusters a group | [model] | 5.9a.1 | `CLAIMS.md` A4.2; `out/a4_model.txt` |
| 5.aj | A row with s ≥ t is kept by a width Y iff e ≤ Y + (s − t) | [proved, refereed] | 5.9a.2 | `CLAIMS.md` A4.3 item 1 |
| 5.ak | ψσ̂ keeps 0.790–0.824 of the true ten at ψ = 4; a neighbour's e/σ̂ follows its height (slope 0.92–1.24); 0.24–0.35 lie beyond 4σ̂ | [measured] | 5.9a.2 | `pair_check.py` |
| 5.30 | Under a linear normal regression of e on s, E[e \| s] = ρ_es·z·σ̂ and the plain rule keeps a threshold row with probability Φ((ψ − ρ_es·ζ)/√(1 − ρ_es²)); slope at most 1 (Editor's query 26) | [proved, refereed] | 5.9a.2 | `CLAIMS.md` A4.3; `referee-2.md` §2 |
| 5.al | The uncorrelated normal account of the loss (Proposition 5.17's model) on these data | [refuted] | 5.9a.2 | `referee-2.md` §2 item 3 |
| 5.am | An uncentred slope through the origin is ρ_es + ē/(σ̂·z) | [proved, unrefereed] | 5.9a.2 | `referee-2.md` §2 |
| 5.an | The rule (5.8): keep a row when â ≥ t̂ − (m·max(ζ_c, 0) + ψ0·√ϑ)·σ̂, with m = 1; fitted, not derived | [measured] | 5.9a.3 | `program/LOG-2026-11-21.md`, last section |
| 5.ao | Under the law e = m·z·σ̂ + √ϑ·σ̂·g, the width keeps a threshold neighbour iff g ≤ ψ0 − (m − 1)·ζ/√ϑ | [proved, refereed] | 5.9a.3 | `CLAIMS.md` A4.5 item 1; `referee-2.md` §4 |
| 5.ap | Over a whole cluster that law would give Var(e) ≈ (m² + ϑ)·σ̂² | [model] | 5.9a.3 | `referee-2.md` §4 item 1 |
| 5.aq | The shared-component model (A4.4) gives a median e/σ̂ of 0.67–0.80 where 0.89–1.21 was measured, and a negative intercept; a gap by the referee's verdict | [model] | 5.9a.3 | `CLAIMS.md` A4.4; `referee-2.md` §3; `ref2_a44.txt` |
| 5.ar | Taking the height from the whole cluster cut iNaturalist's miss at ψ0 = 2 from 0.082 to 0.049 | [measured] | 5.9a.3 | `width_checks.py --whole`; `program/LOG-2026-11-21.md` |
| 5.as | With every quantity from the sample the rule keeps 0.918–0.976 at ψ0 = 2; under in-cluster quarter cuts it reads up to 2.5 times the plain rule's rows | [measured] | 5.9a.3 | `psi_eff.py`, `kappa_check.py` |
| 5.at | With the height from the whole cluster (shift mean(e)) it keeps 0.93–0.99 at ψ0 = 2 across 18 cases, within 0–5% of the best rows but +17% on LAION at 8 groups | [measured] | 5.9a.3 | `width_checks.py --whole` (Editor's queries 17, 25) |
| 5.au | The plain rule at the same dial keeps 0.22–0.95 | [measured] | 5.9a.3 | `program/LOG-2026-11-21.md` "answers to referee 2" |
| 5.av | The sample form reads about 0–4% fewer rows than a re-dialled plain rule on three indexes, 0–15% on LAION | [computed] | 5.9a.3 | `referee-2.md` §4 |
| 5.31 | The law of e given (s, Vis) equals that given s iff P(Vis \| s, e) does not depend on e | [proved, unrefereed] | 5.9a.4 | `referee-2.md` §6 |
| 5.aw | The whole-cluster height is invariant for filters on score, random sets and whole-cluster sets; synthetic laws agree to two decimals [model] | [proved, unrefereed] | 5.9a.4 | `referee-2.md` §6; `ref2_filter.txt` |
| 5.ax | It fails for filters correlated with the error at a given score; a correlated quarter cut moved E[e \| s] by ±0.73σ̂ | [model] | 5.9a.4 | `referee-2.md` §6; `ref2_filter.txt` |
| 5.ay | Adding mean(e) to the midpoint counts the height twice (A4.9 item 1) | [refuted] | 5.9a.4 | `referee-2.md` §6 item 3 |
| 5.az | The shift to add is the sample's mean of e − m·σ̂·(s − μ_c)/σ_c | [model] | 5.9a.4 | `referee-2.md` §6 |
| 5.ba | Under a quarter cut on score σ̂ from the visible sample is 13–21% too narrow | [model] | 5.9a.4 | `referee-2.md` §6 item 2 |
| 5.bb | Below the mean a width with a negative height at the bulk's slope should read fewer rows at the same kept share | [model] | 5.9a.4 | `referee-2.md` §6 item 4 |
| 5.bc | "A visible set needs no separate treatment" (A4.7) and "take every quantity from the visible sample" (A4.9 item 1), for filters that cut a cluster on score | [refuted] | 5.9a.4 | `referee-2.md` §5, §7 |
| 5.bd | The sample form's recorded misses at ψ0 = 2 / 3: 0.024–0.082 / 0.008–0.028 | [measured] | 5.9a.5 | visit-order session record |
| 5.be | ψ0 means about 1 − ½·exp(−√2·ψ0), from a Laplace scatter's chance at the threshold | [refuted] | 5.9a.5 | `referee-2.md` §4 item 2 |
| 5.bf | With the slack counted a Laplace scatter misses 0.012–0.049 at ψ0 = 2 and 0.003–0.012 at 3 | [model] | 5.9a.5 | `ref2_a45.txt` |
| 5.bg | Sampled statistics add a factor 1.2–1.3, and the recorded misses are 1.3–3.8 times a Laplace scatter's: the tail is heavier | [computed] | 5.9a.5 | `out/a4_width_noise.txt`; `referee-2.md` §4 item 2 |
| 5.bh | The scatter's 99.9th percentile is 4.6–7.5 and it widens with height | [measured] | 5.9a.5 | `program/LOG-2026-11-21.md` "The level width under filters" |
| 5.bi | A slope that varies between pairs widens the pooled scatter; the coefficient of ζ for one meaning of ψ0 is an upper quantile of the slopes | [model] | 5.9a.5 | `referee-2.md` §4 item 2 |
| 5.bj | A recall of 0.999 a level needs ψ0 near 4.4 (A4.8) | [refuted] | 5.9a.5 | `referee-2.md` §4 item 2, §7 |
| 5.bk | The sample form's error variance ν² (the referee's repaired formula, not re-read); kept 0.954 at ζ = 3, 32 rows | [model] | 5.9a.6 | `referee-2.md` §5; `ref2_a45.txt` |
| 5.bl | The track's formula for ν² as written | [refuted] | 5.9a.6 | `referee-2.md` §5 |
| 5.bm | 64 sampled rows are enough and 32 marginal for far clusters | [computed] | 5.9a.6 | `CLAIMS.md` A4.7 |
| 5.32 | At a fully coded randomised level, E_ω[â_R] = s, E_ω[(â_R − â)²] = e² + v_R, E_ω[(â_R − â)·â_R] = e·s + v_R (A4.7) | [proved, refereed] | 5.9a.6 | `CLAIMS.md` A4.7; `referee-2.md` §5; `ref2_a47.txt` |
| 5.bn | The sample must be larger by 1 + 2v_R/σ̂² + v_R²/σ̂⁴ for normal errors: 1.2 or 1.6 times | [proved, refereed] | 5.9a.6 | `CLAIMS.md` A4.7 |
| 5.bo | An uncoded block, or an upper bound used for v_R, biases σ̂² and σ_c² low, the unsafe side | [model] | 5.9a.6 | `referee-2.md` §5 |
| 5.bp | A sample read at the first fully coded randomised level estimates σ̂, μ_c, σ_c within 2–6%, at about 10 kB against 256 kB | [measured] | 5.9a.6 | `rand_sample.py`; `program/LOG-2026-11-21.md` |
| 5.bq | The slope far up is √(PR(left)/PR(var)) (A4.4 item 2), except when every coordinate is wholly coded or left out | [refuted] | 5.9a.7 | `referee-2.md` §3 |
| 5.br | The repaired slope far up, (Σdist_i/Σvar_i)·√(Σvar_i²/Σdist_i·var_i) | [model] | 5.9a.7 | `referee-2.md` §3 |
| 5.bs | The efficient rule and the stable-dial rule are the same rule (A4.5 item 3) | [refuted] | 5.9a.7 | `referee-2.md` §4 item 3 |
| 5.bt | The two rules agree within a cluster, not at one price across clusters; with the constant a threshold row is kept with probability 0.46–0.49 / 0.82–0.84 / 0.97–0.98 at ψ0 = 0 / 1 / 2 | [model] | 5.9a.7 | `referee-2.md` §4; `ref2_a44.txt`; `ref2_a45b.txt` |
| 5.bu | Under the track's model the kept share is P(g ≤ ψ0) on every index (A4.5 item 5) | [refuted] | 5.9a.7 | `referee-2.md` §4 item 5 |
| 5.bv | The ψ0 needed for 0.9 kept ranges from −0.91 to 0.71 over settings; one meaning needs the width κ_Δ·(t − μ_c) + c0 + ψ0·s_res | [model] | 5.9a.7 | `out/a4_resemble.txt`; `referee-2.md` §4 item 5 |
| 5.bw | Independent heavy tails cannot reach ϑ above about 0.8 | [refuted] | 5.9a.7 | `referee-2.md` §3 |
| 5.bx | Cov(â, e) > 0; the slope of score on midpoint is 1.04–1.09; the plain rule is 7–18% worse than κ = 1 − ϑ on LAION | [measured] | 5.9a.7 | `referee-2.md` §3 |
| 5.by | "A width one scatter too wide is 1.6 to 2 times the rows" (A4.9), understated; it is 1.8–2.8 [computed] | [refuted] | 5.9a.7 | `referee-2.md` §7 |
| 5.18 | Tree law in model T: beam size B_j; OV remark [cited, unchecked] | [proved, unrefereed] | 5.10.1 | `hierarchy/theory.md` §2.3 |
| 5.o | Tree law within 25% at groups, optimistic 1.5–13× below clusters; middle level 3.6–10× fewer centres | [measured] | 5.10.1 | `hierarchy/theory/hier.py` |
| 5.33 | Model T on a regular tree is Derrida's generalised random energy model; its binary refinement is the continuous model with profile Π; they agree in path laws, counts and hull, differ in x_* and the floor (A1.1) | [proved, unrefereed] | 5.10.1a | `program/approximate/CLAIMS.md` A1.1; `referee/referee-1.md` A1.1 |
| 5.bz | The generalised random energy model is Derrida's | [cited, unchecked] | 5.10.1a | from memory, and as Addario-Berry and Maillard describe it |
| 5.ca | Nodes valued as the mean of their rows make siblings correlated −1/(F − 1), which the track's tests do not test | [proved, unrefereed] | 5.10.1a | `referee-1.md` A1.1 item 5 |
| 5.34 | The leaf floor at a fixed level: E\|𝒬\| ≥ K_D·P(A_{D−1} > θ_r), attained by a threshold on the parent's value (A1.2) | [proved, unrefereed] | 5.10.1a | `CLAIMS.md` A1.2; `referee-1.md` A1.2 |
| 5.cb | The floor holds for any tree shape and side information, so it bounds the rows any hierarchy reads, and one level lower the stored vectors read | [proved, unrefereed] | 5.10.1a | `referee-1.md` A1.2; `CLAIMS.md` A1.2 remarks |
| 5.cc | No leaf-query rule in 12,000 trees beat the floor; the two built to sit on it were within 0.05 targets | [model] | 5.10.1a | `referee/ref1_a12.txt` |
| 5.35 | The leaf floor for the k best, (5.9) (A1.2b, the referee's) | [proved, unrefereed] | 5.10.1a | `CLAIMS.md` A1.2b; `referee-1.md` A1.2 remark (i) |
| 5.cd | With a few hundred parents the floor for the k best is 1.05–3.1 times B_leaf, because τ_k does not concentrate; the static optimum lies 1–4% above | [computed] | 5.10.1a | `referee/ref1_a12c.py`; `ref1_a12.txt`; `scripts/topk_model.py check` |
| 5.ce | On the three suite fits the static optimum is 1.19–1.88 times B_leaf(0.9) | [computed] | 5.10.1a | `out/topk_model_fits.txt` |
| 5.cf | Addario-Berry–Maillard Theorem 1.1: easy below x_*, exponential above, for a continuous random energy model | [cited] | 5.10.1a | arXiv:1810.05129 |
| 5.cg | The ground state x_s, after Bovier and Kurkova | [cited, unchecked] | 5.10.1a | through Addario-Berry and Maillard |
| 5.ch | The hardness theorem applied to model T, so that a non-concave profile costs every hierarchy exponentially many reads | [refuted] | 5.10.1a | `referee-1.md` A1.1 item 2 |
| 5.ci | For the index's tree the beam's count (5.7) bounds the read from above | [model] | 5.10.1a | `CLAIMS.md` A1.6 |
| 5.cj | On yahoo, LAION text and glove the profile is not concave, the last segment steepest | [measured] | 5.10.1a | `CLAIMS.md` A1.7 item 1 (`replay_np.py`) |
| 5.ck | A margin of 2.5–5% read from the hull sum | [refuted] | 5.10.1a | `referee-1.md` A1.5 item 4 |
| 5.cl | The realised level of the best L leaves moves from 0.90 to 1.00 of the expected, so the hull sum predicts to about 0.05 | [model] | 5.10.1a | `ref1_a14_a15.txt` part C |
| 5.cm | Model T's split is Bovier and Kurkova's weak and strong correlation regimes | [refuted] | 5.10.1a | `referee-1.md` A1.5 item 3 |
| 5.cn | Leaves of about 125 rows bring the row term down to the centres' term | [refuted] | 5.10.1a | `referee-1.md` A1.4 item 5 |
| 5.co | By count the row term is the largest in all twelve designs, by 20–100 times | [computed] | 5.10.1a | `ref1_a14_a15.txt` part B |
| 5.cp | The static optimum falls 2.1–2.4 times to 125-row leaves and 1.4–2.1 times more to 31-row leaves, against measured 2.7–4.1 and 2.3–2.5 | [model] | 5.10.1a | `CLAIMS.md` A1.9 |
| 5.cq | The cost exponent above x_* (Addario-Berry and Maillard's open question 2, formula (5.1)) | [conjecture] | 5.10.1a | arXiv:1810.05129 |
| 5.cr | Pemantle 2009: a fraction 1 − ε of the best level costs N·exp(Θ(ε^(−1/2))) | [cited, unchecked] | 5.10.1a | abstract via a search summary; Ho and Maillard, arXiv:2107.11465 |
| 5.19 | σ_g(q)² = qᵀC_g q + mean_c σ_c(q)² | [proved, unrefereed] | 5.10.2 | `hierarchy/theory.md` (3.1) |
| 5.20 | Expand-or-scan values V_g(λ) give the optimal adaptive tree policy under (S1), (S2) | [proved, unrefereed] | 5.10.2 | `hierarchy/theory.md` §3.2 |
| 5.p | Branching-bandit index [cited, unchecked]; DP agreement on 300 instances | [model] | 5.10.2 | `hierarchy/theory.md` §3.2 |
| 5.21 | Never expand a sparse node | [proved, unrefereed] | 5.10.2 | `hierarchy/theory.md` §3.2 |
| 5.q | Total-variance key cuts parents expanded 15–25% | [measured] | 5.10.2 | `hierarchy/theory.md` |
| 5.r | Best ψ does not depend on the unit's size | [model] | 5.10.3 | `hierarchy/theory.md` §3.4 |
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
| 5.cs | A SOAR second assignment read 12–15% fewer items at the same recall and was no faster (glove 863 against 922 queries a second) | [measured] | 5.13 | `bench/REPORT.md` "What the numbers say" item (c) |
| 5.aa | A sub-centre as a 20-byte coded row would not rank sub-centres | [conjecture] | Open | `hierarchy/theory.md` §5.2 |
| 5.ct | With ψ0 = ψ the sampled width is too wide: time to recall 1.25–1.78 times the derived-ρ build on LAION, yahoo, BioCLIP; 0.57–0.88 on iNaturalist | [measured] | 5.9a.8 | `program/LOG-2026-11-21.md`, visit-order session |
| 5.cu | The stop's ψ is roughly a quantile of the threshold's height in the neighbours' home clusters (median 2.8–3.6 spreads); (5.8) already pays for the height | [model] | 5.9a.8 | `program/approximate/DIAL.md`; `program/approximate/referee/referee-3.md` |
| 5.cv | Cost-optimal split of misses between stop and levels: equal risk per row declined, with κ = c₁/(c₀ + c₁φ); equal risk per neighbour would need κ near 80 | [model] | 5.9a.8 | `program/approximate/DIAL.md`; `program/approximate/referee/referee-3.md` |
| 5.cw | The derived formula ψ0 = max(0, ψ − ū − s̄ + (1/√2)·ln(1/(κφ̂))) has nothing fitted | [refuted] | 5.9a.8 | `program/approximate/referee/referee-3.md` §4 |
| 5.cx | The levels' dial must rise with ψ (0.72–0.85 per unit in replay); a constant caps LAION's recall at 0.970 / 0.989 for ψ0 = 2 / 3 | [model] | 5.9a.8 | `program/approximate/DIAL.md`; `program/approximate/referee/referee-3.md` |
| 5.cy | Fixed ψ0 = 3: time over the earlier build 0.84/0.59/0.56 iNaturalist, 0.93/1.06/1.31 yahoo, 1.44/1.77/– LAION; LAION rows past the levels 15,304 → 5,699 | [measured] | 5.9a.8 | `program/LOG-2026-11-21.md`, visit-order session |
| 5.cz | Offset dial ψ0 = max(0, ψ − 1.5) timed: within 7% of ψ0 = 3 at recall 0.9 / 0.95, reaches 0.99 where the constant does not; over the derived-ρ build 1.39/1.70/2.10 LAION, 1.31/1.47/1.76 yahoo, 1.03/0.99/1.14 arxiv, 0.86/0.62/0.60 iNaturalist | [measured] | 5.9a.8 | `program/LOG-2026-11-21.md`, visit-order session |
| 5.da | Where the derived width's error lies (iNaturalist 12 / 1 groups, medians of actual over predicted): derived 1.44 / 2.24 against the coding error; group's measured diagonal 1.15 / 1.52; cluster diagonal 1.15 / 1.47; mean error nothing; 16 σ_c directions 1.04 / 1.06, p90 to 1.13 | [measured] | 5.9a.1 | `program/LOG-2026-11-21.md`, visit-order session (`sample-when-needed/decomp.txt`) |
| 5.db | A per-cluster ratio from corpus rows as queries, no sample, reads the fewest rows at 0.9 kept (iNaturalist 144 against 253 derived, 163 sampled; LAION 428 against 518, 741); errs wide by 1.18 on LAION's text queries; sampling some clusters is worse than either pure rule | [measured] | 5.9a.8 | `program/LOG-2026-11-21.md`, visit-order session (`sample-when-needed/summary.txt`) |
| 7.a | Block norms cut the text half-width by 10% or more only with J ≥ 8, and from J = 4 cost more bytes than they save; the mixed basis cuts text 21–33% and costs items 6–19%; 0 violations | [measured] | 7.1 | `stage0-block-norms/REPORT.md` "Verdict", "The numbers that decide it" |
| 7.b | The best diagonal weighting per (query, item) pair sits at about 0.6 of the single norm for every kind of query | [measured] | 7.1 | `stage0-block-norms/REPORT.md` "What it shows" |
| 7.c | The built code sits 1.31–1.77 times above the cap bound in tan (the bound is 3.bi, 4.ah); 1.31–1.42 where the variance sits, none of it nesting | [measured] | 7.2 | `codes/built.py`; `codes/REPORT.md` "Where the built code stands" |
| 7.d | At one rate, codes with today's decode gain 13–16% of width at 2 bits, 12% at 4 and 15–17% below one bit; E8 7–10%, D4 4–6% | [measured] | 7.2 | `codes/REPORT.md` "Verdict"; `codes/NOTEBOOK.md` entry 8 |
| 7.e | Nested: residual stages lose (1.945 times built tan at the 7-bit rise), a trellis-coded last plane gains nothing, E8 loses 1–10% | [measured] | 7.2 | `codes/NOTEBOOK.md` entry 7 |
| 7.f | Under the phased trellis κ and tan φ keep their meaning and today's tables fit as well (500 items) | [measured] | 7.2 | `codes/NOTEBOOK.md` entry 9 |
| 7.g | The nesting loss: a table-additive trellis keeps 6–8% of the 13–16% it gains at one rate (as 4.bs), E8 nothing; each further 1% at the 2-bit rise costs about 3% at the 1-bit rise | [measured] | 7.2 | `codes/REPORT.md` "Verdict" item 2 |
| 7.h | The best package narrows the last level 8.2% (6.6% with a 16-state encoder), below the 10% bar; as built about 7% fewer full-vector reads, 3% of bytes (as 4.ba), 1% of time (estimates) | [model] | 7.2 | `codes/REPORT.md` "Verdict", "What it would be worth, as built"; `codes/NOTEBOOK.md` entry 10 |
| 7.i | 24 encoder scales in place of six narrow the 6- and 7-bit rises by 6–8% | [measured] | 7.2 | `codes/exp_encoder.py`; `codes/REPORT.md` "Verdict" item 1 |
| 7.j | That change is worth about one point of the last level's width (an estimate) | [model] | 7.2 | `LOG-2026-11-21.md` "Needs the machine"; `codes/NOTEBOOK.md` entry 10 |
| 7.k | A block whose last rise is its only rise can use the one-rate codes; the 16-word codebook below one bit is the cheapest | [measured] | 7.2 | `codes/REPORT.md` "Verdict" item 3; `codes/NOTEBOOK.md` entry 4 |
| 7.l | The corrected stop bound with an estimated tail uses κ_σ = M_σ/M̂_σ (median 27, 90th percentile 300 in the referee's run [model]) | [proved, unrefereed] | 7.3 | `approximate/CLAIMS.md` A2.1 |
| 7.m | Two steps of A2.2's first proof (a shape chosen after the sample, a false independence) | [refuted] | 7.3 | `approximate/CLAIMS.md` A2.2; `approximate/referee/referee-1.md` |
| 7.n | With 1,300–4,000 pooled rows the prediction is off by a factor 2.2–4.0 at the decision | [measured] | 7.3 | `approximate/CLAIMS.md` A2.3 |
| 7.o | One shape over-predicts at τ_k by 2.2–2.5 times, the first clusters over-predict the rest by 1.7–3.1 times, and the stop selects its own low estimates | [measured] | 7.3 | `approximate/CLAIMS.md` A2.5 |
| 7.p | The stops on expected misses read 0.87–3.01 times the built stop's rows; at a nominal 0.9 the delivered mean recall is 0.65–0.99 | [measured] | 7.3 | `approximate/CLAIMS.md` A2.6 |
| 7.q | The per-query spread: ρ_ℓ(q, c)² = E_ω[a]/‖a‖₁ exactly for a fixed offset, returning (5.6) on average | [proved, unrefereed] | 7.3 | `approximate/CLAIMS.md` A3.1 |
| 7.r | It removes 8.5 of the 13.5 points of excess on LAION text at 16 B and changes in-distribution widths by under 1% | [computed] | 7.3 | `approximate/CLAIMS.md` A3.3 (a stand-in allocation) |
| 7.s | The harness test will not overturn A2.6's ranking (A2.8) | [conjecture] | 7.3 | `approximate/CLAIMS.md` A2.8; `approximate/REPLAY-SPEC.md` |
| 7.t | The coding cost of a Gaussian cell is at least D_λ = Σᵢ φ(λ²γᵢ), with D_λ/λ → F and a log-determinant at high price | [proved, unrefereed] | 7.4 | `partition/CLAIMS.md` P1.1, P1.2 |
| 7.u | In a two-segment example the partition k-means prefers costs 2.00 bits a row more at w = 0.05 | [model] | 7.4 | `partition/CLAIMS.md` P1.3 |
| 7.v | Within a group, k-means in the group's metric is coding-optimal | [refuted] | 7.4 | `partition/referee/referee-1.md` P1.4 |
| 7.w | The mismatch of k-means and coding cost appears wherever a covariance is formed | [model] | 7.4 | `partition/CLAIMS.md` P1.4, S.1 |
| 7.x | The sorted range counts the keys' low tail, whose sum spherical k-means ascends; the approximate order is served by a trace in the queries' metric | [proved, unrefereed] | 7.4 | `partition/CLAIMS.md` P1.6, P1.7 |
| 7.y | The built fit leaves 4.4% of rows away from their best centre | [measured] | 7.4 | `partition/experiment/RESULTS.md` item 1 |
| 7.z | Partition experiment: no alternative clears twice the seed spread (B ≤ 2.5% of S1b, C16 2.4%); balanced k-means trades decisions for layout; the neighbour-graph partition was not run | [measured] | 7.4 | `partition/experiment/RESULTS.md` "Summary table", "Drop rules" |
| 7.aa | The built grouping is worth 16% of S1d and 22% of bytes against one basis; a random grouping keeps a quarter; a basis per cell is 10% narrower still | [measured] | 7.4 | `partition/experiment/RESULTS.md` item 3 |
| 7.ab | Fewer bases cost bytes: 3–7% from 64 groups to 31 and 6–10% per halving from 62 to 7 at 1M | [measured] | 7.4 | `rust/REPORT_CLUSTERSIZE.md`; `partition/CLAIMS.md` P2.5 |
| 7.ac | The bases cost 37.7 B a row at G = 64 | [computed] | 7.4 | `partition/CLAIMS.md` P2.5 |
| 7.ad | Log-determinant coding length for segmentation (Ma et al.); coding-optimal partitions beating k-means (Archer and Leen); neighbour-graph partitions (Dong et al.) | [cited] | 7.4 | `library/partition.md` verdicts 1.6, 3.6, 6.1 |
| 7.ae | The entropy-constrained encoder rule (Chou, Lookabaugh and Gray) | [cited, unchecked] | 7.4 | `library/partition.md` |
| 7.af | Pair-uniformity is strictly weaker than Haar (H2) | [proved, unrefereed] | 7.5 | `rotations/CLAIMS.md` H2 |
| 7.ag | A2 as the reason to reject sign–Hadamard products (see 3.av) | [refuted] | 7.5 | `rotations/referee/referee-1.md` "A2" |
| 7.ah | One sign–Hadamard round fails outright, 16 sd at n_b = 256 with probability ½ per side (B1) | [proved, unrefereed] | 7.5 | `rotations/CLAIMS.md` B1; rebuilt by the referee |
| 7.ai | Two to four rounds exceed the budget 2^(−60.8) on subgroup-constant inputs: 2^(−19.1) to 2^(−58.5) at 256, down to 2^(−31.9) at 1,024 | [computed] | 7.5 | `referee-1.md` (B4 recomputed) |
| 7.aj | "At blocks of 64 and below the construction does not reach the constant" | [refuted] | 7.5 | `referee-1.md` "Addendum to B4" |
| 7.ak | Sign–Hadamard rounds fail at blocks of 64 and 128 too, for one to four rounds | [computed] | 7.5 | `referee-1.md` "Addendum to B4" |
| 7.al | Incoherence does not rescue them (B5) | [sketch: gap named] | 7.5 | `rotations/CLAIMS.md` B5 |
| 7.am | An incoherence assumption with constant 4/√n_b, at 256 for two and three rounds | [refuted] | 7.5 | `referee-1.md` |
| 7.an | A seed of 128 bits for N ≥ 2³³ (C2) (see 3.ax) | [refuted] | 7.5 | `referee-1.md` "C2" |
| 7.ao | Butterflies: for y = e₀, Δ = e₁ and any codebook, \|Δ·E\| is a function of z alone and its sign a fair coin (D2, repaired) | [proved, unrefereed] | 7.5 | `rotations/CLAIMS.md` D2; `referee-1.md` "D1 to D4" |
| 7.ap | That error's rms is 3.7–4.1 sd at n_b = 256, and it exceeds 7.8 sd on 20–37% of draws at 1,024 | [computed] | 7.5 | `rotations/CLAIMS.md` D4 |
| 7.aq | A random-pair Kac walk needs at least 21.4·n_b steps to be within 2.5·10⁻¹⁹ of uniform | [proved, unrefereed] | 7.5 | `rotations/CLAIMS.md` D1 |
| 7.ar | No density-ratio bound for Kac walks | [sketch: gap named] | 7.5 | `rotations/CLAIMS.md` D3 |
| 7.as | Under Haar measure on a closed subgroup the error given z is invariant under y's stabiliser, so fewer stored numbers cost more scalars or deterministic terms (H1) | [proved, unrefereed] | 7.5 | `rotations/CLAIMS.md` H1 |
| 7.at | The Kronecker row of H1 for unequal factors | [refuted] | 7.5 | `referee-1.md` |
| 7.au | Unitary and symplectic forms keep the law in n_b − 2 or n_b − 4 dimensions at the price of a second scalar (H4) | [proved, unrefereed] | 7.5 | `rotations/CLAIMS.md` H4 |
| 7.av | The unitary form's deterministic term is 10% of c·sd on average and 21% at the ninth decile (n_b = 64) | [computed] | 7.5 | `rotations/CLAIMS.md` H4 |
| 7.aw | Independent Haar rotations on sub-blocks keep the law (F1–F5) | [proved, unrefereed] | 7.5 | `rotations/CLAIMS.md` F1–F5 |
| 7.ax | With one κ per block a radial term widens the bound 12–20% at two sub-blocks | [computed] | 7.5 | `rotations/CLAIMS.md` F; reproduced by the referee |
| 7.ay | No ensemble with o(n²) parameters is uniform on pairs (H6) | [conjecture] | 7.5 | `rotations/CLAIMS.md` H6 |
| 7.az | A shared subtractive dither gives independent uniform errors depending on the item through s alone: Δ·E = s·Σ_p Δ_p·2^(−d_p)·V_p (G1) | [proved, unrefereed] | 7.6 | `rotations/CLAIMS.md` G1; `referee-1.md` "G1 to G10" |
| 7.ba | The sub-Gaussian constant 9.256 at N = 10⁹ is valid for every weight vector (G5) | [proved, unrefereed] | 7.6 | `rotations/CLAIMS.md` G5 |
| 7.bb | A Rademacher bound gives 9.040 (G5a) | [cited, unchecked] | 7.6 | `rotations/CLAIMS.md` G5a |
| 7.bc | The worst case is 8.896 (G5a) | [conjecture] | 7.6 | `rotations/CLAIMS.md` G5a |
| 7.bd | These are 4.8%, 2.4% and 0.7% above today's 8.830 | [computed] | 7.6 | `rotations/CLAIMS.md` G5; reproduced by the referee |
| 7.be | One step and one norm per block, 8 bytes for four blocks against 15 to 20.5 (G6) | [proved, unrefereed] | 7.6 | `rotations/CLAIMS.md` G6 |
| 7.bf | Nested from one bit the dither's spread is 2.0–2.65 times today's at every depth, 1.0–1.4 bits a position lost | [computed] | 7.6 | `rotations/g_dither.py`; `rotations/CLAIMS.md` G7 |
| 7.bg | A dither over a lattice cell in 8 to 24 dimensions would halve the crest factor (G10) | [conjecture] | 7.6 | `rotations/CLAIMS.md` G10 |
| 7.bh | Against F2 with the head as S a code reaches 1.0023, the ratio falling roughly in inverse proportion to the tail's length; the floor is close to tight for a short head and a long norm-coded tail (the 1.0026 against F1 is 2.x) | [computed] | 7.8 | `stage0-floor-check/RESULTS.md` "Verdicts" and appendix |

## Counts by main tag

| Tag | Rows |
|---|---|
| [proved, refereed] | 66 |
| [proved, unrefereed] | 121 |
| [sketch: gap named] | 17 |
| [conjecture] | 16 |
| [refuted] | 32 |
| [measured] | 138 |
| [computed] | 49 |
| [model] | 101 |
| [cited] | 8 |
| [cited, unchecked] | 15 |
| total | 563 |

| Chapter | Rows |
|---|---|
| 1 | 90 |
| 2 | 59 |
| 3 | 130 |
| 4 | 83 |
| 5 | 141 |
| 7 | 60 |
| total | 563 |

Rows are claims as the chapters group them, so a measured row often holds several figures, and a row's secondary tags
(a [conjecture] inside a [measured] row, for example) are not counted. Six results and one measurement appear in two
chapters (1.5 (d) = 4.10, 1.7 = 2.7, 1.15 = 2.8, 1.16 = 2.9, 2.10 = 4.3, 2.13 = 4.1, and 2.l = 4.i), and each appearance
has its own row; so do the cap bound of `codes/REPORT.md` (3.bi, 4.ah) and the nesting figure 6 to 8% (4.bs, inside
7.g). Conjecture 2.14 and Theorem 2.24 are one statement within chapter 2, and each has its own row. The letters 2.x to
2.ad follow 2.w although section 2.9a comes before section 2.10, and the letters added in chapters 1, 3, 4 and 5 follow
each chapter's last letter in the same way, so that no earlier letter changes. Chapter 7's sections 7.7 and most of 7.3
restate rows of chapters 3, 4 and 5 (3.R9, 3.30, 3.46 to 3.48, 3.bc, 3.bd, 4.bi to 4.bl, 5.ab to 5.af) and add no row
of their own beyond those listed.

## Tags outside the scheme

No row and no secondary tag of chapters 1 to 5 and 7 now carries a tag outside the scheme. The 31 rows that did were
mapped to the weakest README tag that fits, with the qualifier moved into the chapter's sentence in words:

- [measured, simulation] (3.ao, 3.ap, 3.ar, 3.ay, 3.ba, 3.bf, 3.bk, 3.bl, 3.bm, 3.bn, 3.br, 3.bu) became [model], since
  README's [measured] is on real data; each sentence says the figures are by simulation and names the script.
- [proved by example; measured] (3.j) became [model]: the example is argued, and the measurement is a toy simulation.
- [refuted as a constant] (3.l) and [refuted as stated] (3.bd) became [refuted], with "as a constant" and "as stated" in
  the sentence; so did [refuted as a saving] inside 3.af.
- [measured and argued] inside 3.aa became [model], with "as measured and argued" in the sentence.
- [measured on a proxy] (4.w) and [measured, share inferred] (4.ab) became [model], with the proxy and the inferred
  share named in the sentence.
- [computed in the model] (4.an, 4.ao, 4.ap, 4.av), [model and computed] (4.at), [model on measured sizes] (4.bc),
  [model with measured costs] (4.bi, and chapter 7, §7.7) and [model against measured] (4.bj) became [model], with the
  computation or the measured inputs named in the sentence.
- [proved, unrefereed; conditional on the OV conjecture] (4.2) and [proved, unrefereed; given a measured property]
  (4.3) became [proved, unrefereed], with the condition stated after the tag.
- [model, measured] (5.r), [computed and measured, unrefereed] (5.cp) and [checked by referee 2 in simulation] (5.bo)
  became [model]; [computed, unrefereed] (5.ce) became [computed].
- [proved as identities] (7.x) became [proved, unrefereed], with "as identities" in the sentence; section 7.4 records
  no referee's check of P1.6 and P1.7.
- [computed on measured counts] (7.n) became [measured], chapter 5's tag for the same figure (5.ad), with "computed
  from measured counts" in the sentence.

Rows that use a README tag outside its definition: 2.r is [measured] on synthetic data, and the grid check of 2.18 is
[computed] although it evaluates no proved inequality. Defects 4 and 6 of §1.8a (1.be, 1.bg) are [model] although
nothing was run, and defect 7 (1.bh) is an estimate with standard rounding bounds. Chapters 4 and 7 no longer use
[estimate]: the figures the sources tag so enter as [model] with the word "estimate" kept (4.aj, 4.ba, 7.h, 7.j).

## Unrefereed results that other chapters or the built index rely on

Each of these rests on one proof that nobody else has checked, or on a sketch, and something later depends on it.

- **1.4, 1.5a, 1.5b and 1.5, the cluster cut and the contiguous range.** The index's `support_pre` and `sorted_range2`
  rely on them, and chapter 4's "range" exponents measure 1.5. The statements for the index as built are the range
  referee's own; the earlier exact-key form, 1.5 (d) and 4.10, was found correct by that referee and its tag was not
  raised (chapter 1, Editor's query 17). Defects 1 and 2 (1.bb, 1.bc) break their hypotheses in the code.
- **1.6, the ν-sort.** `NuCut` in `rust/src/v2/q.rs` implements it; the restatement is the range referee's own.
- **1.8 and 1.9, weighted Cauchy–Schwarz and the quantised basis.** Every deterministic decision of the index uses
  them, and chapter 5's rule 1 (5.16) takes 1.8's bound as its width. Both are one-line consequences of
  Cauchy–Schwarz.
- **1.10 and 1.ak, the floating-point allowances and the full-vector rounding bound.** The built certificate and the
  full-vector step use bounds that exist only in code comments; verify mode found no violation (1.l).
- **1.18a, 1.18b, 1.19, 1.21′, 1.22 and 1.at, the integer kernel as built.** The kernel referee's own proofs, with
  float constants counted by hand; Theorem 1.19 was refereed in its design form only. The compare above 64 units rests
  on the sketch 1.ax, and the default path does not assert the unit count (1.bd).
- **1.15 (= 2.8) and 4.4–4.7.** Chapter 4's Claim 4.9 combines 2.8 with the refereed 2.4, and 4.7 is the basis of
  "every fixed-slack level is asymptotically linear".
- **2.13 (= 4.1), the entropy bound.** Chapter 4 uses it to set the metric; the source's proof is one sentence.
- **3.n, the one-sided upper end of Theorem 3.11.** The one-sided k-NN constants (3.p) rest on it, and chapter 3's
  Editor's query 12 doubts the argument for Ê > b. The R8 part of 3.48 rests on the same reading (chapter 3, Editor's
  query 20).
- **3.as, the referee's repairs to the use of 3.25.** A rounded rotation on the query side through the kernel's slot is
  sound only with repair 1's factor (1 + dk_j).
- **5.2, 5.4 and 5.10.** The built order μ_c + 3σ_c(q) is justified in §5.8 by these results, and 5.14 by the stand-in
  key's invariance. Rule 1 (§5.9) rests on 5.16, which is a model and not a proof; the visit-order session's build has
  since taken the measured rule 5.an, which is fitted and not derived, and whose shift as built is not measured.

The constants the index uses (chapter 3 §3.9) rest on refereed results (3.18–3.21, 3.23 (i), 3.al) and on computed
values. Two computations behind them were not refereed line by line: the Chernoff fallback's implementation (3.ab) and
`plane_cert.py` (3.22).

## Disagreements found while rebuilding

Three found earlier are resolved by the mapping above: the estimate of the tail at the decision (5.ad and 7.n are both
[measured]), Proposition 3.46's upper end against the plain end (3.bl and §7.7 are both [model]), and the hand-over
budgets for group 6 (§4.9.2 now tags groups 12 and 6 alike [model]).

- **Tags of §3.7a, §3.2a and §3.4 against their tracks.** The confidence and rotations tracks mark every proof
  unrefereed; chapter 3 tags [proved, refereed] the results whose text the referee read unchanged (3.25 to 3.28, 3.34
  to 3.41, 3.43, 3.45), after an audit (chapter 3, Editor's queries 14 and 15). The chapter is consistent with itself,
  so the ledger keeps its tags; the PI may downgrade them.
- **Proposition 5.30's last clause.** Chapter 5 keeps it [proved, refereed] while its Editor's query 26 notes that
  slope 1 needs e to be an increasing linear function of s, not any linear function. The tag is the chapter's.
