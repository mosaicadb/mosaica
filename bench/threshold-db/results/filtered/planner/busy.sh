#!/bin/bash
# exit 0 when another benchmark or build runs or the 1-minute load is 2 or more
ps -eo pid,comm | awk -v me=$$ '$1!=me && ($2=="vexact" || $2=="rustc" || $2=="cargo" || $2 ~ /acorn/ || $2=="clickhouse") {f=1} END {exit !f}' && exit 0
awk '{exit !($1>=2)}' /proc/loadavg
