//! **A membership names its members in a table**: columns keyed by `mosaica_id` and unique field
//! names, one row per member, resolved by the identity rule. A member naming no item, or two, is
//! left out and listed in the answer's `refused`, or, under `strict=true`, refuses the request
//! with nothing written. A content's generating set is refused whole at any such member, strict or
//! not. The Arrow growth carries the table as `list<struct>`, naming a unique timestamp field as
//! `timestamp[us]`, as a build's members file does.

mod common;

use std::sync::Arc;

use arrow::array::{
    Array, ArrayRef, Int64Array, ListArray, RecordBatch, StringArray, StructArray,
    TimestampMicrosecondArray, TimestampMillisecondArray, UInt64Array,
};
use arrow::buffer::OffsetBuffer;
use arrow::datatypes::{DataType, Field, Fields, Schema};
use arrow::ipc::writer::StreamWriter;
use common::*;
use serde_json::{json, Value};
use tempfile::TempDir;

const LAYER: &str = "clusters/tables";

fn artifacts_url(server: &TestServer, layer: &str, strict: bool) -> String {
    let query = if strict { "?strict=true" } else { "" };
    server.control_url(&format!(
        "/control/layers/{}/artifacts{query}",
        layer.replace('/', "%2F")
    ))
}

async fn put(server: &TestServer, strict: bool, artifacts: Value) -> (u16, Value) {
    let resp = server
        .client
        .put(artifacts_url(server, LAYER, strict))
        .bearer_auth(OPERATOR_CREDENTIAL)
        .json(&json!({ "artifacts": artifacts }))
        .send()
        .await
        .unwrap();
    let status = resp.status().as_u16();
    let body = resp.json().await.unwrap_or(Value::Null);
    if status < 300 {
        tick(server).await;
    }
    (status, body)
}

async fn patch_arrow(server: &TestServer, layer: &str, body: Vec<u8>) -> (u16, Value) {
    let resp = server
        .client
        .patch(artifacts_url(server, layer, false))
        .bearer_auth(OPERATOR_CREDENTIAL)
        .header("content-type", "application/vnd.apache.arrow.stream")
        .body(body)
        .send()
        .await
        .unwrap();
    let status = resp.status().as_u16();
    let body = resp.json().await.unwrap_or(Value::Null);
    if status < 300 {
        tick(server).await;
    }
    (status, body)
}

