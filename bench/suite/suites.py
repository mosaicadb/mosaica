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
    # --query-sample from the learn sample on out-of-distribution sets), with the library's default levels (16, 32 B;
    # dlv=[] for the fit's six)
    "vexact": dict(basis="dual", approx=[dict(psi=x) for x in (3.5, 4.5, 6.0)]),
    # faiss IndexFlatIP: search / range_search; under a visible set through IDSelectorBitmap and over a compacted copy
    "faiss-flat": dict(settings=["idselector", "compacted"]),
    # faiss IndexFlatIPPanorama, 8 levels, after an orthogonal PCA rotation (as the earlier threshold-db/ann.py)
    "faiss-panorama": dict(levels=8, settings=["idselector"]),
    # PDX-BOND (cwida/PDX at the last revision with BOND): exact pruned k-NN over the PDX layout, no index
    "pdx-bond": dict(settings=["selection"]),
    # faiss HNSW (M 16, efConstruction 200, as ../threshold-db) with IDSelectorBitmap
    "faiss-hnsw": dict(M=16, efConstruction=200, approx=[dict(ef=e) for e in (64, 256, 1024)]),
    # ACORN-gamma (stanford-futuredata/ACORN with ../threshold-db/acorn.patch), the README's M 32, gamma 12, M_beta 32
    "acorn": dict(M=32, gamma=12, M_beta=32, approx=[dict(ef=e) for e in (64, 256, 1024)]),
    # PyNNDescent (unfiltered only: its query takes no visible set), metric dot over the unit vectors; epsilon from
    # ANN-Benchmarks' grid for it, extended to 0.5 to reach high recall
    "pynndescent": dict(n_neighbors=30, approx=[dict(epsilon=e) for e in (0.0, 0.04, 0.08, 0.12, 0.16, 0.24, 0.32, 0.4, 0.5)]),
    # PostgreSQL + pgvector, sequential scans (no vector index), timed at the client
    "pgvector": dict(),
    # under a memory cap: faiss flat opened memory-mapped (the scan baseline), faiss IVF with on-disk lists, DiskANN
    # a mapped scan reads the whole file per query once it no longer fits the cache (seconds a query): 5 queries, once
    "faiss-flat-mmap": dict(storage="f32", cap_step=20, cap_reps=1),
    "faiss-ivf-disk": dict(storage="f16", approx=[dict(nprobe=p) for p in (16, 64, 256)]),
    "diskann": dict(R=64, L=100, pq_fraction=0.05, approx=[dict(L=l) for l in (20, 50, 100, 200)]),
}

# The memory-cap axis (capmem.py): caps are fractions of the dataset's size, N x d x 4 bytes (its f32 vectors as
# distributed), plus "uncapped" (the same capped code path with no limit). Every system on a dataset gets the same cap.
# Systems that keep their data on files (disk = True) build once and open their index under each cap; an in-memory
# system is built inside each capped process and the cap set after, so a cap below what it holds is refused (or ends
# in an OOM kill) and reported as not fitting.
CAP_SYSTEMS = dict(
    exact=dict(ours=["vexact"], standard=["faiss-flat-mmap"], database=["pgvector"]),
    threshold=dict(ours=["vexact"], standard=["faiss-flat-mmap"], database=["pgvector"]),
    approx=dict(ours=["vexact"], standard=["faiss-ivf-disk"], frontier=["diskann"], **{"in memory": ["faiss-hnsw"]}),
)
IN_MEMORY = ["faiss-flat", "faiss-panorama", "pdx-bond", "faiss-hnsw", "acorn"]

