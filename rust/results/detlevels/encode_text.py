# BioCLIP-2 text embeddings of the 369 committed test prompts (agent2/text_q.json), as agent2/encode_text.py does.
import sys, json, numpy as np, torch
torch.set_num_threads(4)
import open_clip
from open_clip.model import CLIP, CLIPTextCfg, CLIPVisionCfg
M = sys.argv[1]
cfg = json.load(open(f"{M}/open_clip_config.json"))["model_cfg"]
model = CLIP(embed_dim=cfg["embed_dim"], vision_cfg=CLIPVisionCfg(image_size=28, layers=1, width=64, patch_size=14, head_width=32),
             text_cfg=CLIPTextCfg(**cfg["text_cfg"]))
sd = {k: torch.from_numpy(v) for k, v in np.load(f"{M}/text_tower.npz").items()}
res = model.load_state_dict(sd, strict=False)
bad = [k for k in res.missing_keys if not k.startswith("visual.")]
print("missing non-visual:", bad, "unexpected:", res.unexpected_keys); assert not bad and not res.unexpected_keys
model.eval(); tok = open_clip.get_tokenizer("ViT-L-14")
texts = [p[1] for p in json.load(open(sys.argv[2]))]
out = []
with torch.no_grad():
    for i in range(0, len(texts), 32):
        out.append(model.encode_text(tok(texts[i:i+32]), normalize=True).numpy())
E = np.concatenate(out).astype(np.float32); np.save(sys.argv[3], E); print(E.shape)
