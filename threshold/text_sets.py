# Query sets for the text rerun, from the encoded prompts: all 369 test prompts; the test prompts split by kind (species
# names: sci, tax, bare, com, oos, none of whose species is in this sample; higher taxa; free descriptions); and a seeded
# 100 of the "a photo of <species>" prompts for species with at least 20 images in the sample (check_insample.py).
# usage: text_sets.py DATADIR OUTDIR   (writes OUTDIR/<set>.npy)
import sys, json, numpy as np
D, O = sys.argv[1], sys.argv[2]
P = json.load(open(f"{D}/text_q.json")); E = np.load(f"{D}/text_q.npy"); kind = np.array([p[0] for p in P])
sets = {"all": np.ones(len(P), bool), "names": np.isin(kind, ["sci", "tax", "bare", "com", "oos"]), "higher": kind == "higher", "free": kind == "free"}
for k, m in sets.items(): np.save(f"{O}/{k}.npy", E[m]); print(k, int(m.sum()))
I = np.load(f"{D}/text_insample.npy"); pick = np.sort(np.random.default_rng(20261114).choice(len(I), 100, replace=False))
np.save(f"{O}/insample.npy", I[pick]); np.save(f"{O}/insample_idx.npy", pick); print("insample", len(pick), "of", len(I))
