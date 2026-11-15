# usage: fitmeta.py INDEX_DIR NAME: meta.json plus cluster and group statistics, to SC/meta_NAME.json.
import sys, json, numpy as np
idx, name, sc = sys.argv[1], sys.argv[2], sys.argv[3]
m = json.load(open(f"{idx}/meta.json"))
cs = np.load(f"{idx}/cstart.npy"); grp = np.load(f"{idx}/grp.npy")
csz = np.diff(cs); gsz = np.bincount(grp, weights=csz).astype(int); cpg = np.bincount(grp)
m["stats"] = dict(cluster_min=int(csz.min()), cluster_max=int(csz.max()), group_min=int(gsz.min()), group_max=int(gsz.max()),
                  cpg_min=int(cpg.min()), cpg_max=int(cpg.max()), groups_below_768=int((gsz < 768).sum()))
json.dump(m, open(f"{sc}/meta_{name}.json", "w"), indent=1)
print(name, m["K"], m["G"], m["stats"], m["fit"]["cond_max"])
