//! **The `mosaica_id` filter leaf over HTTP**: its two spellings and what is refused, what each
//! viewer matches on the viewport and the bulk read, and how it follows an item through inserts,
//! edits, suppression, deletion and a restart. The engine's answers on every route and over several
//! shards are pinned in `mosaica-engine/tests/it/several_shards_routes.rs` and `records.rs`.

use crate::common;

use common::*;
use serde_json::{json, Value};

/// The items `filters` matches that the principal holding `token` reads from `s0`, sorted.
async fn matched(server: &TestServer, token: &str, filters: Value) -> Vec<u64> {
    let body = json!({ "view": "s0", "fields": [], "filters": filters });
    let mut out: Vec<u64> = read_items(server, token, &body)
        .await
        .iter()
        .flat_map(DecodedRecords::mosaica_ids)
        .collect();
    out.sort_unstable();
    out
}

fn named(ids: &[u64]) -> Value {
    json!({ "mosaica_id": { "in": ids.iter().map(u64::to_string).collect::<Vec<_>>() } })
}

/// How many items the principal holding `token` sees and matches over the whole of `s0`.
async fn viewport_counts(server: &TestServer, token: &str, filters: Value) -> (u64, u64) {
    let resp = server
        .client
        .post(server.viewer_url("/v1/viewport"))
        .bearer_auth(token)
        .json(&json!({
            "view": "s0", "zoom": 0, "bbox": [0.0, 0.0, 1000.0, 1000.0], "k": 0, "filters": filters,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    decode_viewport_frames(&resp.bytes().await.unwrap())
        .tiles
        .iter()
        .fold((0, 0), |(v, m), (_, visible, matched)| (v + visible, m + matched))
}

/// **`eq` and `in` name items by number or decimal string; any other body is `422`.**
#[tokio::test]
async fn the_leaf_takes_eq_and_in_by_number_or_string() {
    let served = Served::build(|dir| build_fixture(dir, 60)).await;
    let (server, token) = (&served.server, &served.token);
    let (a, b) = (mosaica_id_of(server, 3), mosaica_id_of(server, 4));

    assert_eq!(matched(server, token, json!({ "mosaica_id": { "eq": a } })).await, vec![a]);
    let mut both = vec![a, b];
    both.sort_unstable();
    let mixed = json!({ "mosaica_id": { "in": [a, b.to_string()] } });
    assert_eq!(matched(server, token, mixed).await, both);
    assert!(matched(server, token, named(&[])).await.is_empty());

    for refused in [
        json!({ "mosaica_id": { "range": { "gte": 1 } } }),
        json!({ "mosaica_id": { "in": ["one"] } }),
        json!({ "mosaica_id": { "eq": 1, "in": [1] } }),
        json!({ "mosaica_id": [1] }),
    ] {
        let body = json!({ "view": "s0", "fields": [], "filters": refused });
        let resp = post_items(server, token, &body).await;
        assert_eq!(resp.status().as_u16(), 422, "{refused}");
    }
}

/// **Each viewer matches only the named items it may see**, on the viewport and the bulk read
/// alike: an item only the other viewer sees, and an identifier naming nothing, match nothing.
#[tokio::test]
async fn each_viewer_matches_only_the_named_items_it_may_see() {
    let served = Served::build(|dir| build_fixture(dir, 60)).await;
    let server = &served.server;
    // The `1` viewer sees every third item; the `0` viewer sees them all.
    let narrow = token_for(server, &["1"]).await;
    let wide = token_for(server, &["0"]).await;
    let seen = [0, 3, 6].map(|source| mosaica_id_of(server, source));
    let unseen = [1, 2].map(|source| mosaica_id_of(server, source));
    let filter = named(&[&seen[..], &unseen[..], &[u64::MAX]].concat());

    let mut want = seen.to_vec();
    want.sort_unstable();
    assert_eq!(matched(server, &narrow, filter.clone()).await, want);
    assert_eq!(viewport_counts(server, &narrow, filter.clone()).await.1, 3);
    assert_eq!(viewport_counts(server, &wide, filter.clone()).await.1, 5);

    let (visible, _) = viewport_counts(server, &narrow, json!({ "all_of": [] })).await;
    let negated = json!({ "none_of": [filter] });
    assert_eq!(viewport_counts(server, &narrow, negated).await.1, visible - 3);
}

async fn insert(served: &Served, batch_id: &str, rows: &[(Option<u64>, f32, f32, &str)]) -> Vec<u64> {
    let resp = served
        .server
        .client
        .post(served.server.control_url("/control/ingest"))
        .bearer_auth(OPERATOR_CREDENTIAL)
        .header("x-mosaica-batch-id", batch_id)
        .header("x-mosaica-view", "s0")
        .header("content-type", "application/vnd.apache.arrow.stream")
        .body(build_ingest_batch_optional(rows))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let ids = ingested_ids(&resp.json().await.unwrap());
    tick(&served.server).await;
    ids
}

async fn read(served: &Served, ids: Vec<u64>) -> Vec<u64> {
    matched(&served.server, &served.token, named(&ids)).await
}

async fn change(served: &Served, op: &str, id: u64) {
    let body = json!([{ "op": op, "match": { "mosaica_id": id.to_string() } }]);
    let resp = post_changes(&served.server, &body).await;
    assert_eq!(resp.status().as_u16(), 200, "{op}");
    tick(&served.server).await;
}

/// **The leaf follows the item through its life**: an item inserted at a running server matches
/// once its insert is visible, an edit keeps it matching under the same identifier, a suppression
/// hides it until it is lifted, a deletion removes it, and a restart changes nothing.
#[tokio::test]
async fn the_leaf_follows_the_item_through_its_life() {
    let served = Served::build(|dir| build_fixture(dir, 200)).await;
    let built = mosaica_id_of(&served.server, 7);

    let fresh = insert(&served, "fresh", &[(Some(5000), 40.0, 40.0, "0")]).await[0];
    let mut both = vec![built, fresh];
    both.sort_unstable();
    assert_eq!(read(&served, both.clone()).await, both);

    let edited = insert(&served, "edit", &[(Some(5000), 600.0, 600.0, "0")]).await[0];
    assert_eq!(edited, fresh, "an edit keeps the item's identifier");
    assert_eq!(read(&served, vec![fresh]).await, vec![fresh], "the edited item matches once");

    change(&served, "suppress", built).await;
    assert_eq!(read(&served, both.clone()).await, vec![fresh]);
    change(&served, "unsuppress", built).await;
    assert_eq!(read(&served, both.clone()).await, both);

    change(&served, "delete", fresh).await;
    assert_eq!(read(&served, both.clone()).await, vec![built]);

    let served = served.restart().await;
    assert_eq!(read(&served, both).await, vec![built]);
}
