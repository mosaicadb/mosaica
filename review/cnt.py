import numpy as np
se=np.fromfile("b_se.bin",np.int32); sd=np.fromfile("b_sdep.bin",np.int32); U=np.fromfile("b_u.bin",np.float32).reshape(-1,768)
k=np.minimum(np.floor(np.abs(U[:, se[:400]])/0.6745),55).astype(int); cost=np.where(sd[:400][None]==2,k+1,1); cum=np.cumsum(cost,1)
for B in (32,64,128,256):
    st=(cum<=B).sum(1); print(B,"bits: mean steps",st.mean().round(1),"sd",st.std().round(1),"share of depth-2 steps",(sd[:int(st.mean())]==2).mean().round(2), "distinct coords", len(set(se[:int(st.mean())])))
print("first 30 steps (coord,depth):", list(zip(se[:30].tolist(), sd[:30].tolist())))
