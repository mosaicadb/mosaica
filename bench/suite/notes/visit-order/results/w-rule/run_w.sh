#!/bin/bash
# run_w.sh W2REV DS [ARMS...]: k-NN arms back to back on core 11, each build on its own hard-linked copy of the fit
# (opened once beforehand on cores 0-7, which rebuilds it, then constants timed alone on core 11); arms: d999606,
# 99b033c (the stored ratio) and W2REV-w2 (W2REV's build under the W2 rule). Deletes the dataset's index copies once its
# runs end, then tabulates with builds_table.py.
set -uo pipefail
W2=$1; DS=$2; shift 2
ARMS=${*:-"d999606 99b033c $W2-w2"}
E=/home/joe/code/vexact-visitorder-env
S=/home/joe/code/vexact-data/suite
I=/home/joe/code/vexact-data/wrule/idx
R=/home/joe/code/vexact-wrule/bench/suite/notes/visit-order/results/w-rule
AB=/home/joe/code/vexact-wrule/bench/suite/notes/visit-order/scripts/ab_orders.py
mkdir -p $I
echo "== $DS $(date +%T): $ARMS"
for REV in $(for a in $ARMS; do echo ${a%%-*}; done | sort -u); do
  X=$I/$DS-$REV
  [ -d $X ] && [ -f $X/compute-constants.txt ] && continue
  rm -rf $X; cp -al $S/fit/$DS $X; rm -f $X/compute-constants.txt
  nice -n 19 taskset -c 0-7 $E/b$REV/bin/python -I -c "import vexact_py; vexact_py.Index('$X')" 2> $R/open-$DS-$REV.err
  rm -f $X/compute-constants.txt
  taskset -c 11 $E/b$REV/bin/python -I -c "import vexact_py; vexact_py.Index('$X', core=11)" 2>> $R/open-$DS-$REV.err
done
for NAME in $ARMS; do
  REV=${NAME%%-*}
  case $NAME in *-w2) FL="--width-rule w2";; *) FL="";; esac
  echo "-- ab $NAME $(date +%T)"
  SUITE_DATA=$S taskset -c 11 $E/b$REV/bin/python -I $AB $DS spread adaptive --psi 2,2.5,3,3.5,4,4.5,5,5.5,6,6.5,7,8,9 --reps 3 --core 11 --index $I/$DS-$REV $FL > $R/ab-knn-$DS-$NAME.txt 2> $R/ab-knn-$DS-$NAME.err
  echo "   exit $? $(date +%T)"
done
echo "== timed runs done $DS $(date +%T)"
# the phase profile on W2's copy, under the stored ratio and under W2 (iNaturalist and LAION)
case $DS in inaturalist*|laion*)
  for RULE in ratio w2; do taskset -c 11 $E/b$W2/bin/python -I $R/diag_w.py $I/$DS-$W2 $S/prep/$DS 5,6 $RULE 2>/dev/null; done > $R/profile-$DS.txt
  echo "== profile done $DS $(date +%T)";;
esac
for REV in $(for a in $ARMS; do echo ${a%%-*}; done | sort -u); do rm -rf $I/$DS-$REV; done
df -h / | tail -1
