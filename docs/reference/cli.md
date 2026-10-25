<!-- Generated from crates/mosaica-cli/src/main.rs. Edit the help text there, then run: MOSAICA_WRITE_CLI_REFERENCE=1 cargo test -p mosaica cli_reference -->

# CLI

`mosaica <subcommand> --help` prints the text on this page. `mosaica --version` prints the release version and the commit the binary was built from, as in `mosaica 0.1.0 (3c872b11a)`. It prints the release version alone when `MOSAICA_BUILD_COMMIT` was unset at build time and git could not name the commit, as in a build outside a git checkout.

`build`, `check`, `health`, `serve`, and `declare` without a subcommand or `--config`, read the deployment file `mosaica.toml` from the working directory, or from the nearest directory above it that has one. `--deployment` names a different file.

The commands that write to a running service, and the identity commands on its control plane, take its address as `--control` and read the credential from `MOSAICA_CREDENTIAL`. Where it is unset, the request is sent without a credential, as an open server serves it.

## `mosaica build`

```text
mosaica build [OPTIONS]
```

Build a bundle from the corpus declaration and the source files it names.

`mosaica build` needs no flags. It reads `mosaica.toml` from the working directory, or from the nearest directory above it that has one. That file names the corpus declaration in `[build] schema` (default `schema.toml`) and the bundle directory in `[bundle] path`, each relative to the file's own directory. The declaration names the source files.

Every build creates a new bundle with a new key for its `mosaica_id`s, so a `mosaica_id` read from an earlier bundle does not name an item in this one. A copy of a bundle keeps its `mosaica_id`s.

The other flags override `mosaica.toml` or tune the build.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--deployment` | `PATH` |  | Read this `mosaica.toml` instead of searching for one upward from the working directory. |
| `--out` | `PATH` |  | Write the bundle to this directory instead of `[bundle] path` in `mosaica.toml`. `mosaica serve` opens `[bundle] path`, so it does not serve a bundle written elsewhere. |
| `--limit` | `VALUE` |  | Build only the items whose value of the declaration's one unique integer attribute is below this value, and the rows of every file that name them. A view's points row whose value is at or above the limit, or null, creates no item. A negative value counts as its unsigned 64-bit value, at least 2^63, so the limit drops it. Refused when the declaration has no unique integer attribute, or more than one. A row that names a kept item by any unique field is read whatever its own value. A row that names none is left out: in a view's points it creates no item where its value is at or above the limit or null, and in any other file it is reported as outside the limit, since it may name an item the limit left out. The exception is a file that names items by the limit's attribute alone, whose row naming no item is refused where its value is below the limit. |
| `--strict` |  |  | Refuse the build at the first file with a row the identity rule refuses. Without it, a row naming two items, naming an item or a unique value an earlier row of its file names, or, outside a view's points, naming no item, is left out and the build goes on. The refused rows are printed, and written to `reports/refused.json` in the bundle where there are any. |
| `--config` | `PATH` |  | Read this corpus declaration instead of `[build] schema` in `mosaica.toml`. The declaration is compiled into the bundle's `MANIFEST.json`, and the server reads it from there. |
| `--file` | `NAME=PATH` |  | Read the source NAME in the declaration's `[sources]` from PATH instead. Repeatable. A relative PATH is read from the working directory, not from the declaration's directory. Every block that reads the source reads PATH. Refused when `[sources]` has no source called NAME (the message lists the names it has), and when NAME is given twice. The flag replaces a source's path and cannot add a source. |
| `--no-oracle-pairs` |  |  | Do not write `pairs.parquet`. The server does not read the file. The conformance suite and `mosaica verify --deep` do, and `verify --deep` passes a bundle without one. |
| `--batch-items` | `ITEMS` | derived | Assign internal ids in batches of this many items. Default: the largest batch the memory budget allows, which is the whole corpus when it fits. Refused when the batch does not fit the budget, or when it is less than half the size the budget allows. A build of more than one batch records the size in the bundle. |
| `--memory-budget` | `SIZE` | derived | Peak memory for the build's own structures, in bytes or with a `k`, `m` or `g` suffix, such as `24g`. Default: 80% of the available memory or of the process's cgroup limit, whichever is lower, kept between 2 GiB and 1 TiB, and 24 GiB where available memory cannot be read. Batch and band sizes follow from it. A build that would not fit is refused before it assigns internal ids, with the arithmetic in the message. |
| `--stage-timings` |  |  | Print each build stage's wall time, row count and peak resident memory to stderr as the stage ends. Peak resident memory is the process's high-water mark when the stage ended, so it shows the stage in which the peak was reached. |
| `--stage-timings-json` | `PATH` |  | Write the per-stage records to this file as JSON when the build ends, including one that fails partway. One object per stage, in order, with `stage`, `wall_s`, `rows`, `peak_rss_kib`, `started_at` and `ended_at`. It does not need `--stage-timings`. |

## `mosaica check`

```text
mosaica check [OPTIONS]
```

Check the declaration against the column schemas of the files it names, without reading a row.

`mosaica check` finds `mosaica.toml`, the declaration and the `--file` overrides as `mosaica build` does, and stops at the first error in `mosaica.toml` or the declaration, such as a missing key or a TOML syntax error. Once both parse, it reads the footer of each source Parquet file and reports every column that is missing or has the wrong type, not only the first. It reads no rows except the geometry of shape layers, which it reads to size them.

The report goes to stderr: the files read, the findings, warnings, the columns each file names items by, the frames the views will have, the view groups, the shape layers' sizes, and on a clean check the disclosure table. The exit status is non-zero when there is a finding, such as a file other than a view's points with no column to name items by; a warning does not change it.

It cannot check anything that needs a row: whether a closed vocabulary covers the values in the data, which rows the identity rule refuses, or where the data lies in its view's extent. `mosaica build` reports those.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--deployment` | `PATH` |  | Read this `mosaica.toml` instead of searching for one upward from the working directory. |
| `--config` | `PATH` |  | Read this corpus declaration instead of `[build] schema` in `mosaica.toml`. |
| `--file` | `NAME=PATH` |  | Read the source NAME in the declaration's `[sources]` from PATH instead. Repeatable, and the same override `mosaica build` takes. |
| `--payloads` |  |  | Print the declaration to stdout as the JSON request bodies the control plane takes, to declare the same corpus on a running service. Printed only when the check has no finding. The output is one object with the keys `layers`, `attributes`, `vocabularies`, `views` and `view_groups`, each an array in declaration order. A layer or attribute entry is the request body itself. A vocabulary, view or view group entry is `{ "name", "body" }`, because its name goes in the route's path. A vocabulary entry also has `values`, the body for `PATCH /control/vocabularies/{name}/values`, or `values_source`, the `[sources]` name its values are read from. Values read from a source are not printed. Where the declaration writes `[shards]`, the object also has `shards`, the body for `PUT /control/shards`. Send them in this order: the shard size, the vocabularies with their values, the view groups, the views, the attributes, then the layers, since a body may name what an earlier one declares and its route refuses a name it does not hold. |

