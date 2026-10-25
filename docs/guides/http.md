# Query Mosaica over HTTP

This guide sends each of Mosaica's read requests by hand with curl, against the 50,000 arXiv papers that ship in the repository. You would do this to write a client in a language Mosaica has no client for, to call Mosaica from a backend, or to see exactly what the Python and TypeScript clients send for you.

You'll need the `mosaica` binary from [the first tutorial](../start/first-map.md), the repository cloned at `~/mosaica`, curl, and Python 3 with pyarrow.

## What we'll do

1. Build the arXiv example and serve it.
2. Make a principal with a password, and sign in as it.
3. Read the deployment's shape from `/v1/meta`.
4. Draw tiles of the map, then filter and highlight them.
5. Turn category codes into values, and look values up as a user types.
6. Read one paper's card, then read every match page by page.
7. Count papers by archive and by year.
8. Fetch the topics drawn over the map.
9. See what a refusal looks like.

## Serve the arXiv example

The papers are in `examples/data/arxiv`, with a declaration, `schema.toml`, that describes them. Each paper has a position in two layouts, called views, and carries its arXiv archive, its subject class, its submission date, its title and its arXiv identifier. Three annotation layers lie over the papers: Toponymy's clusters at four levels, a name for each cluster, and arXiv's own classification. The [NOTICE](https://github.com/mosaicadb/mosaica/blob/main/examples/data/arxiv/NOTICE.md) beside the data says where each part came from.

Make a directory for the deployment and save this as `mosaica.toml` in it. It is the tutorial's configuration, with the declaration pointing into the clone.

```bash
mkdir ~/arxiv && cd ~/arxiv
```

```toml
[bundle]
path  = "bundle"
cache = "cache"
wal   = "wal.log"

[build]
schema = "../mosaica/examples/data/arxiv/schema.toml"

[disclosure]
token_max_lifetime = 3600

[serve]
viewer                   = "127.0.0.1:9141"
session                  = "127.0.0.1:9142"
control                  = "127.0.0.1:9143"
operator_credential_file = "operator.secret"

[catalogue]
dir = "catalogue"
```

Build it. The build prints a long report, and the last lines are the ones to check.

```bash
mosaica build 2>&1 | tail -n 3
```

```
built /home/you/arxiv/bundle (v00000): 50000 items, 175 terms, 86144 pairs, 15007618 bytes on disk, 0 artifact(s) minted, 0 unclustered member row(s), 0 row(s) refused
  view topics: 50000 row(s)
  view topics-wide: 50000 row(s)
```

Make the operator's secret and start the server in a terminal of its own.

```bash
openssl rand -hex 16 > operator.secret
chmod 600 operator.secret
mosaica serve
```

```
...
{"event":"listening","viewer":"127.0.0.1:9141","session":"127.0.0.1:9142","control":"127.0.0.1:9143"}
```

Every request in this guide goes to the viewer address, port 9141. The control address, 9143, is used once, to make the principal.

## Sign in

Each request on the viewer address carries a session token, and the token decides what the answer is computed over. Signing in with a password is the simplest way to get one. The [access control chapter](../system/access-control.md#getting-a-token) describes the other two: an API key, and a session your own backend mints for a user.

In a second terminal, make a principal called `ada` with a password. Grant it `read`, which lets it sign in and read, and `read-all`, which lets it see every paper whatever the paper's access labels. A principal holding terms in place of `read-all` would see only the papers its terms allow, and every count below would be computed over those.

```bash
cd ~/arxiv
export MOSAICA_CREDENTIAL=$(cat operator.secret)
mosaica principal create ada --kind person --control http://127.0.0.1:9143
echo 'correct horse battery staple' | mosaica principal set-password ada --control http://127.0.0.1:9143
mosaica grant --principal ada --permission read --control http://127.0.0.1:9143
mosaica grant --principal ada --permission read-all --control http://127.0.0.1:9143
```

```
{"sessions_ended":0}
{"sessions_ended":0}
{"sessions_ended":0}
{"sessions_ended":0}
```

Now sign in. `POST /v1/login` takes the credential in a JSON body and answers with a token and the time it expires, in seconds since 1970.

```bash
curl -s http://127.0.0.1:9141/v1/login -H 'content-type: application/json' \
  -d '{"password": {"principal": "ada", "password": "correct horse battery staple"}}'
```

```
{"token":"0411c70b159e226917d99a5db774e81812ce3366e86563826908842f5ae47750","expires_at":1792896004}
```

The token lasts an hour, the `token_max_lifetime` in `mosaica.toml`. Keep it in a shell variable, with the two headers the requests below send.

```bash
TOKEN=$(curl -s http://127.0.0.1:9141/v1/login -H 'content-type: application/json' \
  -d '{"password": {"principal": "ada", "password": "correct horse battery staple"}}' \
  | python3 -c 'import json, sys; print(json.load(sys.stdin)["token"])')
AUTH="authorization: Bearer $TOKEN"
JSON='content-type: application/json'
```

## Read the deployment's shape

`GET /v1/meta` describes the deployment as this session may know it: the views, every declared field, the fields a filter can name, the layers and the sampling settings. A client reads it when a session starts. Views and layers the session cannot reach are left out of it.

```bash
curl -s http://127.0.0.1:9141/v1/meta -H "$AUTH" -o meta.json
```

The document runs to a few kilobytes. Print the parts this guide uses.

```bash
python3 - <<EOF
import json

meta = json.load(open("meta.json"))
for view in meta["views"]:
    print("view", view["id"], view["quantisation"])
for field in meta["filter_operands"]:
    print("field", field["column"], field["family"], field["operands"])
for layer in meta["layers"]:
    print("layer", layer["name"], len(layer.get("levels") or []), "levels")
EOF
```

```
view topics {'x_max': 25.968835859298707, 'x_min': -12.610199003219606, 'y_max': 23.92222264289856, 'y_min': -14.656812219619752}
view topics-wide {'x_max': 18.218203592300416, 'x_min': -6.582906293869019, 'y_max': 17.498645114898682, 'y_min': -7.302464771270753}
field id numeric ['eq', 'in']
field archive category ['eq', 'in']
field primary_category category ['eq', 'in']
field submitted_at numeric ['eq', 'in', 'range']
field title text ['match', 'phrase']
field arxiv_id keyword ['eq', 'in', 'prefix', 'contains']
layer clusters/toponymy 4 levels
layer taxonomy/arxiv 2 levels
layer topics/toponymy 0 levels
```

`quantisation` is the box each view's positions are stored in, in the view's own coordinates. These views are UMAP layouts, so the units mean nothing beyond the layout. A geographic view's are degrees. The fields are those a filter can name, with the operators each takes. `id` is there although it is neither indexed nor rendered, because it is declared unique.

## Draw the map

The map is drawn tile by tile. At zoom 0 one tile covers the whole view. Each zoom level splits every tile into four, so zoom 2 is a grid of 4 by 4. `POST /v1/viewport` takes a view, a zoom, and either a box in the view's coordinates or a list of tile numbers, and answers for every tile the box touches.

```bash
curl -s http://127.0.0.1:9141/v1/viewport -H "$AUTH" -H "$JSON" \
  -d '{"view": "topics", "zoom": 2, "bbox": [-13, -15, 26, 24]}' -o map.bin
```

The answer is not JSON. Five routes answer in frames. Each frame is a byte saying what kind of frame it is, four bytes giving its length, and the frame itself, which is either JSON or a complete Apache Arrow stream. Arrow is a columnar format that a client in most languages can read without copying, and it carries 64-bit integers, which JSON numbers in a browser cannot hold exactly. Save this script as `frames.py`. It prints every frame of any of the five answers, and the first rows of each table.

```{.python notest}
import json
import struct
import sys

import pyarrow.ipc as ipc

NAMES = {1: "tiles", 2: "sub-cells", 3: "points", 4: "trailer", 5: "artifacts",
         6: "head", 7: "records", 8: "page end", 9: "table head"}
JSON_KINDS = {4, 6, 8, 9}


def frames(body):
    at = 0
    while at < len(body):
        kind, length = struct.unpack_from("<BI", body, at)
        yield kind, body[at + 5 : at + 5 + length]
        at += 5 + length


def show(table, rows, columns):
    table = table.select([c for c in columns if c in table.column_names] or table.column_names)
    names = table.column_names
    cells = [[str(v) for v in row.values()] for row in table.slice(0, rows).to_pylist()]
    widths = [max(len(c) for c in col) for col in zip(names, *cells)]
    for line in [names, *cells]:
        print("    " + "  ".join(c.ljust(w) for c, w in zip(line, widths)).rstrip())
    if table.num_rows > rows:
        print(f"    ... {table.num_rows - rows} more")


body = open(sys.argv[1], "rb").read()
if body.startswith(b"{"):
    sys.exit(f"refused: {body.decode()}")
rows = int(sys.argv[2]) if len(sys.argv) > 2 else 5
columns = sys.argv[3].split(",") if len(sys.argv) > 3 else []
last = None
for kind, payload in frames(body):
    last = kind
    if kind not in NAMES:
        sys.exit(f"unknown frame kind {kind}")
    if kind in JSON_KINDS:
        print(f"{NAMES[kind]}: {json.dumps(json.loads(payload))}")
    else:
        table = ipc.open_stream(payload).read_all()
        print(f"{NAMES[kind]}: {table.num_rows} rows")
        show(table, rows, columns)
if last != 4:
    sys.exit("no trailer: the answer is incomplete")
```

The optional second argument is how many rows of each table to print, and the third a list of columns. A refusal is a JSON body, and the script prints it. Run it over the map.

```bash
python3 frames.py map.bin
```

```
tiles: 15 rows
    tile  visible  matched  served  highlighted
    0     40       40       2       40
    1     2028     2028     8       2028
    4     1372     1372     7       1372
    5     2        2        2       2
    2     1595     1595     8       1595
    ... 10 more
points: 226 rows
    mosaica_id          code                 archive  primary_category  submitted_at
    547154083811181745  553996793269111883   23       102               2015-05-10 02:27:07
    858942088197156032  1010110758984411768  23       102               2023-03-28 19:08:42
    936515400207224     2269512654941749449  13       35                2021-08-17 02:20:03
    5304219825387876    2180344531753801795  12       20                2005-01-26 07:56:59
    33007250674807926   1992400071486987493  5        11                2017-09-12 15:12:31
    ... 221 more
trailer: {"arrow_serialise_ns": 56446, "flushes": 1, "points": 226, "stream_us": 9874}
```

The tiles frame comes first, with one row for each tile in the box that holds a paper. One of the sixteen holds none. `tile` is the tile's number at this zoom, the form the `tiles` field of a request takes. `visible` counts the papers this session may see in the tile, `matched` those that pass the filter, and `highlighted` those that also match the highlight. With no filter and no highlight, all three are the same.

`served` is how many points the tile drew. Mosaica draws a sample, so that a tile holding millions of items still answers quickly. Without a filter, each tile draws about the same share of its papers, so the busiest tile, with 10,286 papers, drew 53. No tile with a paper is left blank: tile 5, with two papers, drew both. The points that follow are the sample. Each has its `mosaica_id`, the identifier a client sends back to ask about it; its position, as `code`; and the fields the declaration marks `render`. A category field arrives as a number, its code in the vocabulary, which [the next section](#look-up-category-values) turns into a value. `code` holds the two grid coordinates interleaved bit by bit, and the [wire framing page](../openapi/README.md#the-framing) describes how to unpack it.

The trailer comes last, and marks the answer as complete. A body without one was cut off, whatever the connection reported.

A `mosaica_id` is a 64-bit integer. In JavaScript, read it as a `BigInt` or a string, since a `number` holds integers exactly only up to 2^53.

### Filter and highlight

Add a filter, and the counts and points change. This keeps the computer science papers.

```bash
curl -s http://127.0.0.1:9141/v1/viewport -H "$AUTH" -H "$JSON" \
  -d '{"view": "topics", "zoom": 2, "bbox": [-13, -15, 26, 24], "filters": {"archive": {"eq": "cs"}}}' -o cs.bin
python3 frames.py cs.bin
```

```
tiles: 15 rows
    tile  visible  matched  served  highlighted
    0     40       2        2       2
    1     2028     190      190     190
    4     1372     188      188     188
    5     2        0        0       0
    2     1595     156      156     156
    ... 10 more
points: 3465 rows
    mosaica_id            code                 archive  primary_category  submitted_at
    2971921996016079043   972558361724315752   13       41                2020-06-06 12:55:59
    17152784767210074029  972558408015367271   13       40                2020-05-16 18:10:48
    936515400207224       2269512654941749449  13       35                2021-08-17 02:20:03
    146449460650784274    2147913942153475493  13       46                2020-05-12 01:55:56
    147116639641387896    1904302653117552404  13       51                2011-09-20 16:59:41
    ... 3460 more
trailer: {"arrow_serialise_ns": 54207, "flushes": 1, "points": 3465, "stream_us": 1418}
```

`visible` hasn't moved. `matched` is now the computer science papers in each tile, and every one of them is drawn: 3,465 points, where the whole map drew 226. A filtered map draws every match, up to `k` points a tile. The request didn't set `k`, so it took the deployment's default, `selection.k_max_marks` in `/v1/meta`, which is 500 here. The points that matched and were drawn before are still drawn, and the rest appear around them. Paper `936515400207224` is in both answers.

A filter is a JSON expression over the fields `/v1/meta` lists, combined with `all_of`, `any_of` and `none_of`. The [queries chapter](../system/queries.md#filters) lists the operators for each kind of field.

A highlight keeps everything the filter drew and marks the points that also match a second expression. This lights up the papers with "neural" in the title.

```bash
curl -s http://127.0.0.1:9141/v1/viewport -H "$AUTH" -H "$JSON" \
  -d '{"view": "topics", "zoom": 2, "bbox": [-13, -15, 26, 24], "filters": {"archive": {"eq": "cs"}}, "highlight": {"title": {"match": "neural"}}}' -o neural.bin
python3 frames.py neural.bin 3
```

```
tiles: 15 rows
    tile  visible  matched  served  highlighted
    0     40       2        2       0
    1     2028     190      190     20
    4     1372     188      188     36
    ... 12 more
points: 3465 rows
    mosaica_id            code                 archive  primary_category  submitted_at         highlighted
    2971921996016079043   972558361724315752   13       41                2020-06-06 12:55:59  False
    17152784767210074029  972558408015367271   13       40                2020-05-16 18:10:48  False
    936515400207224       2269512654941749449  13       35                2021-08-17 02:20:03  False
    ... 3462 more
trailer: {"arrow_serialise_ns": 58736, "flushes": 1, "points": 3465, "stream_us": 2739}
```

The same 3,465 points come back, each with a `highlighted` column, and each tile counts its highlighted papers.

The response headers matter to a client that keeps tiles between requests. `x-mosaica-identity-key` changes when what the session may see changes, and when the database is rebuilt. The `ETag` changes when the data has changed in a way that could add something the client does not have. The [serving chapter](../system/serving.md#what-a-client-may-already-hold) describes both.

## Look up category values

The points carry codes: archive 13, subject class 41. `GET /v1/categories/{field}` turns codes into values, with `?codes=`.

```bash
curl -s "http://127.0.0.1:9141/v1/categories/archive?codes=13,23" -H "$AUTH"
```

```
{"column":"archive","next":null,"values":[{"code":13,"key":"cs","title":"cs"},{"code":23,"key":"math","title":"math"}]}
```

The `key` is what a filter names, and the `title` what a client shows. This vocabulary has no titles of its own, so each title is its key. Without `codes`, the route lists the values in order, a page at a time, which is how a client fills a filter list.

For a search box, `/suggest` matches what the user has typed against the start of each key, title or word in a title. With `view` and `counts=true`, each value carries how many papers this session may see with it.

```bash
curl -s "http://127.0.0.1:9141/v1/categories/primary_category/suggest?q=cs.l&view=topics&counts=true" -H "$AUTH"
```

```
{"column":"primary_category","more":false,"q":"cs.l","total":50000,"values":[{"code":50,"count":1844,"key":"cs.LG","match":{"field":"key","len":4,"start":0},"title":"cs.LG"},{"code":51,"count":214,"key":"cs.LO","match":{"field":"key","len":4,"start":0},"title":"cs.LO"}]}
```

A session runs one suggestion at a time. A second sent while the first is running is refused with `429`, so a client waits for each answer before sending the next.

## Read one paper

`POST /v1/items/{mosaica_id}` returns one item's card. This one is a highlighted point from the last map. Your `mosaica_id`s will differ from these, because every build scrambles them with a new key, so take one from your own `neural.bin`.

```bash
curl -s http://127.0.0.1:9141/v1/items/1575995087632635969 -H "$AUTH" -H "$JSON" -d '{}' | python3 -m json.tool
```

```
{
    "fields": {
        "archive": "cs",
        "arxiv_id": "2312.00778",
        "id": 2246546,
        "primary_category": "cs.CV",
        "submitted_at": 1701456953000000,
        "title": "MorpheuS: Neural Dynamic 360{\\deg} Surface Reconstruction from Monocular RGB-D Video"
    },
    "labels": [
        "cs.CV"
    ],
    "views": [
        {
            "id": "topics",
            "x": 2112514579,
            "y": 903960395
        },
        {
            "id": "topics-wide",
            "x": 2223888011,
            "y": 587965932
        }
    ],
    "scoped": {}
}
```

The card has every field the paper holds, with categories as their keys and `submitted_at` in microseconds since 1970. `labels` lists the parts of the paper's access label that let this session see it. `views` gives its position in each view on the 32-bit grid. To turn one back into the view's coordinates, take `x_min + (x + 0.5) / 2^32 × (x_max − x_min)`, with the view's `quantisation` from `/v1/meta`, and the same for `y`.

An identifier that names nothing, and one that names a paper this session may not see, get the same answer: `404` with `{"error":"unknown","detail":"unknown"}`.

## Read every match

A bulk read returns every matching item in a view, with the fields you name, in pages. This asks for the computational linguistics papers with "translation" in the title, 20 rows to a page and one page to the response. `count` adds the totals to the head.

```bash
curl -s http://127.0.0.1:9141/v1/items -H "$AUTH" -H "$JSON" \
  -d '{"view": "topics", "fields": ["arxiv_id", "title", "submitted_at"], "filters": {"all_of": [{"primary_category": {"eq": "cs.CL"}}, {"title": {"match": "translation"}}]}, "count": true, "page_rows": 20, "pages": 1}' -o page1.bin
python3 frames.py page1.bin 3
```

```
head: {"matched": 51, "order": "stored", "page_rows": 20, "visible": 50000}
records: 20 rows
    mosaica_id           arxiv_id    title                                                                                                  submitted_at
    6204859057143870051  2010.11593  A Technical Report: BUT Speech Translation Systems                                                     2020-10-22 10:52:31+00:00
    215136972040183579   2404.16257  Translation of Multifaceted Data without Re-Training of Machine Translation Systems                    2024-04-25 00:05:19+00:00
    3949594906930667878  2309.12137  OSN-MDAD: Machine Translation Dataset for Arabic Multi-Dialectal Conversations on Online Social Media  2023-09-21 14:58:50+00:00
    ... 17 more
page end: {"ended_by": "rows", "next": "DqeIHbJ8ym9yB-xEec8QbIJFNG9KddcdfWSjWRZJts8YxuWWyK-vxP32nRAZJY-FufviaTIUlJNqZA"}
trailer: {"ended_by": "pages", "next": "UQO480izAAOSJskHx4cmBFGIZKvYGAsfUx11mbEfu5qzBjYZB9XjEaIlSBMRdPhkVHa5stU50ECw0A", "pages": 1, "rows": 20, "stream_us": 3338}
```

51 papers match. Each page is a records frame followed by a page end, and the trailer's `next` is the cursor to continue from. Send the same request with `cursor` set to it, and without `count`, which only the first request of a read may carry. With no `pages`, the server sends as many pages as fit in one response.

```bash
curl -s http://127.0.0.1:9141/v1/items -H "$AUTH" -H "$JSON" \
  -d '{"view": "topics", "fields": ["arxiv_id", "title", "submitted_at"], "filters": {"all_of": [{"primary_category": {"eq": "cs.CL"}}, {"title": {"match": "translation"}}]}, "page_rows": 20, "cursor": "UQO480izAAOSJskHx4cmBFGIZKvYGAsfUx11mbEfu5qzBjYZB9XjEaIlSBMRdPhkVHa5stU50ECw0A"}' -o page2.bin
python3 frames.py page2.bin 3
```

```
head: {"order": "stored", "page_rows": 20}
records: 20 rows
    mosaica_id           arxiv_id    title                                                                                             submitted_at
    3681872383712042292  1804.02559  Guiding Neural Machine Translation with Retrieved Translation Pieces                              2018-04-07 13:20:24+00:00
    6996847706710160749  1804.05945  Near Human-Level Performance in Grammatical Error Correction with Hybrid Machine Translation      2018-04-16 21:13:31+00:00
    5051133967814377719  1805.04237  Neural Machine Translation for Bilingually Scarce Scenarios: A Deep Multi-task Learning Approach  2018-05-11 03:36:32+00:00
    ... 17 more
page end: {"ended_by": "rows", "next": "wvKSUrIlvWNhsqRZqrtaBrRcHsGPMhxaVtxlYJiNCb16gXrJCqdIhmxmCB83i9oLv3IVW4L4bilKeA"}
records: 11 rows
    mosaica_id           arxiv_id    title                                                                                              submitted_at
    8536822019196426088  1912.00609  GANCoder: An Automatic Natural Language-to-Programming Language Translation Approach based on GAN  2019-12-02 07:41:25+00:00
    2894422704918265787  2108.10417  Recurrent multiple shared layers in Depth for Neural Machine Translation                           2021-08-23 21:21:45+00:00
    9874226985201020908  2407.11855  Scaling Sign Language Translation                                                                  2024-07-16 15:36:58+00:00
    ... 8 more
page end: {"ended_by": "end", "next": null}
trailer: {"ended_by": "end", "next": null, "pages": 2, "rows": 31, "stream_us": 4204}
```

The other 31 came in two pages, and `next` is null: the read is complete. A program loops until it is. The server keeps nothing between the requests, so a read can be resumed after a dropped connection from the last page end received, and a paper deleted during a read is left out from the next page.

The fields come back as their declared types: `submitted_at` as a timestamp, and a category, had we asked for one, as its key. `"position": true` adds each item's coordinates in the view, as `mosaica:x` and `mosaica:y`.

## Count by group

`POST /v1/aggregate` counts without returning the items. A request lists groupings, and each grouping answers with one table. This one counts the papers with "quantum" in the title, then breaks them down by archive. `reference` is a second set to compare with, and `{}` means every paper this session may see.

```bash
curl -s http://127.0.0.1:9141/v1/aggregate -H "$AUTH" -H "$JSON" \
  -d '{"view": "topics", "filters": {"title": {"match": "quantum"}}, "reference": {}, "groupings": [{"count": true}, {"count": true, "by": {"field": "archive", "top": 5}}]}' -o quantum.bin
python3 frames.py quantum.bin 10
```

```
table head: {"grouping": 0, "reference_total": 50000, "resumed": false, "total": 2435}
records: 1 rows
    count  reference_count  lift
    2435   50000            1.0
page end: {"ended_by": "rows", "next": "chTlFRS22wW4SKN6E5wQ7TmVDOAeJo_4L9HWygHBUbU-sjhIz7MzXWn2i5P6i70Qj-AAJdDpiSibOwSUt7Ms9BGwJsqLqL6nuHf2BOPUnroKxUOLnA"}
table head: {"grouping": 1, "groups": 20, "reference_total": 50000, "resumed": false, "total": 2435}
records: 6 rows
    group   key       title     count  reference_count  lift
    listed  quant-ph  quant-ph  1038   2115             10.077620982422417
    listed  cond-mat  cond-mat  651    5887             2.270690753893746
    listed  hep-th    hep-th    166    2073             1.644295335252999
    listed  math      math      120    10123            0.24341259591216943
    listed  gr-qc     gr-qc     111    1177             1.9365002381371914
    rest    None      None      349    28625            0.2503519453386297
page end: {"ended_by": "end", "next": null}
trailer: {"ended_by": "end", "next": null, "pages": 2, "rows": 7, "stream_us": 878}
```

Each table starts with a table head. The first is one row: 2,435 papers match. The second lists the five archives with the most matches, then a `rest` row for the other 15 archives that hold one, so the counts add up to 2,435. `lift` is how much more common an archive is among the matches than in the reference. Quantum physics holds 4% of all the papers and 43% of these, a lift of 10.

A grouping by a date or number field counts in bins. Without a `range`, Mosaica picks readable edges around the earliest and latest value this session may see.

```bash
curl -s http://127.0.0.1:9141/v1/aggregate -H "$AUTH" -H "$JSON" \
  -d '{"view": "topics", "filters": {"archive": {"eq": "cs"}}, "groupings": [{"count": true, "by": {"field": "submitted_at", "bins": 10}}]}' -o years.bin
python3 frames.py years.bin 10
```

```
table head: {"grouping": 0, "groups": 7, "resumed": false, "total": 10478}
records: 7 rows
    group   lower                      upper                      count
    listed  1990-01-01 00:00:00+00:00  1995-01-01 00:00:00+00:00  2
    listed  1995-01-01 00:00:00+00:00  2000-01-01 00:00:00+00:00  15
    listed  2000-01-01 00:00:00+00:00  2005-01-01 00:00:00+00:00  64
    listed  2005-01-01 00:00:00+00:00  2010-01-01 00:00:00+00:00  251
    listed  2010-01-01 00:00:00+00:00  2015-01-01 00:00:00+00:00  986
    listed  2015-01-01 00:00:00+00:00  2020-01-01 00:00:00+00:00  2731
    listed  2020-01-01 00:00:00+00:00  2025-01-01 00:00:00+00:00  6429
page end: {"ended_by": "end", "next": null}
trailer: {"ended_by": "end", "next": null, "pages": 1, "rows": 7, "stream_us": 1734}
```

`bins` is the most bins to use. Mosaica takes the narrowest readable width that needs no more than that. Two years would have needed more than ten, so it chose five years, which needs seven. The edges come from every paper this session may see, ignoring the filter, so a chart of the computer science papers lines up with one of all papers beside it.

The [HTTP reference](../reference/http.md) lists the other groupings: by the artifacts of a layer, a density grid of the map's cells, and a field's smallest, largest and mean values.

## Fetch the topics

The points of a map can carry each one's topic, but the topics themselves come from `POST /v1/artifacts/viewport`. This asks for the coarsest level of Toponymy's clusters over the whole map at zoom 0, five to a tile, with the layer of names that goes with them.

```bash
curl -s http://127.0.0.1:9141/v1/artifacts/viewport -H "$AUTH" -H "$JSON" \
  -d '{"view": "topics", "zoom": 0, "bbox": [-13, -15, 26, 24], "layers": ["clusters/toponymy", "topics/toponymy"], "levels": [0], "per_tile": 5}' -o topics.bin
python3 frames.py topics.bin 10 layer,mosaica_id,masked_count,content,target
```

```
artifacts: 10 rows
    layer              mosaica_id            masked_count  content                    target
    clusters/toponymy  13079644684018051290  3923          []                         None
    clusters/toponymy  6983585968513505337   3314          []                         None
    clusters/toponymy  14665693188179012702  2845          []                         None
    clusters/toponymy  12339882117041205159  2620          []                         None
    clusters/toponymy  11520693458383137352  1918          []                         None
    topics/toponymy    4930369447574832291   6             ['Computational Biology']  14665693188179012702
    topics/toponymy    8255206733746464212   6             ['Condensed Matter']       13079644684018051290
    topics/toponymy    15560657942123900099  6             ['Geometric Analysis']     11520693458383137352
    topics/toponymy    15776153839920046184  6             ['Integrable Systems']     12339882117041205159
    topics/toponymy    17798611420507067619  6             ['AI Systems']             6983585968513505337
trailer: {"arrow_serialise_ns": 114142, "frames": 1, "rows": 10, "stream_us": 8512}
```

The five largest clusters come first, ranked by `masked_count`, the members this session may see. Each name's `target` is the `mosaica_id` of the cluster it names, so the largest cluster, with 3,923 papers, is Condensed Matter. A name's own count is the six papers its language model was shown. The table has more columns than these five: each artifact's centre and box on the grid, its parents, and whether any of its members in the tile match a filter or highlight. The [annotations chapter](../system/annotations.md) explains which artifacts a viewer may see at all.

## When a request is refused

A refusal is JSON, with an `error` code and a `detail` that says what to change.

```bash
curl -s http://127.0.0.1:9141/v1/viewport -H "$AUTH" -H "$JSON" \
  -d '{"view": "topics", "zoom": 2, "bbox": [-13, -15, 26, 24], "filters": {"year": {"range": {"gte": 2020}}}}'
```

```
{"error":"contract","detail":"'year' is not a filterable column; name one that `/v1/meta`'s `filter_operands` lists"}
```

The code tells a client what to do next.

| Status and code | Means | What to do |
|---|---|---|
| `422 contract` | The request is wrong. | Change it. The detail says how. |
| `404 unknown` | No such view, item or artifact, or one this session may not see. | Nothing to retry. |
| `401 bad-credential` | No token, or one the server does not know. | Sign in. |
| `403 expired-token` | The session has ended. | Sign in again. |
| `429 backpressure` | The server is busy, or a suggestion is already running for this session. | Send the same request after the seconds in `Retry-After`. |
| `500 fail-closed` | The server could not compute a correct answer, and sent none. | Retry, and check the server's log if it keeps happening. |

A session ends when its token expires, when it signs out, and when a change to the principal's grants affects it. Sign out with `POST /v1/logout`, and the token stops working at once.

```bash
curl -s -X POST http://127.0.0.1:9141/v1/logout -H "$AUTH"
curl -s http://127.0.0.1:9141/v1/meta -H "$AUTH"
```

```
{"error":"expired-token","detail":"the session has ended: its token expired, was revoked, or a change to what authorised it ended it; authorise again"}
```

When you've finished, stop the server with Ctrl-C.

## Next

- The [HTTP API reference](../reference/http.md) gives every field of every request, the columns of every frame and every refusal, with worked examples on this same data.
- [Wire framing](../openapi/README.md) describes the frames to the byte, with decoders in Python and JavaScript, and what a client of its own has to handle.
- The [queries chapter](../system/queries.md) lists the Python, TypeScript and CLI calls that send each of these requests.
