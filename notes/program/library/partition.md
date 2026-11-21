# Library: how to partition the vectors (stage 0, partition track)

Librarian's report for the track that asks whether k-means is the right partition for an index that certifies
inner-product thresholds, written 21 November 2026. No computation was run. Symbols follow `notes/00-notation.md`:
N rows, dimension n, K clusters, G groups. Local symbols are declared where used.

How each source was read:

- **[paper §x]**: the paper's own text (arXiv HTML, PDF text, or publisher HTML) at the named place.
- **[abstract]**: only the abstract or the listing page.
- **[summary]**: a search engine's or fetch tool's summary, one step removed.
- **[cited, unchecked]**: from memory or a secondary mention I could not open.

What the index does with a partition (from `SYSTEM.md` §2.1–2.2), so the reader can weigh each source: spherical
k-means into K ≈ N/1000 clusters; the residual r = x − m against the stored centre; a 16-bit key x·m̂ per row; a
support table of exact extremes of anchor·x over each cluster, used to accept or skip whole clusters; clusters
grouped by k-means on centre directions into G groups, each with one basis fitted to the group's residual
covariance and a query-offset covariance. The partition therefore matters in four places: the cut (the cluster's
extent along anchors and the largest residual norm), the key range (how many rows a key cannot decide), the
width of every later bound (Σ_R per group, through F = Σ √γᵢ), and setup cost per opened cluster and group.

(Sections follow as they are written.)
