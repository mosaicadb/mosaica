# Reports

The mathematical and design studies behind the index, with the scripts that produced each one. Each report says
which of its claims are proved, refereed, measured or only sketched.

- `bounds3/` sets the confidence constants for the randomised levels. It certifies the worst case over block
  shares, gives a closed-form fallback, proves the harmful-side budget and the counts that keep k-NN sound, and
  shows why the union over levels cannot be shortened. `bounds3/referee/` holds the independent referee's verdicts
  and checks.
- `surge/` holds the threshold-set constants (`bounds2.txt`), the first referee report on the randomised levels
  (`referee.txt`), the integer decision kernel, the level-0 design and the stage-2 design.
- `surge2/` holds the approximate-mode design, the flat cascade specification, the literature check, a red-team
  review of the benchmarks and the write path at scale.
- `clusters/` covers probing order, bound-ordered search and trees over centres. `ideas/` holds the first round of
  speed proposals, and `yfcc/` the filtered-track notes.
- `../threshold/` measures threshold sets on BioCLIP-2 and GloVe: the mixed rule, constants and exact reads.
