//! **`PUT /control/view_groups/{name}` and `PUT /control/views/{name}`** (`ingest.md` §1.3 and
//! §10 R9; decision 0136, track T6): a view group and a plain view are declared at a running
//! service, an identical redeclaration answers the one that exists, a differing one is refused, a
//! view is created under a runtime group, a runtime plain view takes rows at its first flush and
//! serves a viewport, and both declarations survive a restart and a fold.
//!
//! **The plain-view route is what R9 overturns.** `views.md` §7 ruled that a plain view is
//! declared when the corpus is built; decision 0134's wider rule is that anything a build can
//! create, live ingest can create.
//!
//! The roster half — a view created under a group the *build* declared — is
//! `tests/views_write.rs`; what this file pins is the two declaration routes and what a group
//! declared here can then do.

mod common;

use std::sync::Arc;

use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use common::*;
use serde_json::{json, Value};

/// The items of the built bundle, which holds one plain view, `s0`, and no group: every group and
/// every other view below is one the running service declared.
const N: u64 = 20;

async fn put(served: &Served, path: &str, body: Value) -> (u16, Value) {
    control(&served.server, reqwest::Method::PUT, path, &body).await
}

async fn declare_group(served: &Served, name: &str, body: Value) -> (u16, Value) {
    put(served, &format!("/control/view_groups/{name}"), body).await
}

async fn declare_view(served: &Served, name: &str, body: Value) -> (u16, Value) {
    put(served, &format!("/control/views/{name}"), body).await
}

fn frame() -> Value {
    json!({ "x": [0.0, 1000.0], "y": [0.0, 1000.0] })
}

/// The group the tests below declare: one frame, one metadata name, no gate.
fn quarter() -> Value {
    json!({
        "title": "Quarters",
        "extent": frame(),
        "point_visibility": { "default": "public" },
        "metadata": [{ "name": "label", "type": "text" }]
    })
}

/// A plain view: a frame and nothing else.
fn embedding() -> Value {
    json!({ "extent": frame(), "point_visibility": { "default": "public" } })
}

async fn meta(served: &Served) -> Value {
    let resp = served
        .server
        .client
        .get(served.server.viewer_url("/v1/meta"))
        .bearer_auth(&served.token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    resp.json().await.unwrap()
}

async fn view_ids(served: &Served) -> Vec<String> {
    meta(served).await["views"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["id"].as_str().unwrap().to_string())
        .collect()
}

async fn group_names(served: &Served) -> Vec<String> {
    meta(served).await["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["name"].as_str().unwrap().to_string())
        .collect()
}

/// New items at `(x, y)`.
fn batch(rows: &[(f32, f32)]) -> Vec<u8> {
    labelled_batch(rows, &["0"])
}

/// [`batch`], every row carrying `labels`.
fn labelled_batch(rows: &[(f32, f32)], labels: &[&str]) -> Vec<u8> {
    let labels: Vec<&[&str]> = rows.iter().map(|_| labels).collect();
    let access = access_lists(&labels);
    let schema = Arc::new(Schema::new(vec![
        Field::new("x", DataType::Float32, false),
        Field::new("y", DataType::Float32, false),
        access_field(&access),
    ]));
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(arrow::array::Float32Array::from_iter_values(
                rows.iter().map(|r| r.0),
            )),
            Arc::new(arrow::array::Float32Array::from_iter_values(
                rows.iter().map(|r| r.1),
            )),
            Arc::new(access),
        ],
    )
    .unwrap();
    let mut writer = arrow::ipc::writer::StreamWriter::try_new(Vec::new(), &schema).unwrap();
    writer.write(&batch).unwrap();
    writer.into_inner().unwrap()
}

async fn ingest(served: &Served, batch_id: &str, view: &str, rows: &[(f32, f32)]) -> u16 {
    served
        .server
        .client
        .post(served.server.control_url("/control/ingest"))
        .bearer_auth(OPERATOR_CREDENTIAL)
        .header("x-mosaica-batch-id", batch_id)
        .header("x-mosaica-view", view)
        .header("content-type", "application/vnd.apache.arrow.stream")
        .body(batch(rows))
        .send()
        .await
        .unwrap()
        .status()
        .as_u16()
}

