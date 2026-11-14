D=/tmp/claude-0/-home-user-mosaica/19e29d74-5f90-522f-b0ad-14eb025bda2b/scratchpad/detlv-data; B=$D/bin; E=$D/scripts/e2e.sh
until grep -q CHECKS_DONE $D/checks_c.log; do sleep 3; done
r() { echo "$(date +%T) start $2"; "$@" | head -1; echo "$(date +%T) end $2"; }
for rep in 1 2; do
  order="base b1 b2 c3 c2a c2b"; [ $rep = 2 ] && order="c2b c2a c3 b2 b1 base"
  for v in $order; do case $v in
    base) r $E $B/vexact_base t_base_$rep ;;
    b1) r $E $B/vexact_b2 t_b1_$rep --b repack,ddec=0.4.5 --c repack,ddec=0.5 ;;
    b2) r $E $B/vexact_b2 t_b2_$rep --b repack,ddec=3.5 --c repack,ddec=5 ;;
    c3) r $E $B/vexact_ndl3 t_c3_$rep --dlv 4,16,32 --b repack,ddec=0.2 --c repack,ddec=2 ;;
    c2a) r $E $B/vexact_ndl2 t_c2a_$rep --dlv 8,32 --b repack,ddec=1 ;;
    c2b) r $E $B/vexact_ndl2 t_c2b_$rep --dlv 16,32 --b repack,ddec=none ;;
  esac; done
done
echo TIMING_DONE
