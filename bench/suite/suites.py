# The suites: datasets x query types x filters x systems x parameters, as plain dicts.
#
# Query types: "approx" (approximate k-NN), "exact" (exact k-NN) and "threshold" (every item with q.x >= t, t set so
# that a given share of the corpus, or of the visible set, answers). Filtering is an axis across all three: each suite
# lists its filters ("none" and visible sets), and `filtered` gives the parameters run under a visible set.
# Unfiltered approximate k-NN runs on the public suites (VIBE, ANN-Benchmarks) unchanged, through `public`; this
# runner runs approximate k-NN under the visible sets only (VIBE and ANN-Benchmarks have no filters).
# Systems are named by systems/__init__.py's registry; per type, `systems` names the standard, the frontier and ours.
# `params` holds each system's build and query parameters; `only` limits a system (or a parameter set) to datasets.

FILTERS = {
    # random: a uniform random subset of the ids; region: rows whose nearest k-means centre (of `centres`, fitted on a
    # sample) lies in a random subset of the centres, whole centres added until the next would overshoot, then filled
    # to the share from that centre's rows (a correlated set: the visible items sit together in the space)
    "random-1": dict(kind="random", share=0.01),
    "random-10": dict(kind="random", share=0.10),
    "random-25": dict(kind="random", share=0.25),
    "random-50": dict(kind="random", share=0.50),
    "region-1": dict(kind="region", share=0.01, centres=300),
    "region-25": dict(kind="region", share=0.25, centres=300),
}

SYSTEM_PARAMS = {
    # the index through bench/annb/vexact-py, built as bench/annb/vexact/module.py builds it (fit.py --basis dual, with
    # --query-sample from the learn sample on out-of-distribution sets)
    "vexact": dict(basis="dual", approx=[dict(nprobe=p, eps=0.0) for p in (16, 64, 256)]),
    # faiss IndexFlatIP: search / range_search; under a visible set through IDSelectorBitmap and over a compacted copy
    "faiss-flat": dict(settings=["idselector", "compacted"]),
    # faiss IndexFlatIPPanorama, 8 levels, after an orthogonal PCA rotation (as ../threshold-db/ann.py)
    "faiss-panorama": dict(levels=8, settings=["idselector"]),
    # PDX-BOND (cwida/PDX at the last revision with BOND): exact pruned k-NN over the PDX layout, no index
    "pdx-bond": dict(settings=["selection"]),
    # faiss HNSW (M 16, efConstruction 200, as ../threshold-db) with IDSelectorBitmap
    "faiss-hnsw": dict(M=16, efConstruction=200, approx=[dict(ef=e) for e in (64, 256, 1024)]),
    # ACORN-gamma (stanford-futuredata/ACORN with ../threshold-db/acorn.patch), the README's M 32, gamma 12, M_beta 32
    "acorn": dict(M=32, gamma=12, M_beta=32, approx=[dict(ef=e) for e in (64, 256, 1024)]),
    # PostgreSQL + pgvector, sequential scans (no vector index), timed at the client
    "pgvector": dict(),
}

# The quick grids for the public suites: each algorithm's run_groups, in that suite's own config.yml format, replace
# the checkout's while the driver runs (public.py), and the checkout's file is restored after.
VIBE_QUICK = {
    "algos": ["vexact-approx", "hnswlib", "ivfpqfs(faiss)", "glass"],
    "overlay": {
        "vexact-approx": {"approx": {"args": {"basis": ["dual"]},
                                     "query_args": {"nprobe": [4, 16, 64, 256], "eps": [0, 0.03]}}},
        "hnswlib": {"base": {"args": {"M": [16, 36], "efConstruction": [500]},
                             "query_args": {"ef": [10, 30, 100, 300, 1000]}}},
        "ivfpqfs(faiss)": {"base-1024": {"args": {"n_list": [1024], "dim_reduction": [0]},
                                         "query_args": {"n_probe": [4, 16, 64], "k_factor": [4, 16]}}},
        "glass": {"glass": {"args": {"L": [400], "R": [32], "quant": ["SQ8U"], "search_quant": ["SQ8U"],
                                     "refine_quant": ["FP16"]},
                            "query_args": {"ef": [20, 50, 100, 300, 1000]}}},
    },
}
ANNB_QUICK = {
    "algos": ["vexact-approx", "hnswlib", "faiss-ivfpqfs", "glass"],
    "overlay": {
        "vexact-approx": {"approx": {"args": [{"basis": "dual"}],
                                     "query_args": [[4, 16, 64, 256], [0, 0.03]]}},
        "hnswlib": {"M-16": {"arg_groups": [{"M": 16, "efConstruction": 500}], "args": {},
                             "query_args": [[10, 40, 120, 400, 800]]}},
        "faiss-ivfpqfs": {"base": {"args": [[2048]], "query_args": [[5, 10, 50], [10, 100]]}},
        "glass": {"HNSW": {"args": {"L": 200, "R": [32], "level": [2]},
                           "query_args": [[10, 30, 80, 200, 600]]}},
    },
}