## `mosaica verify`

```text
mosaica verify [OPTIONS] <BUNDLE>
```

Verify a bundle's files and every row's `mosaica_id`.

It opens the bundle as the server does. That checks every manifest digest, the size and SHA-256 of every file except the unique indexes' runs, and that each segment's permutation maps one-to-one onto its rows. `--deep` hashes those runs too. It then confirms that the row space holds exactly the rows the segments claim, and computes each row's `mosaica_id` again from the identity key, failing on the first row that differs.

It also prints to stderr each indexed keyword column's count of distinct values against its rows, with a warning for a column whose values are unique per row. A warning does not fail the verify.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `BUNDLE` |  |  | The bundle directory, the one holding `CURRENT`. |
| `--deep` |  |  | Also check the bundle's internal structures. The term lists must be sorted, free of duplicates and in range; dictionary records must not repeat; record blobs, Morton cells, cell codes and identity bands must agree with the columns they index or copy, as must each level's band label copy; each group-scoped render column must be present in every segment; each unique column's index must be hashed against the manifest, name at most one live item for a value and agree with the column's values in both directions; each row-major column's member file must be what its labels give; and `pairs.parquet`, when present, must match the term lists it was written with. Run it on a bundle no running server is writing to. Writing the member files again needs about 8 B per membership entry of the largest level free beside the bundle, removed when the verify ends. |

## `mosaica tokenise`

```text
mosaica tokenise [OPTIONS]
```

Split text into tokens with an analyser built into this binary. Each input line prints as one line, its tokens separated by tabs.

When a `match` filter returns nothing, this shows how the index split the text. `/v1/meta` publishes each text column's analyser identity as `declared_scalars[].analyser`, such as `unicode/icu4x-2.2/p1`. The analyser's name is the part before the first `/`; run the query text through the analyser of that name. The conformance suite uses the command to compute expected `match` results with the analyser the index was built with.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--text` | `TEXT` |  | Text to split. Repeatable. With none, reads one input per line from stdin. |
| `--analyser` | `NAME` | `unicode` | The analyser to use, by name. An unknown name is refused with the list of names this binary carries. |
| `--identity` |  |  | Print the analyser's identity, the value a text column records, and exit. |

## `mosaica health`

```text
mosaica health [OPTIONS]
```

Ask a running server whether it is ready, and exit 0 if it is or 1 if it is not.

Finds `mosaica.toml` as `mosaica build` does and sends `GET /readyz` to the viewer address in `[serve]`. The server answers 503 while its write-ahead log cannot be written, when the thread that applies writes has stopped, and when part of the bundle is served from an older list of segments because the newest could not be used. Otherwise it answers 200, even while a write to disc hangs.

The command exits 0 on a 200. It exits 1, with the reason on stderr, on any other answer, on no answer within `--timeout`, when nothing is listening, and when `mosaica.toml` is refused or declares no viewer address. A server starts listening only once it has opened its bundle, so the check fails until then.

Exit 1 means stop sending viewer requests to the server, and keep sending it deletions and suppressions, which it still applies when its log has failed. It does not call for a restart: a restart undoes each deletion or suppression the server answered with 500 because the log could not be written, until that change is sent again.

A viewer address of `0.0.0.0` or `[::]` is reached on loopback, so run the command on the machine or in the container the server runs in. The Docker image's health check runs it.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--deployment` | `PATH` |  | Read this `mosaica.toml` instead of searching for one upward from the working directory. |
| `--timeout` | `SECONDS` | `3` | Seconds the whole request may take, from connecting to reading the answer. |

## `mosaica items`

```text
mosaica items [OPTIONS] --view <VIEW> --fields <NAMES> --server <URL>
```

Read every item a session token may see in one view, with the fields named, from a running server, and write them as Arrow IPC or Parquet.

The server answers `POST /v1/items` a page at a time, several pages to a response, and ends each response with a cursor for the next. This requests responses until no row remains and writes each page as it arrives. Each of the route's fields is the argument of the same name, sent only when given, so the server's own setting applies otherwise.

The columns are `mosaica_id`, the fields in the order named, `mosaica:x` and `mosaica:y` under `--position`, `mosaica:labels` under `--labels`, then `mosaica:matched` under `--keep-unmatched`. A category field is a dictionary column of its value keys, each page's dictionary holding the keys of its own rows. A read that returns no row writes these columns with no rows.

A read cut short leaves the whole pages read before it in the output, exits 1 and prints the cursor to read the rest with. The first response's head, with the counts under `--count`, is printed on stderr at the end.

For example, `mosaica items --server http://127.0.0.1:8080 --view papers --fields title,year --labels --out papers.parquet`.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--view` | `VIEW` |  | The view to read, as `/v1/meta` names it. An item with no position in it is not returned. |
| `--fields` | `NAMES` |  | The declared fields to return, comma-separated, in the order wanted. `--fields ''` returns `mosaica_id` alone. A field declared for a view group, read under a view outside that group, is named `<field>@<key>`. An undeclared or repeated field is refused. |
| `--position` |  |  | Return each item's position in the view as the columns `mosaica:x` and `mosaica:y`. |
| `--labels` |  |  | Return the clauses of each item's labels the token satisfies as the column `mosaica:labels`. |
| `--filters` | `JSON` |  | A filter expression as JSON, such as `{"year": {"range": {"gte": 2020}}}`. Only the items that match are returned. |
| `--keep-unmatched` |  |  | Return every item, with a `mosaica:matched` column saying whether it matches `--filters`. |
| `--count` |  |  | Count the items the token may see in the view and those that match. The counts are printed on stderr at the end. Refused with `--cursor`. |
| `--order` | `ORDER` |  | `map` returns the items by their place on the map; `stored` in the order the server stores records, which is faster for a field that is neither rendered nor indexed. Without it the order is the one `--cursor` was read in, or the server's choice. With `--cursor`, another order is refused. |
| `--page-rows` | `N` |  | Rows in a page, at most the server's `selection.max_page_rows`, which applies without it. 0 is refused. |
| `--pages` | `N` |  | The most pages in one response. Responses are requested until the read is done. 0 is refused. |
| `--cursor` | `CURSOR` |  | Start after the last page of an earlier read: the cursor a read cut short printed. |
| `--compression` | `COMPRESSION` |  | `zstd` compresses the pages on their way from the server. The output is written uncompressed either way. |
| `--server` | `URL` |  | The viewer plane's address, such as `http://127.0.0.1:8080`. |
| `--token` | `TOKEN` |  | A session token, as `mosaica login` or `mosaica session authorise` prints one. Without it the token is read from `MOSAICA_TOKEN`, and with neither the read is sent without one, as an open server serves it. |
| `--out` | `PATH` |  | The file to write. Without it, or with `-`, the output goes to stdout. |
| `--format` | `FORMAT` |  | `ipc` writes an Arrow IPC stream and `parquet` a Parquet file. Without it the format is the extension of `--out`, `.arrows` or `.parquet`; stdout and any other extension need it. A format that disagrees with the extension of `--out` is refused. |

