# After stages A and B: build the k-NN harness binary, check fit.py's default output again at 50k, then time exact
# k-NN (k = 10, 100) on every 1M and 500k fit, one short process each.
D=/tmp/claude-0/-home-user-mosaica/19e29d74-5f90-522f-b0ad-14eb025bda2b/scratchpad/detlv-data
until grep -q STAGE_AB_DONE $D/sc/stage_ab.log; do sleep 5; done
cd /tmp/claude-0/-home-user-mosaica/19e29d74-5f90-522f-b0ad-14eb025bda2b/scratchpad/detlv/rust && VEXACT_NDL=2 CARGO_TARGET_DIR=$D/target_ndl2 CARGO_PROFILE_RELEASE_DEBUG=0 cargo build --release 2>&1 | grep -E "^error|Finished"
cp $D/target_ndl2/release/vexact $D/bin/vexact_ndl2k
python3 -I /tmp/claude-0/-home-user-mosaica/19e29d74-5f90-522f-b0ad-14eb025bda2b/scratchpad/detlv/rust/tools/fit.py $D/c50k_x.npy $D/sc/c50k_check --basis dual > $D/sc/fit_c50k_check.log 2>&1
python3 -I -c "
import numpy as np
a='$D/c50k/'; b='$D/sc/c50k_check/'
for f in ['Xs','perm','cstart','grp','mq8','msc','V','lam','profile']:
    print('50k default fit', f, 'identical' if np.array_equal(np.load(a+f+'.npy'), np.load(b+f+'.npy')) else 'DIFFERS')
"
rm -rf $D/sc/c50k_check
idx() { case $1 in m1_1000) echo $D/c1m ;; *) echo $D/sc/$1 ;; esac; }
for f in m1_1000 m1_2000 m1_4000 m1_8000 k500_1000 k500_2000 k500_4000 k500_8000; do
  echo "$(date +%T) start knn_$f"; $D/scripts/srun_knn.sh $(idx $f) knn_$f; echo "$(date +%T) end knn_$f"
done
echo POST_AB_DONE