# The quick grids for the public suites: each algorithm's run_groups, in that suite's own config.yml format, replace
# the checkout's while the driver runs (public.py), and the checkout's file is restored after.
VIBE_QUICK = {
    "algos": ["vexact-approx", "hnswlib", "ivfpqfs(faiss)", "glass"],
    "overlay": {
        "vexact-approx": {"approx": {"args": {"basis": ["dual"]},
                                     "query_args": {"psi": [3.5, 4.5, 6]}}},
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
                                     "query_args": [[3.5, 4.5, 6]]}},
        "hnswlib": {"M-16": {"arg_groups": [{"M": 16, "efConstruction": 500}], "args": {},
                             "query_args": [[10, 40, 120, 400, 800]]}},
        "faiss-ivfpqfs": {"base": {"args": [[2048]], "query_args": [[5, 10, 50], [10, 100]]}},
        "glass": {"HNSW": {"args": {"L": 200, "R": [32], "level": [2]},
                           "query_args": [[10, 30, 80, 200, 600]]}},
    },
}

QUICK = dict(
    name="quick",
    approx_unfiltered=True,
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
        approx=dict(ours=["vexact"], standard=["faiss-hnsw", "pynndescent"], frontier=["acorn"]),
    ),
    params=dict(SYSTEM_PARAMS, vexact=dict(SYSTEM_PARAMS["vexact"], orders=["spread", "adaptive"],
                                         approx=[dict(nprobe=p) for p in (4, 8, 16, 64, 256)],
                                         adaptive=[dict(psi=x) for x in (2, 2.5, 3, 3.5, 4, 4.25, 4.5, 4.75, 5, 5.5, 6, 7)])),
    only={},
    overrides={},
    caps=dict(datasets=["yahoo-minilm-384-normalized", "laion-clip-512-normalized"], fractions=[0.5, 0.25, 0.1],
              systems={t: {r: [x for x in v if x != "pgvector"] for r, v in d.items()} for t, d in CAP_SYSTEMS.items()},
              types=dict(exact=dict(k=[10]), threshold=dict(shares=[1e-4, 1e-2, 1e-1]), approx=dict(k=[10])),
              filters=["none", "random-1"],
              filtered=dict(exact=dict(k=[10]), threshold=dict(shares=[1e-2]), approx=dict(k=[10])),
              step=5, reps=5, in_memory=IN_MEMORY),
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
        approx=dict(ours=["vexact"], standard=["faiss-hnsw", "pynndescent"], frontier=["acorn"]),
    ),
    params=dict(SYSTEM_PARAMS,
                vexact=dict(SYSTEM_PARAMS["vexact"],
                            approx=[dict(psi=x) for x in (3.0, 4.0, 4.5, 5.0, 6.0, 7.0)]),
                **{"faiss-hnsw": dict(SYSTEM_PARAMS["faiss-hnsw"], approx=[dict(ef=e) for e in (32, 128, 512, 2048)]),
                   "acorn": dict(SYSTEM_PARAMS["acorn"], approx=[dict(ef=e) for e in (32, 128, 512, 2048)])}),
    # the database runs on two datasets only
    only={"pgvector": ["yahoo-minilm-384-normalized", "laion-clip-512-normalized"]},
    # the large answer shares on two datasets
    overrides={d: dict(threshold=dict(shares=[1e-4, 1e-3, 1e-2, 1e-1, 0.3, 0.5, 0.75]))
               for d in ("yahoo-minilm-384-normalized", "laion-clip-512-normalized")},
    caps=dict(datasets=FULL_VIBE + ["glove-100-angular", "nytimes-256-angular"], fractions=[0.5, 0.25, 0.1],
              systems=CAP_SYSTEMS,
              types=dict(exact=dict(k=[10, 100]), threshold=dict(shares=[1e-4, 1e-3, 1e-2, 1e-1]), approx=dict(k=[10])),
              filters=["none", "random-1", "random-25", "region-1"],
              filtered=dict(exact=dict(k=[10]), threshold=dict(shares=[1e-2]), approx=dict(k=[10])),
              step=5, reps=5, in_memory=IN_MEMORY),
    # the public suites' own grids, unchanged (no overlay)
    public=dict(vibe=dict(algos=None, overlay={}), annb=dict(algos=None, overlay={}),
                datasets={"vibe": FULL_VIBE, "annb": ["glove-100-angular", "nytimes-256-angular"]}),
)