QUICK = dict(
    name="quick",
    datasets=["glove-100-angular", "yahoo-minilm-384-normalized", "laion-clip-512-normalized",
              "inaturalist-resnet-2048-cosine", "bioclip2-768"],
    nq=100,
    reps=5,
    types=dict(exact=dict(k=[10, 100]), threshold=dict(shares=[1e-4, 1e-2, 1e-1]), approx=dict(k=[10])),
    filters=["none", "random-1", "random-25"],
    # under a visible set: exact k-NN k = 10, threshold at 1% of the visible set, approximate k-NN k = 10
    filtered=dict(exact=dict(k=[10]), threshold=dict(shares=[1e-2]), approx=dict(k=[10])),
    systems=dict(
        exact=dict(ours=["vexact"], standard=["faiss-flat"], frontier=["pdx-bond", "faiss-panorama"]),
        threshold=dict(ours=["vexact"], standard=["faiss-flat"], frontier=["faiss-panorama"]),
        approx=dict(ours=["vexact"], standard=["faiss-hnsw"], frontier=["acorn"]),
    ),
    params=SYSTEM_PARAMS,
    only={},
    overrides={},
    public=dict(vibe=VIBE_QUICK, annb=ANNB_QUICK,
                datasets={"vibe": ["yahoo-minilm-384-normalized", "laion-clip-512-normalized",
                                   "inaturalist-resnet-2048-cosine"],
                          "annb": ["glove-100-angular"]}),
)

FULL_VIBE = ["yahoo-minilm-384-normalized", "glove-200-cosine", "imagenet-clip-512-normalized",
             "arxiv-nomic-768-normalized", "landmark-dino-768-cosine", "inaturalist-resnet-2048-cosine",
             "laion-clip-512-normalized", "imagenet-align-640-normalized", "yandex-200-cosine"]

FULL = dict(
    name="full",
    datasets=FULL_VIBE + ["glove-100-angular", "nytimes-256-angular"],
    nq=100,
    reps=5,
    types=dict(exact=dict(k=[1, 10, 100, 1000]), threshold=dict(shares=[1e-4, 1e-3, 1e-2, 1e-1]),
               approx=dict(k=[10, 100])),
    filters=["none", "random-1", "random-10", "random-25", "random-50", "region-1", "region-25"],
    filtered=dict(exact=dict(k=[10, 100]), threshold=dict(shares=[1e-3, 1e-2]), approx=dict(k=[10])),
    systems=dict(
        exact=dict(ours=["vexact"], standard=["faiss-flat"], frontier=["pdx-bond", "faiss-panorama"],
                   database=["pgvector"]),
        threshold=dict(ours=["vexact"], standard=["faiss-flat"], frontier=["faiss-panorama"],
                       database=["pgvector"]),
        approx=dict(ours=["vexact"], standard=["faiss-hnsw"], frontier=["acorn"]),
    ),
    params=dict(SYSTEM_PARAMS,
                vexact=dict(SYSTEM_PARAMS["vexact"],
                            approx=[dict(nprobe=p, eps=e) for p in (8, 32, 128, 512) for e in (0.0, 0.03)]),
                **{"faiss-hnsw": dict(SYSTEM_PARAMS["faiss-hnsw"], approx=[dict(ef=e) for e in (32, 128, 512, 2048)]),
                   "acorn": dict(SYSTEM_PARAMS["acorn"], approx=[dict(ef=e) for e in (32, 128, 512, 2048)])}),
    # the database runs on two datasets only
    only={"pgvector": ["yahoo-minilm-384-normalized", "laion-clip-512-normalized"]},
    # the large answer shares on two datasets
    overrides={d: dict(threshold=dict(shares=[1e-4, 1e-3, 1e-2, 1e-1, 0.3, 0.5, 0.75]))
               for d in ("yahoo-minilm-384-normalized", "laion-clip-512-normalized")},
    # the public suites' own grids, unchanged (no overlay)
    public=dict(vibe=dict(algos=None, overlay={}), annb=dict(algos=None, overlay={}),
                datasets={"vibe": FULL_VIBE, "annb": ["glove-100-angular", "nytimes-256-angular"]}),
)

SUITES = {"quick": QUICK, "full": FULL}


def types_for(suite, dataset):
    """The query types' parameters on one dataset, with the suite's overrides."""
    t = {k: dict(v) for k, v in suite["types"].items()}
    for k, v in suite["overrides"].get(dataset, {}).items():
        t[k].update(v)
    return t


def cells(suite, dataset, qtype):
    """(filter, parameter list) for one type on one dataset: unfiltered with the type's parameters (none for approx:
    the public suites run it), and each visible set with the `filtered` parameters."""
    t = types_for(suite, dataset)[qtype]
    out = []
    for f in suite["filters"]:
        if f == "none":
            if qtype != "approx":
                out.append(("none", t))
        else:
            out.append((f, suite["filtered"][qtype]))
    return out


def systems_for(suite, dataset, qtype):
    """[(role, system)] for one type on one dataset."""
    out = []
    for role, names in suite["systems"][qtype].items():
        for s in names:
            if s in suite["only"] and dataset not in suite["only"][s]:
                continue
            out.append((role, s))
    return out


def truth_needs(suite, dataset):
    """What prep must compute: the unfiltered k-NN depth and answer shares, and per filter its own."""
    t = types_for(suite, dataset)
    ks = sorted(set(t["exact"]["k"]) | set(t["approx"]["k"]))
    shares = sorted(set(t["threshold"]["shares"]))
    fk = sorted(set(suite["filtered"]["exact"]["k"]) | set(suite["filtered"]["approx"]["k"]))
    fs = sorted(set(suite["filtered"]["threshold"]["shares"]))
    filters = {f: dict(FILTERS[f], k=fk, shares=fs) for f in suite["filters"] if f != "none"}
    return dict(k=ks, shares=shares, filters=filters)