## `mosaica artifacts`

```text
mosaica artifacts [OPTIONS] --view <VIEW> --layer <LAYER> --fields <NAMES> --server <URL>
```

Read every artifact of one layer a session token is served, with the properties named, from a running server, and write them as Arrow IPC or Parquet.

An artifact is one member of a layer: a cluster, a region, a node in a taxonomy. The read is carried across `POST /v1/artifacts` responses, and written, as `mosaica items` carries and writes one. The columns are `mosaica_id`, the properties in the order named, then `matched_count` under `--filters`. The rows are in order of level, and in the order they were published within a level.

For example, `mosaica artifacts --server http://127.0.0.1:8080 --view papers --layer clusters --fields key,masked_count --format ipc > clusters.arrows`.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--view` | `VIEW` |  | The view whose items the counts and the geometry are computed over. |
| `--layer` | `LAYER` |  | The layer to read, as `/v1/meta` lists it. A layer not published to the token in `--view` is refused. |
| `--fields` | `NAMES` |  | Any of `key`, `level`, `parents`, `target`, `masked_count`, `content`, `centroid`, `box`, `shape` and `slot`, comma-separated, in the order wanted. Any other name, or one repeated, is refused. |
| `--palette-size` | `N` |  | The number of colours, 2 to 32, the `slot` field picks among: each artifact's slot is chosen so that artifacts drawn beside each other differ. Without it, `slot` is null. |
| `--level` | `LEVEL` |  | Only the artifacts at this level of a levelled layer. Refused on a layer with one level, and past the levels the layer holds. |
| `--parent` | `MOSAICA_ID` |  | Only the children of this artifact, by its `mosaica_id`. Refused with `--q`. |
| `--q` | `TEXT` |  | Only the artifacts whose key, or first text, contains this, ignoring case. Refused with `--parent`. |
| `--ids` | `MOSAICA_IDS` |  | Only the artifacts these `mosaica_id`s name, comma-separated, such as the tags a points read returned. One the token is not served has no row, as one naming nothing does. |
| `--filters` | `JSON` |  | A filter expression as JSON. Only the artifacts with a visible member that matches are returned, each with a `matched_count` column. |
| `--keep-unmatched` |  |  | With `--filters`, return every artifact, those with no matching member included. |
| `--count` |  |  | Count the artifacts served and those that match. The counts are printed on stderr at the end. Refused with `--cursor`. |
| `--page-rows` | `N` |  | Rows in a page, at most the server's `selection.max_page_rows`, which applies without it. 0 is refused. |
| `--pages` | `N` |  | The most pages in one response. Responses are requested until the read is done. 0 is refused. |
| `--cursor` | `CURSOR` |  | Start after the last page of an earlier read: the cursor a read cut short printed. |
| `--compression` | `COMPRESSION` |  | `zstd` compresses the pages on their way from the server. The output is written uncompressed either way. |
| `--server` | `URL` |  | The viewer plane's address, such as `http://127.0.0.1:8080`. |
| `--token` | `TOKEN` |  | A session token, as `mosaica login` or `mosaica session authorise` prints one. Without it the token is read from `MOSAICA_TOKEN`, and with neither the read is sent without one, as an open server serves it. |
| `--out` | `PATH` |  | The file to write. Without it, or with `-`, the output goes to stdout. |
| `--format` | `FORMAT` |  | `ipc` writes an Arrow IPC stream and `parquet` a Parquet file. Without it the format is the extension of `--out`, `.arrows` or `.parquet`; stdout and any other extension need it. A format that disagrees with the extension of `--out` is refused. |

## `mosaica aggregate`

```text
mosaica aggregate [OPTIONS] --view <VIEW> --grouping <JSON> --server <URL>
```

Count how the items a session token may see in one view are distributed, by one grouping, from a running server, and write the table as Arrow IPC or Parquet.

The grouping is `POST /v1/aggregate`'s, as JSON, and names its figures: `"count": true` counts the items in each row, so `{"count": true}` is the size of the set. With it, `"by": {"field": "venue", "top": 10}` gives the ten commonest values of a category field; `"by": {"field": "year", "bins": 20}` a histogram of a number or timestamp field, and with `"sample": 100000` one counted over about that many of the set's items and scaled to it; `"by": {"layer": "clusters", "top": 10}` the largest artifacts of a layer, and on a `nested` or `dag` layer `"by": {"layer": "topics", "top": 10, "cut": {"zoom": 3, "bbox": [0, 0, 100, 100], "budget": 1000}}` the largest of those the map draws at that tile depth, box and cluster budget, and with `"palette_size": 10` beside `layer` a `slot` column, each artifact's colour among ten; `"cells": {"depth": 8}` a density surface. `{"summary": ["year"]}` is one row of a number or timestamp field's `values_year`, `missing_year`, `min_year`, `max_year` and `mean_year` over every item the token may see in the view, and `"values"`, `"missing"`, `"min"`, `"max"` and `"mean"` name one of those each; they take no `by`, `cells`, `--filters` or `--reference`. The read is carried across responses, and written, as `mosaica items` carries and writes one. The table's head, with the set's total, is printed on stderr at the end.

For example, `mosaica aggregate --server http://127.0.0.1:8080 --view papers --grouping '{"count": true, "by": {"field": "submitted_at", "bins": 12}}' --format ipc > submitted.arrows`.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--view` | `VIEW` |  | The view the counts are taken in. An item with no position in it is not counted. |
| `--grouping` | `JSON` |  | One grouping as JSON, such as `{"count": true, "by": {"field": "year", "bins": 20}}`. `{"count": true}` is the size of the set. |
| `--filters` | `JSON` |  | A filter expression as JSON. Only the items that match are counted. |
| `--reference` | `JSON` |  | A second set to compare each count with, as a filter expression in JSON; `{}` is every item the token may see in the view. A table with `count` then has `reference_count` and `lift`. |
| `--page-rows` | `N` |  | Rows in a page, at most the server's `selection.max_page_rows`, which applies without it. 0 is refused. |
| `--pages` | `N` |  | The most pages in one response. Responses are requested until the read is done. 0 is refused. |
| `--cursor` | `CURSOR` |  | Start after the last page of an earlier read: the cursor a read cut short printed. |
| `--compression` | `COMPRESSION` |  | `zstd` compresses the pages on their way from the server. The output is written uncompressed either way. |
| `--server` | `URL` |  | The viewer plane's address, such as `http://127.0.0.1:8080`. |
| `--token` | `TOKEN` |  | A session token, as `mosaica login` or `mosaica session authorise` prints one. Without it the token is read from `MOSAICA_TOKEN`, and with neither the read is sent without one, as an open server serves it. |
| `--out` | `PATH` |  | The file to write. Without it, or with `-`, the output goes to stdout. |
| `--format` | `FORMAT` |  | `ipc` writes an Arrow IPC stream and `parquet` a Parquet file. Without it the format is the extension of `--out`, `.arrows` or `.parquet`; stdout and any other extension need it. A format that disagrees with the extension of `--out` is refused. |

