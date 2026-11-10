//! **What a client is told it is looking at** (`projections.md` §9): `/v1/meta`'s `projection`,
//! `world_aspect`, `tile_scheme` and `tile`, per view.
//!
//! The four are deployment constants derived from the declaration — the same class as the frame
//! beside them, identical for every principal — so what is at stake here is not disclosure but
//! whether a host draws the right thing. One of them decides that on its own.
//!
//! **`tile_scheme` is the field a boolean would get wrong**, and
//! [`an_equirectangular_view_publishes_no_scheme_though_its_frame_is_aligned`] is the case that
//! shows it: an equirectangular frame is a square of a square tiling exactly as a Web Mercator
//! frame is, and no tile server serves that tiling — the published longitude/latitude schemes are
//! 2:1 at their top level. A host reading alignment as availability would draw a Mercator basemap
//! under a corpus that cannot line up with one, which is a wrong map rather than a missing one.

mod common;

use std::path::Path;

use common::*;
use serde_json::Value;
use tempfile::TempDir;
use mosaica_build::build;
use mosaica_spatial::frame::AlignedSquare;
use mosaica_spatial::{Bounds, Projection};

/// A bundle built under `projection` against `frame`.
///
/// `frame` is what a projected build records — the aligned square a declared box snapped to, over
/// the unit square the transform produces (`projections.md` §4.2) — and is passed outright here so
/// that a test can state the square it means and then compute the tile address it expects by hand.
fn build_projected(out: &Path, tmp: &Path, projection: Projection, frame: Bounds) {
    let points = tmp.join(format!(
        "points-{}.parquet",
        out.display().to_string().len()
    ));
    let pairs = tmp.join(format!("pairs-{}.parquet", out.display().to_string().len()));
    // A coarse graticule over the whole world, so every frame below holds some of it and no build
    // refuses for want of data.
    let places: Vec<(f64, f64)> = (0..N_ITEMS)
        .map(|e| (-180.0 + (e % 360) as f64, -80.0 + (e % 160) as f64))
        .collect();
    write_lon_lat(&points, &places);
    write_pairs_n(&pairs, N_ITEMS);
    let args = build_args(
        out,
        vec![mosaica_build::ViewArgs {
            projection,
            extent: frame,
            // `lon`/`lat` become the canonical `x`/`y` at the declaration, which is what
            // `compile_projected_fields` does for a `[[view]]` block; built outright here because
            // there is no document around this build.
            point_fields: mosaica_build::config::Fields::moved(
                "points",
                [("x", "lon"), ("y", "lat")],
            ),
            ..view_args("s0", &points, AccessInput::relation(&pairs))
        }],
    );
    build(&with_id(args, &points)).expect("a projected fixture build should succeed");
}

/// `/v1/meta`'s single view, fetched over real HTTP with a real session.
async fn meta_view(bundle_root: &Path, tmp: &Path, tag: &str) -> Value {
    let server = spawn_server(
        bundle_root,
        &tmp.join(format!("cache-{tag}")),
        &tmp.join(format!("wal-{tag}.log")),
    )
    .await;
    let auth = authorise(&server, &["0"]).await;
    let token = auth["token"].as_str().unwrap();
    let resp = server
        .client
        .get(server.viewer_url("/v1/meta"))
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    let views = body["views"].as_array().unwrap();
    assert_eq!(views.len(), 1, "the fixture builds one view: {body}");
    views[0].clone()
}

