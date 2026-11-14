#!/usr/bin/env bash
# ClickHouse for the threshold comparison: start (on cores 8-11, 16 GB), load, sizes, stop.
# usage: bash clickhouse.sh start|load|sizes|stop
# DATA (default /home/joe/code/vexact-data/bioclip2-1m) holds prep/ and corpus.parquet; the server's files go to
# DATA/clickhouse.
set -euo pipefail
DATA=${DATA:-/home/joe/code/vexact-data/bioclip2-1m}
IMAGE=clickhouse/clickhouse-server:26.10.4.207   # sha256:4f491abc44d9f4a1747c033c35ddd5bc385521305d85ca8251ba0824ff597035
NAME=vexact-threshold-ch
q() { docker exec -i $NAME clickhouse-client --max_threads 4 --multiquery "$@"; }
case $1 in
start)
    # the server runs as uid 101 and the entrypoint does not chown (it would walk the read-only /data); DATA/clickhouse must be writable by uid 101
    mkdir -p "$DATA/clickhouse"
    docker run -d --name $NAME --cpuset-cpus 8-11 --memory 16g --memory-swap 16g \
        --ulimit nofile=262144:262144 -e CLICKHOUSE_SKIP_USER_SETUP=1 -e CLICKHOUSE_DO_NOT_CHOWN=1 \
        -v "$DATA/clickhouse:/var/lib/clickhouse" -v "$DATA:/data:ro" \
        -v "$(cd "$(dirname "$0")" && pwd)/ch-user-files.xml:/etc/clickhouse-server/config.d/user-files.xml:ro" -p 127.0.0.1:18123:8123 $IMAGE
    for _ in $(seq 60); do docker exec $NAME clickhouse-client -q "SELECT 1" >/dev/null 2>&1 && break; sleep 1; done
    docker exec $NAME clickhouse-client -q "SELECT version()"
    ;;
load)
    # Vectors uncompressed: float embeddings barely compress, and LZ4 only adds decompression to every scan.
    q <<'SQL'
DROP TABLE IF EXISTS t32; DROP TABLE IF EXISTS tbf;
CREATE TABLE t32 (id UInt32, vec Array(Float32) CODEC(NONE)) ENGINE = MergeTree ORDER BY id;
INSERT INTO t32 SELECT id, vec FROM file('/data/corpus.parquet', Parquet) SETTINGS max_insert_threads = 4;
OPTIMIZE TABLE t32 FINAL;
CREATE TABLE tbf (id UInt32, vec Array(BFloat16) CODEC(NONE)) ENGINE = MergeTree ORDER BY id;
INSERT INTO tbf SELECT id, CAST(vec, 'Array(BFloat16)') FROM t32;
OPTIMIZE TABLE tbf FINAL;
SQL
    for v in vis_50 vis_10 vis_1; do
        q <<SQL
DROP TABLE IF EXISTS v_$v; DROP TABLE IF EXISTS s_$v; DROP TABLE IF EXISTS b_$v;
CREATE TABLE v_$v (id UInt32) ENGINE = MergeTree ORDER BY id;
INSERT INTO v_$v SELECT toUInt32(c1) FROM file('/data/prep/$v.csv', CSV);
CREATE TABLE s_$v (id UInt32) ENGINE = Set;
INSERT INTO s_$v SELECT id FROM v_$v;
CREATE TABLE b_$v (bm AggregateFunction(groupBitmap, UInt32)) ENGINE = Memory;
INSERT INTO b_$v SELECT groupBitmapState(id) FROM v_$v;
SQL
    done
    q -q "SELECT table, sum(rows), formatReadableSize(sum(bytes_on_disk)) FROM system.parts WHERE active AND database = 'default' GROUP BY table ORDER BY table"
    ;;
sizes)
    q -q "SELECT table, sum(rows), sum(bytes_on_disk), sum(data_uncompressed_bytes) FROM system.parts WHERE active AND database = 'default' GROUP BY table ORDER BY table"
    docker stats --no-stream --format '{{.Name}} {{.MemUsage}}' $NAME
    ;;
stop)
    docker stop $NAME && docker rm $NAME
    ;;
esac