## `mosaica declare`

```text
mosaica declare [OPTIONS] --control <ADDRESS>
       mosaica declare <COMMAND>
```

Declare views, view groups, attributes, vocabularies, layers and the shard size on a running service, through the control plane.

With no subcommand, it declares every block of a corpus declaration: the file `--config` names, or else the one `mosaica.toml` names, found as `mosaica check` finds it. It sends the bodies `mosaica check --payloads` prints, in the order the routes need: the shard size, the vocabularies, each followed by the values its `source` names, the view groups, each after any group whose views it shares, the views, each group's own views, the attributes, then the layers. A group's views are read as `mosaica build` reads them: its `[[view_group.view]]` blocks, the rows of its `[view_group.views]` table, or the distinct values of its points file's view column. A declaration identical to one the service holds answers what exists, so running it again after adding a block declares only that block. It sends no rows. It prints one JSON object on stdout with each request, its status and the server's answer, goes on past a refused request, and exits 1 when any was refused.

Each subcommand sends one declaration and prints the server's answer. Every declaration needs `write-all`. The credential is read from `MOSAICA_CREDENTIAL`, as `mosaica principal` reads it.

For example, `mosaica declare --control http://127.0.0.1:8082 --config schema.toml`.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `--deployment` | `PATH` |  | Without `--config`, read this `mosaica.toml` instead of searching for one upward from the working directory. |
| `--config` | `PATH` |  | Read this corpus declaration, and no `mosaica.toml`, instead of `[build] schema` in `mosaica.toml`. |
| `--file` | `NAME=PATH` |  | Read the source NAME in the declaration's `[sources]` from PATH instead. Repeatable, and the same override `mosaica check` takes. Of the sources, only a vocabulary's values are read. |

## `mosaica declare attribute`

```text
mosaica declare attribute --control <ADDRESS> <--body <JSON>|--body-file <PATH>>
```

Declare an attribute, a column the items carry, named in the body.

The body is the `[[attribute]]` block as JSON, as `mosaica check --payloads` prints it under `attributes`, such as `{"name": "year", "type": "i32", "index": true}`. An attribute declared on a running service cannot be rendered.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `--body` | `JSON` |  | The request body as JSON. |
| `--body-file` | `PATH` |  | Read the request body from this file, or from stdin with `-`. |

## `mosaica declare vocabulary`

```text
mosaica declare vocabulary --control <ADDRESS> <--body <JSON>|--body-file <PATH>> <NAME>
```

Declare a vocabulary, the values a category attribute draws on.

The body is the `[[vocabulary]]` block as JSON without its name, as `mosaica check --payloads` prints it under `vocabularies`. Its `values`, each a `key` and an optional `title`, are added as `mosaica insert --vocabulary` adds them, and the server draws every code.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `NAME` |  |  | The vocabulary's name. |
| `--body` | `JSON` |  | The request body as JSON. |
| `--body-file` | `PATH` |  | Read the request body from this file, or from stdin with `-`. |

## `mosaica declare view`

```text
mosaica declare view --control <ADDRESS> <--body <JSON>|--body-file <PATH>> <NAME>
```

Declare a view, or add a view to a view group.

For a view, NAME is its name and the body is the `[[view]]` block as JSON without its name, as `mosaica check --payloads` prints it under `views`. For a view of a view group, NAME is `<group>:<key>` and the body is the view's record: its `metadata`, one value per name the group declares, and its `visibility` where it has one, such as `{"metadata": {"year": 2020}}`. The view takes the group's frame and projection, and its record cannot be changed once it exists.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `NAME` |  |  | The view's name, or `<group>:<key>` for a view of a view group. |
| `--body` | `JSON` |  | The request body as JSON. |
| `--body-file` | `PATH` |  | Read the request body from this file, or from stdin with `-`. |

## `mosaica declare view-group`

```text
mosaica declare view-group --control <ADDRESS> <--body <JSON>|--body-file <PATH>> <NAME>
```

Declare a view group, with no views. `mosaica declare view <group>:<key>` adds each one.

The body is the `[[view_group]]` block as JSON without its name, as `mosaica check --payloads` prints it under `view_groups`.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `NAME` |  |  | The group's name. |
| `--body` | `JSON` |  | The request body as JSON. |
| `--body-file` | `PATH` |  | Read the request body from this file, or from stdin with `-`. |

## `mosaica declare layer`

```text
mosaica declare layer --control <ADDRESS> <--body <JSON>|--body-file <PATH>>
```

Declare an annotation layer, named in the body.

The body is the `[[layer]]` block as JSON, as `mosaica check --payloads` prints it under `layers`. The answer carries the layer's `mosaica_id`.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `--body` | `JSON` |  | The request body as JSON. |
| `--body-file` | `PATH` |  | Read the request body from this file, or from stdin with `-`. |

## `mosaica declare shards`

```text
mosaica declare shards --control <ADDRESS> --size <ITEMS>
```

Set the shard size: the high water at which the shard taking new items is sealed and the next one opened. A size at or below the open shard's high water seals it at once.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `--size` | `ITEMS` |  | The size, a number of items above 0. |

## `mosaica insert`

```text
mosaica insert [OPTIONS] --control <ADDRESS> <FILE>
```

Insert rows into a running service from a Parquet, Arrow IPC or NDJSON file, a page at a time, through `POST /control/ingest`.

A row names items by its `mosaica_id` and its values of unique attributes. A row naming no item creates one at its coordinates: `x` and `y`, or `lon` and `lat` on a projected view. A row naming an item edits it, adds it to the view, or changes nothing. A column left out keeps what the item stores, and a null clears it. A row naming two items, or an item an earlier row of its page names, is refused and the page's other rows are applied. Columns go as the file names and types them, and the route refuses a column it does not know; `--columns` sends only those named.

Each page is sent before the next is read, under a fresh batch id. A page the server answers with `429` is sent again, the same bytes under the same id, once the wait it names has passed, at least 1 second and at most 30, while the waits total at most five minutes. A page no answer came for is sent once more, a second later and outside that budget, which the server answers as a replay where the first arrived. Inserting a file again inserts its rows again, except that a row naming its item by a unique attribute is then answered as unchanged.