/// The whole world under every entry of the enumerated set, and the alias that is distinguishable
/// only by the aspect it publishes.
///
/// The expectations are the design's own table (`projections.md` §5.2, §9), not the code's: a
/// square world for `web_mercator`, 2:1 for `equirectangular` and `plate_carree` — which are one
/// name on the wire — and √2:1 for `gall_isographic`, whose standard parallel is 45°.
#[tokio::test]
async fn each_projection_publishes_its_own_name_and_world_aspect() {
    let root2 = 2.0_f64.sqrt();
    for (projection, name, aspect, scheme) in [
        (Projection::WebMercator, "web_mercator", 1.0, Some("xyz")),
        (Projection::PLATE_CARREE, "equirectangular", 2.0, None),
        (Projection::GALL_ISOGRAPHIC, "gall_isographic", root2, None),
    ] {
        let tmp = TempDir::new().unwrap();
        let bundle_root = tmp.path().join("bundle");
        build_projected(
            &bundle_root,
            tmp.path(),
            projection,
            AlignedSquare::WORLD.bounds(),
        );
        let view = meta_view(&bundle_root, tmp.path(), name).await;

        assert_eq!(view["projection"], name, "{view}");
        let published = view["world_aspect"].as_f64().unwrap();
        assert!(
            (published - aspect).abs() < 1e-12,
            "{name} publishes world_aspect {published}, expected {aspect}"
        );
        assert_eq!(
            view["tile_scheme"],
            match scheme {
                Some(s) => Value::from(s),
                None => Value::Null,
            },
            "{name}: {view}"
        );
        // The world under a scheme is that scheme's own top tile.
        assert_eq!(
            view["tile"],
            match scheme {
                Some(_) => serde_json::json!({"z": 0, "x": 0, "y": 0}),
                None => Value::Null,
            },
            "{name}: {view}"
        );
    }
}

/// **The case a boolean gets wrong.** An equirectangular frame is aligned — it is the same
/// aligned square a Web Mercator view would publish `xyz` and a tile address for — and it
/// addresses no published scheme, so it publishes none and no tile.
///
/// The frame is the z2 tile (1, 1): `x [0.25, 0.5], y [0.25, 0.5]`, which under
/// `x = (λ+180)/360, y = 0.5 − φ/180` is longitude −90°…0° and latitude 0°…45°. Alignment is
/// asserted directly rather than assumed, because without it this test would pass for the wrong
/// reason — a frame that was not a square would publish nothing under any projection.
#[tokio::test]
async fn an_equirectangular_view_publishes_no_scheme_though_its_frame_is_aligned() {
    let frame = AlignedSquare { z: 2, x: 1, y: 1 }.bounds();
    assert_eq!(
        AlignedSquare::of_bounds(&frame),
        Some(AlignedSquare { z: 2, x: 1, y: 1 }),
        "the premise: this frame is a square of the tiling"
    );

    let tmp = TempDir::new().unwrap();
    let bundle_root = tmp.path().join("bundle");
    build_projected(&bundle_root, tmp.path(), Projection::PLATE_CARREE, frame);
    let view = meta_view(&bundle_root, tmp.path(), "equirect-sub").await;

    assert_eq!(view["projection"], "equirectangular");
    assert_eq!(view["world_aspect"], 2.0);
    assert_eq!(
        view["tile_scheme"],
        Value::Null,
        "an aligned equirectangular frame addresses no published scheme — the published \
         longitude/latitude schemes are 2:1 at their top level, so a host that read alignment as \
         availability would draw a Mercator basemap under a corpus that cannot line up: {view}"
    );
    assert_eq!(view["tile"], Value::Null, "{view}");
}

/// A Web Mercator sub-square frame publishes the tile it is.
///
/// **Computed by hand, not recorded.** At zoom 3 the world is 8 tiles per axis, so a tile is
/// `1/8 = 0.125` of the unit square and the tile (5, 2) is `x [0.625, 0.75], y [0.25, 0.375]` —
/// east of the prime meridian (which is `x = 0.5`) and north of the equator (`y = 0.5`), with
/// **y counting south from the north edge**, the XYZ convention. That is the address `/v1/meta`
/// must publish, and the one a `TileLayer` asks a tile server for.
#[tokio::test]
async fn a_web_mercator_sub_square_publishes_the_tile_it_is() {
    let frame = Bounds {
        x_min: 0.625,
        x_max: 0.75,
        y_min: 0.25,
        y_max: 0.375,
    };
    let tmp = TempDir::new().unwrap();
    let bundle_root = tmp.path().join("bundle");
    build_projected(&bundle_root, tmp.path(), Projection::WebMercator, frame);
    let view = meta_view(&bundle_root, tmp.path(), "mercator-sub").await;

    assert_eq!(view["projection"], "web_mercator");
    assert_eq!(view["world_aspect"], 1.0);
    assert_eq!(view["tile_scheme"], "xyz");
    assert_eq!(
        view["tile"],
        serde_json::json!({"z": 3, "x": 5, "y": 2}),
        "{view}"
    );
}

