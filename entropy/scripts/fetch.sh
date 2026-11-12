#!/bin/bash
# downloads into dl/<set>/ ; conversion by scripts/convert.py (run with python3 -I)
E=/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/entropy
cd $E
curl -sS -L --retry 5 -C - -o dl/glove/glove.hdf5 https://ann-benchmarks.com/glove-100-angular.hdf5
curl -sS -L --retry 5 -C - -o dl/nyt/nyt.hdf5 https://ann-benchmarks.com/nytimes-256-angular.hdf5
python3 -I scripts/convert.py h5 dl/glove/glove.hdf5 data/glove100.npy
python3 -I scripts/convert.py h5 dl/nyt/nyt.hdf5 data/nyt256.npy
for f in train-00000-of-00026-3c7b99d1c7eda36e train-00001-of-00026-2b24035a6390fdcb train-00002-of-00026-b05ce48965853dad; do
 curl -sS -L --retry 5 -o dl/ada/$f.parquet "https://huggingface.co/datasets/KShivendu/dbpedia-entities-openai-1M/resolve/main/data/$f.parquet"; done
python3 -I scripts/convert.py pq dl/ada data/ada1536.npy && rm -f dl/ada/*.parquet
for i in 00000 00001 00002 00003 00004 00005; do
 curl -sS -L --retry 5 -o dl/te3/$i.parquet "https://huggingface.co/datasets/Qdrant/dbpedia-entities-openai3-text-embedding-3-large-3072-1M/resolve/main/data/train-$i-of-00063.parquet"; done
python3 -I scripts/convert.py pq dl/te3 data/te3_3072.npy && rm -f dl/te3/*.parquet
echo done
