# Stage 0 floor check: results

Falsifier's results for the PI, 21 November 2026, written from the logs in this directory after the machine restarted
at about 07:13. `program/floors/` was not read. Every number is **[computed]** by the script named beside it; none is
timed. Coordinates are whitened throughout: z ~ N(0, I) in the eigenbasis, a cell is A, K = Γ^½ A, P is the Gaussian
measure of A, and the exact mean half-width of a cell is E h_K(g) (the support function of the hull, which is the
smallest sound certificate).

## Verdicts

| Statement | Verdict | Closest approach found |
|---|---|---|
| Φ, one-dimensional lemma C(μ) ≥ s(KL) | not refuted | 1.00000000007 (mixtures, adaptive quadrature); equality only at N(0, s²) |
| Φ, per cell E h_K ≥ Φ(ln 1/P) | not refuted, on the families that finished | 1.0266 (small ball, n = 100) = κ₁₀₀/100; tends to 1 as n grows |
| Φ, averaged E h ≥ Φ(H) | not refuted | 1.168 (norm-only code, n = 100) |
| F1: E h ≥ W(H) | not refuted | 1.0026 (ECSQ head + tail norm, 1 + 2000 eigenvalues) |
| F2: E h ≥ min_u [κ_k GM_S e^(−(H−u)/k) + W_{S^c}(u)] | not refuted | 1.0005 against max over S (that S is all n indices, i.e. Theorem 1); 1.0023 against S = the head |

