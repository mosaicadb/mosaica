# Library: cheap structured rotations in place of Haar (stage 0)

Librarian's report for the confidence and codes tracks, written 21 November 2026. The question is whether the dense
Haar rotation of each randomised block can be replaced by a cheap structured one while a failure probability near
10⁻¹⁹ still holds for every input. No computation was run. Symbols follow `notes/00-notation.md`.

How each source was read:

- **[paper §x]**: I read the paper's own text (arXiv HTML, a PDF text extraction or a publisher page) at the named place.
- **[abstract]**: only the abstract or the listing page.
- **[summary]**: a search engine's or fetch tool's summary, one step removed from the text.
- **[cited, unchecked]**: from memory or from a secondary mention; I could not open the source.

A fetch tool returns a paraphrase produced by a small model; statements tagged [paper §x] were asked for verbatim and
are given as returned, but a theorist relying on a constant should re-open the source at the named place.

## 0. What property is needed

Lemma 3.1 (chapter 3 §3.2) uses Haar invariance in one step: for O fixing y, R·Oᵀ is again Haar. What the guarantee
consumes is the joint law of the pair (Ry, RΔ) for a fixed item block y and a fixed query weight Δ, both chosen without
knowledge of ω: the code is a function of Ry, and the estimator is linear in RΔ given the code. So a replacement
distribution μ on O(n) suffices for the per-event bound if and only if, for every fixed pair (y, Δ), the law of
(Ry, RΔ) under μ equals its law under Haar, or is close to it in a sense strong enough to move a probability of
2.5·10⁻¹⁹ by a small factor (librarian's reduction, not from a source).

Two consequences follow from the reduction, and the sections below test the literature against them.

- **Closeness must be multiplicative, not additive.** Total-variation distance τ only gives |Pr_μ(F) − Pr_Haar(F)| ≤ τ,
  which is useless unless τ ≲ 10⁻¹⁹. What would serve is a density ratio dμ_pair/dHaar_pair ≤ 1 + η on the events
  that matter (a relative-error or L^∞ bound), or τ itself below 10⁻²⁰.
- **The function is not a polynomial.** The code is a quantised function of Ry, so moment matching (designs) controls
  the event only through a polynomial approximation of an indicator.