/// The rows a viewport answers for `view`, by `mosaica_id`.
async fn points(served: &Served, view: &str) -> Vec<u64> {
    let resp = served
        .server
        .client
        .post(served.server.viewer_url("/v1/viewport"))
        .bearer_auth(&served.token)
        .json(&json!({
            "view": view, "zoom": 0, "bbox": [0.0, 0.0, 1000.0, 1000.0], "k": 200
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status().as_u16(),
        200,
        "a declared view answers: {view}"
    );
    let (_, points) = decode_viewport(&resp.bytes().await.unwrap());
    points.into_iter().map(|(id, _)| id).collect()
}

// ---------------------------------------------------------------------------------------------

/// **The group route's answers** (contracts §3.4): `201` for a new group, `200` for an identical
/// redeclaration, `409` for a held name under another identity, `422` for a declaration the rules
/// refuse, and `404` for a `members` group naming a group this deployment does not carry.
#[tokio::test]
async fn the_group_route_declares_answers_redeclarations_and_refuses_what_the_rules_refuse() {
    let mut served = Served::build(|dir| build_fixture(dir, N)).await;
    assert!(group_names(&served).await.is_empty());

    let (status, body) = declare_group(&served, "quarter", quarter()).await;
    assert_eq!(status, 201, "{body}");
    assert_eq!(
        without_publication(body),
        json!({ "group": "quarter", "existing": false })
    );

    let (status, body) = declare_group(&served, "quarter", quarter()).await;
    assert_eq!(status, 200, "identical: the group that exists: {body}");
    assert_eq!(
        without_publication(body),
        json!({ "group": "quarter", "existing": true })
    );

    let mut moved = quarter();
    moved["extent"] = json!({ "x": [0.0, 500.0], "y": [0.0, 1000.0] });
    let (status, body) = declare_group(&served, "quarter", moved).await;
    assert_eq!(status, 409, "a held name at another frame: {body}");
    assert_eq!(body["error"], "conflict");

    // A sharing group, and the two ways of getting one wrong.
    let (status, body) = declare_group(
        &served,
        "quarter_map",
        json!({ "extent": frame(), "members": "quarter" }),
    )
    .await;
    assert_eq!(status, 201, "{body}");
    let (status, body) = declare_group(
        &served,
        "chained",
        json!({ "extent": frame(), "members": "quarter_map" }),
    )
    .await;
    assert_eq!(status, 422, "a chain is refused: {body}");
    assert_eq!(body["error"], "contract", "{body}");
    let (status, body) = declare_group(
        &served,
        "orphan",
        json!({ "extent": frame(), "members": "nothing" }),
    )
    .await;
    assert_eq!(status, 404, "an unknown owner: {body}");

    for bad in [
        json!({ "extent": { "x": [0.0, 0.0], "y": [0.0, 1000.0] } }),
        json!({ "extent": frame(), "projection": "nonsense" }),
        json!({ "extent": frame(), "metadata": [{ "name": "key", "type": "text" }] }),
        json!({ "extent": frame(), "visibility": [] }),
    ] {
        let (status, body) = declare_group(&served, "other", bad.clone()).await;
        assert_eq!(status, 422, "{bad}: {body}");
        assert!(body["detail"].is_string(), "{bad}: {body}");
    }

    // A view and a group are one namespace: `s0` is the build's plain view.
    let (status, body) = declare_group(&served, "s0", quarter()).await;
    assert_eq!(status, 422, "{body}");

    served.reauthorise().await;
    assert_eq!(group_names(&served).await, ["quarter", "quarter_map"]);
}

/// **A group created at runtime accepts a view under it** — the roster route, unchanged, over a
/// group no build declared — and the view is empty until its first flush, which is what a view of
/// a build-declared group already gets (`views.md` §3.2).
#[tokio::test]
async fn a_group_created_at_runtime_accepts_a_view_under_it() {
    let mut served = Served::build(|dir| build_fixture(dir, N)).await;
    assert_eq!(declare_group(&served, "quarter", quarter()).await.0, 201);

    let (status, body) = put(
        &served,
        "/control/views/quarter/2026-Q1",
        json!({ "metadata": { "label": "Q1 2026" } }),
    )
    .await;
    assert_eq!(status, 201, "{body}");

    // A record the group's declaration refuses is refused here too: the metadata is the group's.
    let (status, body) = put(
        &served,
        "/control/views/quarter/2026-Q2",
        json!({ "metadata": { "nothing": "x" } }),
    )
    .await;
    assert_eq!(status, 422, "{body}");

    served.reauthorise().await;
    let ids = view_ids(&served).await;
    assert!(
        ids.contains(&"quarter:2026-Q1".to_string()),
        "the created view is served: {ids:?}"
    );
    assert!(
        points(&served, "quarter:2026-Q1").await.is_empty(),
        "it starts with an empty row space"
    );

    assert_eq!(
        ingest(&served, "q1", "quarter:2026-Q1", &[(100.0, 100.0)]).await,
        200
    );
    drain(&served.server).await;
    served.reauthorise().await;
    assert_eq!(
        points(&served, "quarter:2026-Q1").await.len(),
        1,
        "and takes rows at its first flush"
    );
}

/// **A roster integer that does not fit its declared width is refused** when a view is created at
/// a running service, as a build refuses it in a roster table; one that fits is accepted and
/// served as written.
#[tokio::test]
async fn a_roster_integer_past_its_declared_width_is_refused_at_a_create() {
    let mut served = Served::build(|dir| build_fixture(dir, N)).await;
    let group = json!({
        "extent": frame(),
        "point_visibility": { "default": "public" },
        "metadata": [{ "name": "tier", "type": "u8" }]
    });
    assert_eq!(declare_group(&served, "tiers", group).await.0, 201);

    let (status, body) = put(
        &served,
        "/control/views/tiers/wide",
        json!({ "metadata": { "tier": 300 } }),
    )
    .await;
    assert_eq!(status, 422, "{body}");

    let (status, body) = put(
        &served,
        "/control/views/tiers/fits",
        json!({ "metadata": { "tier": 255 } }),
    )
    .await;
    assert_eq!(status, 201, "{body}");

    served.reauthorise().await;
    let document = meta(&served).await;
    let views = document["views"].as_array().unwrap();
    assert!(
        !views.iter().any(|v| v["id"] == "tiers:wide"),
        "the refused view was not created"
    );
    let fits = views
        .iter()
        .find(|v| v["id"] == "tiers:fits")
        .expect("the accepted view is served");
    assert_eq!(fits["metadata"]["tier"]["value"], 255, "{fits}");
}

/// The roster `timestamp_us` values the test below writes: each view's key, the value as a create
/// sends it in JSON, the value as a build reads it in TOML, and the microseconds it names.
const ROSTER_TIMES: [(&str, &str, &str, i64); 4] = [
    ("offset", r#""2026-04-01T10:30:00+01:00""#, r#""2026-04-01T10:30:00+01:00""#, 1_775_035_800_000_000),
    // A TOML local date and offset date-time, against the same text in a JSON string.
    ("date", r#""2026-04-02""#, "2026-04-02", 1_775_088_000_000_000),
    ("toml", r#""2026-04-01T09:30:00.5Z""#, "2026-04-01T09:30:00.5Z", 1_775_035_800_500_000),
    ("digits", r#""1775035800000000""#, r#""1775035800000000""#, 1_775_035_800_000_000),
];

/// Each `eras:<key>` view's `starts` as `/v1/meta` serves it.
async fn roster_times(served: &Served) -> Vec<(String, Value)> {
    let document = meta(served).await;
    let mut out: Vec<(String, Value)> = document["views"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|v| v["id"].as_str().unwrap().starts_with("eras:"))
        .map(|v| (v["id"].as_str().unwrap().to_string(), v["metadata"]["starts"].clone()))
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// A declaration of the `eras` group, its views each from `points.parquet`, with `starts` written
/// as `starts` gives it for each key.
fn eras_declaration(starts: &[(&str, &str)]) -> String {
    let mut out = r#"
[sources]
points = "points.parquet"

[defaults]
allocation_view = "world"

[[view]]
name             = "world"
source           = "points"
extent           = { x = [0.0, 1000.0], y = [0.0, 1000.0] }
point_visibility = { default = "public" }

[[view_group]]
name             = "eras"
extent           = { x = [0.0, 1000.0], y = [0.0, 1000.0] }
point_visibility = { default = "public" }
metadata         = { starts = "timestamp_us" }
"#
    .to_string();
    for (key, value) in starts {
        out.push_str(&format!(
            "\n[[view_group.view]]\nkey    = \"{key}\"\nsource = \"points\"\nstarts = {value}\n"
        ));
    }
    out
}

/// **A roster timestamp written as text is stored alike by a build and a create**: a date, a
/// date-time with an offset and a string of digits are each the microseconds they name, on both
/// paths and after a restart, and a date-time with no offset or a seventh fractional digit is
/// refused on both.
#[tokio::test]
async fn a_roster_timestamp_written_as_text_is_stored_alike_by_a_build_and_a_create() {
    let expected: Vec<(String, Value)> = {
        let mut out: Vec<(String, Value)> = ROSTER_TIMES
            .iter()
            .map(|(key, _, _, micros)| {
                (format!("eras:{key}"), json!({ "type": "timestamp_us", "value": micros }))
            })
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    };
    const REFUSED: [&str; 2] = ["2026-04-01T09:30:00", "2026-04-01T09:30:00.1234567Z"];

    // A create at a running service.
    let mut served = Served::build(|dir| build_fixture(dir, N)).await;
    let group = json!({
        "extent": frame(),
        "point_visibility": { "default": "public" },
        "metadata": [{ "name": "starts", "type": "timestamp_us" }]
    });
    assert_eq!(declare_group(&served, "eras", group).await.0, 201);
    for (key, sent, _, _) in ROSTER_TIMES {
        let starts: Value = serde_json::from_str(sent).unwrap();
        let (status, body) =
            put(&served, &format!("/control/views/eras/{key}"), json!({ "metadata": { "starts": starts } })).await;
        assert_eq!(status, 201, "{key}: {body}");
    }
    for (n, refused) in REFUSED.iter().enumerate() {
        let (status, body) =
            put(&served, &format!("/control/views/eras/refused{n}"), json!({ "metadata": { "starts": refused } })).await;
        assert_eq!(status, 422, "{refused}: {body}");
    }
    served.reauthorise().await;
    assert_eq!(roster_times(&served).await, expected, "as created");
    let served = served.restart().await;
    assert_eq!(roster_times(&served).await, expected, "after a restart");

    // The same values declared at a build.
    let written: Vec<(&str, &str)> = ROSTER_TIMES.iter().map(|(key, _, toml, _)| (*key, *toml)).collect();
    let built = Served::build(|dir| {
        write_points_n(&dir.join("points.parquet"), N);
        let path = dir.join("corpus.toml");
        std::fs::write(&path, eras_declaration(&written)).unwrap();
        mosaica_build::build(&args_from_declaration(&path, &dir.join("bundle"))).expect("the build runs");
    })
    .await;
    assert_eq!(roster_times(&built).await, expected, "as built");

    let dir = tempfile::TempDir::new().unwrap();
    write_points_n(&dir.path().join("points.parquet"), N);
    for refused in ["2026-04-01T09:30:00", r#""2026-04-01T09:30:00""#, r#""2026-04-01T09:30:00.1234567Z""#] {
        let path = dir.path().join("corpus.toml");
        std::fs::write(&path, eras_declaration(&[("refused", refused)])).unwrap();
        assert!(
            mosaica_build::config::Config::parse(&path, &Default::default()).is_err(),
            "{refused}"
        );
    }
}

/// Each `zones:<key>` view's `region` as `/v1/meta` serves it.
async fn roster_regions(served: &Served) -> Vec<(String, Value)> {
    let document = meta(served).await;
    let mut out: Vec<(String, Value)> = document["views"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|v| v["id"].as_str().unwrap().starts_with("zones:"))
        .map(|v| (v["id"].as_str().unwrap().to_string(), v["metadata"]["region"].clone()))
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// The `zones` views and the region key each carries.
const ZONES: [(&str, &str); 2] = [("east", "north"), ("west", "south")];

/// **A category roster value is stored alike by a build and a create**: a group declared at a
/// running service names a vocabulary for a metadata name as a build's does, and each view of it
/// carries its key, served as a build serves it, after a restart, a fold and a restart after that.
/// A category naming no vocabulary, or one not declared, is refused, and so is a code where a key
/// belongs.
#[tokio::test]
async fn a_roster_category_is_stored_alike_by_a_build_and_a_create() {
    let expected: Vec<(String, Value)> = ZONES
        .iter()
        .map(|(key, region)| (format!("zones:{key}"), json!({ "type": "text", "value": region })))
        .collect();
    let group = |vocabulary: Option<&str>| {
        let mut field = json!({ "name": "region", "type": "category" });
        if let Some(vocabulary) = vocabulary {
            field["vocabulary"] = json!(vocabulary);
        }
        json!({
            "extent": frame(),
            "point_visibility": { "default": "public" },
            "metadata": [field]
        })
    };

    let mut served = Served::build(|dir| build_fixture(dir, N)).await;
    let (status, body) = put(
        &served,
        "/control/vocabularies/regions",
        json!({
            "value_set": "closed",
            "visibility": "public",
            "width": "u8",
            "values": [{ "key": "north" }, { "key": "south" }]
        }),
    )
    .await;
    assert_eq!(status, 201, "{body}");
    for refused in [group(None), group(Some("nothing"))] {
        let (status, body) = declare_group(&served, "zones", refused).await;
        assert_eq!(status, 422, "{body}");
    }
    let (status, body) = declare_group(&served, "zones", group(Some("regions"))).await;
    assert_eq!(status, 201, "{body}");
    for (key, region) in ZONES {
        let (status, body) = put(
            &served,
            &format!("/control/views/zones/{key}"),
            json!({ "metadata": { "region": region } }),
        )
        .await;
        assert_eq!(status, 201, "{key}: {body}");
    }
    let (status, body) = put(
        &served,
        "/control/views/zones/coded",
        json!({ "metadata": { "region": 3 } }),
    )
    .await;
    assert_eq!(status, 422, "a code where a key belongs: {body}");

    served.reauthorise().await;
    assert_eq!(roster_regions(&served).await, expected, "as created");
    let mut served = served.restart().await;
    served.reauthorise().await;
    assert_eq!(roster_regions(&served).await, expected, "after a restart");
    assert_eq!(
        declare_group(&served, "zones", group(Some("regions"))).await.0,
        200,
        "the replayed group is the one a redeclaration meets"
    );
    assert_eq!(ingest(&served, "rows", "zones:east", &[(100.0, 100.0)]).await, 200);
    drain(&served.server).await;
    fold(&served.server).await;
    let mut served = served.restart().await;
    served.reauthorise().await;
    assert_eq!(roster_regions(&served).await, expected, "after a fold and a restart");

    // The same declared at a build.
    let built = Served::build(|dir| {
        write_points_n(&dir.join("points.parquet"), N);
        let mut declaration = r#"
[sources]
points = "points.parquet"

[defaults]
allocation_view = "world"

[[vocabulary]]
name       = "regions"
width      = "u8"
value_set  = "closed"
visibility = "public"
values     = ["north", "south"]

[[view]]
name             = "world"
source           = "points"
extent           = { x = [0.0, 1000.0], y = [0.0, 1000.0] }
point_visibility = { default = "public" }

[[view_group]]
name             = "zones"
extent           = { x = [0.0, 1000.0], y = [0.0, 1000.0] }
point_visibility = { default = "public" }
metadata         = { region = { type = "category", vocabulary = "regions" } }
"#
        .to_string();
        for (key, region) in ZONES {
            declaration.push_str(&format!(
                "\n[[view_group.view]]\nkey    = \"{key}\"\nsource = \"points\"\nregion = \"{region}\"\n"
            ));
        }
        let path = dir.join("corpus.toml");
        std::fs::write(&path, declaration).unwrap();
        mosaica_build::build(&args_from_declaration(&path, &dir.join("bundle")))
            .expect("the build runs");
    })
    .await;
    assert_eq!(roster_regions(&built).await, expected, "as built");
}

/// The keys `/v1/categories/area` lists: the values of vocabulary `areas`, public, so every one.
async fn area_keys(served: &Served) -> Vec<String> {
    let resp = served
        .server
        .client
        .get(served.server.viewer_url("/v1/categories/area"))
        .bearer_auth(&served.token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    body["values"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["key"].as_str().unwrap().to_string())
        .collect()
}

/// A category column over `areas`, whose values `/v1/categories/area` lists.
fn area_column() -> Value {
    json!({ "name": "area", "type": "category", "vocabulary": "areas", "index": true })
}

/// A declaration of the `zones` group over a closed `regions` and an open `areas`, with one view
/// `east` carrying `region` and `area`.
fn zones_declaration(region: &str, area: &str) -> String {
    format!(
        r#"
[sources]
points = "points.parquet"

[defaults]
allocation_view = "world"

[[vocabulary]]
name       = "regions"
width      = "u8"
value_set  = "closed"
visibility = "public"
values     = ["north", "south"]

[[vocabulary]]
name       = "areas"
width      = "u8"
value_set  = "open"
visibility = "public"

[[view]]
name             = "world"
source           = "points"
extent           = {{ x = [0.0, 1000.0], y = [0.0, 1000.0] }}
point_visibility = {{ default = "public" }}

[[view_group]]
name             = "zones"
extent           = {{ x = [0.0, 1000.0], y = [0.0, 1000.0] }}
point_visibility = {{ default = "public" }}
metadata         = {{ region = {{ type = "category", vocabulary = "regions" }}, area = {{ type = "category", vocabulary = "areas" }} }}

[[view_group.view]]
key    = "east"
source = "points"
region = "{region}"
area   = "{area}"
"#
    )
}

/// **A category roster value follows its vocabulary's value set on both paths**, as a row's
/// category value does: a key a closed vocabulary does not list is refused, by a build and by a
/// create, and a key an open vocabulary does not hold is added to it, served among its values and
/// kept across a restart.
#[tokio::test]
async fn a_roster_category_key_follows_its_vocabulary_value_set_on_both_paths() {
    let mut served = Served::build(|dir| build_fixture(dir, N)).await;
    for (name, body) in [
        (
            "regions",
            json!({ "value_set": "closed", "visibility": "public", "width": "u8",
                    "values": [{ "key": "north" }, { "key": "south" }] }),
        ),
        (
            "areas",
            json!({ "value_set": "open", "visibility": "public", "width": "u8" }),
        ),
    ] {
        let (status, answer) = put(&served, &format!("/control/vocabularies/{name}"), body).await;
        assert_eq!(status, 201, "{name}: {answer}");
    }
    let (status, answer) = put(&served, "/control/attributes", area_column()).await;
    assert_eq!(status, 201, "{answer}");
    let group = json!({
        "extent": frame(),
        "point_visibility": { "default": "public" },
        "metadata": [
            { "name": "region", "type": "category", "vocabulary": "regions" },
            { "name": "area", "type": "category", "vocabulary": "areas" },
        ]
    });
    assert_eq!(declare_group(&served, "zones", group).await.0, 201);
    let (status, answer) = put(
        &served,
        "/control/views/zones/west",
        json!({ "metadata": { "region": "nowhere", "area": "unseen" } }),
    )
    .await;
    assert_eq!(status, 422, "a key the closed vocabulary does not list: {answer}");
    let (status, answer) = put(
        &served,
        "/control/views/zones/east",
        json!({ "metadata": { "region": "north", "area": "fresh" } }),
    )
    .await;
    assert_eq!(status, 201, "{answer}");

    served.reauthorise().await;
    assert_eq!(area_keys(&served).await, ["fresh"], "the open vocabulary took the key");
    let mut served = served.restart().await;
    served.reauthorise().await;
    assert_eq!(area_keys(&served).await, ["fresh"], "after a restart");

    let built = Served::build(|dir| {
        write_points_n(&dir.join("points.parquet"), N);
        let path = dir.join("corpus.toml");
        std::fs::write(&path, zones_declaration("north", "fresh")).unwrap();
        mosaica_build::build(&args_from_declaration(&path, &dir.join("bundle")))
            .expect("the build runs");
    })
    .await;
    // The column that lists the vocabulary's values, declared as the running service's was.
    let (status, answer) = put(&built, "/control/attributes", area_column()).await;
    assert_eq!(status, 201, "{answer}");
    assert_eq!(area_keys(&built).await, ["fresh"], "as built");

    let dir = tempfile::TempDir::new().unwrap();
    write_points_n(&dir.path().join("points.parquet"), N);
    let path = dir.path().join("corpus.toml");
    std::fs::write(&path, zones_declaration("nowhere", "fresh")).unwrap();
    assert!(
        mosaica_build::build(&args_from_declaration(&path, &dir.path().join("bundle"))).is_err(),
        "a build refuses a key the closed vocabulary does not list"
    );
}

/// **A plain view created at runtime takes rows at its first flush and serves a viewport**
/// (decision 0136, R9). Its answers are the group route's, and it shares a namespace with the
/// groups.
#[tokio::test]
async fn a_plain_view_created_at_runtime_takes_rows_at_its_first_flush() {
    let mut served = Served::build(|dir| build_fixture(dir, N)).await;
    let (status, body) = declare_view(&served, "embedding", embedding()).await;
    assert_eq!(status, 201, "{body}");
    assert_eq!(
        without_publication(body),
        json!({ "view": "embedding", "existing": false })
    );

    assert_eq!(
        declare_view(&served, "embedding", embedding()).await.0,
        200,
        "an identical redeclaration answers the view that exists"
    );
    let mut moved = embedding();
    moved["extent"] = json!({ "x": [0.0, 10.0], "y": [0.0, 10.0] });
    assert_eq!(
        declare_view(&served, "embedding", moved).await.0,
        409,
        "a held name at another frame"
    );
    assert_eq!(
        declare_view(&served, "s0", embedding()).await.0,
        200,
        "the build's own view is a held name, and a declaration matching it is the no-op any \
         identical redeclaration is"
    );
    let mut other_frame = embedding();
    other_frame["extent"] = json!({ "x": [0.0, 10.0], "y": [0.0, 10.0] });
    assert_eq!(
        declare_view(&served, "s0", other_frame).await.0,
        409,
        "and one differing from it is refused, a frame being immutable for a view's life"
    );
    assert_eq!(declare_group(&served, "quarter", quarter()).await.0, 201);
    let (status, body) = declare_view(&served, "quarter", embedding()).await;
    assert_eq!(status, 422, "a group's name is not free for a view: {body}");

    served.reauthorise().await;
    let ids = view_ids(&served).await;
    assert!(ids.contains(&"embedding".to_string()), "{ids:?}");
    assert!(
        points(&served, "embedding").await.is_empty(),
        "it starts with an empty row space"
    );

    assert_eq!(
        ingest(
            &served,
            "into-embedding",
            "embedding",
            &[(100.0, 100.0), (200.0, 300.0)]
        )
        .await,
        200
    );
    drain(&served.server).await;
    served.reauthorise().await;
    assert_eq!(points(&served, "embedding").await.len(), 2);
    assert_eq!(
        points(&served, "s0").await.len(),
        N as usize,
        "and the build's view is untouched"
    );
}

/// **A gate is one label or a list** (decision 0132), on both routes, and a label that is not an
/// access expression is refused rather than stored as a gate nobody could satisfy.
#[tokio::test]
async fn a_gate_is_one_label_or_a_list_on_both_routes() {
    let served = Served::build(|dir| build_fixture(dir, N)).await;

    let mut one = embedding();
    one["visibility"] = json!("0");
    assert_eq!(declare_view(&served, "gated_one", one).await.0, 201);

    let mut many = embedding();
    many["visibility"] = json!(["0", "1"]);
    assert_eq!(declare_view(&served, "gated_many", many).await.0, 201);

    let mut group_gate = quarter();
    group_gate["visibility"] = json!(["0", "1"]);
    assert_eq!(declare_group(&served, "gated_group", group_gate).await.0, 201);

    let mut public_beside = embedding();
    public_beside["visibility"] = json!(["public", "0"]);
    let (status, body) = declare_view(&served, "bad_gate", public_beside).await;
    assert_eq!(status, 422, "{body}");
    assert_eq!(body["error"], "contract", "{body}");
    assert!(body["detail"].as_str().unwrap().contains("public"), "{body}");

    let mut empty_element = embedding();
    empty_element["visibility"] = json!(["0", ""]);
    let (status, body) = declare_view(&served, "bad_gate", empty_element).await;
    assert_eq!(status, 422, "{body}");

    // A quoted term with whitespace at an end is one no credential holds.
    let mut edge = embedding();
    edge["visibility"] = json!(["0", "\"team a \""]);
    let (status, body) = declare_view(&served, "bad_gate", edge).await;
    assert_eq!(status, 422, "{body}");
    let mut edge_group = quarter();
    edge_group["visibility"] = json!("\" team a\"");
    let (status, body) = declare_group(&served, "bad_group", edge_group).await;
    assert_eq!(status, 422, "{body}");
    let (status, body) = put(
        &served,
        "/control/views/gated_group/edge",
        json!({ "visibility": ["0&\"1 \""], "metadata": { "label": "E" } }),
    )
    .await;
    assert_eq!(status, 422, "{body}");

    // The stored gate is the list as written, so a view gated on a label this session holds is
    // served and one gated on a label it does not is a 404.
    let token = token_for(&served.server, &["0"][..]).await;
    let resp = served
        .server
        .client
        .get(served.server.viewer_url("/v1/meta"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let meta: Value = resp.json().await.unwrap();
    let ids: Vec<&str> = meta["views"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"gated_one"), "{ids:?}");
    assert!(
        ids.contains(&"gated_many"),
        "a list gate is satisfied where the principal holds one of its labels: {ids:?}"
    );

    // **And the half that matters**: a principal holding none of a gate's terms cannot see the
    // view exists, and a request naming it is the 404 an unknown name is (`views.md` §6). The
    // fixture's principal `1` holds term `1` and not term `0`.
    let outside = token_for(&served.server, &["1"][..]).await;
    let resp = served
        .server
        .client
        .get(served.server.viewer_url("/v1/meta"))
        .bearer_auth(&outside)
        .send()
        .await
        .unwrap();
    let meta: Value = resp.json().await.unwrap();
    let ids: Vec<&str> = meta["views"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["id"].as_str().unwrap())
        .collect();
    assert!(
        !ids.contains(&"gated_one"),
        "a gate-failed view is absent from /v1/meta: {ids:?}"
    );
    assert!(
        ids.contains(&"gated_many"),
        "and one whose list names a term this principal does hold is not: {ids:?}"
    );
    let resp = served
        .server
        .client
        .post(served.server.viewer_url("/v1/viewport"))
        .bearer_auth(&outside)
        .json(&json!({
            "view": "gated_one", "zoom": 0, "bbox": [0.0, 0.0, 1000.0, 1000.0], "k": 200
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status().as_u16(),
        404,
        "a request naming a gate-failed view is the 404 an unknown name is"
    );
}

/// **`point_visibility.default` is read as an access expression**, on the gate's rule: it is
/// given to every point that carries no label of its own (decision 0133), so a label that is not
/// one would put those points in no principal's mask, and the refusal belongs at the
/// declaration rather than at every batch. The empty string, `inherited` and a quoted term with
/// whitespace at an end are refused, as the build refuses them.
#[tokio::test]
async fn a_point_default_is_read_as_an_expression_on_both_routes() {
    let served = Served::build(|dir| build_fixture(dir, N)).await;

    for default in ["", "inherited", "\"0 \""] {
        let mut view = embedding();
        view["point_visibility"] = json!({ "default": default });
        let (status, body) = declare_view(&served, "bad_default", view).await;
        assert_eq!(status, 422, "{default:?}: {body}");

        let mut group = quarter();
        group["point_visibility"] = json!({ "default": default });
        let (status, body) = declare_group(&served, "bad_default", group).await;
        assert_eq!(status, 422, "{default:?} on a group: {body}");
    }

    // A label, and `public`, are both accepted.
    let mut labelled = embedding();
    labelled["point_visibility"] = json!({ "default": "0" });
    assert_eq!(declare_view(&served, "labelled", labelled).await.0, 201);
    assert_eq!(declare_view(&served, "public_default", embedding()).await.0, 201);
}

/// **Declared words are stored trimmed**, on both routes: a padded gate is the same declaration
/// as its trimmed spelling, a view gated ` 0 ` is reached by a credential holding `0`, a gate
/// spaced between its tokens is the compact gate, a gate of ` public ` is no gate, and a spaced
/// point default holding a quoted `"team a"` is given to an unlabelled row as its label, reached
/// by a credential presenting ` team a `, before and after a restart.
#[tokio::test]
async fn padded_gates_and_point_defaults_are_stored_trimmed() {
    let served = Served::build(|dir| build_fixture(dir, N)).await;

    let mut gated = embedding();
    gated["visibility"] = json!(" 0 ");
    assert_eq!(declare_view(&served, "gated", gated).await.0, 201);
    let mut same = embedding();
    same["visibility"] = json!("0");
    assert_eq!(declare_view(&served, "gated", same).await.0, 200, "the gate is stored trimmed");

    let mut spaced = embedding();
    spaced["visibility"] = json!(" 0 & ( 1 | 2 ) ");
    assert_eq!(declare_view(&served, "spaced", spaced).await.0, 201);
    let mut same = embedding();
    same["visibility"] = json!("0&(1|2)");
    assert_eq!(declare_view(&served, "spaced", same).await.0, 200, "spaces are not stored");

    let mut open = embedding();
    open["visibility"] = json!([" public "]);
    assert_eq!(declare_view(&served, "open", open).await.0, 201);
    assert_eq!(declare_view(&served, "open", embedding()).await.0, 200, "` public ` is no gate");

    let mut group = quarter();
    group["visibility"] = json!([" 0 ", "1 "]);
    group["point_visibility"] = json!({ "default": " public " });
    assert_eq!(declare_group(&served, "gated_group", group).await.0, 201);
    let mut same = quarter();
    same["visibility"] = json!(["0", "1"]);
    assert_eq!(declare_group(&served, "gated_group", same).await.0, 200);
    let (status, body) = put(
        &served,
        "/control/views/gated_group/k",
        json!({ "visibility": [" 1 "], "metadata": { "label": "K" } }),
    )
    .await;
    assert_eq!(status, 201, "{body}");

    let mut defaulted = embedding();
    defaulted["point_visibility"] = json!({ "default": " 5 | \"team a\" " });
    assert_eq!(declare_view(&served, "defaulted", defaulted).await.0, 201);
    let resp = served
        .server
        .client
        .post(served.server.control_url("/control/ingest"))
        .bearer_auth(OPERATOR_CREDENTIAL)
        .header("x-mosaica-batch-id", "blank")
        .header("x-mosaica-view", "defaulted")
        .header("content-type", "application/vnd.apache.arrow.stream")
        .body(labelled_batch(&[(100.0, 100.0)], &["  "]))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    drain(&served.server).await;

    async fn views_for(served: &Served, terms: &[&str]) -> Vec<String> {
        let token = token_for(&served.server, terms).await;
        let resp = served
            .server
            .client
            .get(served.server.viewer_url("/v1/meta"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap();
        let meta: Value = resp.json().await.unwrap();
        meta["views"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["id"].as_str().unwrap().to_string())
            .collect()
    }
    async fn count_for(served: &Served, terms: &[&str]) -> usize {
        let token = token_for(&served.server, terms).await;
        let resp = served
            .server
            .client
            .post(served.server.viewer_url("/v1/viewport"))
            .bearer_auth(&token)
            .json(&json!({
                "view": "defaulted", "zoom": 0, "bbox": [0.0, 0.0, 1000.0, 1000.0], "k": 200
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        decode_viewport(&resp.bytes().await.unwrap()).1.len()
    }
    async fn assert_trimmed(served: &Served) {
        let zero = views_for(served, &["0"]).await;
        assert!(zero.contains(&"gated".to_string()), "{zero:?}");
        assert!(zero.contains(&"open".to_string()), "{zero:?}");
        let one = views_for(served, &["1"]).await;
        assert!(!one.contains(&"gated".to_string()), "{one:?}");
        assert!(one.contains(&"open".to_string()), "{one:?}");
        assert!(one.contains(&"gated_group:k".to_string()), "{one:?}");
        assert!(!zero.contains(&"spaced".to_string()), "{zero:?}");
        let zero_two = views_for(served, &["0", "2"]).await;
        assert!(zero_two.contains(&"spaced".to_string()), "{zero_two:?}");
        assert_eq!(count_for(served, &["5"]).await, 1, "the default is stored as `5|\"team a\"`");
        assert_eq!(count_for(served, &[" team a "]).await, 1, "a held term is trimmed");
        assert_eq!(count_for(served, &["team  a"]).await, 0);
        assert_eq!(count_for(served, &["0"]).await, 0);
    }
    assert_trimmed(&served).await;
    let served = served.restart().await;
    assert_trimmed(&served).await;
}

/// **Both declarations survive a restart and a fold** (`ingest.md` §1.3): from the log alone
/// before any publication, from the segments manifest after one, and from `MANIFEST.json` after
/// the fold that writes them there. A view created under a runtime group survives with it, which
/// is what the manifest's merge order exists for.
#[tokio::test]
async fn the_declarations_survive_a_restart_and_a_fold() {
    let served = Served::build(|dir| build_fixture(dir, N)).await;
    assert_eq!(declare_group(&served, "quarter", quarter()).await.0, 201);
    assert_eq!(declare_view(&served, "embedding", embedding()).await.0, 201);
    assert_eq!(
        put(
            &served,
            "/control/views/quarter/2026-Q1",
            json!({ "metadata": { "label": "Q1 2026" } })
        )
        .await
        .0,
        201
    );

    // Replayed from the log, nothing having been published yet.
    let mut served = served.restart().await;
    served.reauthorise().await;
    assert_eq!(group_names(&served).await, ["quarter"]);
    let ids = view_ids(&served).await;
    assert!(ids.contains(&"embedding".to_string()), "{ids:?}");
    assert!(
        ids.contains(&"quarter:2026-Q1".to_string()),
        "a view created under a runtime group comes back with it: {ids:?}"
    );
    assert_eq!(
        declare_group(&served, "quarter", quarter()).await.0,
        200,
        "the replayed group is the one a redeclaration meets"
    );

    // Published into a segments manifest, then replayed from it.
    assert_eq!(
        ingest(&served, "rows", "embedding", &[(100.0, 100.0)]).await,
        200
    );
    drain(&served.server).await;
    let mut served = served.restart().await;
    served.reauthorise().await;
    assert_eq!(group_names(&served).await, ["quarter"]);
    assert_eq!(points(&served, "embedding").await.len(), 1);

    // Folded into `MANIFEST.json`, then replayed from it.
    fold(&served.server).await;
    let mut served = served.restart().await;
    served.reauthorise().await;
    assert_eq!(group_names(&served).await, ["quarter"]);
    let ids = view_ids(&served).await;
    assert!(ids.contains(&"embedding".to_string()), "{ids:?}");
    assert!(ids.contains(&"quarter:2026-Q1".to_string()), "{ids:?}");
    assert_eq!(
        points(&served, "embedding").await.len(),
        1,
        "and the rows it took are still served"
    );
    assert_eq!(
        declare_view(&served, "embedding", embedding()).await.0,
        200,
        "the folded view is still the one a redeclaration meets"
    );
}

/// `auto` fits a frame to a view's rows, and a view or group declared here has none. Each
/// spelling of it is refused, on both routes and under a projection.
#[tokio::test]
async fn auto_is_refused_on_both_routes() {
    let served = Served::build(|dir| build_fixture(dir, N)).await;
    let declared = |projection: &str, extent: Value| {
        json!({
            "projection": projection,
            "extent": extent,
            "point_visibility": { "default": "public" },
        })
    };
    for (projection, extent) in [
        ("none", json!("auto")),
        ("none", json!({ "auto": true })),
        ("none", json!({ "auto": true, "margin": 0.25 })),
        ("web_mercator", json!("auto")),
    ] {
        let (status, body) = declare_view(&served, "fitted", declared(projection, extent.clone())).await;
        assert_eq!(status, 422, "{projection} {extent}: {body}");
        let (status, body) = declare_group(&served, "fitted", declared(projection, extent.clone())).await;
        assert_eq!(status, 422, "{projection} {extent}: {body}");
    }
}

/// A box in degrees is a projected view's frame here as at a build: the route snaps it to the
/// square a build of the same box quantises against. Declaring the built view again with the box
/// is the no-op an identical declaration is, and a view declared with it beside it gets the
/// built view's frame, as does a view of a group declared with it.
#[tokio::test]
async fn a_degree_box_declared_here_is_the_frame_a_build_gives_it() {
    let mut served = Served::build(build_projected).await;
    let degrees = json!({
        "projection": "web_mercator",
        "extent": { "lon": [-8.6, 1.8], "lat": [49.9, 60.9] },
        "point_visibility": { "default": "public" },
    });
    let (status, body) = declare_view(&served, "built", degrees.clone()).await;
    assert_eq!(status, 200, "the box is the built view's own frame: {body}");
    let (status, body) = declare_view(&served, "declared", degrees.clone()).await;
    assert_eq!(status, 201, "{body}");
    let (status, body) = declare_group(&served, "regions", degrees).await;
    assert_eq!(status, 201, "{body}");
    let (status, body) = put(&served, "/control/views/regions/north", json!({})).await;
    assert_eq!(status, 201, "{body}");

    served.reauthorise().await;
    let meta = meta(&served).await;
    let frame = |id: &str| {
        meta["views"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["id"] == id)
            .unwrap_or_else(|| panic!("view '{id}' is served: {meta}"))["quantisation"]
            .clone()
    };
    assert_eq!(frame("declared"), frame("built"));
    assert_eq!(frame("regions:north"), frame("built"));
}

/// A bundle built with one projected view, `built`, framed by a box in degrees and holding no
/// rows.
fn build_projected(dir: &std::path::Path) {
    let path = dir.join("corpus.toml");
    std::fs::write(
        &path,
        r#"
[[view]]
name             = "built"
projection       = "web_mercator"
extent           = { lon = [-8.6, 1.8], lat = [49.9, 60.9] }
point_visibility = { default = "public" }
"#,
    )
    .unwrap();
    mosaica_build::build(&args_from_declaration(&path, &dir.join("bundle")))
        .expect("a declaration naming no file builds");
}

/// A projected view takes its frame in the coordinates the projection produces, as `/v1/meta`
/// reports a view's `quantisation`, so a view declared with the built view's reported frame gets
/// exactly that frame, and the built view declared again with it is the no-op an identical
/// declaration is.
#[tokio::test]
async fn a_projected_view_declared_with_another_views_frame_gets_that_frame() {
    let mut served = Served::build(build_projected).await;
    let frame = |meta: &Value, id: &str| {
        meta["views"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["id"] == id)
            .unwrap_or_else(|| panic!("view '{id}' is served: {meta}"))["quantisation"]
            .clone()
    };
    let built = frame(&meta(&served).await, "built");
    let declared = json!({
        "projection": "web_mercator",
        "extent": {
            "x": [built["x_min"], built["x_max"]],
            "y": [built["y_min"], built["y_max"]],
        },
        "point_visibility": { "default": "public" },
    });
    let (status, body) = declare_view(&served, "built", declared.clone()).await;
    assert_eq!(status, 200, "the built view's own frame: {body}");
    let (status, body) = declare_view(&served, "copied", declared).await;
    assert_eq!(status, 201, "{body}");

    served.reauthorise().await;
    assert_eq!(frame(&meta(&served).await, "copied"), built);
}

/// A declaration's answer with `publication` taken out, so a whole-object comparison stays a
/// whole-object comparison. The number is a running count and a test cannot name it, but its
/// absence would be a route that stopped telling a caller when its declaration becomes visible
/// (contracts §3.4), so this asserts it was there.
fn without_publication(mut body: Value) -> Value {
    assert!(
        body.as_object_mut()
            .expect("the answer is an object")
            .remove("publication")
            .is_some(),
        "every write acknowledgement carries a publication number: {body}"
    );
    body
}