No ratio below 1 survived a precision check. Three rows of `f12_target_m1.log` / `f12_target_m23.log` report ratios to W
of 0.000040, 0.077 and 0.016. They are a bug in `f12_target.py`, not counterexamples: when the optimiser pushed the tail
grid's first edge o + D past the grid's top radius, `tail_norm` built no cells and returned zero tail width (E h in those
rows equals the head's width alone). Those rows are excluded below; the fix and a rerun are in the appendix.

## 1. The one-dimensional lemma (`lemma1d.py`, `lemma1d.log`; `lemma1d_mixcheck.py`, `lemma1d_mixcheck.log`)

C(μ) was computed as ∫ I(F_μ(x)) dx with I(u) = φ(Φ⁻¹(u)), which follows from C = ∫₀¹ F⁻¹Φ⁻¹ du by parts and is exact
for laws with gaps. Each family was minimised at fixed KL = κ (a free minimisation of C/s(KL) drifts to κ → 0, where
everything tends to C = s = 1).

Smallest C/s(κ) per family [computed]:

| κ | interval | 2 intervals | 2-mixture (grid) | 3-mixture (grid) | uniform | Laplace | convex programme M = 800 |
|---|---|---|---|---|---|---|---|
| 0.01 | 1.0654 | 1.0654 | 0.999999 | 0.999997 | – | – | 1.0000788 |
| 0.1 | 1.1256 | 1.1256 | 0.999999 | 1.000000 | – | 1.1718 | 1.0000290 |
| 1 | 1.1633 | 1.1633 | 0.999999 | 0.999995 | 1.1799 | 1.0596 | 1.0000155 |
| 2 | 1.1655 | 1.1655 | 0.999999 | 0.999993 | 1.1675 | 1.0556 | 1.0000148 |
| 5 | 1.1658 | (24.3, search failed) | 0.999994 | 0.999999 | 1.1658 | 1.0550 | 1.0000147 |
| 10 | 1.1658 | (no feasible start) | (none within 1% of κ) | (none) | 1.1658 | 1.0550 | 1.0000147 |

(κ = 0.03, 0.3, 0.5, 3, 7 are in the log and lie between these rows.) Narrow intervals tend to √(e/2) = 1.16582, matched
to 8 digits.

- **Mixtures below 1 are grid error.** On the 3,001-point grid the best mixtures read 0.999993 to 0.999999. Re-evaluated
  by adaptive quadrature at relative tolerance 1e-13 (`lemma1d_mixcheck.log`), the smallest values are
  1.00000000007 (κ = 0.1), 1.0000000007 (κ = 1), 1.0000000001 (κ = 2) and 1.0000000006 (κ = 5); the worst grid reading,
  0.999988 at κ = 5, is 1.0000000006 by quadrature. One candidate reads 0.99999999999 ± 1.3e-14: its second component
  has weight about 0, so it is N(0, s²) itself, and the 1e-11 residual is the rounding of s(κ) and of the quadrature
  (quad's error figure is an estimate, not a bound).
- **The convex programme.** In the quantile parametrisation KL(μ‖N) = ∫₀¹ [−ln Q′ + Q²/2] du + ½ ln 2π is convex in Q and
  C is linear, so minimising C at KL ≤ κ is a convex programme. In the density (or CDF) parametrisation it is not, since
  C = ∫ I(F) dx is concave in F. The programme over piecewise-uniform laws (exact KL and C for that family) gives
  C/s(κ) − 1 = 1.5e-5 to 7.9e-5 at M = 800 cells and 2.4e-4 to 1.3e-3 at M = 200, falling with M. Its Hessian is positive
  definite (smallest eigenvalue 1e-12 to 3e-18, scaling with κ) and its optimum matches s·Φ⁻¹(u) to 1.4e-4 at κ = 1.
- N(m, σ²) in closed form (C = σ, KL = k(σ) + m²/2) never goes below 1; equality at m = 0, σ ≤ 1.

An observation, not leaned on above, unrefereed: Q = s·Φ⁻¹ satisfies the Euler–Lagrange equation
Φ⁻¹ + λQ + λ(1/Q′)′ = 0 with λ = s/(1 − s²), so by convexity it is the global minimum. Then the per-cell inequality
follows from the Knothe–Rosenblatt coupling of g with the cell's law: the chain rule splits ln(1/P) into conditional KLs,
and s is convex and decreasing, so Jensen gives Σ √γᵢ s(κ̄ᵢ) ≥ Φ(ln 1/P). The averaged form follows because Φ is convex
and decreasing in H. If this holds, step 2's results are what one should see.

## 2. The per-cell inequality (`percell.py`; sections 1–3 in `percell_part1.log`)

R = E h_K / Φ(ln 1/P). E h_K and ln P exact (closed form or 1-D quadrature); spectra flat, i⁻¹, i⁻², 0.8ⁱ and the leading
eigenvalues of a measured spectrum (`agent1/spec2_n128.json` group 5).

| Family | n = 1 | n = 2 | n = 5 | n = 20 | n = 100 |
|---|---|---|---|---|---|
| boxes aligned, centred and off-centre | 1.648721 | 1.648721 | 1.648721 | 1.648721 | 1.648721 |
| boxes in a Haar or slightly rotated basis | – | 1.648721 | 1.648721 | 1.648721 | 1.648721 |
| balls, centred and off-centre | 1.648754 | 1.461160 | 1.261592 | 1.095745 | 1.026614 |

- Boxes reach √e = 1.648721 only in the limit of vanishing width, in every dimension and spectrum: a box's ratio is at
  least the worst of its coordinates' interval ratios, and narrow intervals give √e.
- Balls are smallest at the smallest radius tried (ρ = 0.02, flat spectrum), where R → κ_n/n (Theorem 1's constant:
  1.649, 1.461, 1.262, 1.096, 1.027). The per-cell bound is therefore tight only as n → ∞. At n = 100 the centred ball's R
  rises from 1.0268 (ρ/√n = 0.1) to 1.084 (ρ = √n) and 1.203 (ρ/√n = 1.2).
- Errors: quadrature, below 1e-8 relative.

**Did not finish** (sections 4–9; cut first by run time, then by the restart): aligned and misaligned ellipsoids, thin
shells, balls cut by half-spaces and slabs, L1 balls, arbitrary convex polygons in 2-D. Half-spaces and slabs alone are
unbounded in n ≥ 2, so E h = ∞ and they cannot violate. Shells cannot do better than their hull: replacing any A by its
convex hull keeps E h and raises P, so the infimum is over convex cells.

## 3. Whole codes: Φ averaged, F1 and F2 (`f12.py`, `f12.log`; `f12_target.py`, `f12_target_m1.log`, `f12_target_m23.log`)

Every E h and H is exact except D4 (Monte Carlo, standard errors given). F2 is evaluated as the maximum over S = top-k
for k = 1…12, the powers of two, n, and the water-level k ± 1. Each S is claimed separately, so that maximum is the claim's
strength; the S that attains it is given.

Smallest E h / bound per family [computed, `f12.log`]:

| Family (spectra: flat n = 2, 5, 20, 100; [1]·m + [ε]·N; i⁻¹, i⁻²; measured n = 128) | / W (F1) | / F2 (best S) | / Φ |
|---|---|---|---|
| 0. ECSQ on every coordinate, aligned or Haar-rotated, common or shaped steps | 1.6488 | 1.1284 (k = n = 2) | 2.7184 |
| A. ECSQ on the head (k = 1, 2, 4, 8) + tail by whitened norm, dual-weighted norm or coarse ECSQ | 1.0395 | 1.0126 (k = n) | 1.2088 |
| B. norm only (shell index), weights I, γ^¼, γ^−¼, geometric and uniform radial grids | 1.0323 | 1.0056 (k = n) | 1.1677 |
| C. signs of the head + shell of a (weighted) norm: orthant × shell cells | 1.0379 | 1.0110 (k = n) | 1.2014 |
| D. 2-D hexagonal and square lattices, rotated and sheared, spectra (1,1), (1,0.1), (1,0.01), (1,1e-4) | 1.5408 | 1.0545 (k = 2) | 2.5375 |
| E. D4 lattice (24-cell Voronoi cells), 4-D, isotropic and dual-scaled | 1.5115 ± 0.0028 | 1.1598 ± 0.0021 (k = 4) | 2.4690 ± 0.0046 |

Targeted at the steep head, with steps optimised continuously [computed, `f12_target_m1.log`, `f12_target_m23.log`;
invalid rows excluded]:

| Head over tail [1]·m + [ε]·N | / F2, S = head | / F2, best S | / W |
|---|---|---|---|
| m = 1, N = 20 | 1.0584 (ε = 1e-6) | 1.0490 | 1.1458 |
| m = 1, N = 99 | 1.0323 | 1.0105 | 1.0374 |
| m = 1, N = 400 | 1.0097 | 1.0026 | 1.0110 |
| m = 1, N = 2000 | 1.0023 | 1.0005 | 1.0026 |
| m = 2 (ECSQ or hexagonal head), N = 20 | 1.0876 (hex) | 1.0633 (hex) | 1.1649 |
| m = 2, N = 99 | 1.0331 (hex) | 1.0141 (hex) | 1.0422 |

(For m = 1 the rows are the same across ε = 1e-2, 1e-4, 1e-6 to 4 digits; the table gives the smallest.)

- **Nothing falls below 1.** Where codes come closest, F2's maximising S is all n indices, i.e. Theorem 1, which is
  proved; there the comparison checks the arithmetic rather than F2's new content.
- **F2's own content (S = the head) is approached as N grows:** 1.119, 1.032, 1.0097, 1.0023 at N = 20, 99, 400, 2000,
  falling roughly as 1/N. The code there is a 1-D ECSQ head plus a uniform grid on the tail's norm. This is the family to
  push further: a tail code that beats norm shells at second order, or cells that couple head and tail, is where a
  violation would have to come from.
- On flat spectra at low rate, Theorem 1 (κ_n e^(−H/n)) exceeds W and even Σ√γ at H = 0, since κ_n > n. W is therefore not
  the strongest bound there, and F2's maximum over S picks k = n.

**Not run or not finished:** F2 targeted at m = 2 with N = 400 and 2000, and all of m = 3 (stopped by the restart);
spherical-cap or polar codes in n ≥ 3; E8 and other lattices beyond D4; cells mixing head and tail beyond the 2-D sheared
lattices; F1 with mutual information in place of H(c) (for a deterministic encoder they are equal, so nothing separate
was owed); S other than top-k sets.

## 4. The measured spectra (`measured.py`, `measured.log`)

agent1/spec_v1.json groups 12 and 2 (n = 768), the spectra `agent1/theory.py` used. Bits as theory.py counts them:
B = 8·(code bytes) + 48; H = B ln 2. The two code widths are from `agent1/theory.log`: Monte Carlo on synthetic Gaussian
data with these spectra, weighted Cauchy–Schwarz certificate (an upper bound on the code's exact half-width). They are
not measurements on real data; the real-data index behind `quant/floor2.py` is gone from /tmp, so its widths could not be
set beside Φ.

Group 12 (Σ√γ = 0.0728):

| bytes | B | Thm 1 | Thm 2 | Thm 3 | Φ | Φ/Thm 3 | Φ/best proved | W (= conjectured floor = F1) | Φ/W | F2 (best k) | uniform | escape | escape/Φ | escape/W | escape/F2 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 0 | 48 | 0.0318 | 0.0092 | 0.0237 | 0.0361 | 1.52 | 1.135 | 0.0453 | 0.797 | 0.0462 (48) | 0.0724 | 0.0724 | 2.01 | 1.60 | 1.57 |
| 4 | 80 | 0.0278 | 0.0037 | 0.0186 | 0.0295 | 1.58 | 1.060 | 0.0379 | 0.778 | 0.0386 (64) | 0.0534 | 0.0539 | 1.83 | 1.42 | 1.40 |
| 8 | 112 | 0.0247 | 0.0014 | 0.0156 | 0.0250 | 1.60 | 1.014 | 0.0327 | 0.765 | 0.0332 (80) | 0.0446 | 0.0445 | 1.78 | 1.36 | 1.34 |
| 16 | 176 | 0.0204 | 0.0000 | 0.0123 | 0.0193 | 1.56 | 0.946 | 0.0258 | 0.748 | 0.0261 (108) | 0.0344 | 0.0339 | 1.76 | 1.31 | 1.30 |
| 32 | 304 | 0.0150 | 0.0000 | 0.0091 | 0.0130 | 1.43 | 0.866 | 0.0180 | 0.723 | 0.0181 (167) | 0.0238 | 0.0229 | 1.76 | 1.28 | 1.26 |
| 96 | 816 | 0.0063 | 0.0000 | 0.0038 | 0.0046 | 1.20 | 0.729 | 0.0068 | 0.674 | 0.0068 (337) | 0.0095 | 0.0087 | 1.90 | 1.28 | 1.27 |

Group 2 (Σ√γ = 0.1523):

| bytes | B | Thm 1 | Thm 2 | Thm 3 | Φ | Φ/Thm 3 | Φ/best proved | W | Φ/W | F2 (best k) | uniform | escape | escape/Φ | escape/W | escape/F2 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 0 | 48 | 0.0671 | 0.0166 | 0.0483 | 0.0739 | 1.53 | 1.102 | 0.0934 | 0.791 | 0.0955 (48) | 0.1520 | 0.1520 | 2.06 | 1.63 | 1.59 |
| 4 | 80 | 0.0577 | 0.0057 | 0.0381 | 0.0599 | 1.57 | 1.036 | 0.0775 | 0.772 | 0.0790 (64) | 0.1115 | 0.1128 | 1.88 | 1.45 | 1.43 |
| 8 | 112 | 0.0509 | 0.0015 | 0.0317 | 0.0505 | 1.59 | 0.994 | 0.0666 | 0.759 | 0.0676 (78) | 0.0928 | 0.0924 | 1.83 | 1.39 | 1.37 |
| 16 | 176 | 0.0412 | 0.0000 | 0.0249 | 0.0385 | 1.54 | 0.934 | 0.0518 | 0.742 | 0.0525 (107) | 0.0707 | 0.0694 | 1.80 | 1.34 | 1.32 |
| 32 | 304 | 0.0295 | 0.0000 | 0.0178 | 0.0254 | 1.42 | 0.860 | 0.0352 | 0.720 | 0.0356 (161) | 0.0479 | 0.0459 | 1.81 | 1.30 | 1.29 |
| 96 | 816 | 0.0115 | 0.0000 | 0.0070 | 0.0085 | 1.22 | 0.739 | 0.0126 | 0.675 | 0.0127 (320) | 0.0180 | 0.0164 | 1.93 | 1.30 | 1.29 |

Theorem 2 uses the consolidated constant 1/√2 (theory.py used 1), and T in Theorems 2 and 3 ranges over contiguous windows
of the sorted spectrum. "Thm 3 head + Φ tail", a bound that would follow from the per-cell conjecture, equals Φ at every
row and is left out.

- **Φ against Theorem 3:** Φ is stronger at every byte count from 0 to 96 on these spectra, by 1.20 to 1.60. Over all
  16 groups of spec_v1 and spec2_v2, the smallest Φ/Thm 3 between 4 and 32 bytes is 1.326. On spec2_n128 it is 1.003.
  **The stop condition is not met:** Φ beats Theorem 3 everywhere between 4 and 32 bytes.
- **Φ against the best proved bound (Theorem 1 here; Theorem 3 never leads on these spectra):** Φ is stronger only at low
  rate: by 13.5% / 10.2% at 0 bytes and 6.0% / 3.6% at 4 bytes (groups 12 / 2). It falls below Theorem 1 from 1 to
  11 bytes depending on the group, and is weaker by 13–14% at 32 bytes. On the n = 128 spectra it is below Theorem 1 at
  every byte count. At high rate Φ ≈ k√θ/√e, against Theorem 1's κ_k√θ ≥ k√θ.
- **Against W:** Φ/W is 0.62–0.80 (0 to 576 bytes). max(Φ, best proved)/W is 0.797 at 0 bytes and 0.778 at 4 bytes,
  against 0.702 and 0.733 for the best proved alone. Φ therefore narrows the gap to the conjectured floor only below
  about 10 bytes.

## Numerical safeguards worth knowing

- Interval masses use Gauss–Legendre for narrow intervals (`common.log_mass`): the naive log-survival difference loses all
  precision below width 1e-12 and produced spurious R < 1 for boxes in an early run.
- Φ is solved in log space with a bracket that adapts to H (`common.phi_alloc`). A fixed bracket overstated Φ for
  H > ~69 nats per coordinate, which would understate R. It is checked against SLSQP (`check_phi.log`, agreement 1e-12).
- W uses an O(n) closed form (`common.WFloor`), checked against bisection to 3e-14.

## Files

`common.py`, `gaussq.py`, `check_phi.py`, `lemma1d.py`, `lemma1d_mixcheck.py`, `percell.py`, `percell_run.py`, `f12.py`,
`f12_target.py`, `measured.py`, and their logs. `quantisers.log` is the empty remnant of a stopped run; `f12.py`
sections 0, A and B replace that script.

## Appendix: reruns after the restart (21 November, after 07:13)

Small foreground jobs on cores 0,1 at nice 10, each under five minutes. Peak resident memory 96–105 MB; the L1-ball run
touched 123 MB for under a second. Importing scipy alone costs 91 MB here.

**F1/F2, the three invalid rows, with `tail_norm` corrected** (`f12_target_fix.py`, `f12_target_fix.log`) [computed]:

| Head over tail | target | E h / target | H (nats) |
|---|---|---|---|
| m = 1, N = 2000, ε = 1e-4 | W | 1.002592 | 9.285 |
| m = 2, N = 20, ε = 1e-6 | W | 1.164894 | 16.566 |
| m = 2, N = 99, ε = 1e-6 | W | 1.042213 | 17.483 |

These agree with the neighbouring valid rows (same codes at other ε). In `f12_target.py` the first edge o + D is now
always a cell boundary.

**F2, cases the restart cut off** (same script) [computed]:

| Head over tail | / F2, S = head | / F2, best S (k) | / W |
|---|---|---|---|
| m = 2, N = 400, ε = 1e-4 | 1.010292 | 1.003873 (k = 402 = n) | 1.012200 |
| m = 3, N = 99, ε = 1e-4 | 1.037402 | 1.020197 (k = 102 = n) | 1.046910 |

The S = head ratio again falls with N (m = 2: 1.0331 hex at N = 99, 1.0103 at N = 400), above 1. m = 2, N = 2000 and m = 3
with N = 20, 400, 2000 were not run; the hexagonal head was not rerun.

**Per cell, Φ: sections that the restart cut off, run reduced** (`percell_run.py` with text substitutions; logs
`percell_s4.log`, `percell_s4_n20.log`, `percell_s4_n5.log`, `percell_s5.log`, `percell_s6.log`, `percell_s7.log`,
`percell_s8.log`, `percell_s9.log`) [computed]:

| Family | smallest R | where | error |
|---|---|---|---|
| aligned ellipsoids, n = 2 | 1.461141 | flat, vanishing size (limit κ₂/2 = 1.4611) | quadrature 1.5e-10 |
| aligned ellipsoids, n = 20 (flat, i⁻², measured) | 1.095744 | flat, vanishing size (= κ₂₀/20) | 1.3e-14 |
| misaligned ellipsoids, n = 20 (i⁻², 0.8ⁱ, measured; 2 rotations) | 1.272670 | measured spectrum, slight rotation | 1.3e-6 |
| thin shells, n = 1…100 | 1.028404 at n = 100 | flat; degenerates to a ball at small radius | exact |
| balls cut by a half-space or slab, n = 2, 5, 20 | 1.543 / 1.284 / 1.111 | flat | MC s.e. 0.006 / 0.003 / 0.001 (20,000 draws) |
| L1 balls, aligned and rotated, n = 2, 5, 20 | 1.612 / 1.628 / 1.916 | flat | MC s.e. 0.036 / 0.047 / 0.018 |
| convex polygons in 2-D, 3–12 vertices, 4 spectra | 1.540673 | flat, 8 vertices | quadrature ~1e-8 |

No family goes below the ball's limit κ_n/n at the same n, and none below 1. **Not finished:** aligned ellipsoids at
n = 5 (two attempts timed out in the Laplace inversion) and n = 100; misaligned ellipsoids at n = 2, 5, 100; cut balls and
L1 balls at n = 100; polygons with more than 2 starts per shape. The polygon search is short (600 iterations); with more
vertices it should approach the disc's 1.4611.

**Verdicts are unchanged:** no statement is refuted. The closest approaches remain the small ball for Φ per cell (R → 1
only as n → ∞), and for F1 and F2 the ECSQ head with a tail norm code on a long flat tail (1.0026 against W, 1.0023
against F2 with S = the head, 1.0005 against F2's maximum over S, which there is Theorem 1).