/// The member counts the broad principal is shown for the layer's artifacts, ascending, having
/// checked that `served` artifacts are served.
async fn count(server: &TestServer, served: usize) -> Vec<u64> {
    let token = token_for(server, &["0"]).await;
    let resp = server
        .client
        .post(server.viewer_url("/v1/artifacts/viewport"))
        .bearer_auth(&token)
        .json(&json!({
            "view": "s0", "zoom": 0, "per_tile": 1000, "bbox": [0.0, 0.0, 1000.0, 1000.0], "layers": [LAYER],
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let rows = decode_artifact_frames(&resp.bytes().await.unwrap())
        .artifacts
        .unwrap_or_default();
    assert_eq!(rows.len(), served, "{rows:?}");
    let mut counts: Vec<u64> = rows.iter().map(|row| row.masked_count).collect();
    counts.sort_unstable();
    counts
}

/// Members named by `mosaica_id`, by a unique field, and by both agreeing, in one table.
#[tokio::test]
async fn a_table_names_members_by_any_identifier_they_hold() {
    let tmp = TempDir::new().unwrap();
    let server = serve(&tmp).await;
    register(&server, flat_layer(LAYER)).await;
    let (zero, two) = (mosaica_id_of(&server, 0), mosaica_id_of(&server, 2));

    let (status, body) = put(
        &server,
        false,
        json!([{
            "key": "a",
            "members": {
                "mosaica_id": [zero.to_string(), null, two.to_string()],
                "id": [null, "1", 2],
            },
        }]),
    )
    .await;
    assert_eq!(status, 201, "{body}");
    assert_eq!(body["joined"], 3, "{body}");
    assert_eq!(body["refused"], json!([]));
    assert_eq!(count(&server, 1).await, vec![3]);
}

/// A member naming no item, or two, is left out and listed; under `strict=true` the request is
/// `404` or `409` and nothing is published.
#[tokio::test]
async fn a_member_naming_no_item_or_two_is_left_out_or_refuses_a_strict_request() {
    let tmp = TempDir::new().unwrap();
    let server = serve(&tmp).await;
    register(&server, flat_layer(LAYER)).await;
    let three = mosaica_id_of(&server, 3);

    let nothing = json!([{ "key": "a", "members": { "id": [0, 999_999] } }]);
    let two = json!([{
        "key": "a",
        "members": { "mosaica_id": [null, three.to_string()], "id": [0, 4] },
    }]);
    let (status, body) = put(&server, true, nothing).await;
    assert_eq!(status, 404, "{body}");
    let (status, body) = put(&server, true, two).await;
    assert_eq!(status, 409, "{body}");
    count(&server, 0).await;

    let (status, body) = put(
        &server,
        false,
        json!([
            { "key": "a", "members": { "id": [0, 999_999, 5, null] } },
            {
                "key": "b",
                "members": { "mosaica_id": [three.to_string(), three.to_string()], "id": [4, 3] },
            },
        ]),
    )
    .await;
    assert_eq!(status, 201, "{body}");
    assert_eq!(
        body["refused"],
        json!([
            { "artifact": 0, "list": "members", "row": 1, "reason": "names_no_item" },
            { "artifact": 0, "list": "members", "row": 3, "reason": "names_no_item" },
            { "artifact": 1, "list": "members", "row": 0, "reason": "names_two_items" },
        ])
    );
    assert_eq!(count(&server, 2).await, vec![1, 2]);
}

/// A member deleted after the request arrived and before it applies is refused alone where it
/// applies: the answer lists its row, the other members are published, and a strict request is
/// `404` with nothing published.
#[tokio::test]
async fn a_member_deleted_before_the_publication_applies_is_refused_alone() {
    let tmp = TempDir::new().unwrap();
    let server = serve(&tmp).await;
    register(&server, flat_layer(LAYER)).await;
    let engine = &server.state.engine;
    let server = &server;
    let held = |strict: bool, key: &'static str, gone: u64| async move {
        let publishing = put(
            server,
            strict,
            json!([{ "key": key, "members": { "id": [0, gone, 2] } }]),
        );
        let deleting = async {
            let id = mosaica_id_of(server, gone).to_string();
            let deleted = post_changes(
                server,
                &json!([{ "op": "delete", "match": { "mosaica_id": id } }]),
            )
            .await;
            assert_eq!(deleted.status().as_u16(), 200);
        };
        while_write_held(engine, publishing, deleting).await
    };

    let (status, body) = held(true, "s", 3).await;
    assert_eq!(status, 404, "{body}");
    count(server, 0).await;

    let (status, body) = held(false, "a", 1).await;
    assert_eq!(status, 201, "{body}");
    assert_eq!(body["joined"], 2, "{body}");
    assert_eq!(
        body["refused"],
        json!([{ "artifact": 0, "list": "members", "row": 1, "reason": "names_no_item" }])
    );
    assert_eq!(count(server, 1).await, vec![2]);
}

/// A content's generating set is refused whole at a member naming nothing, strict or not: a
/// viewer must see the whole set to be served the content.
#[tokio::test]
async fn a_generating_set_member_naming_nothing_refuses_the_request() {
    let tmp = TempDir::new().unwrap();
    let server = serve(&tmp).await;
    let mut layer = flat_layer(LAYER);
    layer["content"]["supplied"] =
        json!([{ "name": "topic", "type": "text", "require_member_visibility": "all" }]);
    register(&server, layer).await;

    let (status, body) = put(
        &server,
        false,
        json!([{
            "key": "a",
            "members": { "id": [0, 1] },
            "content": [{ "values": ["t"], "generated_from": { "id": [0, 999_999] } }],
        }]),
    )
    .await;
    assert_eq!(status, 404, "{body}");
    count(&server, 0).await;
}

/// A column that is neither `mosaica_id` nor a unique field names nothing: it is ignored whatever
/// its cells hold, as a build ignores it, and the answer names it.
#[tokio::test]
async fn a_column_that_names_nothing_is_ignored_and_named() {
    let tmp = TempDir::new().unwrap();
    let server = serve(&tmp).await;
    register(&server, flat_layer(LAYER)).await;
    let (status, body) = put(
        &server,
        false,
        json!([{
            "key": "a",
            "members": {
                "id": [0, 1],
                "name": ["x", "y"],
                "tags": [[1, "b"], { "c": 2 }],
                "weight": [1.5, true],
            },
        }]),
    )
    .await;
    assert_eq!(status, 201, "{body}");
    assert_eq!(body["ignored_columns"], json!(["name", "tags", "weight"]));
    assert_eq!(body["refused"], json!([]));
    assert_eq!(count(&server, 1).await, vec![2]);
}

/// Members joining a generating set through a growth are refused whole in the same way, strict or
/// not; a member leaving it that names nothing is listed and the rest apply.
#[tokio::test]
async fn a_generating_set_grown_by_a_member_naming_nothing_or_two_refuses_the_request() {
    let tmp = TempDir::new().unwrap();
    let server = serve(&tmp).await;
    let mut layer = flat_layer(LAYER);
    layer["content"]["supplied"] =
        json!([{ "name": "topic", "type": "text", "require_member_visibility": "all" }]);
    register(&server, layer).await;
    let (status, body) = put(
        &server,
        false,
        json!([{
            "key": "a",
            "members": { "id": [0, 1, 2] },
            "content": [{ "values": ["t"], "generated_from": { "id": [0, 1] } }],
        }]),
    )
    .await;
    assert_eq!(status, 201, "{body}");

    let patch = |body: Value| {
        server
            .client
            .patch(artifacts_url(&server, LAYER, false))
            .bearer_auth(OPERATOR_CREDENTIAL)
            .json(&body)
            .send()
    };
    let three = mosaica_id_of(&server, 3);
    for (members, status) in [
        (json!({ "id": [2, 999_999] }), 404),
        (
            json!({ "id": [2, 4], "mosaica_id": [null, three.to_string()] }),
            409,
        ),
    ] {
        let resp = patch(json!({ "artifacts": [{ "key": "a", "rank": 0, "members": members }] }))
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), status);
    }
    let resp = patch(json!({ "artifacts": [
        { "key": "a", "rank": 0, "leaving": { "id": [1, 999_999] } }
    ] }))
    .await
    .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["artifacts"][0]["left"], 1, "{body}");
    assert_eq!(
        body["refused"],
        json!([{ "artifact": 0, "list": "leaving", "row": 1, "reason": "names_no_item" }])
    );
}