/// **A view that projects nothing is told so, and the client path is unchanged.** `none` is the
/// default and every corpus built before this document existed is one: the name is published, the
/// scheme is null and there is no tile, and a client must treat that exactly as it treats today's
/// corpora — which is what the frame's presence, and the absence of any other change to the
/// document, says here.
#[tokio::test]
async fn a_none_view_publishes_none_and_no_scheme() {
    let tmp = TempDir::new().unwrap();
    let bundle_root = build_fixture(tmp.path(), N_ITEMS);
    let view = meta_view(&bundle_root, tmp.path(), "none").await;

    assert_eq!(view["projection"], "none");
    assert_eq!(
        view["world_aspect"],
        Value::Null,
        "`none` has no world to draw: {view}"
    );
    assert_eq!(view["tile_scheme"], Value::Null, "{view}");
    assert_eq!(view["tile"], Value::Null, "{view}");
    assert_eq!(view["id"], "s0");
    assert_eq!(view["display_name"], "s0");
}

/// `POST /v1/viewport` with `body` on the whole-world Web Mercator fixture: the status and, on a
/// 200, the decoded tiles and points.
async fn world_viewport(server: &TestServer, token: &str, body: Value) -> (u16, Vec<TileRow>, Vec<PointRow>) {
    let resp = server
        .client
        .post(server.viewer_url("/v1/viewport"))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .unwrap();
    let status = resp.status().as_u16();
    let bytes = resp.bytes().await.unwrap();
    if status != 200 {
        return (status, Vec::new(), Vec::new());
    }
    let (tiles, points) = decode_viewport(&bytes);
    (status, tiles, points)
}

/// `degrees` in the unit square `projection` draws the world in, from the projection's published
/// definition and not from the server's own transform. y runs south, so the northern edge is the
/// smaller y.
fn unit_square(degrees: [f64; 4], projection: Projection) -> [f64; 4] {
    let y = |lat: f64| match projection {
        Projection::WebMercator => {
            0.5 - lat.to_radians().tan().asinh() / (2.0 * std::f64::consts::PI)
        }
        _ => 0.5 - lat / 180.0,
    };
    [
        (degrees[0] + 180.0) / 360.0,
        y(degrees[3]),
        (degrees[2] + 180.0) / 360.0,
        y(degrees[1]),
    ]
}

/// One `POST` of `body` to `path`: the status and the body's bytes.
async fn posted(server: &TestServer, token: &str, path: &str, body: Value) -> (u16, Vec<u8>) {
    let resp = server
        .client
        .post(server.viewer_url(path))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .unwrap();
    (resp.status().as_u16(), resp.bytes().await.unwrap().to_vec())
}

/// How many items a `region` filter keeps over `whole`, the view's frame.
async fn matched(server: &TestServer, token: &str, whole: [f64; 4], region: Value) -> u64 {
    let (status, tiles, _) = world_viewport(
        server,
        token,
        serde_json::json!({
            "view": "s0", "zoom": 0, "bbox": whole, "k": 0,
            "filters": {"region": region},
        }),
    )
    .await;
    assert_eq!(status, 200);
    tiles.iter().map(|t| t.2).sum()
}

/// The tiles an artifacts viewport over `bbox` answers for, in order.
async fn artifact_tiles(
    server: &TestServer,
    token: &str,
    bbox: [f64; 4],
    space: &str,
) -> Vec<Option<u32>> {
    let (status, bytes) = posted(
        server,
        token,
        "/v1/artifacts/viewport",
        serde_json::json!({"view": "s0", "zoom": 6, "bbox": bbox, "space": space, "per_tile": 1}),
    )
    .await;
    assert_eq!(status, 200, "{}", String::from_utf8_lossy(&bytes));
    decode_artifact_frames(&bytes)
        .frames
        .iter()
        .map(|frame| frame.0)
        .collect()
}

