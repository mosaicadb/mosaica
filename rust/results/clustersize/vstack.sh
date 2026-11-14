D=/tmp/claude-0/-home-user-mosaica/19e29d74-5f90-522f-b0ad-14eb025bda2b/scratchpad/detlv-data
until grep -q STACK_DONE $D/cl/stack.log; do sleep 5; done
taskset -c 0-3 $D/bin/vexact_ndl2 --v2 --int --no-approx --plan-file /tmp/claude-0/-home-user-mosaica/19e29d74-5f90-522f-b0ad-14eb025bda2b/scratchpad/detlv/rust/results/bounds/plan_fixed.txt --index $D/cl/p4 --dlv 16,32 --queries $D/text/text_q.npy --item-queries $D/c1m_item_q.npy --results $D/res/v1m_p4_16 --gtab --flags repack verify --vitem 20 --vtext 20 > $D/res/v1m_p4_16.out 2>&1
grep -E "^VERIFY2|panicked" $D/res/v1m_p4_16.out | cut -c1-250
echo VSTACK_DONE
