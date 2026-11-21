# Index-plan records of the verify dump as columns, sorted by (query, size, row, level). Writes work/rec_*.npy.
# Record layout as reports/bounds3/dump_stats.py: 0 tag (bit 0 index plan, bits 1-5 size, bits 6+ query), 1 row, 2 level,
# 3 t, 4 truth - t, 5 estimate - t, 6 deterministic part of the half-width, 7 sd, then per block j: coded flag, a_j, R_j, e_j.
import numpy as np, sys
raw = np.memmap("work/dump.bin", dtype=np.float32, mode="r").reshape(-1, 24)
tag = np.asarray(raw[:, 0]).view(np.uint32); keep = (tag & 1) == 1
x = np.asarray(raw[keep]); tag = tag[keep]; row = x[:, 1].copy().view(np.uint32); lv = x[:, 2].astype(np.uint8)
qs = (tag >> 1).astype(np.uint32)                      # query * 32 + size
key = (qs.astype(np.uint64) << np.uint64(32)) | row.astype(np.uint64)
o = np.lexsort((lv, key))
cols = {"key": key, "lv": lv, "t": x[:, 3], "tr": x[:, 4], "es": x[:, 5], "hn": x[:, 6], "sd": x[:, 7],
        "cod": ((x[:, 8::4] > 0) * (1 << np.arange(4))).sum(1).astype(np.uint8), "a": x[:, 9::4], "R": x[:, 10::4], "e": x[:, 11::4]}
for k, v in cols.items(): np.save(f"work/rec_{k}.npy", np.ascontiguousarray(v[o]))
k = cols["key"][o]; l = cols["lv"][o]
# every deeper record must follow a record of the previous level of the same item
prev_ok = np.ones(len(k), bool); prev_ok[1:] = ~((l[1:] > 0) & ~((k[1:] == k[:-1]) & (l[1:] == l[:-1] + 1))); prev_ok[0] = l[0] == 0
print("records", len(k), "levels", np.bincount(l), "deeper records without their previous level:", int((~prev_ok).sum()))
q = (k >> np.uint64(37)).astype(int); s = ((k >> np.uint64(32)) & np.uint64(31)).astype(int)
print("queries", np.unique(q).tolist(), "sizes", np.unique(s).tolist())
print("level-0 records per (query class, size):")
for c in (0, 1):
    m = (l == 0) & ((q >= 10) == c)
    print("  ", "text" if c else "item", np.bincount(s[m], minlength=7).tolist())
