# Call Mosaica from a backend

When Mosaica's map sits inside an app of your own, your app's server stands between your users and Mosaica. It signs each user in and asks Mosaica for a session for them. Then it hands the session's token to the browser, where the TypeScript client draws the map. It may also look things up itself: the values for a filter list, the topics for a sidebar, one record for a page of its own. This guide makes each of those requests with curl, against the 50,000 arXiv papers that ship in the repository.

Every request here takes and returns JSON. The map's tiles, the topics by tile, bulk reads and aggregates answer in [Arrow frames](../openapi/README.md) instead, which a client decodes for you. The [last section](#the-requests-a-client-makes) says which call to use for each.

You'll need the `mosaica` binary from [the first tutorial](../start/first-map.md), the repository cloned at `~/mosaica`, curl, and Python 3, which reads the JSON. One step uses pyarrow.

## What we'll do

1. Build the arXiv example and serve it.
2. Make a principal for your backend, and one for a user.
3. Start a session for the user, as your backend would.
4. Read the deployment's shape from `/v1/meta`.
5. Fill a filter list and a typeahead.
6. List and search the topics.
7. Read one paper's record.
8. See what a refusal looks like, and end the session.

## Serve the arXiv example

The papers are in `examples/data/arxiv`, with a declaration, `schema.toml`, that describes them. Each paper has a position in two layouts, called views, and carries its arXiv archive, its subject class, its submission date, its title and its arXiv identifier. Over the papers lie Toponymy's clusters at four levels, with a layer of names for them, and arXiv's own classification. The [NOTICE](https://github.com/mosaicadb/mosaica/blob/main/examples/data/arxiv/NOTICE.md) beside the data says where each part came from.

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
built /home/you/arxiv/bundle (v00000): 50000 items, 175 terms, 86144 pairs, 15025516 bytes on disk, 0 artifact(s) minted, 0 unclustered member row(s), 0 row(s) refused
  view topics: 50000 row(s)
  view topics-wide: 50000 row(s)
```

Make the operator's secret and start the server in a terminal of its own.

```bash
cd ~/arxiv
openssl rand -hex 16 > operator.secret
chmod 600 operator.secret
mosaica serve
```

```
...
{"event":"listening","viewer":"127.0.0.1:9141","session":"127.0.0.1:9142","control":"127.0.0.1:9143"}
```

Your backend uses two of the three addresses. It starts sessions on the session address, port 9142, which only answers the operator credential and API keys allowed to act for other users. It reads on the viewer address, port 9141, as the browser does.

## Make the principals

Mosaica keeps its users, which it calls principals, in its catalogue. We need two.

`portal` is your backend. It holds `authorise-as`, which lets it start a session for another principal, and it proves who it is with an API key. Mosaica shows a key once, when it makes it, so save it to a file only you can read.

```bash
cd ~/arxiv
export MOSAICA_CREDENTIAL=$(cat operator.secret)
mosaica principal create portal --kind service --control http://127.0.0.1:9143
mosaica grant --principal portal --permission authorise-as --control http://127.0.0.1:9143
umask 077; mosaica key create portal --control http://127.0.0.1:9143 > portal.key; cat portal.key
```

```
{"sessions_ended":0}
{"sessions_ended":0}
{"key":"msk_ab824ddfa336_4150f39602b6ec27edcc0658b3751d8f37456f5132fd1a11cdcb476b69629ea1","prefix":"ab824ddfa336"}
```

`ada` is a user. She holds `read`, and two terms, `cs.LG` and `stat.ML`. Each paper's access label lists its arXiv categories, and a viewer holding any one of them may see the paper, so Ada sees the machine learning papers and nothing else.

```bash
mosaica principal create ada --kind person --control http://127.0.0.1:9143
mosaica grant --principal ada --permission read --control http://127.0.0.1:9143
mosaica grant --principal ada --term cs.LG --term stat.ML --control http://127.0.0.1:9143
```

```
{"sessions_ended":0}
{"sessions_ended":0}
{"sessions_ended":0}
```

## Start a session for a user

When Ada opens your app, your backend has already signed her in its own way. It then asks Mosaica for a session for her, with `POST /session/authorise`. The API key goes in the `authorization` header, and the body names the principal.

```bash
KEY=$(python3 -c 'import json; print(json.load(open("portal.key"))["key"])')
SESSION=$(curl -s http://127.0.0.1:9142/session/authorise -H "authorization: Bearer $KEY" \
  -H 'content-type: application/json' -d '{"principal": "ada"}')
echo "$SESSION"
```

```
{"token":"84d55c858f62d6b8ce1b1bc0a76a62b3f065c42ed7477628728f62c2fb4f4e58","token_id":0,"expires_at":1792898566}
```

The session holds Ada's terms and her `read`. A session started this way never carries `read-all`, `write-all`, `admin` or `authorise-as`, even for a principal that holds them. It lasts until `expires_at`, in seconds since 1970, which is an hour away: the `token_max_lifetime` in `mosaica.toml`.

Your backend sends `token` to the browser and keeps `token_id`, the handle it ends the session with later. The `plain-html` example's [server.mjs](https://github.com/mosaicadb/mosaica/blob/main/clients/ts/examples/plain-html/server.mjs) mints sessions this way, in 140 lines of Node.

If Ada signs in with your OpenID Connect provider, send her access token in place of the name, as `{"access_token": "..."}`. A user who holds a password or an API key of their own can also sign in directly, with `POST /v1/login`. The [access control chapter](../system/access-control.md#getting-a-token) covers all three. From a shell, `mosaica session authorise` and `mosaica session revoke` send the same requests.

For the rest of this guide we'll read as Ada. Keep the token and the two headers in shell variables.

```bash
TOKEN=$(echo "$SESSION" | python3 -c 'import json, sys; print(json.load(sys.stdin)["token"])')
TOKEN_ID=$(echo "$SESSION" | python3 -c 'import json, sys; print(json.load(sys.stdin)["token_id"])')
AUTH="authorization: Bearer $TOKEN"
JSON='content-type: application/json'
```

## Read the deployment's shape

`GET /v1/meta` describes the deployment as this session may know it: the views, every declared field, the fields a filter can name, the layers and the settings a client needs to draw the map. Views and layers the session cannot reach are left out, so read it for each session rather than once for all.

```bash
curl -s http://127.0.0.1:9141/v1/meta -H "$AUTH" -o meta.json
```

The document runs to a few kilobytes. Print the parts a backend most often wants.

```bash
python3 - <<EOF
import json

meta = json.load(open("meta.json"))
for view in meta["views"]:
    print("view", view["id"])
for field in meta["filter_operands"]:
    print("field", field["column"], field["family"], field["operands"])
for layer in meta["layers"]:
    print("layer", layer["name"], [level["title"] for level in layer["levels"] or []])
EOF
```

```
view topics
view topics-wide
field id numeric ['eq', 'in']
field archive category ['eq', 'in']
field primary_category category ['eq', 'in']
field submitted_at numeric ['eq', 'in', 'range']
field title text ['match', 'phrase']
field arxiv_id keyword ['eq', 'in', 'prefix', 'contains']
layer clusters/toponymy ['16 topics, 832+ papers each', '46 topics, 263+ papers each', '161 topics, 78+ papers each', '574 topics, 20+ papers each']
layer taxonomy/arxiv ['archive', 'subject class']
layer topics/toponymy []
```

The fields are those a filter can name, with the operators each takes. A filter is a JSON expression over them, combined with `all_of`, `any_of` and `none_of`, and the [queries chapter](../system/queries.md#filters) describes it. `topics/toponymy` has no levels: it holds the names of the clusters, one for each.

## Fill a filter list and a typeahead

`GET /v1/categories/{field}` lists a category field's values, a page at a time. The `key` is what a filter names, and the `title` what you show. This vocabulary's titles are the same as its keys.

```bash
curl -s "http://127.0.0.1:9141/v1/categories/archive?limit=4" -H "$AUTH"
```

```
{"column":"archive","next":"ao-sci","values":[{"code":1,"key":"acc-phys","title":"acc-phys"},{"code":2,"key":"adap-org","title":"adap-org"},{"code":3,"key":"alg-geom","title":"alg-geom"},{"code":4,"key":"ao-sci","title":"ao-sci"}]}
```

Pass `next` back as `after` for the next page. The archive vocabulary is declared `public`, so every session gets every value, including archives Ada has no paper in. A vocabulary declared `derived` lists only the values held by a paper the session may see.

For a search box, `/suggest` matches what the user has typed against the start of each key, each title and each word in a title. With `view` and `counts=true`, every value carries its count.

```bash
curl -s "http://127.0.0.1:9141/v1/categories/primary_category/suggest?q=stat.m&view=topics&counts=true" -H "$AUTH"
```

```
{"column":"primary_category","more":false,"q":"stat.m","total":4081,"values":[{"code":168,"count":48,"key":"stat.ME","match":{"field":"key","len":6,"start":0},"title":"stat.ME"},{"code":169,"count":347,"key":"stat.ML","match":{"field":"key","len":6,"start":0},"title":"stat.ML"}]}
```

The counts are Ada's. `total` is the 4,081 papers she may see, out of 50,000. Of those, 347 have `stat.ML` as their subject class, and 48 have `stat.ME`, which she sees because they are cross-listed under one of her terms. To count under a filter, send the same fields as a JSON body with `POST`, adding `filters`.

```bash
curl -s http://127.0.0.1:9141/v1/categories/primary_category/suggest -H "$AUTH" -H "$JSON" \
  -d '{"q": "stat.m", "view": "topics", "counts": true, "filters": {"title": {"match": "bayesian"}}}'
```

```
{"column":"primary_category","more":false,"q":"stat.m","total":82,"values":[{"code":168,"count":6,"key":"stat.ME","match":{"field":"key","len":6,"start":0},"title":"stat.ME"},{"code":169,"count":18,"key":"stat.ML","match":{"field":"key","len":6,"start":0},"title":"stat.ML"}]}
```

Each session runs one suggestion at a time, and a second sent while the first is running is refused with `429`. Wait for each answer before sending the next.

## List and search the topics

`POST /v1/artifacts/browse` lists a layer's artifacts by their place in its hierarchy, whatever part of the map is on screen, with those holding the most papers Ada may see first. This asks for the three largest clusters at the coarsest level.

```bash
curl -s http://127.0.0.1:9141/v1/artifacts/browse -H "$AUTH" -H "$JSON" \
  -d '{"view": "topics", "layer": "clusters/toponymy", "level": 0, "limit": 3}' | python3 -m json.tool
```

```
{
    "artifacts": [
        {
            "mosaica_id": "9880638538854942963",
            "key": "tp0-000014",
            "masked_count": 875,
            "rung": 0,
            "parent_ids": [],
            "child_count": 23,
            "slot": null
        },
        {
            "mosaica_id": "17375019890995107651",
            "key": "tp0-000006",
            "masked_count": 575,
            "rung": 0,
            "parent_ids": [],
            "child_count": 4,
            "slot": null
        },
        {
            "mosaica_id": "9016650185214543757",
            "key": "tp0-000015",
            "masked_count": 388,
            "rung": 0,
            "parent_ids": [],
            "child_count": 10,
            "slot": null
        }
    ],
    "parents": [],
    "next": "388:9016650185214543757"
}
```

`masked_count` is how many of the cluster's papers Ada may see. Send `parent` with one of these `mosaica_id`s to list its children, or `next` back as `cursor` for the next page. The ids arrive as strings, because a JavaScript number cannot hold a 64-bit integer exactly.

These rows have no `name`. Each cluster's label holds two texts: a title, written from six of its papers, and keyphrases, drawn from three. The declaration serves each text only to a viewer who can see every paper it was drawn from, and a row's `name` is the first of the two the viewer gets. For none of these coarse clusters can Ada see all of either set. She gets names further down, where the clusters are smaller. Searching with `q` matches a key or a name.

```bash
curl -s http://127.0.0.1:9141/v1/artifacts/browse -H "$AUTH" -H "$JSON" \
  -d '{"view": "topics", "layer": "clusters/toponymy", "level": 3, "q": "privacy"}' | python3 -m json.tool
```

```
{
    "artifacts": [
        {
            "mosaica_id": "16138676707608042943",
            "key": "tp3-000283",
            "name": "federated learning, malware detection, differential privacy",
            "masked_count": 38,
            "rung": 3,
            "parent_ids": [],
            "child_count": 0,
            "slot": null
        }
    ],
    "parents": []
}
```

This name is the cluster's keyphrases: Ada can see the three papers they came from, but not all six behind its title. A user with other terms would get other counts, other names and possibly no match.

`POST /v1/artifacts/{mosaica_id}`, with a body naming the `view`, returns one artifact's card: its count, centre, box and outline over the papers the session may see, on the grid described [below](#read-one-paper). It carries no name, which comes from browse.

## Read one paper

`POST /v1/items/{mosaica_id}` returns one item's record. In your app the `mosaica_id` usually comes from the browser, when the user clicks a point. To follow along, find one by its arXiv identifier with `mosaica items`, which writes a Parquet file.

```bash
mosaica items --server http://127.0.0.1:9141 --token $TOKEN --view topics --fields arxiv_id \
  --filters '{"arxiv_id": {"eq": "2405.13758"}}' --out found.parquet
python3 -c 'import pyarrow.parquet as pq; print(pq.read_table("found.parquet").to_pylist())'
```

```
mosaica items: 1 rows in 1 pages from 1 responses; head {"order":"map","page_rows":100000}
[{'mosaica_id': 8190908901818807410, 'arxiv_id': '2405.13758'}]
```

Your `mosaica_id` will differ, because every build scrambles them with a new key. Use your own in the request.

```bash
curl -s http://127.0.0.1:9141/v1/items/8190908901818807410 -H "$AUTH" -H "$JSON" -d '{}' | python3 -m json.tool
```

```
{
    "fields": {
        "archive": "cs",
        "arxiv_id": "2405.13758",
        "id": 2348405,
        "primary_category": "cs.CV",
        "submitted_at": 1716392394000000,
        "title": "Counterfactual Gradients-based Quantification of Prediction Trust in Neural Networks"
    },
    "labels": [
        "cs.LG"
    ],
    "views": [
        {
            "id": "topics",
            "x": 1345618443,
            "y": 1013899074
        },
        {
            "id": "topics-wide",
            "x": 3075095501,
            "y": 1163436518
        }
    ],
    "scoped": {}
}
```

The record has every field the paper holds, with categories as their keys and `submitted_at` in microseconds since 1970. The paper's subject class is computer vision, outside Ada's terms. `labels` shows why she may see it: it is cross-listed under `cs.LG`.

`views` gives the paper's position in each view on a grid of 2^32 steps a side. To turn it into the view's own coordinates, take `x_min + (x + 0.5) / 2^32 × (x_max − x_min)`, with the view's `quantisation` from `/v1/meta`, and the same for `y`.

An identifier that names nothing, and one that names a paper Ada may not see, get the same answer: `404` with `{"error":"unknown","detail":"unknown"}`.

## When a request is refused

A refusal is JSON, with an `error` code and a `detail` that says what to change. The exceptions are a body that is not JSON, answered `400`, and a request without `content-type: application/json`, answered `415`, which come back as plain text.

```bash
curl -s http://127.0.0.1:9141/v1/artifacts/browse -H "$AUTH" -H "$JSON" \
  -d '{"view": "topics", "layer": "clusters/toponymy", "level": 4}'
```

```
{"error":"contract","detail":"browse refused: layer 'clusters/toponymy': it holds 4 level(s), so level 4 names nothing — /v1/meta publishes the level set"}
```

| Status and code | Means | What to do |
|---|---|---|
| `422 contract` | The request is wrong. | Change it, as the detail says. |
| `404 unknown` | No such view, item, artifact or principal, or one this session may not see. | Nothing to retry. |
| `401 bad-credential` | No token or key, or one the server does not know. A session token gets this once the server has forgotten the session, and after a restart. | For a session token, start a new session. For an API key, check the key. |
| `403 expired-token` | The session has ended. | Start a new one. |
| `403 forbidden` | The credential lacks a permission: a key without `authorise-as`, or a principal without `read`. | Grant the permission. |
| `429 backpressure` | The server is busy, or a suggestion is already running for this session. | Send the same request after the seconds in `Retry-After`. |
| `500 fail-closed` | The server could not compute a correct answer, and sent none. | Retry, and check the server's log if it keeps happening. |

## End the session

A session ends at `expires_at`, and earlier when a change to the catalogue affects it, such as a grant to Ada or to `portal`. Sessions are held in memory, so a restart ends them all. When Ada signs out of your app, end it yourself with `POST /session/revoke` and the `token_id`. The token is never sent back to the session address.

```bash
curl -s -w '%{http_code}\n' http://127.0.0.1:9142/session/revoke -H "authorization: Bearer $KEY" \
  -H 'content-type: application/json' -d "{\"token_id\": $TOKEN_ID}"
curl -s http://127.0.0.1:9141/v1/meta -H "$AUTH"
```

```
204
{"error":"expired-token","detail":"the session has ended: its token expired, was revoked, or a change to what authorised it ended it; authorise again"}
```

The revoke answers `204` with no body, and the token stops working at once. A `token_id` that names no live session, or a session another principal started, gets the same `204` and changes nothing.

For a load balancer, `/healthz` answers `200` while the process is up, and `/readyz` answers `200` when the server is ready for traffic and `503` when it is not. Neither needs a token, and both answer on the viewer and session addresses.

```bash
curl -s -o /dev/null -w "%{http_code}\n" http://127.0.0.1:9141/healthz
curl -s -o /dev/null -w "%{http_code}\n" http://127.0.0.1:9141/readyz
```

```
200
200
```

When you've finished, stop the server with Ctrl-C.

## The requests a client makes

The map and the reads of many items answer in Arrow frames, which the clients decode.

| To get | Route | Python | TypeScript | CLI |
|---|---|---|---|---|
| The points and counts of the map | `POST /v1/viewport` | `Viewer.map` | `Store.setView` | Not built yet |
| The topics in each tile | `POST /v1/artifacts/viewport` | `Viewer.viewport_artifacts` | `Store.setLayers` | Not built yet |
| Every matching item | `POST /v1/items` | `Viewer.items` | `MosaicaClient.items` | `mosaica items` |
| Every matching artifact | `POST /v1/artifacts` | `Viewer.artifacts` | `MosaicaClient.artifacts` | `mosaica artifacts` |
| Counts by group, histograms, density grids | `POST /v1/aggregate` | `Viewer.aggregate` | `MosaicaClient.aggregate` | `mosaica aggregate` |

A backend in Python reads them with the Python client, and one in Node with `MosaicaClient`. In another language, [Wire framing](../openapi/README.md) describes the frames to the byte, with decoders in Python and JavaScript, and what a client of its own has to handle.

## Next

- The [HTTP API reference](../reference/http.md) gives every field of every request and every refusal, with examples on this same data.
- [Access control](../system/access-control.md) explains labels, terms and sessions.
- [Put Mosaica behind TLS](tls.md) sets up nginx in front of the viewer address, for a page on another origin.
