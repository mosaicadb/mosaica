# A separate training set of real BioCLIP-2 text embeddings, with no prompt text in common with text_q.json (the test set).
import sys, os, json, collections, random
HERE = sys.argv[1]; 
import numpy as np, torch, pyarrow.parquet as pq
torch.set_num_threads(2)
import open_clip
from open_clip.model import CLIP, CLIPTextCfg, CLIPVisionCfg
cfg = json.load(open(os.path.join(HERE, "model/open_clip_config.json")))["model_cfg"]
model = CLIP(embed_dim=cfg["embed_dim"], vision_cfg=CLIPVisionCfg(image_size=28, layers=1, width=64, patch_size=14, head_width=32), text_cfg=CLIPTextCfg(**cfg["text_cfg"]))
sd = {k: torch.from_numpy(v) for k, v in np.load(os.path.join(HERE, "model/text_tower.npz")).items()}
res = model.load_state_dict(sd, strict=False); assert not [k for k in res.missing_keys if not k.startswith("visual.")]
model.eval(); tok = open_clip.get_tokenizer("ViT-L-14")
test = set(p[1] for p in json.load(open(os.path.join(HERE, "text_q.json"))))
rnd = random.Random(2); prompts = []
tax_cols = ["kingdom", "phylum", "class", "order", "family", "genus", "species"]
sp = {}; com = set(); hi = set()
for i in (0, 5, 10, 15):
    t = pq.read_table(os.path.join(HERE, f"meta/m{i:02d}.parquet"), columns=tax_cols + ["common_name"]).to_pydict()
    for j in range(len(t["genus"])):
        g, s, c = t["genus"][j], t["species"][j], t["common_name"][j]
        if g and s: sp[(g, s)] = " ".join(t[k][j] for k in tax_cols if t[k][j])
        if c and ";" not in c: com.add(c)
        for k in ("order", "family", "genus"):
            if t[k][j]: hi.add(t[k][j])
for (g, s), tx in sorted(sp.items()):
    kind = rnd.choice(["sci", "sci", "tax", "bare"])
    prompts.append((kind, {"sci": f"a photo of {g} {s}", "tax": f"a photo of {tx}", "bare": f"{g} {s}"}[kind]))
for c in sorted(com): prompts.append(("com", f"a photo of {c}"))
for h in sorted(hi): prompts.append(("higher", f"a photo of {h}"))
nouns = ["scorpion", "mite", "bee", "bumblebee", "bird", "swallow", "flower", "plant", "insect", "spider", "wasp", "beetle", "butterfly", "leaf", "nest",
         "specimen", "animal", "fly", "ant", "moth", "seedling", "shrub", "feather", "egg", "larva", "tick", "herbarium sheet", "daisy", "weed", "arachnid"]
adjs = ["", "small ", "large ", "yellow ", "black ", "brown ", "hairy ", "dead ", "young ", "striped ", "pinned ", "dried ", "tiny ", "blurry ", "colourful "]
ctx = ["", " on a leaf", " on a flower", " on the ground", " in flight", " on a white background", " under a microscope", " at night", " in a hand",
       " on a rock", " with a label", " in a museum drawer", " in the desert", " in a garden", " against the sky", " close-up", " on a wall", " in grass"]
tpl = ["a photo of a {}", "a {}", "{}", "an image of a {}", "a picture of a {}"]
while len([p for p in prompts if p[0] == "free"]) < 700:
    s = rnd.choice(tpl).format(rnd.choice(adjs) + rnd.choice(nouns) + rnd.choice(ctx))
    if s not in test and ("free", s) not in prompts: prompts.append(("free", s))
oos = ["Vulpes vulpes", "Fagus sylvatica", "Pieris rapae", "Boletus edulis", "Gadus morhua", "Felis catus", "Ursus arctos", "Vespa crabro", "Turdus merula",
       "Bellis perennis", "Sepia officinalis", "Bufo bufo", "Picea abies", "Drosophila melanogaster", "Helix pomatia", "Lycosa tarantula", "Bombus pascuorum",
       "Riparia riparia", "Taraxacum officinale", "Dermacentor variabilis", "Acer rubrum", "Lynx rufus", "Corvus corax", "Apis cerana", "Rosa canina",
       "Zea mays", "Anas platyrhynchos", "Tyto alba", "Papilio machaon", "Lacerta agilis"]
for o in oos: prompts.append(("oos", f"a photo of {o}"))
prompts = [p for p in prompts if p[1] not in test]
texts = [p[1] for p in prompts]; out = []
with torch.no_grad():
    for i in range(0, len(texts), 64):
        out.append(model.encode_text(tok(texts[i:i+64]), normalize=True).numpy())
E = np.concatenate(out).astype(np.float32)
np.save(os.path.join(HERE, "text_train.npy"), E); json.dump(prompts, open(os.path.join(HERE, "text_train.json"), "w"), indent=0)
print(E.shape, collections.Counter(p[0] for p in prompts))