/// The shape of a table is checked before anything is resolved.
#[tokio::test]
async fn a_malformed_table_is_refused_whole() {
    let tmp = TempDir::new().unwrap();
    let server = serve(&tmp).await;
    register(&server, flat_layer(LAYER)).await;
    for (what, artifacts) in [
        (
            "columns of different lengths",
            json!([{ "key": "a", "members": { "id": [0, 1], "mosaica_id": [null] } }]),
        ),
        (
            "no column that names items",
            json!([{ "key": "a", "members": { "name": ["x"] } }]),
        ),
        (
            "a list in place of a table",
            json!([{ "key": "a", "members": ["0"] }]),
        ),
        (
            "a mosaica_id that is a number",
            json!([{ "key": "a", "members": { "mosaica_id": [12] } }]),
        ),
        (
            "a fraction for an integer field",
            json!([{ "key": "a", "members": { "id": [0, 1.5] } }]),
        ),
    ] {
        let (status, body) = put(&server, false, artifacts).await;
        assert_eq!(status, 422, "{what}: {body}");
    }
    let resp = server
        .client
        .put(artifacts_url(&server, LAYER, false))
        .bearer_auth(OPERATOR_CREDENTIAL)
        .json(&json!({ "field": "id", "artifacts": [{ "key": "a", "members": { "id": [0] } }] }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 422, "the batch-level field is gone");
    count(&server, 0).await;
}

/// An Arrow growth names its members as `list<struct>`, the struct's fields the table's columns,
/// and lists a member naming nothing as the JSON form does.
#[tokio::test]
async fn an_arrow_growth_names_members_as_a_list_of_structs() {
    let tmp = TempDir::new().unwrap();
    let server = serve(&tmp).await;
    register(&server, flat_layer(LAYER)).await;
    let (status, body) = put(
        &server,
        false,
        json!([{ "key": "a", "members": { "id": [0] } }]),
    )
    .await;
    assert_eq!(status, 201, "{body}");

    let one = mosaica_id_of(&server, 1);
    let fields = Fields::from(vec![
        Field::new("mosaica_id", DataType::Utf8, true),
        Field::new("id", DataType::UInt64, true),
    ]);
    let elements = StructArray::new(
        fields.clone(),
        vec![
            Arc::new(StringArray::from(vec![Some(one.to_string()), None, None])) as ArrayRef,
            Arc::new(UInt64Array::from(vec![None, Some(2), Some(999_999)])) as ArrayRef,
        ],
        None,
    );
    let item = Arc::new(Field::new("item", DataType::Struct(fields), false));
    let members = ListArray::new(
        item.clone(),
        OffsetBuffer::from_lengths([3]),
        Arc::new(elements),
        None,
    );
    let schema = Arc::new(Schema::new(vec![
        Field::new("key", DataType::Utf8, false),
        Field::new("members", DataType::List(item), false),
    ]));
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![Arc::new(StringArray::from(vec!["a"])), Arc::new(members)],
    )
    .unwrap();
    let mut writer = StreamWriter::try_new(Vec::new(), &schema).unwrap();
    writer.write(&batch).unwrap();

    let (status, body) = patch_arrow(&server, LAYER, writer.into_inner().unwrap()).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["artifacts"][0]["joined"], 2, "{body}");
    assert_eq!(
        body["refused"],
        json!([{ "artifact": 0, "list": "members", "row": 2, "reason": "names_no_item" }])
    );
    assert_eq!(count(&server, 1).await, vec![3]);
}