/// The rows of a count over the depth-6 cells that touch `area`.
async fn cell_rows(
    server: &TestServer,
    token: &str,
    area: [f64; 4],
    space: Option<&str>,
) -> Vec<(u64, AggregateRow)> {
    let mut cells = serde_json::json!({"depth": 6, "area": area});
    if let Some(space) = space {
        cells["space"] = space.into();
    }
    let (status, bytes) = posted(
        server,
        token,
        "/v1/aggregate",
        serde_json::json!({"view": "s0", "groupings": [{"count": true, "cells": cells}]}),
    )
    .await;
    assert_eq!(status, 200, "{}", String::from_utf8_lossy(&bytes));
    decode_aggregate(&bytes).rows()
}

/// A box in degrees, sent as `space = "wgs84"` or as `inserted`, is answered as the same box sent
/// in the view's own coordinates, under every projection and in a frame that is part of the
/// world: as the viewport's `bbox`, as a `region` filter, as the artifacts viewport's `bbox` and
/// as an aggregate's `cells.area`. The `region` filter keeps the places the degrees say it does.
#[tokio::test]
async fn a_box_in_degrees_is_answered_as_its_projected_box() {
    // Longitude −90°…0° and latitude 0°…66.5°, the z2 tile (1, 1) of a Web Mercator world.
    let part = AlignedSquare { z: 2, x: 1, y: 1 }.bounds();
    for (projection, frame, degrees) in [
        (Projection::WebMercator, AlignedSquare::WORLD.bounds(), [-10.5, 0.5, 60.5, 60.5]),
        (Projection::PLATE_CARREE, AlignedSquare::WORLD.bounds(), [-10.5, 0.5, 60.5, 60.5]),
        (Projection::GALL_ISOGRAPHIC, AlignedSquare::WORLD.bounds(), [-10.5, 0.5, 60.5, 60.5]),
        (Projection::WebMercator, part, [-70.5, 10.5, -20.5, 50.5]),
    ] {
        let name = format!("{} in {frame:?}", projection.name());
        let tmp = TempDir::new().unwrap();
        let bundle_root = tmp.path().join("bundle");
        build_projected(&bundle_root, tmp.path(), projection, frame);
        let server = spawn_server(
            &bundle_root,
            &tmp.path().join("cache"),
            &tmp.path().join("wal.log"),
        )
        .await;
        let auth = authorise(&server, &["0"]).await;
        let token = auth["token"].as_str().unwrap();
        let projected = unit_square(degrees, projection);
        let whole = [frame.x_min, frame.y_min, frame.x_max, frame.y_max];

        // The viewport's own box.
        let viewport = |bbox: [f64; 4], space: Option<&str>| {
            let mut body = serde_json::json!({"view": "s0", "zoom": 6, "bbox": bbox, "k": 50});
            if let Some(space) = space {
                body["space"] = space.into();
            }
            world_viewport(&server, token, body)
        };
        let in_view = viewport(projected, Some("view")).await;
        assert_eq!(in_view.0, 200, "{name}");
        assert!(!in_view.1.is_empty() && !in_view.2.is_empty(), "{name}");
        assert_eq!(viewport(degrees, Some("wgs84")).await, in_view, "{name}");
        assert_eq!(viewport(degrees, Some("inserted")).await, in_view, "{name}");
        // The same numbers with no `space` are view coordinates, and name a different box.
        let unstated = viewport(degrees, None).await;
        assert_eq!(unstated.0, 200, "{name}");
        assert_ne!(unstated.1, in_view.1, "{name}");

        // The `region` filter, over the whole frame.
        // The fixture's own places: a point per degree of longitude along diagonals of latitude.
        let expected = (0..N_ITEMS)
            .map(|e| (-180.0 + (e % 360) as f64, -80.0 + (e % 160) as f64))
            .filter(|(lon, lat)| {
                (degrees[0]..=degrees[2]).contains(lon) && (degrees[1]..=degrees[3]).contains(lat)
            })
            .count() as u64;
        assert!(expected > 0, "{name}");
        for region in [
            serde_json::json!({"bbox": degrees, "space": "wgs84"}),
            serde_json::json!({"bbox": degrees, "space": "inserted"}),
            serde_json::json!({"bbox": projected}),
        ] {
            assert_eq!(matched(&server, token, whole, region.clone()).await, expected, "{name}: {region}");
        }

        // The artifacts viewport's box and an aggregate's area.
        let artifacts = |bbox: [f64; 4], space: &'static str| artifact_tiles(&server, token, bbox, space);
        let in_view = artifacts(projected, "view").await;
        assert_eq!(artifacts(degrees, "wgs84").await, in_view, "{name}");
        assert_eq!(artifacts(degrees, "inserted").await, in_view, "{name}");

        let cells = |area: [f64; 4], space: Option<&'static str>| cell_rows(&server, token, area, space);
        let in_view = cells(projected, None).await;
        assert!(!in_view.is_empty(), "{name}");
        assert_eq!(cells(degrees, Some("wgs84")).await, in_view, "{name}");
        assert_eq!(cells(degrees, Some("inserted")).await, in_view, "{name}");
        assert_ne!(cells(degrees, None).await, in_view, "{name}");
    }
}

