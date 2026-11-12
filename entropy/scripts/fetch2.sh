#!/bin/bash
E=/tmp/claude-0/-home-user-mosaica/252074fe-d322-54df-bd82-fcf5f63ee10f/scratchpad/vec/entropy
cd $E
for i in 00010 00011 00012; do
 curl -sS -L --retry 5 -o dl/te3/$i.parquet "https://huggingface.co/datasets/Qdrant/dbpedia-entities-openai3-text-embedding-3-large-3072-1M/resolve/main/data/train-$i-of-00063.parquet"; done
python3 -I scripts/convert.py pq3072 dl/te3 data/te3_3072.npy && rm -f dl/te3/*.parquet
echo done
