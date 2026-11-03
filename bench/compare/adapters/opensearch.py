"""OpenSearch: a pinned single-node image with its security plugin, one index loaded by the bulk
API, and the frame as aggregations.

The build creates an ingest pipeline that quantises `x` and `y` by the frame's rule in Painless
and drops them, and an index of one shard and no replicas with `_source` disabled: `qx` and `qy`
as `long`, `licence` and `type` as `keyword`, `publication_year` as `integer` and `is_oa` as
`boolean`, each with doc values. Worker processes read the tier's Parquet and send it through the
pipeline in bulk requests while the index does not refresh; the index is then refreshed,
force-merged to one segment, given back its default refresh interval and flushed, and every file
in its data directory synced. All of that is timed.

A frame is one search with `size: 0`, filtered to the camera's tiles by two ranges and to the
filter by its terms and ranges, and two aggregations over it: the count grid as a `histogram` on
`qx` with a `histogram` on `qy` inside it, both at interval `2^(32 - grid_depth)`, and the tiles
as the same pair at interval `2^(32 - zoom)`, each holding a `top_hits` of K positions read from
doc values and a `bucket_selector` that keeps only the tiles of at most K. Histogram counts are
exact: each document falls in the bucket `floor(value / interval)`, which for a power-of-two
interval and a value below 2^53 is exact in double precision, and with one shard nothing is
merged from shards that each kept only their top buckets.

The aggregation workload is one search with a `terms` aggregation on `type` (every type, a missing
type as `none`) and a year histogram at interval 1 inside a `filter` on the binned range, beside a
`filter` for the years outside it and a `missing` aggregation for no year.

Access: the security plugin with the image's demo certificates and its `admin` user. After the
build the admin creates the internal user `viewer`, a role `viewer_read` with `read` on the index
and a document-level security query keeping the documents whose `licence` is one of the viewer's
terms, and a mapping of the role to the user. Every frame and table is a search sent with the
viewer's credentials, and no search names a licence. `access_checks` records, from the running
server, that the restriction holds under aggregation and what the shard request cache did with
the viewer's searches.
"""

from __future__ import annotations

import json
import os
import secrets
import subprocess
import time
from concurrent.futures import ProcessPoolExecutor, ThreadPoolExecutor
from pathlib import Path

import numpy as np
import pyarrow.parquet as pq
import requests
import urllib3

from frame import EXTENT, K, TYPES, YEAR_BINS, Camera, Filter, Frame, fixed32, grid_depth, points_by_tile
from oracle import VIEWER

from .base import MEMORY, Adapter, Container, docker_info, du_bytes, evict_files, fsync_tree, wait_for

IMAGE = "opensearchproject/opensearch:3.9.0"
IMAGE_DIGEST = "sha256:3b55f6b6249cfdb92b52ee59adc8a266203ba9ef9f5d82544fad6f1af2f3609e"
PORT = 19200
URL = f"https://127.0.0.1:{PORT}"
INDEX = "points"
PIPELINE = "quantise"
VIEWER_USER = "viewer"
VIEWER_ROLE = "viewer_read"
HOME = "/usr/share/opensearch"
#: Half the container's memory, at most 31 GB, as OpenSearch's documentation recommends.
HEAP = f"{min(int(MEMORY.rstrip('g')) // 2, 31)}g"
#: Enough buckets for the largest grid: 20 tiles of 256 by 256 cells, and the columns above them.
MAX_BUCKETS = 1 << 21
#: Bulk requests of this many documents (about 7 MB), from this many client processes.
BULK_DOCS = 50_000
BULK_CLIENTS = 6
#: The two result caches, off for the compute figures. Both are static index settings.
COMPUTE_CACHES = {"index.requests.cache.enable": False, "index.queries.cache.enabled": False}
#: The settings of the caches a search can meet; the rest of what `include_defaults` lists with
#: "cache" in its name belongs to plugins this workload does not touch.
CACHE_PREFIXES = ("index.", "indices.", "plugins.security.", "script.", "node.search.")
MAPPING = {
    "_source": {"enabled": False},
    "dynamic": "strict",
    "properties": {"qx": {"type": "long"}, "qy": {"type": "long"},
                   "licence": {"type": "keyword"}, "type": {"type": "keyword"},
                   "publication_year": {"type": "integer"}, "is_oa": {"type": "boolean"}},
}
QUANTISE = """
for (String a : ['x', 'y']) {
  double lo = params[a + '_lo'];
  double f = Math.floor((ctx[a] - lo) / (params[a + '_hi'] - lo) * 4294967296.0);
  ctx['q' + a] = f <= 0 ? 0L : (long) Math.min(f, 4294967295.0);
  ctx.remove(a);
}
"""
PIPELINE_BODY = {
    "description": "x and y onto the frame's 2^32 grid, as frame.fixed32",
    "processors": [{"script": {"lang": "painless", "source": QUANTISE, "params": {
        "x_lo": EXTENT[0], "x_hi": EXTENT[1], "y_lo": EXTENT[2], "y_hi": EXTENT[3]}}}],
}

