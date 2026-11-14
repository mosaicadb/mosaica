# Range-read only the text-tower tensors of BioCLIP-2 from the safetensors file (no vision tower).
import json, struct, sys, numpy as np
from huggingface_hub import HfFileSystem
fs = HfFileSystem()
p = "imageomics/bioclip-2/open_clip_model.safetensors"
f = fs.open(p, block_size=16 << 20)
n = struct.unpack("<Q", f.read(8))[0]
hdr = json.loads(f.read(n)); base = 8 + n
keys = [k for k in hdr if k != "__metadata__" and not k.startswith("visual.")]
print(len(hdr), "tensors,", len(keys), "non-visual")
tot = sum(hdr[k]["data_offsets"][1] - hdr[k]["data_offsets"][0] for k in keys); print("bytes", tot)
keys.sort(key=lambda k: hdr[k]["data_offsets"][0])
out = {}
DT = {"F32": np.float32, "F16": np.float16, "I64": np.int64, "BOOL": np.bool_}
for k in keys:
    a, b = hdr[k]["data_offsets"]; f.seek(base + a); buf = f.read(b - a)
    out[k] = np.frombuffer(buf, DT[hdr[k]["dtype"]]).reshape(hdr[k]["shape"]).copy()
np.savez(sys.argv[1], **out)
for k in keys[:8] + keys[-6:]: print(k, out[k].shape, out[k].dtype)
