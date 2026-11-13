#!/bin/bash
# Core 8 sequence after the nytimes build: tree mode, then Experiment B on nytimes and BioCLIP.
cd /home/joe/code/vexact-probe-clusters/probe
while kill -0 44732 2>/dev/null; do sleep 15; done
export OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1
D=/home/joe/code/vexact-data
taskset -c 8 .venv/bin/python -I expA.py $D/nytimes-256-angular/nytimes-256-angular.hdf5 out tree 256 1024 4096 > logs/expA_tree_nytimes.log 2>&1
taskset -c 8 .venv/bin/python -I expB.py $D/nytimes-256-angular/nytimes-256-angular.hdf5 out 1024 > logs/expB_nytimes.log 2>&1
taskset -c 8 .venv/bin/python -I expB.py $D/bioclip2-200k out 1024 > logs/expB_bioclip.log 2>&1
