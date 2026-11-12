# Exact threshold vector search: research

Research into exact, filter-aware vector search at billion scale: return every item whose
similarity to a query meets a threshold (and, from the same index, the exact k nearest), computed
only over the items a viewer may see, reading a small fraction of the data. It started as a
possible feature for Mosaica and may become its own project.

This branch holds no Mosaica code and shares no history with it.

## Start here

- [`DESIGN.md`](DESIGN.md) — the design as it stands: requirements, what is stored, query paths,
  write path, measured costs, open problems, what was ruled out.
- [`FINDINGS.md`](FINDINGS.md) — the original problem statement and metric, the early
  measurements, the dead list with reasons (section 7), and the owner's decisions (section 10).
- [`proofcheck/CONSOLIDATED_PROOFS.md`](proofcheck/CONSOLIDATED_PROOFS.md) — refereed proofs of
  the certification floors, level-by-level optimality and the randomised guarantees.
  [`proofcheck/REFEREE.md`](proofcheck/REFEREE.md) is the referee report;
  [`PROOFS.md`](PROOFS.md) the proofs as originally submitted.

## Layout

| Path | What |
|---|---|
| `exp*.py`, `two.py`, `wide.py`, `mih.py`, `kurt.py`, `report*.py`, `results*.json` | Early experiments: cluster pruning, codes, bounds, tolerance, two codes, wide sparse features, multi-index hashing |
| `agent1/` | Tightening the code: escape code, floor theorems, nested streams |
| `agent2/` | Real text queries (BioCLIP-2 text tower) and the split-bound allocation |
| `agent3a/` | Guarantees per bit: adaptivity, class-uniform bounds, the deterministic floor, sub-bit sketches |
| `agent3b/` | Reading less: entropy lower bound, sorted ranges, the support table, LSH |
| `combined/` | The combined design built once and measured under one accounting, with an ablation |
| `entropy/` | How far real embeddings are from Gaussian, across six datasets |
| `knn/` | k-nearest-neighbour on the same index, against HNSW and IVF-PQ, filtered and unfiltered |
| `proofcheck/` | Referee report, consolidated proofs, numerical checks |
| `site/` | Builds the published proofs page from `proofcheck/CONSOLIDATED_PROOFS.md` (`python3 build.py` in `site/`) |

Each directory's `REPORT.md` summarises its results; the scripts and logs beside it are the
evidence. Reports from agents were saved from their returned text and are condensed.

## Data (not committed)

The scripts expect data that is not in this branch:

- BioCLIP-2 image embeddings (768 dimensions): `imageomics/TreeOfLife-200M-Embeddings` on Hugging
  Face, folder `bioclip-2_float16`; `fetch.py` takes 20 random row groups (1M vectors) into `data/`.
- BioCLIP-2.5 (1,024 dimensions): same dataset, `agent3b/fetch1024.py`.
- BioCLIP-2 text tower and prompts: `agent2/fetch_text_tower.py`, `agent2/encode_text.py`.
- GloVe-100 and NYTimes-256: `https://ann-benchmarks.com/` HDF5 files.
- OpenAI DBpedia embeddings: `KShivendu/dbpedia-entities-openai-1M` and a Qdrant repository on
  Hugging Face (`entropy/scripts/fetch*.sh`).

Scripts use absolute paths from the machine they ran on; adjust them to rerun.

## Status

Every figure is a byte count or a model from numpy prototypes; nothing has been timed natively.
Work in progress: a rework of the per-item code, an end-to-end review, then a standalone Rust
prototype for real timings.
