"""`mosaica demo NAME...`: datasets of `mosaica.datasets`, made where they have not been, served on
this machine and shown in a browser, with a picker for the dataset and for whom to view it as.

The `mosaica` command that the `mosaica-native` wheel installs hands `demo` to `main` here and
every other command to the binary. The page, `demo.html`, is the components' `<mosaica-explorer>`
under a header, served by this process on 127.0.0.1 with the bundle `mosaica[widget]` packages.
It reads `/site.json`, which names each dataset's own server and the personas of its catalogue
entry, and asks `POST /token?dataset=ID&persona=ID` for a token holding that persona's terms,
which this process makes with the database's operator credential. Any process on this machine can
ask for one, as any can read an open server.

This page server answers only a request whose `Host` is this machine's, and makes a token only for
a request sent as `application/json`, which a page from another site cannot send without the
browser asking first.
"""

from __future__ import annotations

import argparse
import http.server
import json
import math
import sys
import webbrowser
from pathlib import Path
from typing import Optional, Sequence
from urllib.parse import parse_qs, urlsplit

from ._refusal import Refusal

PAGE = Path(__file__).with_name("demo.html")
LINKS = {
    "docs": "https://docs.mosaicadb.com/",
    "repo": "https://github.com/mosaicadb/mosaica",
}
#: The attributes set on each dataset's `<mosaica-explorer>`: the field that titles an item, the
#: field that colours the points and the filters the panel always shows.
EXPLORER = {
    "places": {"title-field": "name", "colour-by": "feature_class"},
    "arxiv-sample": {
        "title-field": "title",
        "colour-by": "archive",
        "pinned-filters": "archive title submitted_at",
    },
    "geonames": {"title-field": "name", "colour-by": "feature_class"},
    "gbif": {"title-field": "scientificname", "colour-by": "kingdom"},
}


def _table() -> str:
    from .datasets import _CATALOGUE

    rows = [("name", "items", "download", "on disk", "bundle", "needs")]
    for d in _CATALOGUE:
        rows.append(
            (d.name, f"{d.items:,}", _size(d.download), _size(d.disk), _size(d.bundle),
             ", ".join(d.needs) or "-")
        )
    widths = [max(len(row[i]) for row in rows) for i in range(len(rows[0]))]
    lines = ["  ".join(c.ljust(w) for c, w in zip(row, widths)).rstrip() for row in rows]
    for d in _CATALOGUE:
        if d.per_sample is not None:
            download, disk = (_size(n) for n in d.per_sample)
            lines.append(
                f"\n{d.name}:N takes {d.sample}: about {download} to download and {disk} on disk "
                "for each"
            )
    return "\n".join(lines)


def _size(n: int) -> str:
    if n == 0:
        return "-"
    for unit, scale in (("GB", 1e9), ("MB", 1e6), ("kB", 1e3)):
        if n >= scale:
            return f"{n / scale:.1f} {unit}"
    return f"{n} B"


def _named(argument: str) -> tuple[str, Optional[int]]:
    """`NAME` or `NAME:N`, as the command line takes a dataset."""
    name, colon, sample = argument.partition(":")
    if not colon:
        return name, None
    if not sample.isdigit():
        raise argparse.ArgumentTypeError(
            f"{argument!r}: write a sample as NAME:N with N a whole number, as gbif:64"
        )
    return name, int(sample)


def _site(title: str, served: dict) -> dict:
    """What the page reads at `/site.json`: each dataset's server and personas, by its id."""
    entries = []
    for id, (dataset, sample, database) in served.items():  # noqa: A001
        summary = "" if sample is None else f"Built from {dataset.sample}, with N = {sample:,}."
        entries.append(
            {
                "id": id,
                "name": id,
                "items": dataset.title,
                "summary": summary,
                "sources": [{"label": dataset.publisher, "url": "", "note": dataset.licence}],
                "viewer_url": database.viewer_url,
                "default_persona": dataset.personas[0].id,
                "explorer": EXPLORER.get(dataset.name, {}),
                "personas": [
                    {"id": p.id, "label": p.label, "description": p.description}
                    for p in dataset.personas
                ],
            }
        )
    return {"title": title, "links": LINKS, "default_dataset": entries[0]["id"], "datasets": entries}