/// An Arrow member struct's columns are checked only where they name items: a unique field of
/// another type than its values, and a column named twice, are refused; a column naming nothing is
/// ignored whatever its type; and a `mosaica_id` column may be uint64, as a read sends it.
#[tokio::test]
async fn an_arrow_member_column_is_checked_only_where_it_names_items() {
    let tmp = TempDir::new().unwrap();
    let server = serve(&tmp).await;
    register(&server, flat_layer(LAYER)).await;
    let (status, body) = put(
        &server,
        false,
        json!([{ "key": "a", "members": { "id": [0] } }]),
    )
    .await;
    assert_eq!(status, 201, "{body}");

    let body_of = |fields: Vec<Field>, columns: Vec<ArrayRef>| {
        let rows = columns[0].len();
        let fields = Fields::from(fields);
        let elements = StructArray::new(fields.clone(), columns, None);
        let item = Arc::new(Field::new("item", DataType::Struct(fields), false));
        let members = ListArray::new(
            item.clone(),
            OffsetBuffer::from_lengths([rows]),
            Arc::new(elements),
            None,
        );
        let schema = Arc::new(Schema::new(vec![
            Field::new("key", DataType::Utf8, false),
            Field::new("members", DataType::List(item), false),
        ]));
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![Arc::new(StringArray::from(vec!["a"])), Arc::new(members)],
        )
        .unwrap();
        let mut writer = StreamWriter::try_new(Vec::new(), &schema).unwrap();
        writer.write(&batch).unwrap();
        writer.into_inner().unwrap()
    };
    let float = body_of(
        vec![Field::new("id", DataType::Float64, true)],
        vec![Arc::new(arrow::array::Float64Array::from(vec![1.0])) as ArrayRef],
    );
    let (status, body) = patch_arrow(&server, LAYER, float).await;
    assert_eq!(status, 422, "{body}");
    let twice = body_of(
        vec![
            Field::new("id", DataType::UInt64, true),
            Field::new("id", DataType::UInt64, true),
        ],
        vec![
            Arc::new(UInt64Array::from(vec![1])) as ArrayRef,
            Arc::new(UInt64Array::from(vec![2])) as ArrayRef,
        ],
    );
    let (status, body) = patch_arrow(&server, LAYER, twice).await;
    assert_eq!(status, 422, "{body}");
    assert_eq!(count(&server, 1).await, vec![1]);

    let ignored = body_of(
        vec![
            Field::new("id", DataType::UInt64, true),
            Field::new("weight", DataType::Float64, true),
            Field::new("seen", DataType::Boolean, true),
        ],
        vec![
            Arc::new(UInt64Array::from(vec![1])) as ArrayRef,
            Arc::new(arrow::array::Float64Array::from(vec![0.5])) as ArrayRef,
            Arc::new(arrow::array::BooleanArray::from(vec![true])) as ArrayRef,
        ],
    );
    let (status, body) = patch_arrow(&server, LAYER, ignored).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["ignored_columns"], json!(["weight", "seen"]));
    assert_eq!(count(&server, 1).await, vec![2]);

    let read_back = body_of(
        vec![Field::new("mosaica_id", DataType::UInt64, true)],
        vec![Arc::new(UInt64Array::from(vec![mosaica_id_of(&server, 2)])) as ArrayRef],
    );
    let (status, body) = patch_arrow(&server, LAYER, read_back).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["refused"], json!([]));
    assert_eq!(count(&server, 1).await, vec![3]);
}

