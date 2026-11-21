# Partition track: notebook

Researcher: the partition theorist. Reports to the PI. Started 21 November 2026, 07:57.

The question: LIMINAL groups its rows by spherical k-means for filtering. Is that the right thing to group by, and what
are the alternatives? Claims go to `CLAIMS.md` as they are reached; this file is the dated log.

## 21 November 2026

- 07:57. Read `notes/README.md` (tags) and `notes/00-notation.md` (symbols). Directory created. Next: check what is
  built in the source (`rust/tools/fit.py`, `SYSTEM.md` sections 1 to 2), then the floor (`floors/CLAIMS.md`, "For the
  notes"), then the measured rules.
- 08:10. What is built, checked in the source:
  - `rust/tools/fit.py` lines 201 to 223: `kmeans` (`combined/core.py` line 11: 12 Lloyd iterations of spherical
    k-means from random rows, on a 200,000-row sample; empty centres re-seeded), then every row to the centre of
    largest inner product. No balancing: measured sizes at 1M with 1,000 a cluster are 84 to 4,635
    (`rust/REPORT_CLUSTERSIZE.md`, "Fits"), with 4,000 a cluster 579 to 15,490. The stored centre is the mean of the
    assigned rows (not normalised), rounded to int8 with one scale per centre; the residual is against the stored centre.
  - Rows per cluster: `rules.py::per_cluster`, 2,000·√(N/1M) rounded to a power-of-two multiple of 2,000, so 2,000 up
    to 2M and 64,000 at 1B. (The brief said 1,000 to 4,000; 1,000 was the earlier default.)
  - Groups: `kmeans(Mh, G, rng, 20)` on the unit centre directions, G = min(64, K, N/(20n)). One dual basis per group.
  - Σ_Q is ONE matrix for the whole index (`sigma_q`: offsets q − (q·m̂)m̂ of 3,000 pseudo-queries from 40 clusters each,
    drawn by size, ridge 0.1). Only Σ_R is per group. So the dual metric differs between groups only through Σ_R.
  - Rows are laid out in (group, cluster) order; within a cluster by the 16-bit key x·m̂ (the Rust build sorts).
- 08:15. Read the floor (`floors/CLAIMS.md` N.0 to N.6, F5, B4b to B4d), `groups/REPORT.md` (all),
  `rust/REPORT_CLUSTERSIZE.md` (all), `hierarchy/layered.md` section 8 (all). Numbers I will use:
  - rms residual 0.3765 / 0.3509 / 0.3288 / 0.3034 at K = 1,000 / 3,997 / 15,880 / 58,750 on BioCLIP-2 1M: the rms falls as
    K^(−0.053), so the mean square falls as K^(−0.106), which is K^(−2/D) with D ≈ 19 (`groups/REPORT.md` 3.2).
  - 16 times more cells save a text query 4.5 of 45 coordinates and cost 11.4 in centres (same table).
  - k-means in the M⁻¹ norm: coordinates read change by under 1%, the sorted range by under 1 point, the group ball by
    3 to 13 points (3.4).
  - Tightness against G: 8 times fewer bases widen the median certified half-width by 6% at 16 B (layered 8.8), so
    about 2% per doubling; code bytes 2.2% (LAION) to 3.7% (yahoo) per halving of G; the PCA proxy said 8 to 12%.
  - κ: rows per basis = κn², κ of 0.5 to 1.1 by counts and time (8.7 to 8.10); today's 0.06 to 0.07 at n = 384 to 512.
  - A group's setup: 60 to 130 µs, n² bytes. Cut 3 µs a cluster; an opened cluster 8 to 17 µs at 1,000 rows.
