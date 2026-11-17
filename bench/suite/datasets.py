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

# The LAION scale sets (prep_scale.py): CLIP ViT-B/32 embeddings of LAION-400M as LAION released them (410 parts of
# 1,000,448 image and text embeddings each, f16, 512 dimensions; the last part is about half), from
# deploy.laion.ai. Part 0's images are exactly VIBE's laion-clip-512 corpus (VIBE's datasets.py reads img_emb_0.npy and
# draws its queries from text_emb_0.npy), checked here to 9e-8 after normalisation. A population is the first `parts`
# parts; each size takes the same uniform random fraction of every part's rows (seeded by part), and a smaller size's
# rows are a prefix of the same per-part permutation, so 10M is inside 40M inside 100M, stratified by part (as
# rust/REPORT_SCALING.md built R1M inside R3M). Queries are text embeddings of part 0 (out of distribution, as VIBE's).
LAION_URL = "https://deploy.laion.ai/8f83b608504d46bb81708ec86e912220/embeddings/{kind}_emb/{kind}_emb_{part}.npy"


def laion_scale(parts, fraction, n):
    return dict(source="laion-scale", url=None, N=n, d=512, ood=True, kind="text->images", parts=parts,
                fraction=fraction)


DATASETS.update({
    "laion-10m": laion_scale(100, 0.1, 10_004_500),
    "laion-40m": laion_scale(100, 0.4, 40_017_900),
    "laion-100m": laion_scale(100, 1.0, 100_044_800),
    # the same code on two parts: 200k in 500k in 1M
    "laion-s200k": laion_scale(2, 0.1, 200_090),
    "laion-s500k": laion_scale(2, 0.25, 500_224),
    "laion-s1m": laion_scale(2, 0.5, 1_000_448),
})


def url(name):
    ds = DATASETS[name]
    if ds["source"] == "vibe":
        return VIBE_URL.format(name=name)
    if ds["source"] == "annb":
        return ANNB_URL.format(name=name)
    return None