# The LAION scale suite: speed only. CLIP ViT-B/32 image embeddings of LAION-400M (512 dimensions; part 0 is VIBE's
# laion-clip-512 corpus) at 10M, 40M and 100M, nested and stratified by part (datasets.py, prep_scale.py), with text
# embeddings as out-of-distribution queries. No answer dumps and no check stage: thresholds are estimated from a sample
# of the corpus (the answer sizes returned are recorded), and the exact top 100 is computed in a streaming pass for
# `truth_nq` of the queries, for the recall of approximate k-NN and an agreement count for exact k-NN. Only f16 vectors
# are stored, so faiss runs on f16 (QT_fp16) at scale. The scans see few queries and one repetition.
SCALE_SYSTEMS = dict(
    exact=dict(ours=["vexact"], standard=["faiss-flat-mmap"]),
    threshold=dict(ours=["vexact"], standard=["faiss-flat-mmap"]),
    approx=dict(ours=["vexact"], standard=["faiss-ivf-disk"], frontier=["diskann"], **{"in memory": ["faiss-hnsw"]}),
)


def scale_suite(name, datasets):
    return dict(
        name=name, scale=True, datasets=datasets, nq=100, truth_nq=30, reps=3,
        types=dict(exact=dict(k=[10, 100]), threshold=dict(shares=[1e-5, 1e-4, 1e-3]), approx=dict(k=[10])),
        filters=["none", "random-1"],
        filtered=dict(exact=dict(k=[10]), threshold=dict(shares=[1e-3]), approx=dict(k=[10])),
        # approximate k-NN runs unfiltered here too: the public suites do not run these sizes
        approx_unfiltered=True,
        systems=SCALE_SYSTEMS,
        params=dict(SYSTEM_PARAMS,
                    vexact=dict(SYSTEM_PARAMS["vexact"], disk_mode=True),
                    **{"faiss-flat-mmap": dict(storage="f16", step=10, reps=1, cap_step=20, cap_reps=1),
                       "faiss-ivf-disk": dict(storage="f16", approx=[dict(nprobe=p) for p in (16, 64, 256)]),
                       "faiss-hnsw": dict(SYSTEM_PARAMS["faiss-hnsw"], storage="f16")}),
        only={}, overrides={}, public=dict(datasets={"vibe": [], "annb": []}),
        # the optional cap axis (synthetic; README), on the smallest test size only
        caps=dict(datasets=[d for d in datasets if d == "laion-s200k"], fractions=[0.5, 0.25, 0.1],
                  systems={t: {r: [x for x in v if x != "pgvector"] for r, v in d.items()} for t, d in CAP_SYSTEMS.items()},
                  types=dict(exact=dict(k=[10]), threshold=dict(shares=[1e-4]), approx=dict(k=[10])),
                  filters=["none", "random-1"],
                  filtered=dict(exact=dict(k=[10]), threshold=dict(shares=[1e-3]), approx=dict(k=[10])),
                  step=5, reps=3, in_memory=IN_MEMORY),
    )


SCALE = scale_suite("scale", ["laion-10m", "laion-40m", "laion-100m"])
# the same code on a small nested sample (2 parts: 200k in 500k in 1M), to test the scale path end to end
SCALE_TEST = scale_suite("scale-test", ["laion-s200k", "laion-s500k", "laion-s1m"])

