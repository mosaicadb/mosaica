# Build real BioCLIP-2 text embeddings (CPU) for prompts derived from the metadata of the 200k subset.
import sys, os, json, collections, random
HERE = sys.argv[1]; 
import numpy as np, torch, pyarrow.parquet as pq
torch.set_num_threads(4)
import open_clip
from open_clip.model import CLIP, CLIPTextCfg, CLIPVisionCfg
cfg = json.load(open(os.path.join(HERE, "model/open_clip_config.json")))["model_cfg"]
# tiny vision tower: only the text tower is used; its weights are the real ones
model = CLIP(embed_dim=cfg["embed_dim"], vision_cfg=CLIPVisionCfg(image_size=28, layers=1, width=64, patch_size=14, head_width=32),
             text_cfg=CLIPTextCfg(**cfg["text_cfg"]))
sd = {k: torch.from_numpy(v) for k, v in np.load(os.path.join(HERE, "model/text_tower.npz")).items()}
res = model.load_state_dict(sd, strict=False)
bad = [k for k in res.missing_keys if not k.startswith("visual.")]
print("missing non-visual keys:", bad, "unexpected:", res.unexpected_keys)
assert not bad and not res.unexpected_keys
model.eval(); tok = open_clip.get_tokenizer("ViT-L-14")
rnd = random.Random(1)
prompts = []   # (kind, text, label-column, label-value)
tax_cols = ["kingdom", "phylum", "class", "order", "family", "genus", "species"]
sp = collections.Counter(); com = collections.Counter(); hi = collections.Counter(); taxs = {}
for i in (0, 5, 10, 15):
    t = pq.read_table(os.path.join(HERE, f"meta/m{i:02d}.parquet"), columns=tax_cols + ["scientific_name", "common_name"]).to_pydict()
    for j in range(len(t["genus"])):
        g, s, c = t["genus"][j], t["species"][j], t["common_name"][j]
        if g and s:
            sp[(g, s)] += 1; taxs[(g, s)] = " ".join(t[k][j] for k in tax_cols if t[k][j])
            if c and ";" not in c: com[(c, g, s)] += 1
        for k in ("class", "order", "family", "genus"):
            if t[k][j]: hi[(k, t[k][j])] += 1
species = [k for k, v in sp.items() if v >= 20]; rnd.shuffle(species); species = sorted(species[:120])
for g, s in species:
    prompts.append(("sci", f"a photo of {g} {s}", "species", f"{g} {s}"))
for g, s in species[:60]:
    prompts.append(("tax", f"a photo of {taxs[(g, s)]}", "species", f"{g} {s}"))
for g, s in species[60:90]:
    prompts.append(("bare", f"{g} {s}", "species", f"{g} {s}"))
seen = set()
for (c, g, s), v in com.most_common():
    if v >= 10 and c.lower() not in seen and len(seen) < 40:
        seen.add(c.lower()); prompts.append(("com", f"a photo of {c}", "species", f"{g} {s}"))
hk = [k for k, v in hi.items() if v >= 200]; rnd.shuffle(hk)
for k, v in sorted(hk[:60]):
    prompts.append(("higher", f"a photo of {v}", k, v))
free = ["a scorpion", "a photo of a scorpion on sand", "a scorpion glowing under ultraviolet light", "a mite", "a microscope slide of a mite",
        "a bee", "a bumblebee on a purple flower", "a bee on a yellow flower", "a stingless bee nest entrance", "a pinned bee specimen with a label",
        "a bird", "a swallow in flight", "a swallow perched on a wire", "a bird nest made of mud", "baby birds in a nest", "a bird on a fence",
        "a yellow flower", "a yellow daisy-like flower", "a pressed plant on a herbarium sheet", "a herbarium specimen", "a plant with hairy leaves",
        "a museum specimen", "a specimen label", "a close-up photo of an insect", "an insect on a leaf", "a wasp", "a spider", "a fly on a flower",
        "a blurry photo", "a photo taken at night", "a photo with a blue sky background", "a hand holding an animal", "a drawing of an animal",
        "a black and white image", "a dead bird", "flowers in a field", "a green leaf", "seeds", "an animal on a rock", "a photo of an animal",
        "a photo of a plant", "a photo of an arthropod", "a photo of a vertebrate", "something yellow", "wings", "a tail", "desert", "a tick",
        "an arachnid with a long tail and pincers", "a small brown animal on a white background"]
for f in free: prompts.append(("free", f, "", ""))
oos = ["Panthera leo", "Quercus alba", "Danaus plexippus", "Amanita muscaria", "Salmo salar", "Homo sapiens", "Canis lupus", "Apis mellifera",
       "Passer domesticus", "Helianthus annuus", "Octopus vulgaris", "Rana temporaria", "Pinus sylvestris", "Musca domestica", "Lumbricus terrestris",
       "Araneus diadematus", "Bombus terrestris", "Delichon urbicum", "Solidago canadensis", "Ixodes ricinus"]
for o in oos: prompts.append(("oos", f"a photo of {o}", "", ""))
texts = [p[1] for p in prompts]
out = []
with torch.no_grad():
    for i in range(0, len(texts), 32):
        e = model.encode_text(tok(texts[i:i+32]), normalize=True); out.append(e.numpy())
E = np.concatenate(out).astype(np.float32)
np.save(os.path.join(HERE, "text_q.npy"), E); json.dump(prompts, open(os.path.join(HERE, "text_q.json"), "w"), indent=0)
print(E.shape, collections.Counter(p[0] for p in prompts))