It prints one JSON object on stdout totalling the answers: `pages` and `rows` sent; `created`, `edited`, `added`, `unchanged`, `joined`, `minted`, `clamped`, `clipped` and `over_bound`, summed; `replayed`, the requests the server had already applied; `refused`, each refused row by its position in the file, from 0, and its reason; `refused_pages`, each refused request by its first row, with its status and the server's answer; and `publication`, from which the rows are visible. A line on stderr says the same. It exits 1 when a request was refused or went unanswered, and a page unanswered twice stops the insert there, with `stopped` saying where. Rows refused inside an accepted request do not change the exit status. Needs `write`, and the credential is read from `MOSAICA_CREDENTIAL`.

For example, `mosaica insert --control http://127.0.0.1:8082 --view papers papers.parquet`.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `FILE` |  |  | The rows: a file, or `-` for stdin. |
| `--view` | `VIEW` |  | The view the rows are for, sent as `x-mosaica-view`. Without it a deployment with one view takes the rows into that view, and one with several into no view, where they carry no coordinates and change only what the items store. |
| `--vocabulary` | `NAME` |  | Add the rows to this vocabulary's values instead, through `PATCH /control/vocabularies/{name}/values`. Each row has a `key` and may have a `title`. A key the vocabulary does not hold is added with a code the server draws, and a title replaces a held key's title. The report counts `added`, `existing` and `titles` in place of the rows' counts. |
| `--format` | `FORMAT` |  | `parquet`, `ipc` for an Arrow IPC stream or file, or `ndjson` for one JSON object per line. Without it the format is the file's extension: `.parquet`, `.arrows` or `.arrow`, `.ndjson` or `.jsonl`. Stdin needs it, and takes `ipc` as a stream, or `ndjson`. |
| `--columns` | `NAMES` |  | Send only these columns, comma-separated. Without it every column is sent. A name the file has no column for is refused; in NDJSON, a row without one is sent without it. |
| `--strict` |  |  | Refuse a whole page at its first refused row, with `409`, instead of applying its other rows. |
| `--page-rows` | `N` | derived | The most rows in one request, above 0. Default: `limits.ingest.max_batch_rows` from `/control/status`, or 10,000 with `--vocabulary`; where the credential cannot read `/control/status`, the server's documented default, 10,000. The figures used are printed on stderr. |
| `--page-bytes` | `SIZE` | derived | The most bytes in one request body, above 0, in bytes or with a `k`, `m` or `g` suffix. A page whose body is larger is split in half until each part fits. Default: `limits.ingest.max_batch_bytes` from `/control/status`, or `limits.declarations.max_body_bytes` with `--vocabulary`; where the credential cannot read `/control/status`, the server's documented default, 16 MiB, or 2 MiB with `--vocabulary`. |
| `--wait` |  |  | Once every page is sent, send `POST /control/flush?wait=visible`, and end once readers see the rows, or after the server's `serve.visible_wait_max_secs` with `visible` false. Without it the rows become visible at the server's next publication. |
| `--ids-out` | `PATH` |  | Write each row's `mosaica_id` to this file, ending in `.arrows` or `.parquet`: the columns `row`, its position in the input from 0, and `mosaica_id`, null for a row that was refused or whose page was. |

## `mosaica delete`

```text
mosaica delete [OPTIONS] --control <ADDRESS> <--id <MOSAICA_ID>|--match <JSON>|--file <PATH>>
```

Delete items or annotations through `POST /control/changes`.

Each is named by `--id`, by `--match` or by a row of `--file`. A deleted item stops being served at once, and the next compaction removes its rows from disc. A layer is removed with `mosaica drop layer`, not here.

The changes are sent as many to a request as `/control/status` publishes under `limits.changes`, 10,000 where the credential cannot read it. A change naming no item, or two, is refused and the others are applied. It prints one JSON object on stdout: `pages` and `rows` sent; `accepted`; `refused`, each refused change by its position, counting the `--id`s, then the `--match`es, then the file's rows, from 0, with what it named and its reason; `refused_pages`; `ignored_columns`, the columns that name no item; and `publication`. It exits 1 when a request was refused or went unanswered. Needs `write`, and the credential is read from `MOSAICA_CREDENTIAL`.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `--id` | `MOSAICA_ID` |  | An item or an annotation by its `mosaica_id`. Repeatable. |
| `--match` | `JSON` |  | An item by the values that name it, as a JSON object of its `mosaica_id` and unique attributes, such as `{"doi": "10.1/x"}`. Repeatable. |
| `--file` | `PATH` |  | A file whose every row names one item by its columns, `mosaica_id` and unique attributes, or `-` for stdin. A column that names no item is not read, and is listed in `ignored_columns`, as is a column of a type a change does not carry, such as a list, which is not sent. |
| `--format` | `FORMAT` |  | `parquet`, `ipc` for an Arrow IPC stream or file, or `ndjson` for one JSON object per line. Without it the format is the file's extension: `.parquet`, `.arrows` or `.arrow`, `.ndjson` or `.jsonl`. Stdin needs it, and takes `ipc` as a stream, or `ndjson`. |
| `--columns` | `NAMES` |  | Send only these columns, comma-separated. Without it every column is sent. A name the file has no column for is refused; in NDJSON, a row without one is sent without it. |
| `--strict` |  |  | Refuse a whole request at its first change naming no item, with `404`, or naming two, with `409`, instead of applying its other changes. |
| `--wait` |  |  | Once every change is sent, send `POST /control/flush?wait=visible`, and end once readers see the publication. A deletion or suppression is in force from the moment it is accepted either way. |

## `mosaica suppress`

```text
mosaica suppress [OPTIONS] --control <ADDRESS> <--id <MOSAICA_ID>|--match <JSON>|--file <PATH>>
```

Suppress items, annotations or layers through `POST /control/changes`: each is left out of every answer from the moment the change is accepted until `mosaica unsuppress` lifts it.

A layer's `mosaica_id` suppresses the layer with all its artifacts. It takes the same arguments and prints the same report as `mosaica delete`.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `--id` | `MOSAICA_ID` |  | An item or an annotation by its `mosaica_id`. Repeatable. |
| `--match` | `JSON` |  | An item by the values that name it, as a JSON object of its `mosaica_id` and unique attributes, such as `{"doi": "10.1/x"}`. Repeatable. |
| `--file` | `PATH` |  | A file whose every row names one item by its columns, `mosaica_id` and unique attributes, or `-` for stdin. A column that names no item is not read, and is listed in `ignored_columns`, as is a column of a type a change does not carry, such as a list, which is not sent. |
| `--format` | `FORMAT` |  | `parquet`, `ipc` for an Arrow IPC stream or file, or `ndjson` for one JSON object per line. Without it the format is the file's extension: `.parquet`, `.arrows` or `.arrow`, `.ndjson` or `.jsonl`. Stdin needs it, and takes `ipc` as a stream, or `ndjson`. |
| `--columns` | `NAMES` |  | Send only these columns, comma-separated. Without it every column is sent. A name the file has no column for is refused; in NDJSON, a row without one is sent without it. |
| `--strict` |  |  | Refuse a whole request at its first change naming no item, with `404`, or naming two, with `409`, instead of applying its other changes. |
| `--wait` |  |  | Once every change is sent, send `POST /control/flush?wait=visible`, and end once readers see the publication. A deletion or suppression is in force from the moment it is accepted either way. |