/// Items placed by [`scatter`], each holding a unique `seen`, a timestamp an hour after the last.
const STAMPED_ITEMS: u64 = 40;
const STAMPED_LAYER: &str = "clusters/stamped";
/// Three artifacts, by key, and the items each holds.
const STAMPED: [(&str, &[u64]); 3] = [("one", &[0]), ("two", &[1, 2]), ("three", &[3, 4, 5])];

/// Item `e`'s `seen`, in microseconds since the epoch: 2020-01-01T00:00:00Z plus `e` hours.
fn seen_of(e: u64) -> i64 {
    1_577_836_800_000_000 + e as i64 * 3_600_000_000
}

fn microseconds(values: Vec<i64>) -> ArrayRef {
    Arc::new(TimestampMicrosecondArray::from(values).with_timezone("UTC"))
}

fn milliseconds(values: Vec<i64>) -> ArrayRef {
    let values = values.into_iter().map(|us| us / 1_000).collect::<Vec<_>>();
    Arc::new(TimestampMillisecondArray::from(values).with_timezone("UTC"))
}

/// One list per artifact of [`STAMPED`], of structs naming each member by its `seen`, as `seen`
/// writes the values.
fn stamped_lists(seen: fn(Vec<i64>) -> ArrayRef) -> ListArray {
    let values = STAMPED
        .iter()
        .flat_map(|(_, items)| items.iter().map(|&e| seen_of(e)))
        .collect();
    let column = seen(values);
    let fields = Fields::from(vec![Field::new("seen", column.data_type().clone(), true)]);
    let elements = StructArray::new(fields.clone(), vec![column], None);
    let item = Arc::new(Field::new("item", DataType::Struct(fields), true));
    ListArray::new(
        item,
        OffsetBuffer::from_lengths(STAMPED.iter().map(|(_, items)| items.len())),
        Arc::new(elements),
        None,
    )
}

fn stamped_keys() -> StringArray {
    StringArray::from_iter_values(STAMPED.iter().map(|(key, _)| *key))
}

