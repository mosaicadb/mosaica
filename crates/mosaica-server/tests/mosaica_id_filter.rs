//! **A `mosaica_id` filter follows the item through its life**: an item inserted at a running
//! server matches once its insert is visible, an edit keeps it matching under the same identifier,
//! a suppression hides it until it is lifted, a deletion removes it, and a restart changes
//! nothing. `items_route.rs` pins the leaf's shape and what each viewer matches.

mod common;

use common::*;
use serde_json::json;

/// The items `ids` name that the served principal reads from `s0`, sorted.
async fn matched(served: &Served, ids: &[u64]) -> Vec<u64> {
    let ids: Vec<String> = ids.iter().map(u64::to_string).collect();
    let body = json!({
        "view": "s0", "fields": [], "filters": { "mosaica_id": { "in": ids } },
    });
    let mut out: Vec<u64> = read_items(&served.server, &served.token, &body)
        .await
        .iter()
        .flat_map(DecodedRecords::mosaica_ids)
        .collect();
    out.sort_unstable();
    out
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

async fn change(served: &Served, op: &str, id: u64) {
    let body = json!([{ "op": op, "match": { "mosaica_id": id.to_string() } }]);
    let resp = post_changes(&served.server, &body).await;
    assert_eq!(resp.status().as_u16(), 200, "{op}");
    tick(&served.server).await;
}

#[tokio::test]
async fn a_mosaica_id_filter_follows_the_item_through_its_life() {
    let served = Served::build(|dir| build_fixture(dir, 200)).await;
    let built = mosaica_id_of(&served.server, 7);

    let fresh = insert(&served, "fresh", &[(Some(5000), 40.0, 40.0, "0")]).await[0];
    assert_eq!(matched(&served, &[built, fresh]).await, sorted(vec![built, fresh]));

    let edited = insert(&served, "edit", &[(Some(5000), 600.0, 600.0, "0")]).await[0];
    assert_eq!(edited, fresh, "an edit keeps the item's identifier");
    assert_eq!(matched(&served, &[fresh]).await, vec![fresh], "the edited item matches once");

    change(&served, "suppress", built).await;
    assert_eq!(matched(&served, &[built, fresh]).await, vec![fresh]);
    change(&served, "unsuppress", built).await;
    assert_eq!(matched(&served, &[built, fresh]).await, sorted(vec![built, fresh]));

    change(&served, "delete", fresh).await;
    assert_eq!(matched(&served, &[built, fresh]).await, vec![built]);

    let served = served.restart().await;
    assert_eq!(matched(&served, &[built, fresh]).await, vec![built]);
}

fn sorted(mut ids: Vec<u64>) -> Vec<u64> {
    ids.sort_unstable();
    ids
}