# The approximate mode (notes/visit-order/PLAN.md): the quick suite's public datasets, approximate k-NN only,
# unfiltered and under random and correlated (region) visible sets; vexact alone, since the comparison is between its
# arms: the library's approximate k-NN (`adaptive`, over psi) and the legacy `spread` order over nprobe.
ORDER = dict(QUICK, name="order", datasets=["glove-100-angular", "yahoo-minilm-384-normalized", "laion-clip-512-normalized"],
             filters=["none", "random-1", "random-25", "region-1", "region-25"],
             types=dict(QUICK["types"], approx=dict(k=[10])), approx_unfiltered=True,
             systems=dict(QUICK["systems"], approx=dict(ours=["vexact"])),
             params=dict(SYSTEM_PARAMS, vexact=dict(SYSTEM_PARAMS["vexact"], orders=["spread", "adaptive"],
                                                    approx=[dict(nprobe=p) for p in (4, 8, 16, 32, 64, 128, 256, 512)],
                                                    adaptive=[dict(psi=x) for x in (3, 3.5, 4, 4.25, 4.5, 4.75, 5, 5.5, 6, 7)])),
             public=dict(QUICK["public"], datasets={"vibe": [], "annb": []}))

# The same cells with the library's default order only, for comparing revisions of the library (which may not have
# the orders as settings).
ORDER_DEFAULT = dict(ORDER, name="order-default",
                     params=dict(ORDER["params"], vexact={k: v for k, v in ORDER["params"]["vexact"].items() if k != "orders"}))

SUITES = {"quick": QUICK, "full": FULL, "scale": SCALE, "scale-test": SCALE_TEST, "order": ORDER,
          "order-default": ORDER_DEFAULT}


def types_for(suite, dataset):
    """The query types' parameters on one dataset, with the suite's overrides."""
    t = {k: dict(v) for k, v in suite["types"].items()}
    for k, v in suite["overrides"].get(dataset, {}).items():
        t[k].update(v)
    return t


def cells(suite, dataset, qtype, cap=False):
    """(filter, parameter list) for one type on one dataset: unfiltered with the type's parameters (none for approx:
    the public suites run it, except under a cap and at scale), and each visible set with the `filtered` parameters.
    With cap, the cap axis's own types, filters and parameters."""
    if cap:
        c = suite["caps"]
        return [(f, c["types"][qtype] if f == "none" else c["filtered"][qtype]) for f in c["filters"]]
    t = types_for(suite, dataset)[qtype]
    out = []
    for f in suite["filters"]:
        if f == "none":
            if qtype != "approx" or suite.get("approx_unfiltered"):
                out.append(("none", t))
        else:
            out.append((f, suite["filtered"][qtype]))
    return out


def systems_for(suite, dataset, qtype, cap=False):
    """[(role, system)] for one type on one dataset (with cap: the cap axis's systems)."""
    out = []
    if cap and dataset not in suite.get("caps", {}).get("datasets", []):
        return out
    for role, names in (suite["caps"]["systems"] if cap else suite["systems"])[qtype].items():
        for s in names:
            if s in suite["only"] and dataset not in suite["only"][s]:
                continue
            out.append((role, s))
    return out


def truth_needs(suite, dataset):
    """What prep must compute: the unfiltered k-NN depth and answer shares, and per filter its own."""
    t = types_for(suite, dataset)
    ks = set(t["exact"]["k"]) | set(t["approx"]["k"])
    shares = set(t["threshold"]["shares"])
    fk = set(suite["filtered"]["exact"]["k"]) | set(suite["filtered"]["approx"]["k"])
    fs = set(suite["filtered"]["threshold"]["shares"])
    fl = [f for f in suite["filters"] if f != "none"]
    c = suite.get("caps")
    if c and dataset in c["datasets"]:
        ks |= set(c["types"]["exact"]["k"]) | set(c["types"]["approx"]["k"])
        shares |= set(c["types"]["threshold"]["shares"])
        fk |= set(c["filtered"]["exact"]["k"]) | set(c["filtered"]["approx"]["k"])
        fs |= set(c["filtered"]["threshold"]["shares"])
        fl += [f for f in c["filters"] if f != "none" and f not in fl]
    filters = {f: dict(FILTERS[f], k=sorted(fk), shares=sorted(fs)) for f in fl}
    return dict(k=sorted(ks), shares=sorted(shares), filters=filters)