/// A bundle over items holding a unique timestamp `seen`, built into `dir/bundle`. With `members`,
/// the build publishes [`STAMPED`] into [`STAMPED_LAYER`] from a members file naming each member
/// by its `seen`, as `members` writes the values.
fn build_stamped(
    dir: &std::path::Path,
    members: Option<fn(Vec<i64>) -> ArrayRef>,
) -> Result<mosaica_build::BuildReport, mosaica_build::BuildError> {
    let points = dir.join("points.parquet");
    let pairs = dir.join("pairs.parquet");
    let ids: Vec<u64> = (0..STAMPED_ITEMS).collect();
    let seen = microseconds(ids.iter().map(|&e| seen_of(e)).collect());
    write_points(
        &points,
        &ids,
        scatter,
        vec![(Field::new("seen", seen.data_type().clone(), false), seen)],
    );
    write_pairs_n(&pairs, STAMPED_ITEMS);
    let layer = match members {
        Some(members) => {
            write_parquet(
                &dir.join("members.parquet"),
                vec![
                    column("key", false, stamped_keys()),
                    column("members", false, stamped_lists(members)),
                ],
            );
            format!(
                r#"
[sources]
points  = "points.parquet"
members = "members.parquet"

[defaults]
source = "points"

[[view]]
name             = "s0"
extent           = {{ min = 0.0, max = 1000.0 }}
point_visibility = {{ default = "public" }}

[[layer]]
name                      = "{STAMPED_LAYER}"
views                     = ["s0"]
membership                = "enumerated"
source                    = "members"
hierarchy                 = {{ kind = "flat", prune_children = false }}
visibility                = "public"
artifact_visibility       = {{ default = "inherited" }}
require_member_visibility = "none"
"#
            )
        }
        None => String::new(),
    };
    let declaration = dir.join("corpus.toml");
    std::fs::write(
        &declaration,
        format!(
            "{ID_ATTRIBUTE}\n[[attribute]]\nname = \"seen\"\ntype = \"timestamp_us\"\nunique = true\n\
             {layer}"
        ),
    )
    .unwrap();
    let config = mosaica_build::config::Config::parse(&declaration, &Default::default())
        .expect("the declaration parses");
    mosaica_build::build(&mosaica_build::BuildArgs {
        attribute_sources: mosaica_build::config::AttributeSource::over(&points, &config.schema),
        schema: config.schema,
        layers: config.layers.clone(),
        layer_inputs: config.layer_sources.clone(),
        ..build_args(
            &dir.join("bundle"),
            vec![view_args("s0", &points, AccessInput::relation(&pairs))],
        )
    })
}

/// A server over a bundle of [`build_stamped`]'s items, with [`STAMPED`]'s artifacts published in
/// [`STAMPED_LAYER`] and holding no member.
async fn serve_stamped(dir: &std::path::Path) -> TestServer {
    build_stamped(dir, None).expect("the stamped fixture builds");
    let server = open(dir).await;
    let mut layer = flat_layer(STAMPED_LAYER);
    layer["visibility"] = json!("public");
    register(&server, layer).await;
    let (status, body) = publish(
        &server,
        STAMPED_LAYER,
        "",
        json!({ "artifacts": STAMPED
            .iter()
            .map(|(key, _)| json!({ "key": key, "members": { "seen": [] } }))
            .collect::<Vec<_>>() }),
    )
    .await;
    assert_eq!(status, 201, "{body}");
    tick(&server).await;
    server
}

/// A growth of [`STAMPED`]'s members, each named by its `seen` as `seen` writes the values.
fn stamped_growth(seen: fn(Vec<i64>) -> ArrayRef) -> Vec<u8> {
    let members = stamped_lists(seen);
    let schema = Arc::new(Schema::new(vec![
        Field::new("key", DataType::Utf8, false),
        Field::new("members", members.data_type().clone(), false),
    ]));
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![Arc::new(stamped_keys()), Arc::new(members)],
    )
    .unwrap();
    let mut writer = StreamWriter::try_new(Vec::new(), &schema).unwrap();
    writer.write(&batch).unwrap();
    writer.into_inner().unwrap()
}