## `mosaica unsuppress`

```text
mosaica unsuppress [OPTIONS] --control <ADDRESS> <--id <MOSAICA_ID>|--match <JSON>|--file <PATH>>
```

Lift the suppression of items, annotations or layers through `POST /control/changes`. It takes the same arguments and prints the same report as `mosaica delete`.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `--id` | `MOSAICA_ID` |  | An item or an annotation by its `mosaica_id`. Repeatable. |
| `--match` | `JSON` |  | An item by the values that name it, as a JSON object of its `mosaica_id` and unique attributes, such as `{"doi": "10.1/x"}`. Repeatable. |
| `--file` | `PATH` |  | A file whose every row names one item by its columns, `mosaica_id` and unique attributes, or `-` for stdin. A column that names no item is not read, and is listed in `ignored_columns`, as is a column of a type a change does not carry, such as a list, which is not sent. |
| `--format` | `FORMAT` |  | `parquet`, `ipc` for an Arrow IPC stream or file, or `ndjson` for one JSON object per line. Without it the format is the file's extension: `.parquet`, `.arrows` or `.arrow`, `.ndjson` or `.jsonl`. Stdin needs it, and takes `ipc` as a stream, or `ndjson`. |
| `--columns` | `NAMES` |  | Send only these columns, comma-separated. Without it every column is sent. A name the file has no column for is refused; in NDJSON, a row without one is sent without it. |
| `--strict` |  |  | Refuse a whole request at its first change naming no item, with `404`, or naming two, with `409`, instead of applying its other changes. |
| `--wait` |  |  | Once every change is sent, send `POST /control/flush?wait=visible`, and end once readers see the publication. A deletion or suppression is in force from the moment it is accepted either way. |

## `mosaica publish`

```text
mosaica publish [OPTIONS] --control <ADDRESS> <--body <JSON>|--body-file <PATH>> <LAYER>
```

Publish annotations into one level of a layer through `PUT /control/layers/{name}/artifacts`, and print the server's answer.

The body is the route's JSON: each artifact with its key and its members, each member naming an item by its `mosaica_id` and its values of unique attributes. A member naming no item, or two, is left out and listed in the answer's `refused`, and the rest is published whole or not at all. An artifact whose key the level holds gains the members and fills the parts it lacks. The answer gives each artifact's `mosaica_id`. A large membership is published with its first page here and the rest with `mosaica grow`. Needs `write`, and the credential is read from `MOSAICA_CREDENTIAL`.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `LAYER` |  |  | The layer's name. |
| `--body` | `JSON` |  | The request body as JSON. |
| `--body-file` | `PATH` |  | Read the request body from this file, or from stdin with `-`. |
| `--strict` |  |  | Refuse the whole request at its first member naming no item, with `404`, or naming two, with `409`, instead of leaving the member out. |

## `mosaica grow`

```text
mosaica grow [OPTIONS] --control <ADDRESS> <--body <JSON>|--body-file <PATH>> <LAYER>
```

Change annotations a level already holds through `PATCH /control/layers/{name}/artifacts`, and print the server's answer.

It adds members, fills parts an annotation lacks, and takes items out of the set a text was written from. The body is the route's JSON. A member is refused as `mosaica publish` refuses one, and a part that differs from the one held is refused with `409`. Needs `write`, and the credential is read from `MOSAICA_CREDENTIAL`.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `LAYER` |  |  | The layer's name. |
| `--body` | `JSON` |  | The request body as JSON. |
| `--body-file` | `PATH` |  | Read the request body from this file, or from stdin with `-`. |
| `--strict` |  |  | Refuse the whole request at its first member naming no item, with `404`, or naming two, with `409`, instead of leaving the member out. |

## `mosaica drop`

```text
mosaica drop <COMMAND>
```

Drop an annotation layer or a view of a view group, through the control plane, and print the server's answer.

These are the only two that can be dropped: no route drops an attribute, a vocabulary, a view outside a view group, a view group or the shard size. Needs `write-all`, and the credential is read from `MOSAICA_CREDENTIAL`.


## `mosaica drop layer`

```text
mosaica drop layer [OPTIONS] --control <ADDRESS> <NAME>
```

Drop an annotation layer. It and its artifacts stop being served at once. Its name may be declared again, as a new layer that none of the dropped layer's `mosaica_id`s names.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `NAME` |  |  | The layer's name. |
| `--wait` |  |  | Answer only once readers no longer see the layer, or after the server's `serve.visible_wait_max_secs` with `visible` false. |

## `mosaica drop view`

```text
mosaica drop view [OPTIONS] --control <ADDRESS> <VIEW>
```

Drop a view of a view group. The items it leaves in no view are deleted, and the answer's `deleted` counts them. Its key may be created again, as a view with none of the old one's rows.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `VIEW` |  |  | The view, as `<group>:<key>`. |
| `--wait` |  |  | Answer only once readers no longer see the view, or after the server's `serve.visible_wait_max_secs` with `visible` false. |

## `mosaica flush`

```text
mosaica flush [OPTIONS] --control <ADDRESS>
```

Bring the next publication forward through `POST /control/flush`, and print the server's answer.

A publication is when buffered writes become visible to readers. The answer's `publication` names the first one certain to include every write sent before this request. Needs `write`, and the credential is read from `MOSAICA_CREDENTIAL`.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `--wait` |  |  | Answer only once readers see the publication, or after the server's `serve.visible_wait_max_secs` with `visible` false. |

## `mosaica compact`

```text
mosaica compact --control <ADDRESS>
```

Ask for a compaction through `POST /control/compact`, and print the server's answer, the request's number in `request`.

A compaction removes deleted items' rows from disc, one shard after another, and runs for minutes to hours on a large corpus. `mosaica status` reports it under `compaction.requests`, where `answered` reaches the request's number once it is answered. Needs `write`, and the credential is read from `MOSAICA_CREDENTIAL`.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |

## `mosaica status`

```text
mosaica status --control <ADDRESS>
```

Print the server's status report, from `GET /control/status`: how far writes have got, queue depths, compaction, shards, and the limits on each write request. Needs `admin`, and the credential is read from `MOSAICA_CREDENTIAL`.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |

## `mosaica serve`

```text
mosaica serve [OPTIONS]
```

Serve the bundle that `mosaica.toml` names.