def _handler(served: dict, page: bytes, site: bytes, bundle: bytes):
    class Handler(http.server.BaseHTTPRequestHandler):
        def log_message(self, *_args) -> None:
            pass

        def _send(self, status: int, kind: str, body: bytes) -> None:
            self.send_response(status)
            self.send_header("Content-Type", kind)
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(body)

        def _refuse(self, status: int, error: str) -> None:
            self._send(status, "application/json", json.dumps({"error": error}).encode())

        def _ours(self) -> bool:
            port = self.server.server_address[1]
            if self.headers.get("Host") in (f"127.0.0.1:{port}", f"localhost:{port}"):
                return True
            self._refuse(403, "this page is served to this machine only")
            return False

        def do_GET(self) -> None:  # noqa: N802
            if not self._ours():
                return
            path = urlsplit(self.path).path
            if path in ("/", "/index.html"):
                self._send(200, "text/html; charset=utf-8", page)
            elif path == "/site.json":
                self._send(200, "application/json", site)
            elif path == "/mosaica-components.js":
                self._send(200, "text/javascript; charset=utf-8", bundle)
            else:
                self._refuse(404, "not found")

        def do_POST(self) -> None:  # noqa: N802
            if not self._ours():
                return
            url = urlsplit(self.path)
            if url.path != "/token":
                self._refuse(404, "not found")
                return
            kind = (self.headers.get("Content-Type") or "").split(";")[0].strip().lower()
            if kind != "application/json":
                self._refuse(415, "send the request as application/json")
                return
            query = parse_qs(url.query)
            entry = served.get(query.get("dataset", [""])[0])
            personas = {} if entry is None else {p.id: p for p in entry[0].personas}
            persona = personas.get(query.get("persona", [""])[0])
            if persona is None:
                self._refuse(404, "name a dataset and one of its personas in /site.json")
                return
            try:
                token = entry[2].token(None if persona.terms is None else list(persona.terms))
            except Refusal as refused:
                self._refuse(502, str(refused))
                return
            expires = token.expires_at
            body = {
                "token": token.token,
                "expiresAt": expires if expires is not None and math.isfinite(expires) else None,
            }
            self._send(200, "application/json", json.dumps(body).encode())

    return Handler


def main(argv: Optional[Sequence[str]] = None) -> int:
    from .datasets import load, names

    parser = argparse.ArgumentParser(
        prog="mosaica demo",
        description=(
            "Fetch, prepare and build datasets where that has not been done, serve them on this "
            "machine and open their map in a browser, with a picker for the dataset and for whom "
            "to view it as."
        ),
        epilog="The datasets: " + ", ".join(names()) + ". `--list` says what each one takes.",
    )
    parser.add_argument(
        "datasets", nargs="*", type=_named, metavar="NAME[:N]",
        help="the datasets to show; NAME:N builds from a sample of one that takes it, as gbif:64",
    )
    parser.add_argument("--list", action="store_true", help="list the datasets and their sizes")
    parser.add_argument("--dir", help="where datasets are kept; the user cache by default")
    parser.add_argument("--title", default="Mosaica demo", help="the page's title")
    parser.add_argument("--port", type=int, default=0, help="the port the map page is on")
    parser.add_argument("--no-open", action="store_true", help="print the address, open nothing")
    args = parser.parse_args(argv)

    if args.list or not args.datasets:
        print(_table())
        return 0

    from ._bundle import bundle_path
    from .datasets import _checked

    chosen = list(dict.fromkeys(args.datasets))
    served: dict = {}
    try:
        bundle = bundle_path()
        if bundle is None:
            raise Refusal(
                "demo: this mosaica has no map bundle in mosaica/static/; reinstall it with "
                "`pip install --force-reinstall mosaica`, or in a checkout run "
                "`npm run bundle -w @mosaicajs/components` in clients/ts"
            )
        datasets = [(_checked(name, sample), sample) for name, sample in chosen]
        for dataset, sample in datasets:
            download, disk = dataset.download, dataset.disk
            if sample is not None and dataset.per_sample is not None:
                download, disk = (sample * n for n in dataset.per_sample)
            print(
                f"mosaica: {dataset.name} downloads up to {_size(download)}, less what is on "
                f"disk already, and takes about {_size(disk)} on disk once built"
            )
            database = load(dataset.name, dir=args.dir, sample=sample)
            key = dataset.name if sample is None else f"{dataset.name}:{sample}"
            served[key] = (dataset, sample, database)
            database.serve()

        site = json.dumps(_site(args.title, served)).encode()
        handler = _handler(served, PAGE.read_bytes(), site, bundle.read_bytes())
        server = http.server.ThreadingHTTPServer(("127.0.0.1", args.port), handler)
        address = f"http://127.0.0.1:{server.server_address[1]}/"
        print(f"mosaica: the map is at {address}; Ctrl-C stops it", flush=True)
        if not args.no_open:
            webbrowser.open(address)
        try:
            server.serve_forever()
        except KeyboardInterrupt:
            pass
        finally:
            server.server_close()
    except Refusal as refused:
        print(refused, file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        print("mosaica: stopped", file=sys.stderr)
        return 130
    finally:
        for _, _, database in served.values():
            database.close()
    return 0

if __name__ == "__main__":
    sys.exit(main())