/// Each artifact of [`STAMPED_LAYER`] the broad principal is served, by key: its member count and
/// the `seen` of each member, in microseconds, ascending.
async fn stamped_state(server: &TestServer) -> std::collections::BTreeMap<String, (u64, Vec<i64>)> {
    let token = token_for(server, &["0"]).await;
    let resp = server
        .client
        .post(server.viewer_url("/v1/artifacts/viewport"))
        .bearer_auth(&token)
        .json(&json!({
            "view": "s0", "zoom": 0, "per_tile": 1000, "bbox": [0.0, 0.0, 1000.0, 1000.0],
            "layers": [STAMPED_LAYER],
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let mut out = std::collections::BTreeMap::new();
    for artifact in decode_artifact_frames(&resp.bytes().await.unwrap())
        .artifacts
        .unwrap_or_default()
    {
        let resp = server
            .client
            .post(server.viewer_url("/v1/items"))
            .bearer_auth(&token)
            .json(&json!({
                "view": "s0",
                "fields": ["seen"],
                "filters": { "member_of": {
                    "layer": STAMPED_LAYER, "artifact": artifact.mosaica_id.to_string()
                } },
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status().as_u16(), 200);
        let decoded = decode_records(&resp.bytes().await.unwrap());
        assert!(
            decoded.trailer["next"].is_null(),
            "one page holds every member"
        );
        let mut seen = Vec::new();
        for (batch, _) in &decoded.pages {
            let column =
                arrow::compute::cast(batch.column_by_name("seen").unwrap(), &DataType::Int64)
                    .unwrap();
            let column = column.as_any().downcast_ref::<Int64Array>().unwrap();
            seen.extend(column.iter().flatten());
        }
        seen.sort_unstable();
        out.insert(
            artifact.key.clone().unwrap_or_default(),
            (artifact.masked_count, seen),
        );
    }
    out
}

/// **An Arrow growth names members by a unique timestamp field as `timestamp[us]`**, as a build's
/// members file does: the members a service joins this way are those the build joins from the
/// same column, and a restart keeps them.
#[tokio::test]
async fn an_arrow_growth_names_members_by_a_unique_timestamp_as_a_build_does() {
    let expected: std::collections::BTreeMap<String, (u64, Vec<i64>)> = STAMPED
        .iter()
        .map(|(key, items)| {
            let seen = items.iter().map(|&e| seen_of(e)).collect::<Vec<_>>();
            (key.to_string(), (items.len() as u64, seen))
        })
        .collect();
    let built = TempDir::new().unwrap();
    build_stamped(built.path(), Some(microseconds)).expect("the stamped layer builds");
    let built = open(&built).await;
    assert_eq!(
        stamped_state(&built).await,
        expected,
        "the build joins every member"
    );

    let tmp = TempDir::new().unwrap();
    let server = serve_stamped(tmp.path()).await;
    let (status, body) = patch_arrow(&server, STAMPED_LAYER, stamped_growth(microseconds)).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["refused"], json!([]), "{body}");
    assert_eq!(stamped_state(&server).await, expected);

    let server = restart(server, &tmp).await;
    assert_eq!(
        stamped_state(&server).await,
        expected,
        "a restart keeps the members"
    );
}

/// A timestamp in another unit than microseconds names no member, at a service or a build, and
/// the service's refusal says to cast it to `timestamp[us]`.
#[tokio::test]
async fn an_arrow_member_timestamp_in_another_unit_is_refused_with_the_cast() {
    let tmp = TempDir::new().unwrap();
    let server = serve_stamped(tmp.path()).await;
    let (status, body) = patch_arrow(&server, STAMPED_LAYER, stamped_growth(milliseconds)).await;
    assert_eq!(status, 422, "{body}");
    let detail = body["detail"].as_str().unwrap_or_default();
    assert!(detail.contains("timestamp[us]"), "{detail}");

    let built = TempDir::new().unwrap();
    assert!(
        build_stamped(built.path(), Some(milliseconds)).is_err(),
        "the build refuses the same column"
    );
}