/// A viewport `space` is refused where it cannot be honoured: `wgs84` on a view with no
/// projection, a word that is not a space, degrees that are not a coordinate, and beside `tiles`.
#[tokio::test]
async fn a_viewport_space_is_refused_where_it_cannot_be_honoured() {
    let tmp = TempDir::new().unwrap();
    let plain = build_fixture(tmp.path(), N_ITEMS);
    let server = spawn_server(&plain, &tmp.path().join("cache"), &tmp.path().join("wal.log")).await;
    let auth = authorise(&server, &["0"]).await;
    let token = auth["token"].as_str().unwrap();
    for body in [
        serde_json::json!({"view": "s0", "zoom": 0, "bbox": [0.0, 0.0, 10.0, 10.0], "space": "wgs84"}),
        serde_json::json!({"view": "s0", "zoom": 0, "bbox": [0.0, 0.0, 10.0, 10.0], "space": "utm"}),
        serde_json::json!({"view": "s0", "zoom": 0, "tiles": [0], "space": "view"}),
    ] {
        assert_eq!(world_viewport(&server, token, body.clone()).await.0, 422, "{body}");
    }
    for (path, body) in [
        (
            "/v1/artifacts/viewport",
            serde_json::json!({"view": "s0", "zoom": 0, "bbox": [0.0, 0.0, 10.0, 10.0], "space": "wgs84", "per_tile": 1}),
        ),
        (
            "/v1/artifacts/viewport",
            serde_json::json!({"view": "s0", "zoom": 0, "tiles": [0], "space": "view", "per_tile": 1}),
        ),
        (
            "/v1/aggregate",
            serde_json::json!({"view": "s0", "groupings": [{"count": true, "cells": {"depth": 2, "area": [0.0, 0.0, 10.0, 10.0], "space": "wgs84"}}]}),
        ),
        (
            "/v1/aggregate",
            serde_json::json!({"view": "s0", "groupings": [{"count": true, "cells": {"depth": 2, "space": "view"}}]}),
        ),
    ] {
        assert_eq!(posted(&server, token, path, body.clone()).await.0, 422, "{path} {body}");
    }
    // `inserted` on a view with no projection is the view's own space.
    let inserted = world_viewport(
        &server,
        token,
        serde_json::json!({"view": "s0", "zoom": 0, "bbox": [0.0, 0.0, 1000.0, 1000.0], "space": "inserted"}),
    )
    .await;
    assert_eq!(inserted.0, 200);
    assert!(!inserted.1.is_empty());
    // Stated as the view's own space, the box is read as it always was.
    let stated = world_viewport(
        &server,
        token,
        serde_json::json!({"view": "s0", "zoom": 0, "bbox": [0.0, 0.0, 1000.0, 1000.0], "space": "view"}),
    )
    .await;
    assert_eq!(stated.0, 200);

    let world_dir = TempDir::new().unwrap();
    let bundle_root = world_dir.path().join("bundle");
    build_projected(
        &bundle_root,
        world_dir.path(),
        Projection::WebMercator,
        AlignedSquare::WORLD.bounds(),
    );
    let server = spawn_server(
        &bundle_root,
        &world_dir.path().join("cache"),
        &world_dir.path().join("wal.log"),
    )
    .await;
    let auth = authorise(&server, &["0"]).await;
    let token = auth["token"].as_str().unwrap();
    let beyond = serde_json::json!({"view": "s0", "zoom": 0, "bbox": [0.0, 0.0, 10.0, 91.0], "space": "wgs84"});
    assert_eq!(world_viewport(&server, token, beyond).await.0, 422);
}
