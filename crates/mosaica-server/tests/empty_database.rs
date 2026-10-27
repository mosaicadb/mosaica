//! A database that starts empty. A build from a declaration that names no file, or from an empty
//! declaration, writes a bundle a server opens and serves. Every declaration and row then arrives
//! through the control plane, is served, and survives a restart.

mod common;

use std::path::Path;

use arrow::array::{Array, Float64Array, Int64Array, UInt64Array};
use common::*;
use serde_json::{json, Value};
use tempfile::TempDir;

const LAYER: &str = "groups";

/// A view and two attributes, and no `[sources]`.
const VIEW_AND_ATTRIBUTES: &str = r#"
[[view]]
name             = "world"
extent           = { min = 0.0, max = 100.0 }
point_visibility = { default = "public" }

[[attribute]]
name   = "a"
type   = "u64"
unique = true

[[attribute]]
name = "score"
type = "i64"
"#;

/// [`VIEW_AND_ATTRIBUTES`] and a layer drawn on its view.
fn with_layer() -> String {
    format!(
        r#"{VIEW_AND_ATTRIBUTES}
[[layer]]
name                      = "{LAYER}"
views                     = ["world"]
membership                = "enumerated"
value_set                 = "open"
hierarchy                 = {{ kind = "flat" }}
visibility                = "public"
artifact_visibility       = {{ default = "inherited" }}
require_member_visibility = "any"
"#
    )
}

/// Build the bundle `declaration` declares into `dir/bundle`, where [`open`] serves it from.
fn build_from(dir: &Path, declaration: &str) {
    let path = dir.join("corpus.toml");
    std::fs::write(&path, declaration).unwrap();
    mosaica_build::build(&args_from_declaration(&path, &dir.join("bundle")))
        .expect("a declaration naming no file builds");
}

/// Three new items in the view `world`, each carrying both declared columns.
async fn ingest_rows(server: &TestServer) {
    let resp = server
        .client
        .post(server.control_url("/control/ingest"))
        .bearer_auth(OPERATOR_CREDENTIAL)
        .header("x-mosaica-batch-id", "rows")
        .header("x-mosaica-view", "world")
        .json(&json!([
            { "a": 1, "score": -3, "x": 10.0, "y": 20.0 },
            { "a": 2, "score": 7, "x": 55.5, "y": 40.25 },
            { "a": 3, "score": 0, "x": 90.0, "y": 80.0 },
        ]))
        .send()
        .await
        .unwrap();
    let status = resp.status().as_u16();
    let answer: Value = resp.json().await.unwrap_or(Value::Null);
    assert_eq!(status, 200, "{answer}");
    tick(server).await;
}

/// The items [`ingest_rows`] sends, as [`items`] reads them.
fn ingested() -> Vec<(u64, i64, i64, i64)> {
    vec![
        (1, -3, 10_000, 20_000),
        (2, 7, 55_500, 40_250),
        (3, 0, 90_000, 80_000),
    ]
}

/// Every item a principal holding no term sees in `view`: its `a`, its `score`, and its position
/// in thousandths, sorted.
async fn items(server: &TestServer, view: &str) -> Vec<(u64, i64, i64, i64)> {
    let token = token_for(server, &[]).await;
    let body = json!({
        "view": view,
        "fields": ["a", "score"],
        "position": true,
        "order": "map",
    });
    let mut out = Vec::new();
    for decoded in read_items(server, &token, &body).await {
        for (batch, _) in &decoded.pages {
            let a = arrow::compute::cast(
                batch.column_by_name("a").unwrap(),
                &arrow::datatypes::DataType::UInt64,
            )
            .unwrap();
            let a = a.as_any().downcast_ref::<UInt64Array>().unwrap();
            let score = arrow::compute::cast(
                batch.column_by_name("score").unwrap(),
                &arrow::datatypes::DataType::Int64,
            )
            .unwrap();
            let score = score.as_any().downcast_ref::<Int64Array>().unwrap();
            let x = batch.column_by_name("mosaica:x").unwrap();
            let x = x.as_any().downcast_ref::<Float64Array>().unwrap();
            let y = batch.column_by_name("mosaica:y").unwrap();
            let y = y.as_any().downcast_ref::<Float64Array>().unwrap();
            for i in 0..batch.num_rows() {
                assert!(!a.is_null(i) && !score.is_null(i), "every row carries both");
                out.push((
                    a.value(i),
                    score.value(i),
                    (x.value(i) * 1e3).round() as i64,
                    (y.value(i) * 1e3).round() as i64,
                ));
            }
        }
    }
    out.sort();
    out
}

