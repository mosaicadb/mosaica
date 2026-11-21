#!/bin/bash
# run_sc.sh NEW DS [ARMS...]: k-NN arms back to back on core 11, each build on its own hard-linked copy of the fit
# (opened once beforehand on cores 0-7, which rebuilds it, then constants timed alone on core 11); arms: d999606,
# a351f13, NEW (the stored correction at the default level dial) and NEW-lo15 (level psi = max(0, psi - 1.5)).
# Deletes the dataset's index copies once its runs end, then tabulates with builds_table.py.
set -uo pipefail
NEW=$1; DS=$2; shift 2
ARMS=${*:-"d999606 a351f13 $NEW $NEW-lo15"}
E=/home/joe/code/vexact-visitorder-env
S=/home/joe/code/vexact-data/suite
I=/home/joe/code/vexact-data/corr/idx
R=/home/joe/code/vexact-corr/bench/suite/notes/visit-order/results/stored-correction
AB=/home/joe/code/vexact-corr/bench/suite/notes/visit-order/scripts/ab_orders.py
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
  case $NAME in *-lo15) FL="--level-psi-offset 1.5";; *) FL="";; esac
  echo "-- ab $NAME $(date +%T)"
  SUITE_DATA=$S taskset -c 11 $E/b$REV/bin/python -I $AB $DS spread adaptive --psi 2,2.5,3,3.5,4,4.5,5,5.5,6,6.5,7,8,9 --reps 3 --core 11 --index $I/$DS-$REV $FL > $R/ab-knn-$DS-$NAME.txt 2> $R/ab-knn-$DS-$NAME.err
  echo "   exit $? $(date +%T)"
done
echo "== timed runs done $DS $(date +%T)"
for REV in $(for a in $ARMS; do echo ${a%%-*}; done | sort -u); do rm -rf $I/$DS-$REV; done
df -h / | tail -1
B=/home/joe/code/vexact-corr/bench/suite/notes/visit-order/scripts/builds_table.py
python3 -I $B $R d999606,a351f13,$NEW,$NEW-lo15 --targets 0.9,0.95,0.99 --datasets $DS | sed -n '/^== k-NN/,/^$/p' > $R/$DS-table.txt 2>&1
