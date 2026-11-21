# Codes track: closing the gap to the sphere bound

Written by the PI from the codes researcher's hand-back of 21 November 2026 (the agent could not write this file).
Every table is in `NOTEBOOK.md`, entries 2 to 11, and in the `out_*.txt` files beside it; the scripts are listed there.
Simulations are on uniform directions, which is exact for a rotated block whatever the data. "tan" is the root of the
mean of tan²φ over items. Nothing was timed.

## Verdict

**The stop condition is not met as a drop-in, so the track is closed as a change to the index.** With the schedule,
the nesting and the decode kept, the best package narrows the last level by 8.2% (6.6% with a 16-state encoder), below
the 10% bar. [estimate from measured tan ratios] Three things are kept:

1. **A low-priority change to measure natively: 24 encoder scales in place of six.** It narrows the 6- and 7-bit rises
   by 6 to 8%. [measured] The scales are the encoder's trial values (`LAMS`), so nothing more is stored or read at
   query time, but the change is not free: the encode loop is four times longer, and the stored κ grid under a wider
   range of scales has not been checked. It is worth about one point of the last level's width. (Corrected 21 November
   2026; an earlier version of this line called the change free.)
2. **The nesting loss, which is the track's result.** At one rate, codes with today's decode (one 16-entry lookup per
   nibble) gain 13 to 16% of width at 2 bits, 12% at 4 bits and 15 to 17% below one bit. Nested across the schedule,
   a table-additive trellis keeps 6 to 8% and E8 keeps nothing. The frontier between rises is steep: each further 1%
   at the 2-bit rise costs about 3% at the 1-bit rise. No published figure for the loss from nesting such codes was
   found. [measured]
3. **A rule for a redesigned schedule.** A block whose last rise is its only rise can use the one-rate codes, and the
   16-word codebook below one bit is the cheapest of them.

## Where the built code stands [measured, `built.py`, 3,000 directions per block size]

| Block | Rise (bits) | Built tan | Bound | Built / bound | Of which nesting |
|---|---|---|---|---|---|
| 96 | 2 | 0.4195 | 0.2582 | 1.625 | 1.157 |
| 96 | 7 | 0.0138 | 0.0078 | 1.767 | 1.047 |
| 128 | 3 | 0.1951 | 0.1260 | 1.549 | 1.018 |
| 256 | 1 | 0.7547 | 0.5774 | 1.307 | 1.000 |
| 256 | 2 | 0.3653 | 0.2582 | 1.415 | 1.000 |
| 256 | 1/4 | 2.3279 | 1.5538 | 1.498 | 1.000 |

The three losses at one rate on a long block, as tan over the bound: scalar against vector 1.19 to 1.27 at every rate;
fixed rate against entropy-coded adds 3% at 1 bit, 17% at 2 and 37% at 7. Where the variance sits (the block of 256
at 1 and 2 bits) the gap is 1.31 to 1.42 and none of it is nesting.

The bound is a cap bound [proved, unrefereed]: for any M points and y uniform on the sphere,
Pr[φ ≤ θ] ≤ min(1, M·Ω(θ)), Ω the normalised cap measure. Its finite-dimension correction is 1 to 2% at 1 bit.

## Published against ours

Published (see `../library/codes-and-search.md`): a Lloyd codebook per block of coordinates after a rotation
(BlockQuant), E8 with a ball-shaped codebook (QuIP#), nested E8 (NestQuant), the sliding-window trellis (Mao, Gray and
Linder; QTIP). The simulation reproduces their single-rate figures to about 1%.

Not found in print: a trellis whose window table is a sum over aligned nibbles, so that an inner product is one query
table lookup per nibble; nested, prefix-readable trellis and lattice codes with an exact joint search; the loss from
nesting them; the stored scalars under such codes.

## What it would be worth, as built [estimate]

About 7% fewer full-vector reads, 3% of bytes and 1% of time at large answers. For scale, a deeper last level cut full
reads by 42 to 63% at a cost in bytes and time, and the constants work was worth 4%.

## Limits

Variance shares come from an older build and only group 0's rates are logged. The trained codes were tested at one
scale per item. Barnes–Wall, Leech, a long sliding-window trellis and the rate design with the new tan² table were not
run. BlockQuant was read through a summary of its HTML.