/// Each artifact of [`LAYER`] a principal holding no term is served in `view`, by key, with its
/// masked count.
async fn artifacts(server: &TestServer, view: &str) -> Vec<(String, u64)> {
    let token = token_for(server, &[]).await;
    let body = json!({
        "view": view, "zoom": 0, "per_tile": 1000, "bbox": [0.0, 0.0, 100.0, 100.0],
        "layers": [LAYER],
    });
    let resp = post_viewport_artifacts(server, &token, &body).await;
    assert_eq!(resp.status().as_u16(), 200);
    let mut out: Vec<(String, u64)> = decode_artifact_frames(&resp.bytes().await.unwrap())
        .artifacts
        .unwrap_or_default()
        .into_iter()
        .map(|artifact| (artifact.key.unwrap_or_default(), artifact.masked_count))
        .collect();
    out.sort();
    out
}

fn ids(meta: &Value, list: &str, key: &str) -> Vec<String> {
    meta[list]
        .as_array()
        .unwrap_or_else(|| panic!("/v1/meta lists {list}: {meta}"))
        .iter()
        .map(|entry| entry[key].as_str().unwrap().to_string())
        .collect()
}

/// A declaration with a view and attributes and no file builds a bundle with no rows; rows sent
/// to the control plane are served, and served again after a restart.
#[tokio::test]
async fn a_declaration_naming_no_file_builds_a_database_that_takes_its_rows_through_control() {
    let tmp = TempDir::new().unwrap();
    build_from(tmp.path(), VIEW_AND_ATTRIBUTES);
    let server = open(tmp.path()).await;
    let meta = meta(&server, &[]).await;
    assert_eq!(ids(&meta, "views", "id"), ["world"]);
    assert!(items(&server, "world").await.is_empty(), "built with no rows");

    ingest_rows(&server).await;
    assert_eq!(items(&server, "world").await, ingested());

    let server = restart(server, tmp.path()).await;
    assert_eq!(items(&server, "world").await, ingested());
    server.shutdown().await;
}

/// An empty declaration builds a bundle with nothing in it. Attributes, a view and a layer are
/// then declared at the control plane, from the payloads `mosaica check --payloads` emits for
/// them, rows and an artifact are sent, and everything is served, and served again after a
/// restart.
#[tokio::test]
async fn an_empty_declaration_builds_a_database_declared_and_filled_through_control() {
    let tmp = TempDir::new().unwrap();
    build_from(tmp.path(), "");
    let server = open(tmp.path()).await;
    assert!(control_status(&server).await.is_object(), "the control plane answers");
    let meta_before = meta(&server, &[]).await;
    assert!(ids(&meta_before, "views", "id").is_empty(), "{meta_before}");
    assert!(ids(&meta_before, "layers", "name").is_empty(), "{meta_before}");

    let declared = TempDir::new().unwrap();
    let path = declared.path().join("corpus.toml");
    std::fs::write(&path, with_layer()).unwrap();
    let config = mosaica_build::config::Config::parse(&path, &Default::default())
        .expect("the declaration parses");
    let payloads = mosaica_build::config::control_payloads(&config);
    for body in payloads["attributes"].as_array().unwrap() {
        let (status, answer) =
            control(&server, reqwest::Method::PUT, "/control/attributes", body).await;
        assert_eq!(status, 201, "{answer}");
    }
    for entry in payloads["views"].as_array().unwrap() {
        let path = format!("/control/views/{}", entry["name"].as_str().unwrap());
        let (status, answer) =
            control(&server, reqwest::Method::PUT, &path, &entry["body"]).await;
        assert_eq!(status, 201, "{answer}");
    }
    for body in payloads["layers"].as_array().unwrap() {
        let (status, answer) =
            control(&server, reqwest::Method::PUT, "/control/layers", body).await;
        assert_eq!(status, 201, "{answer}");
    }
    ingest_rows(&server).await;
    let (status, answer) = control(
        &server,
        reqwest::Method::PUT,
        &format!("/control/layers/{LAYER}/artifacts"),
        &json!({ "artifacts": [{ "key": "c0", "members": { "a": [1, 2] } }] }),
    )
    .await;
    assert!(status < 300, "{status} {answer}");
    tick(&server).await;

    let state = async |server: &TestServer| {
        let meta = meta(server, &[]).await;
        (
            ids(&meta, "views", "id"),
            ids(&meta, "layers", "name"),
            items(server, "world").await,
            artifacts(server, "world").await,
        )
    };
    let served = state(&server).await;
    assert_eq!(
        served,
        (
            vec!["world".to_string()],
            vec![LAYER.to_string()],
            ingested(),
            vec![("c0".to_string(), 2)],
        )
    );

    let server = restart(server, tmp.path()).await;
    assert_eq!(state(&server).await, served);
    server.shutdown().await;
}