urllib3.disable_warnings(urllib3.exceptions.InsecureRequestWarning)


def filter_query(filt: Filter | None) -> list[dict]:
    out = []
    for leaf in (filt.leaves if filt else []):
        col, value = leaf.get("col"), leaf["value"]
        if leaf["op"] == "eq":
            out.append({"term": {col: value}})
        elif leaf["op"] == "in":
            out.append({"terms": {col: value}})
        elif leaf["op"] == "range":
            out.append({"range": {col: {k: int(v) for k, v in value.items()}}})
        elif leaf["op"] == "bbox":
            x0, y0, x1, y1 = (float(v) for v in value)
            out.append({"range": {"qx": {"gte": fixed32(x0), "lte": fixed32(x1)}}})
            out.append({"range": {"qy": {"gte": fixed32(y0, EXTENT[2], EXTENT[3]),
                                         "lte": fixed32(y1, EXTENT[2], EXTENT[3])}}})
        else:
            raise ValueError(leaf)
    return out


def where(camera: Camera, filt: Filter | None) -> dict:
    """The camera's tiles and the filter, as a filter-context query."""
    tx0, tx1, ty0, ty1 = camera.tile_range()
    s = 32 - camera.zoom
    tiles = [{"range": {"qx": {"gte": tx0 << s, "lte": ((tx1 + 1) << s) - 1}}},
             {"range": {"qy": {"gte": ty0 << s, "lte": ((ty1 + 1) << s) - 1}}}]
    return {"bool": {"filter": tiles + filter_query(filt)}}


def cells(depth: int, inner: dict | None = None) -> dict:
    """Nested histograms on `qx` then `qy`, one bucket per occupied depth-`depth` cell."""
    hist = lambda f: {"field": f, "interval": 1 << (32 - depth), "min_doc_count": 1}
    qy = {"histogram": hist("qy")}
    if inner:
        qy["aggs"] = inner
    return {"histogram": hist("qx"), "aggs": {"qy": qy}}


def frame_request(camera: Camera, filt: Filter | None) -> dict:
    sparse = {"top": {"top_hits": {"size": K, "sort": ["_doc"], "_source": False,
                                   "docvalue_fields": ["qx", "qy"]}},
              "sparse": {"bucket_selector": {"buckets_path": {"n": "_count"},
                                             "script": f"params.n <= {K}"}}}
    return {"size": 0, "track_total_hits": False, "query": where(camera, filt),
            "aggs": {"grid": cells(grid_depth(camera.zoom)), "tiles": cells(camera.zoom, sparse)}}


def tables_request(camera: Camera, filt: Filter | None) -> dict:
    lo, hi = YEAR_BINS
    binned = {"range": {"publication_year": {"gte": lo, "lt": hi}}}
    return {"size": 0, "track_total_hits": False, "query": where(camera, filt), "aggs": {
        "type": {"terms": {"field": "type", "size": len(TYPES) + 1, "missing": "none"}},
        "binned": {"filter": binned, "aggs": {"year": {"histogram": {
            "field": "publication_year", "interval": 1, "min_doc_count": 1}}}},
        "rest": {"filter": {"bool": {"filter": [{"exists": {"field": "publication_year"}}],
                                     "must_not": [binned]}}},
        "none": {"missing": {"field": "publication_year"}}}}


def load_part(tier: str, groups: list[int], auth: tuple[str, str]) -> int:
    """Send the tier's row groups `groups` through the pipeline in bulk requests. Runs in a worker
    process."""
    head = json.dumps({"index": {"_index": INDEX}})
    columns = ["x", "y", "licence", "publication_year", "type", "is_oa"]
    sent = 0
    with requests.Session() as http:
        http.auth, http.verify = auth, False
        for batch in pq.ParquetFile(tier).iter_batches(BULK_DOCS, row_groups=groups, columns=columns):
            lines = []
            for row in batch.to_pylist():
                lines.append(head)
                lines.append(json.dumps({k: v for k, v in row.items() if v is not None}))
            r = http.post(f"{URL}/_bulk", params={"pipeline": PIPELINE},
                          data=("\n".join(lines) + "\n").encode(),
                          headers={"Content-Type": "application/x-ndjson"}, timeout=3600)
            body = r.json() if r.ok else None
            if body is None or body["errors"]:
                item = next((i for i in (body or {}).get("items", []) if "error" in i["index"]), None)
                raise RuntimeError(f"bulk load failed: {r.status_code} {item or r.text[:800]}")
            sent += len(body["items"])
    return sent