Finds `mosaica.toml` as `mosaica build` does, opens the bundle at `[bundle] path` and replays the write-ahead log at `[bundle] wal`, all before it binds any address. It then listens on the viewer, session and control addresses in `[serve]`. When all three are bound it prints one line of JSON to stdout naming them. Diagnostics go to stderr, in colour only when stderr is a terminal.

SIGTERM or SIGINT stops the server at once, even while it opens the bundle, and it exits 0. Stopping ends every viewer session, because sessions are held in memory. A write is acknowledged only once the write-ahead log holds it on disc, so stopping loses no acknowledged write. A deletion or suppression answered with 500 because the log could not be written is in force but not on disc, and a restart undoes it until the change is sent again.

It refuses to start, and exits 1, when `mosaica.toml` is refused as `mosaica build` would refuse it or lacks one of the three `[serve]` addresses. It refuses, unless the server is open, when the operator credential is not set, and in any case when it is empty or its file cannot be read: under `[serve]`, `operator_credential_file` or `operator_credential_env` names the file or environment variable holding it. It refuses when `[catalogue] dir` is not set, or the catalogue there cannot be opened or is held open by another process. It also refuses when the bundle cannot be read and when the write-ahead log fails its checksum. An address that cannot be bound, such as one already in use, stops it with exit 1 after the bundle has opened.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--deployment` | `PATH` |  | Read this `mosaica.toml` instead of searching for one upward from the working directory. |
| `--exit-on-stdin-close` |  |  | Stop, as SIGTERM stops it, when standard input reaches its end. A process that starts the server and holds a pipe to its standard input stops it by closing the pipe, and stops it too when that process ends in any way, a kill included. Standard input that is empty or `/dev/null` stops the server as soon as it starts. |
| `--open` |  |  | Serve every request as the operator, with no credential, as `[serve] open = true` does: for one person on this machine. It refuses to start as that key does: off loopback, or beside settings or a catalogue for a server with users. The session and control addresses, the catalogue and the token lifetime take defaults where `mosaica.toml` names none. |

## `mosaica login`

```text
mosaica login --server <URL> <--principal <NAME>|--api-key|--access-token>
```

Log in on the viewer plane and print the session token and its `expires_at` as JSON.

The password, API key or OIDC access token is read from the first line of stdin, never from an argument. The principal must hold `read`.

For example, `mosaica login --server http://127.0.0.1:8080 --principal ann < password.txt`.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--server` | `URL` |  | The viewer plane's address, such as `http://127.0.0.1:8080`. |
| `--principal` | `NAME` |  | Log in as this local principal, with the password read from the first line of stdin. |
| `--api-key` |  |  | Log in with the API key read from the first line of stdin. |
| `--access-token` |  |  | Log in with the OIDC access token read from the first line of stdin. |

## `mosaica logout`

```text
mosaica logout --server <URL>
```

End a session on the viewer plane. The session token is read from `MOSAICA_TOKEN`, never from an argument.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--server` | `URL` |  | The viewer plane's address, such as `http://127.0.0.1:8080`. The session token to end is read from `MOSAICA_TOKEN`. |

## `mosaica session`

```text
mosaica session <COMMAND>
```

Mint and revoke sessions on the session plane, and list and end sessions on the control plane.

The session plane's verbs read their credential from `MOSAICA_API_KEY`: an API key holding `authorise-as`, or the operator credential, which alone may name the session's terms or ask for a session reading every item. The control plane's read their credential from `MOSAICA_CREDENTIAL` and need `admin`. Either variable unset sends the request without a credential, as an open server serves it.


## `mosaica session authorise`

```text
mosaica session authorise --session <URL> <--principal <NAME>|--access-token|--term <TERM>|--read-all>
```

Mint a session on the session plane, with the credential in `MOSAICA_API_KEY`: an API key whose principal holds `authorise-as`, or the operator credential.

A session for a principal carries the target's terms and its `read` and `write`, and never its `read-all` or `write-all`. With the operator credential alone, a session for `--term`s holds those terms and `read`, and a session for `--read-all` reads every item. It prints `token`, `token_id` and `expires_at` as JSON.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--session` | `URL` |  | The session plane's address, such as `http://127.0.0.1:8081`. |
| `--principal` | `NAME` |  | Act as this local principal. |
| `--access-token` |  |  | Act as the OIDC identity whose access token is read from the first line of stdin. |
| `--term` | `TERM` |  | A term the session holds, with the operator credential. Repeatable. |
| `--read-all` |  |  | A session of the superuser itself, which reads every item, with the operator credential. |

## `mosaica session revoke`

```text
mosaica session revoke --session <URL> --token-id <N>
```

End a session by its `token_id` on the session plane, with the credential in `MOSAICA_API_KEY`: an API key ends a session minted with a key of the same principal, and the operator credential ends any session.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--session` | `URL` |  | The session plane's address, such as `http://127.0.0.1:8081`. |
| `--token-id` | `N` |  | The `token_id` `mosaica session authorise` printed. |

## `mosaica session list`

```text
mosaica session list [OPTIONS] --control <ADDRESS>
```

List live sessions on the control plane: all of them, or a local principal's or a provider's.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `--principal` | `NAME` |  | Only this local principal's sessions, including those minted for it. |
| `--provider` | `NAME` |  | Only the sessions authorised through this provider. |

## `mosaica session end`

```text
mosaica session end --control <ADDRESS> <--token-id <N>|--principal <NAME>|--provider <NAME>>
```

End one session by `token_id`, or every session of a local principal or a provider, on the control plane.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `--token-id` | `N` |  | The one session with this `token_id`. |
| `--principal` | `NAME` |  | Every session of this local principal, including those minted for it. |
| `--provider` | `NAME` |  | Every session authorised through this provider. |

## `mosaica principal`

```text
mosaica principal <COMMAND>
```

Manage local principals on the control plane: people and services.

Every verb reads the control plane's credential from `MOSAICA_CREDENTIAL`, which is the operator credential, an API key or an OIDC access token, and needs `admin`. Each prints the server's JSON answer; a change answers how many sessions it ended.


## `mosaica principal list`

```text
mosaica principal list --control <ADDRESS>
```

List every local principal.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |

## `mosaica principal show`

```text
mosaica principal show --control <ADDRESS> <NAME>
```

Show one principal: its kind, flags, terms, permissions and groups.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `NAME` |  |  | The principal's name. |

## `mosaica principal create`

```text
mosaica principal create --control <ADDRESS> --kind <KIND> <NAME>
```

Create a local principal, holding nothing.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `NAME` |  |  | The principal's name. |
| `--kind` | `KIND` |  | `person` or `service`. |

## `mosaica principal delete`

```text
mosaica principal delete --control <ADDRESS> <NAME>
```

Delete a principal with its password, API keys, grants and memberships.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `NAME` |  |  | The principal's name. |