/// A group naming no file and no roster, and a group whose file holds no rows, are built with no
/// view and served so. A view created under each at the control plane is served with its group's
/// stated frame, and so are both groups and views after a restart.
#[tokio::test]
async fn a_group_built_with_no_view_takes_its_views_through_control() {
    let tmp = TempDir::new().unwrap();
    write_parquet(
        &tmp.path().join("quarters.parquet"),
        vec![
            column("x", false, Float64Array::from(Vec::<f64>::new())),
            column("y", false, Float64Array::from(Vec::<f64>::new())),
            column("quarter", false, arrow::array::StringArray::from(Vec::<&str>::new())),
        ],
    );
    build_from(
        tmp.path(),
        r#"
[sources]
quarters = "quarters.parquet"

[[view_group]]
name             = "regions"
extent           = { min = 0.0, max = 100.0 }
point_visibility = { default = "public" }

[[view_group]]
name             = "quarters"
source           = "quarters"
fields           = { view = "quarter" }
extent           = { min = 0.0, max = 100.0 }
point_visibility = { default = "public" }
"#,
    );
    let server = open(tmp.path()).await;
    let groups = |meta: &Value| -> Vec<(String, Vec<String>)> {
        meta["groups"]
            .as_array()
            .unwrap_or_else(|| panic!("/v1/meta lists groups: {meta}"))
            .iter()
            .map(|g| {
                let views = g["views"].as_array().unwrap().iter();
                (
                    g["name"].as_str().unwrap().to_string(),
                    views.map(|v| v.as_str().unwrap().to_string()).collect(),
                )
            })
            .collect()
    };
    assert_eq!(
        groups(&meta(&server, &[]).await),
        [("regions".to_string(), vec![]), ("quarters".to_string(), vec![])]
    );

    for (group, key) in [("regions", "north"), ("quarters", "q1")] {
        let path = format!("/control/views/{group}/{key}");
        let (status, answer) = control(&server, reqwest::Method::PUT, &path, &json!({})).await;
        assert_eq!(status, 201, "{answer}");
    }
    let state = async |server: &TestServer| {
        let meta = meta(server, &[]).await;
        let frames: Vec<(String, Value)> = meta["views"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| (v["id"].as_str().unwrap().to_string(), v["quantisation"].clone()))
            .collect();
        (groups(&meta), frames)
    };
    let frame = json!({ "x_min": 0.0, "x_max": 100.0, "y_min": 0.0, "y_max": 100.0 });
    let served = state(&server).await;
    assert_eq!(
        served,
        (
            vec![
                ("regions".to_string(), vec!["regions:north".to_string()]),
                ("quarters".to_string(), vec!["quarters:q1".to_string()]),
            ],
            vec![
                ("regions:north".to_string(), frame.clone()),
                ("quarters:q1".to_string(), frame),
            ],
        )
    );

    let server = restart(server, tmp.path()).await;
    assert_eq!(state(&server).await, served);
    server.shutdown().await;
}