class OpenSearch(Adapter):
    name = "opensearch"
    access = (f"security plugin: role {VIEWER_ROLE} with read on {INDEX} and a document-level "
              f"security query licence in (the viewer's terms), mapped to the internal user "
              f"{VIEWER_USER}; every frame and table searched as {VIEWER_USER}")

    def __init__(self, tier: Path, work: Path, nice: int = 0, caches: str = "compute"):
        super().__init__(tier, work, nice, caches)
        self.index_made = False
        self.container = Container(f"bench-compare-os-{os.getpid()}")
        self.data = self.work / "os-data"
        self.logs = self.work / "os-logs"
        # The security plugin refuses a weak initial admin password.
        self.admin_auth = ("admin", "Aa1!" + secrets.token_hex(16))
        self.viewer_auth = (VIEWER_USER, "Vv1!" + secrets.token_hex(16))
        self.admin = self._session(self.admin_auth)
        self.viewer: requests.Session | None = None
        self.access_checks: dict = {}

    @staticmethod
    def _session(auth: tuple[str, str]) -> requests.Session:
        s = requests.Session()
        s.auth, s.verify = auth, False
        return s

    # -- the container ---------------------------------------------------------------------------

    def _create(self, nice: int = 0) -> None:
        """Create the container, its server at niceness `nice`: the build's container, which is
        removed once the index is built, and the serving one, created at 0."""
        for d in (self.data, self.logs):
            d.mkdir(parents=True, exist_ok=True)
        self.container.create(
            f"{IMAGE.split(':')[0]}@{IMAGE_DIGEST}", entrypoint=f"{HOME}/opensearch-docker-entrypoint.sh",
            args=("opensearch",), nice=nice, ports={PORT: 9200},
            mounts={str(self.data): f"{HOME}/data", str(self.logs): f"{HOME}/logs"},
            env={"discovery.type": "single-node", "OPENSEARCH_JAVA_OPTS": f"-Xms{HEAP} -Xmx{HEAP}",
                 "OPENSEARCH_INITIAL_ADMIN_PASSWORD": self.admin_auth[1],
                 "DISABLE_PERFORMANCE_ANALYZER_AGENT_CLI": "true"})

    def start(self) -> None:
        """Start the container, ready once the admin's search of the index answers, or before the
        index exists, once the admin is authenticated. The viewer's first authentication and the
        resolution of its role fall in its first search, after `connect`."""
        self.container.start()
        wait_for(self._ready, 600, f"OpenSearch answering; see {self.logs}")

    def _ready(self) -> bool:
        """One probe on a connection of its own, closed after it."""
        close = {"Connection": "close"}
        try:
            if not self.index_made:
                r = requests.get(f"{URL}/", auth=self.admin_auth, verify=False, headers=close, timeout=60)
            else:
                r = requests.post(f"{URL}/{INDEX}/_search", auth=self.admin_auth, verify=False,
                                  headers=close, json={"size": 1, "terminate_after": 1}, timeout=60)
            return r.ok
        except requests.RequestException:
            return False

    def connect(self) -> None:
        """OpenSearch has no session to open: the viewer's credential travels with each search on
        this connection, and the server caches the authenticated user."""
        self.viewer = self._session(self.viewer_auth)

    def stop(self) -> None:
        if self.viewer is not None:
            self.viewer.close()
            self.viewer = None
        self.admin.close()
        self.container.stop()

    def remove(self) -> None:
        self.container.remove()

    def evict(self) -> None:
        evict_files(self.data)

    def serve_memory(self) -> dict:
        return self.container.memory()

    # -- requests ----------------------------------------------------------------------------------

    def call(self, method: str, path: str, body=None, http: requests.Session | None = None,
             timeout: float = 3600, **params):
        r = (http or self.admin).request(method, f"{URL}{path}", json=body, params=params or None,
                                         timeout=timeout)
        if not r.ok:
            raise RuntimeError(f"OpenSearch {method} {path}: {r.status_code} {r.text[:800]}")
        return r.json()

    def version(self) -> dict:
        v = self.call("GET", "/")["version"]
        return {"version": v["number"], "lucene": v["lucene_version"], "build": v["build_hash"],
                "image": IMAGE, "digest": IMAGE_DIGEST, "docker": docker_info()}

    def config(self) -> dict:
        index = self.call("GET", f"/{INDEX}/_settings", include_defaults="true", flat_settings="true")[INDEX]
        cluster = self.call("GET", "/_cluster/settings", include_defaults="true", flat_settings="true")
        cache = lambda s: {k: v for k, v in s.items() if "cache" in k and k.startswith(CACHE_PREFIXES)}
        named = ("search.max_buckets", "search.concurrent_segment_search.mode",
                 "thread_pool.search.size", "node.processors", "http.compression")
        node = {**cluster["defaults"], **cluster["persistent"]}
        return {"caches": self.caches, "heap": HEAP, "container_memory": MEMORY,
                "index_settings": index["settings"],
                "index_cache_settings": cache(index["defaults"]),
                "cluster_settings": cluster["persistent"],
                "node_cache_settings": cache(node),
                "node_settings": {k: node.get(k) for k in named},
                "mapping": MAPPING, "pipeline": PIPELINE_BODY,
                "bulk": {"docs": BULK_DOCS, "clients": BULK_CLIENTS},
                "role": self.call("GET", f"/_plugins/_security/api/roles/{VIEWER_ROLE}"),
                "access_checks": self.access_checks,
                "performance_analyzer_agent": "off"}

    def build(self) -> dict:
        self.remove()
        self.index_made = False
        subprocess.run(["rm", "-rf", str(self.data), str(self.logs)], check=True)
        self._create(self.nice)
        self.start()
        self.call("PUT", "/_cluster/settings", {"persistent": {"search.max_buckets": MAX_BUCKETS}})
        settings = {"index.number_of_shards": 1, "index.number_of_replicas": 0,
                    "index.refresh_interval": "-1", "index.max_inner_result_window": K,
                    **(COMPUTE_CACHES if self.caches == "compute" else {})}
        groups = list(range(pq.ParquetFile(self.tier).metadata.num_row_groups))
        t0 = time.monotonic()
        self.call("PUT", f"/_ingest/pipeline/{PIPELINE}", PIPELINE_BODY)
        self.call("PUT", f"/{INDEX}", {"settings": settings, "mappings": MAPPING})
        with ProcessPoolExecutor(BULK_CLIENTS, initializer=os.nice, initargs=(self.nice,)) as pool:
            parts = [pool.submit(load_part, str(self.tier), groups[i::BULK_CLIENTS], self.admin_auth)
                     for i in range(BULK_CLIENTS)]
            sent = sum(p.result() for p in parts)
        t_load = time.monotonic() - t0
        self.call("POST", f"/{INDEX}/_refresh")
        self.call("POST", f"/{INDEX}/_forcemerge", max_num_segments=1)
        self.call("PUT", f"/{INDEX}/_settings", {"index.refresh_interval": None})
        self.call("POST", f"/{INDEX}/_flush", wait_if_ongoing="true", force="true")
        fsync_tree(self.data)
        build_s = time.monotonic() - t0
        self.index_made = True
        mem = self.serve_memory()
        rows = self.call("GET", f"/{INDEX}/_count")["count"]
        segments = self.call("GET", f"/{INDEX}/_segments")["indices"][INDEX]["shards"]["0"][0]["segments"]
        self._make_viewer()
        self.access_checks = self._check_access(rows)
        disk = du_bytes(self.data)
        index_bytes = self.call("GET", f"/{INDEX}/_stats/store")["indices"][INDEX]["primaries"]["store"]["size_in_bytes"]
        self.stop()
        self._create()
        return {"build_s": build_s, "load_s": t_load, **mem, "disk_bytes": disk,
                "disk": "du of the data directory, which holds the security index too",
                "index_bytes": index_bytes, "rows": rows, "rows_sent": sent,
                "segments": len(segments), "nice": self.nice,
                "boundary": "from creating the pipeline and the index to the end of refresh, "
                            "force-merge, flush and an fsync of every file and directory in the "
                            "data directory, on a running server; the bulk clients run on the host"}

    def _make_viewer(self) -> None:
        api = "/_plugins/_security/api"
        dls = json.dumps({"terms": {"licence": VIEWER["terms"]}})
        self.call("PUT", f"{api}/internalusers/{VIEWER_USER}", {"password": self.viewer_auth[1]})
        self.call("PUT", f"{api}/roles/{VIEWER_ROLE}", {"index_permissions": [
            {"index_patterns": [INDEX], "dls": dls, "allowed_actions": ["read"]}]})
        self.call("PUT", f"{api}/rolesmapping/{VIEWER_ROLE}", {"users": [VIEWER_USER]})

    def _check_access(self, rows: int) -> dict:
        """What the running server does with the viewer's searches: the documents an aggregation
        counts as the admin and as the viewer, beside the admin's count of the viewer's terms; and
        the shard request cache's hits and misses over the same aggregation sent twice by the
        admin, then twice by the viewer."""
        viewer = self._session(self.viewer_auth)
        body = {"size": 0, "track_total_hits": False,
                "aggs": {"licence": {"terms": {"field": "licence", "size": 100}}}}
        total = lambda r: sum(b["doc_count"] for b in r["aggregations"]["licence"]["buckets"])
        stats = lambda: self.call("GET", f"/{INDEX}/_stats/request_cache,query_cache")["indices"][INDEX]["primaries"]
        out = {"rows": rows}
        out["admin_terms_rows"] = self.call("POST", f"/{INDEX}/_count", {
            "query": {"terms": {"licence": VIEWER["terms"]}}})["count"]
        before = stats()
        out["admin_agg_rows"] = [total(self.call("POST", f"/{INDEX}/_search", body)) for _ in range(2)]
        mid = stats()
        out["viewer_agg_rows"] = [total(self.call("POST", f"/{INDEX}/_search", body, viewer)) for _ in range(2)]
        after = stats()
        viewer.close()
        rc = lambda a, b: {k: b["request_cache"][k] - a["request_cache"][k] for k in ("hit_count", "miss_count")}
        out["request_cache_admin"] = rc(before, mid)
        out["request_cache_viewer"] = rc(mid, after)
        out["query_cache_after"] = {k: after["query_cache"][k] for k in ("hit_count", "miss_count", "cache_count")}
        out["dls_holds"] = (out["viewer_agg_rows"] == [out["admin_terms_rows"]] * 2
                            and out["admin_agg_rows"] == [rows] * 2)
        if not out["dls_holds"]:
            raise RuntimeError(f"the viewer's aggregation is not restricted to its terms: {out}")
        return out

    # -- a frame -----------------------------------------------------------------------------------

    def search(self, body: dict) -> bytes:
        r = self.viewer.post(f"{URL}/{INDEX}/_search", json=body, timeout=3600)
        if r.status_code != 200:
            raise RuntimeError(f"OpenSearch {r.status_code}: {r.text[:800]}")
        return r.content

    def frame(self, camera: Camera, filt: Filter | None) -> tuple[Frame, float]:
        body = frame_request(camera, filt)
        t0 = time.perf_counter()
        raw = self.search(body)
        elapsed = time.perf_counter() - t0
        return self._decode(raw, camera.zoom, grid_depth(camera.zoom)), elapsed

    def tables(self, camera: Camera, filt: Filter | None) -> tuple[dict, float]:
        body = tables_request(camera, filt)
        t0 = time.perf_counter()
        raw = self.search(body)
        elapsed = time.perf_counter() - t0
        aggs = json.loads(raw)["aggregations"]
        types = {b["key"]: b["doc_count"] for b in aggs["type"]["buckets"]}
        years = {str(int(b["key"])): b["doc_count"] for b in aggs["binned"]["year"]["buckets"]}
        years["rest"] = aggs["rest"]["doc_count"]
        years["none"] = aggs["none"]["doc_count"]
        return {"type": types, "year": years}, elapsed

    @staticmethod
    def _decode(raw: bytes, z: int, dg: int) -> Frame:
        aggs = json.loads(raw)["aggregations"]
        grid, tiles = {}, {}
        shift = dg - z
        for bx in aggs["grid"]["buckets"]:
            gx = int(bx["key"]) >> (32 - dg)
            for by in bx["qy"]["buckets"]:
                gy = int(by["key"]) >> (32 - dg)
                grid[(gx, gy)] = by["doc_count"]
                t = (gx >> shift, gy >> shift)
                tiles[t] = tiles.get(t, 0) + by["doc_count"]
        qx, qy = [], []
        for bx in aggs["tiles"]["buckets"]:
            for by in bx["qy"]["buckets"]:
                for hit in by["top"]["hits"]["hits"]:
                    qx.append(hit["fields"]["qx"][0])
                    qy.append(hit["fields"]["qy"][0])
        points = points_by_tile(np.array(qx, np.uint32), np.array(qy, np.uint32), z) if qx else {}
        return Frame(tiles=tiles, grid=grid, points=points, bytes=len(raw))
