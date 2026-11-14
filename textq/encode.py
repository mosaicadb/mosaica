# Encode the prompts of text_q.json and text_train.json with BioCLIP-2's text tower on CPU (model construction as in
# vexact-bench agent2/encode_text.py: real text-tower weights, a tiny unused vision tower). Writes text_q.npy and
# text_train.npy (float32, unit norm) beside the prompts.
# usage: encode.py DATADIR   (DATADIR holds model/text_tower.npz, open_clip_config.json and the two prompt files)
import sys, os, json, numpy as np, torch
torch.set_num_threads(2)
import open_clip
from open_clip.model import CLIP, CLIPTextCfg, CLIPVisionCfg
D = sys.argv[1]; cfg = json.load(open(f"{D}/open_clip_config.json"))["model_cfg"]
model = CLIP(embed_dim=cfg["embed_dim"], vision_cfg=CLIPVisionCfg(image_size=28, layers=1, width=64, patch_size=14, head_width=32),
             text_cfg=CLIPTextCfg(**cfg["text_cfg"]))
res = model.load_state_dict({k: torch.from_numpy(v) for k, v in np.load(f"{D}/model/text_tower.npz").items()}, strict=False)
assert not [k for k in res.missing_keys if not k.startswith("visual.")] and not res.unexpected_keys
model.eval(); tok = open_clip.get_tokenizer("ViT-L-14")
for name in ("text_q", "text_train"):
    texts = [p[1] for p in json.load(open(f"{D}/{name}.json"))]; out = []
    with torch.no_grad():
        for i in range(0, len(texts), 64): out.append(model.encode_text(tok(texts[i:i + 64]), normalize=True).numpy())
    E = np.concatenate(out).astype(np.float32); E /= np.linalg.norm(E, axis=1, keepdims=True)
    np.save(f"{D}/{name}.npy", E); print(name, E.shape, flush=True)
