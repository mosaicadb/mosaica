# Encoder check on the 200k sample itself: "a photo of <Genus species>" for every species with at least 20 images in the
# sample (meta_g0-3.parquet), encoded as encode.py does; share of the 10 nearest images (and the top 1) with that label.
# Writes text_insample.json / .npy (kind "sci-in") beside the other prompts.
# usage: check_insample.py DATADIR CORPUSDIR
import sys, json, collections, numpy as np, torch, pyarrow.parquet as pq
torch.set_num_threads(2)
import open_clip
from open_clip.model import CLIP, CLIPTextCfg, CLIPVisionCfg
D, CD = sys.argv[1], sys.argv[2]; cfg = json.load(open(f"{D}/open_clip_config.json"))["model_cfg"]
model = CLIP(embed_dim=cfg["embed_dim"], vision_cfg=CLIPVisionCfg(image_size=28, layers=1, width=64, patch_size=14, head_width=32),
             text_cfg=CLIPTextCfg(**cfg["text_cfg"]))
res = model.load_state_dict({k: torch.from_numpy(v) for k, v in np.load(f"{D}/model/text_tower.npz").items()}, strict=False)
assert not [k for k in res.missing_keys if not k.startswith("visual.")] and not res.unexpected_keys
model.eval(); tok = open_clip.get_tokenizer("ViT-L-14")
lab = []
for j in range(4):
    t = pq.read_table(f"{D}/meta_g{j}.parquet", columns=["genus", "species"]).to_pydict()
    lab += [f"{g} {s}" if g and s else None for g, s in zip(t["genus"], t["species"])]
cnt = collections.Counter(l for l in lab if l); sp = sorted(k for k, v in cnt.items() if v >= 20)
P = [("sci-in", f"a photo of {s}", "species", s) for s in sp]
with torch.no_grad():
    E = np.concatenate([model.encode_text(tok([p[1] for p in P[i:i + 64]]), normalize=True).numpy() for i in range(0, len(P), 64)]).astype(np.float32)
E /= np.linalg.norm(E, axis=1, keepdims=True)
X = np.concatenate([np.load(f"{CD}/g{j}.npy") for j in range(4)]).astype(np.float32); X /= np.linalg.norm(X, axis=1, keepdims=True)
top = np.argsort(-(E @ X.T), axis=1)[:, :10]; lab = np.array(lab, object)
h = lab[top] == np.array(sp, object)[:, None]
print("species with >= 20 images:", len(sp), "of", len(cnt), "; images", sum(cnt[s] for s in sp))
print("top-1 hit", round(h[:, 0].mean(), 3), "top-10 hit share", round(h.mean(), 3), "; prompts with >= 5 of 10 hits", int((h.sum(1) >= 5).sum()))
w = np.array([cnt[s] for s in sp]); print("by species size <100 / 100-999 / >=1000 images: top-10 share",
      [round(h[m].mean(), 3) if m.any() else None for m in (w < 100, (w >= 100) & (w < 1000), w >= 1000)], [int(m.sum()) for m in (w < 100, (w >= 100) & (w < 1000), w >= 1000)])
np.save(f"{D}/text_insample.npy", E); json.dump(P, open(f"{D}/text_insample.json", "w"), indent=0)
