#!/usr/bin/env bash
# ClickHouse BFloat16 under a memory cap: its own server and data directory, so the uncapped one is untouched.
# usage: bash clickhouse_capped.sh load            (uncapped server, 8 GB, loads tbf and s_vis_1 from prep/, then stops)
#        bash clickhouse_capped.sh start CAP       (docker --memory = --memory-swap = CAP, e.g. 675m, after the data
#                                                   files are dropped from the page cache)
#        bash clickhouse_capped.sh mem|evict|stop
# DATA is the directory holding prep/; the server's files go to OUT/clickhouse.
set -euo pipefail
DATA=${DATA:-/home/joe/code/vexact-data/bioclip2-1m}
OUT=${OUT:-/home/joe/code/vexact-capped-data}
IMAGE=clickhouse/clickhouse-server:26.10.4.207
NAME=vexact-capped-ch
HERE=$(cd "$(dirname "$0")/.." && pwd)
q() { docker exec -i $NAME clickhouse-client --multiquery "$@"; }
run() {
    docker run -d --name $NAME --cpuset-cpus ${CPUS:-8-11} --memory "$1" --memory-swap "$1" \
        --ulimit nofile=262144:262144 -e CLICKHOUSE_SKIP_USER_SETUP=1 -e CLICKHOUSE_DO_NOT_CHOWN=1 \
        -v "$OUT/clickhouse:/var/lib/clickhouse" -v "$DATA:/data:ro" \
        -v "$HERE/ch-user-files.xml:/etc/clickhouse-server/config.d/user-files.xml:ro" -p 127.0.0.1:18124:8123 $IMAGE >/dev/null
    for _ in $(seq 90); do docker exec $NAME clickhouse-client -q "SELECT 1" >/dev/null 2>&1 && break; sleep 1; done
    docker exec $NAME clickhouse-client -q "SELECT version()"
}
# drop every file of the data directory from the page cache (as root in a throwaway container: the files are uid 101's)
evict() {
    docker run --rm -v "$OUT/clickhouse:/d" --entrypoint /bin/sh $IMAGE -c \
        'find /d -type f -exec dd if={} iflag=nocache count=0 status=none \;'
}
cg() { echo "/sys/fs/cgroup/system.slice/docker-$(docker inspect -f '{{.Id}}' $NAME).scope"; }
case $1 in
load)
    mkdir -p "$OUT/clickhouse" && chmod 777 "$OUT/clickhouse"
    run 8g
    q <<'SQL'
DROP TABLE IF EXISTS tbf;
CREATE TABLE tbf (id UInt32, vec Array(BFloat16) CODEC(NONE)) ENGINE = MergeTree ORDER BY id;
INSERT INTO tbf SELECT toUInt32(rowNumberInAllBlocks()) AS id, CAST(array, 'Array(BFloat16)') FROM file('/data/prep/corpus32.npy', Npy) SETTINGS max_threads = 1, max_insert_threads = 1;
OPTIMIZE TABLE tbf FINAL;
DROP TABLE IF EXISTS s_vis_1;
CREATE TABLE s_vis_1 (id UInt32) ENGINE = Set;
INSERT INTO s_vis_1 SELECT toUInt32(c1) FROM file('/data/prep/vis_1.csv', CSV);
SQL
    q -q "SELECT table, sum(rows), formatReadableSize(sum(bytes_on_disk)) FROM system.parts WHERE active AND database = 'default' GROUP BY table"
    docker stop $NAME >/dev/null && docker rm $NAME >/dev/null
    ;;
start)
    evict
    run "$2"
    ;;
evict)
    evict
    ;;
mem)
    d=$(cg); echo "memory.current $(cat $d/memory.current) max $(cat $d/memory.max) swap.max $(cat $d/memory.swap.max)"
    grep -E '^(anon|file) ' $d/memory.stat; grep -E 'max|oom' $d/memory.events | tr '\n' ' '; echo
    ;;
stop)
    docker stop $NAME >/dev/null && docker rm $NAME >/dev/null
    ;;
esac
