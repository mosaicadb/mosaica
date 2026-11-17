# The datasets the suites can name, and where each comes from.
#   source "annb": an ANN-Benchmarks HDF5 file (train, test, neighbors, distances), from ann-benchmarks.com
#   source "vibe": a VIBE HDF5 file (the same keys, and on out-of-distribution sets learn and learn_neighbors, a sample
#                  of the query distribution), from VIBE's Hugging Face dataset, as VIBE's run.py downloads it
#   source "bioclip": BioCLIP-2 image embeddings fetched by ../threshold-db/fetch.py (RAW dir of e00..e19.npy) and the
#                  BioCLIP-2 text-prompt embeddings (TEXT dir with text_q.npy, text_insample.npy and optionally
#                  text_train.npy), built as ../threshold-db/prep.py builds them; not hosted anywhere public
# Every dataset is cosine: vectors are normalised in f64 at prep. `ood`: queries come from another distribution than
# the corpus (text against images), and the learn sample (when the source has one) goes to the index's dual fit.
VIBE_URL = "https://huggingface.co/datasets/vector-index-bench/vibe/resolve/main/{name}.hdf5"
ANNB_URL = "http://ann-benchmarks.com/{name}.hdf5"


def vibe(n, d, ood=False, kind="text"):
    return dict(source="vibe", url=None, N=n, d=d, ood=ood, kind=kind)


def annb(n, d, kind="words"):
    return dict(source="annb", url=None, N=n, d=d, ood=False, kind=kind)


DATASETS = {
    # ANN-Benchmarks
    "glove-100-angular": annb(1_183_514, 100),
    "nytimes-256-angular": annb(290_000, 256, "text"),
    # VIBE's cosine and normalized sets up to 1.5M vectors (bench/vibe/README.md)
    "yahoo-minilm-384-normalized": vibe(677_305, 384),
    "glove-200-cosine": vibe(1_193_514, 200, kind="words"),
    "imagenet-clip-512-normalized": vibe(1_281_167, 512, kind="images"),
    "arxiv-nomic-768-normalized": vibe(1_344_643, 768),
    "landmark-dino-768-cosine": vibe(760_757, 768, kind="images"),
    "inaturalist-resnet-2048-cosine": vibe(499_000, 2048, kind="images"),
    "laion-clip-512-normalized": vibe(1_000_448, 512, ood=True, kind="text->images"),
    "imagenet-align-640-normalized": vibe(1_281_167, 640, ood=True, kind="text->images"),
    "yandex-200-cosine": vibe(1_000_000, 200, ood=True, kind="text->images"),
    # BioCLIP-2: 999,950 image embeddings; 50 held-out images and 100 text prompts as queries
    "bioclip2-768": dict(source="bioclip", url=None, N=999_950, d=768, ood=True, kind="images+text->images"),
}


def url(name):
    ds = DATASETS[name]
    if ds["source"] == "vibe":
        return VIBE_URL.format(name=name)
    if ds["source"] == "annb":
        return ANNB_URL.format(name=name)
    return None
