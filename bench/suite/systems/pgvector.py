# PostgreSQL with pgvector, as a database baseline (full suite, two datasets). No vector index: exact k-NN is a
# sequential scan ordered by the negative inner product (ORDER BY v <#> q LIMIT k), a threshold query is
# WHERE (v <#> q) <= -t, and a visible set is a table of ids joined in (id IN (SELECT id FROM vis_<name>)).
# One worker (max_parallel_workers_per_gather = 0). Timing follows the earlier threshold-db/dbrun.py: at the client, from
# sending the SQL to holding the ids as a numpy array. Pin the server to the timing core yourself (taskset on the
# postmaster before the run); the client runs where the suite runs.
# Connection: params["dsn"] or $PGVECTOR_DSN (e.g. "host=127.0.0.1 dbname=bench user=bench"); needs psycopg (3).
import os

import numpy as np

from .base import System, Unavailable


def lit(q):
    return "[" + ",".join(repr(float(x)) for x in np.asarray(q, np.float32)) + "]"


class PgVector(System):
    name = "pgvector"
    types = ("exact", "threshold")

    def build(self, threads):
        try:
            import psycopg
        except ImportError as e:
            raise Unavailable(f"psycopg is not installed ({e})")
        dsn = self.params.get("dsn") or os.environ.get("PGVECTOR_DSN")
        if not dsn:
            raise Unavailable("no PostgreSQL connection given (params dsn or $PGVECTOR_DSN)")
        try:
            self.con = psycopg.connect(dsn, autocommit=True)
        except Exception as e:
            raise Unavailable(f"cannot connect to PostgreSQL: {e}")
        c = self.con.cursor()
        c.execute("CREATE EXTENSION IF NOT EXISTS vector")
        self.table = "t_" + "".join(ch if ch.isalnum() else "_" for ch in os.path.basename(self.prep.path))
        X = self.prep.corpus()
        # the table is reused across runs while it holds this corpus (its comment: N, d and a checksum)
        tag = f"{X.shape[0]}:{X.shape[1]}:{float(np.asarray(X[:1000], np.float64).sum()):.9g}"
        c.execute(f"SELECT obj_description(to_regclass('{self.table}'), 'pg_class') WHERE to_regclass('{self.table}') IS NOT NULL")
        row = c.fetchone()
        if row is None or row[0] != tag:
            c.execute(f"DROP TABLE IF EXISTS {self.table}")
            c.execute(f"CREATE TABLE {self.table} (id integer PRIMARY KEY, v vector({self.prep.d}))")
            with c.copy(f"COPY {self.table} (id, v) FROM STDIN") as cp:
                for i in range(X.shape[0]):
                    cp.write_row((i, lit(X[i])))
            c.execute(f"VACUUM ANALYZE {self.table}")
            c.execute(f"COMMENT ON TABLE {self.table} IS '{tag}'")
        c.execute(f"SET max_parallel_workers_per_gather = {threads}")
        return dict(table=self.table)

    def single_thread(self):
        self.vname = None
        c = self.con.cursor()
        c.execute("SET max_parallel_workers_per_gather = 0")
        c.execute("SET enable_indexscan = off")
        self.cur = c

    def set_visible(self, ids, name=None):
        self.vis, self.vname = ids, None
        if ids is None:
            return
        self.vname = f"vis_{self.table}_{name.replace('-', '_')}"
        c = self.con.cursor()
        c.execute(f"DROP TABLE IF EXISTS {self.vname}")
        c.execute(f"CREATE TABLE {self.vname} (id integer PRIMARY KEY)")
        with c.copy(f"COPY {self.vname} (id) FROM STDIN") as cp:
            for i in ids:
                cp.write_row((int(i),))
        c.execute(f"ANALYZE {self.vname}")

    def _where(self):
        return f"id IN (SELECT id FROM {self.vname})" if self.vname else "TRUE"

    def _ids(self, sql):
        self.cur.execute(sql)
        return np.array([r[0] for r in self.cur.fetchall()], dtype=np.int64)

    def knn(self, q, k):
        return self._ids(f"SELECT id FROM {self.table} WHERE {self._where()} ORDER BY v <#> '{lit(q)}' LIMIT {k}")

    def range(self, q, t):
        return self._ids(f"SELECT id FROM {self.table} WHERE {self._where()} AND (v <#> '{lit(q)}') <= {-float(t)!r}")

    def close(self):
        if getattr(self, "con", None):
            self.con.close()
