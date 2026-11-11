"""Write the notebook corpus and print the directory it is in.

```bash
python -m corpora.notebook                    # into the temporary directory, once
python -m corpora.notebook --out data/notebook
```
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from . import PAPERS, SEED, cached, write


def main() -> None:
    ap = argparse.ArgumentParser(
        prog="python -m corpora.notebook", description=__doc__.split("\n\n")[0]
    )
    ap.add_argument("--out", type=Path, help="write here rather than into the temporary directory")
    ap.add_argument("--papers", type=int, default=PAPERS)
    ap.add_argument("--seed", type=int, default=SEED)
    args = ap.parse_args()
    try:
        import numpy  # noqa: F401
        import pyarrow  # noqa: F401
    except ImportError as missing:
        sys.exit(f"{missing.name} is not installed, and the notebook corpus is written with it: "
                 f"run `{Path(sys.executable).name} -m pip install numpy pyarrow`")
    if args.out:
        args.out.mkdir(parents=True, exist_ok=True)
        write(args.out, papers=args.papers, seed=args.seed)
        print(args.out)
    else:
        print(cached(args.papers, args.seed))


main()