## `mosaica principal disable`

```text
mosaica principal disable --control <ADDRESS> <NAME>
```

Disable a principal. Its sessions end, and none of its credentials is accepted.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `NAME` |  |  | The principal's name. |

## `mosaica principal enable`

```text
mosaica principal enable --control <ADDRESS> <NAME>
```

Enable a disabled principal.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `NAME` |  |  | The principal's name. |

## `mosaica principal set-password`

```text
mosaica principal set-password --control <ADDRESS> <NAME>
```

Set a principal's password, read from the first line of stdin.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `NAME` |  |  | The principal's name. |

## `mosaica principal clear-password`

```text
mosaica principal clear-password --control <ADDRESS> <NAME>
```

Remove a principal's password.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `NAME` |  |  | The principal's name. |

## `mosaica key`

```text
mosaica key <COMMAND>
```

Manage API keys on the control plane. The credential is read as `mosaica principal` reads it.


## `mosaica key list`

```text
mosaica key list --control <ADDRESS> <PRINCIPAL>
```

List a principal's API keys, by prefix, without their secrets.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `PRINCIPAL` |  |  | The principal's name. |

## `mosaica key create`

```text
mosaica key create [OPTIONS] --control <ADDRESS> <PRINCIPAL>
```

Issue an API key for a principal. The whole key is printed once, here.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `PRINCIPAL` |  |  | The principal's name. |
| `--expires-at` | `SECONDS` |  | When the key, and every session authorised with it, ends, in seconds since the Unix epoch. Without it the key does not expire. |
| `--permission` | `NAME` |  | A permission the key holds: `read`, `write`, `authorise-as`, `admin`, `read-all` or `write-all`. Repeatable. Without it the key holds its principal's permissions. |

## `mosaica key revoke`

```text
mosaica key revoke --control <ADDRESS> <PREFIX>
```

Revoke an API key by its prefix. Every session authorised or minted with it ends.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `PREFIX` |  |  | The key's prefix, as `mosaica key list` prints it. |

## `mosaica group`

```text
mosaica group <COMMAND>
```

Manage local groups and their members on the control plane. The credential is read as `mosaica principal` reads it.


## `mosaica group list`

```text
mosaica group list --control <ADDRESS>
```

List every local group.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |

## `mosaica group show`

```text
mosaica group show --control <ADDRESS> <NAME>
```

Show one group: its terms, permissions and members.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `NAME` |  |  | The group's name. |

## `mosaica group create`

```text
mosaica group create --control <ADDRESS> <NAME>
```

Create a local group.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `NAME` |  |  | The group's name. |

## `mosaica group delete`

```text
mosaica group delete --control <ADDRESS> <NAME>
```

Delete a group with its grants and memberships.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `NAME` |  |  | The group's name. |

## `mosaica group add-member`

```text
mosaica group add-member --control <ADDRESS> <GROUP> <PRINCIPAL>
```

Add a principal to a group.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `GROUP` |  |  | The group's name. |
| `PRINCIPAL` |  |  | The principal's name. |

## `mosaica group remove-member`

```text
mosaica group remove-member --control <ADDRESS> <GROUP> <PRINCIPAL>
```

Remove a principal from a group.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `GROUP` |  |  | The group's name. |
| `PRINCIPAL` |  |  | The principal's name. |

## `mosaica grant`

```text
mosaica grant --control <ADDRESS> <--principal <NAME>|--group <NAME>> <--term <TERM>|--permission <NAME>>
```

Grant a term or a permission to a principal or a group, on the control plane.

A term says what the grantee may see, and a permission what it may do. The sessions the grant affects end, so they pick it up when they authorise again. The credential is read as `mosaica principal` reads it.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `--principal` | `NAME` |  | The local principal the grant is made to. |
| `--group` | `NAME` |  | The group the grant is made to. |
| `--term` | `TERM` |  | A term, which says what the grantee may see. Repeatable: the terms are granted or revoked in one change, and one refused term refuses them all. |
| `--permission` | `NAME` |  | A permission, which says what the grantee may do: `read`, `write`, `authorise-as`, `admin`, `read-all` or `write-all`. With `write`, the grantee writes only within its terms; `write-all` writes everywhere and is needed to declare and drop. |

## `mosaica revoke-grant`

```text
mosaica revoke-grant --control <ADDRESS> <--principal <NAME>|--group <NAME>> <--term <TERM>|--permission <NAME>>
```

Revoke a term or a permission from a principal or a group, on the control plane. The credential is read as `mosaica principal` reads it.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `--principal` | `NAME` |  | The local principal the grant is made to. |
| `--group` | `NAME` |  | The group the grant is made to. |
| `--term` | `TERM` |  | A term, which says what the grantee may see. Repeatable: the terms are granted or revoked in one change, and one refused term refuses them all. |
| `--permission` | `NAME` |  | A permission, which says what the grantee may do: `read`, `write`, `authorise-as`, `admin`, `read-all` or `write-all`. With `write`, the grantee writes only within its terms; `write-all` writes everywhere and is needed to declare and drop. |

## `mosaica provider`

```text
mosaica provider <COMMAND>
```

Manage OIDC providers on the control plane. The credential is read as `mosaica principal` reads it.


## `mosaica provider list`

```text
mosaica provider list --control <ADDRESS>
```

List every OIDC provider, declared through the API or in `mosaica.toml`.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |

## `mosaica provider show`

```text
mosaica provider show --control <ADDRESS> <NAME>
```

Show one provider.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `NAME` |  |  | The provider's name. |

## `mosaica provider put`

```text
mosaica provider put [OPTIONS] --control <ADDRESS> --issuer <ISSUER> --audience <AUDIENCE> --jwks-url <URL> <NAME>
```

Declare an OIDC provider, or replace one whole. Every session authorised through a provider it replaces ends.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `NAME` |  |  | The provider's name. |
| `--issuer` | `ISSUER` |  | The `iss` its tokens carry. |
| `--audience` | `AUDIENCE` |  | The `aud` its tokens must hold. |
| `--jwks-url` | `URL` |  | Where its signing keys are published: `https`, or `http` to a loopback address. |
| `--claim-rule` | `CLAIM TEMPLATE` |  | A claim rule: a claim path and a template, such as `groups[*] {value}`. Repeatable. |
| `--role-mapping` | `CLAIM VALUE GROUP` |  | A role mapping: a claim path, the exact value, and the local group it gives, such as `groups[*] mosaica-admins admins`. Repeatable. |

## `mosaica provider delete`

```text
mosaica provider delete --control <ADDRESS> <NAME>
```

Remove a provider. Every session authorised through it ends.

| Argument | Value | Default | Description |
| --- | --- | --- | --- |
| `--control` | `ADDRESS` |  | The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket. |
| `NAME` |  |  | The provider's name. |
