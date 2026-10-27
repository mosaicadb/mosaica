//! `GET /v1/basemaps/world`: the built-in world outline, the same bytes for every session.

mod common;

use tempfile::TempDir;

use common::*;

const ORIGIN: &str = "https://app.example";

async fn get(
    server: &TestServer,
    token: Option<&str>,
    if_none_match: Option<&str>,
) -> reqwest::Response {
    let mut req = server.client.get(server.viewer_url("/v1/basemaps/world"));
    if let Some(token) = token {
        req = req.bearer_auth(token);
    }
    if let Some(tag) = if_none_match {
        req = req.header("if-none-match", tag);
    }
    req.send().await.unwrap()
}

/// The status and error code of `resp`, a refusal.
async fn refusal(resp: reqwest::Response) -> (u16, String) {
    let status = resp.status().as_u16();
    (status, error_code(&resp.text().await.unwrap()))
}

/// What `/v1/meta`, another viewer route, answers to `token`.
async fn meta_refusal(server: &TestServer, token: Option<&str>) -> (u16, String) {
    let mut req = server.client.get(server.viewer_url("/v1/meta"));
    if let Some(token) = token {
        req = req.bearer_auth(token);
    }
    refusal(req.send().await.unwrap()).await
}

#[tokio::test]
async fn no_token_is_refused_as_on_the_other_viewer_routes() {
    let tmp = TempDir::new().unwrap();
    let server = serve_standard(tmp.path()).await;
    let expected = meta_refusal(&server, None).await;
    assert_eq!(expected.0, 401);
    assert_eq!(refusal(get(&server, None, None).await).await, expected);
    // A revalidation is refused too, so a 304 says nothing to a caller without a session.
    let etag = get(&server, Some(&token_for(&server, &["0"]).await), None)
        .await
        .headers()["etag"]
        .to_str()
        .unwrap()
        .to_string();
    assert_eq!(
        refusal(get(&server, None, Some(&etag)).await).await,
        expected
    );
}

#[tokio::test]
async fn an_invalid_token_is_refused_as_on_the_other_viewer_routes() {
    let tmp = TempDir::new().unwrap();
    let server = serve_standard(tmp.path()).await;
    let expected = meta_refusal(&server, Some("not-a-session-token")).await;
    assert_eq!(expected.0, 401);
    assert_eq!(
        refusal(get(&server, Some("not-a-session-token"), None).await).await,
        expected
    );
    assert_eq!(
        refusal(get(&server, Some("not-a-session-token"), Some("*")).await).await,
        expected
    );
}

#[tokio::test]
async fn a_valid_token_gets_the_outline_the_same_for_every_principal() {
    let tmp = TempDir::new().unwrap();
    let server = serve_standard(tmp.path()).await;
    let mut bodies = Vec::new();
    for terms in [&["0"][..], &["1"][..]] {
        let resp = get(&server, Some(&token_for(&server, terms).await), None).await;
        assert_eq!(resp.status(), 200, "for a principal holding {terms:?}");
        let body = resp.bytes().await.unwrap();
        assert!(body.starts_with(b"MBW1"));
        bodies.push(body);
    }
    assert_eq!(bodies[0], bodies[1]);
}

#[tokio::test]
async fn it_is_cached_for_a_year_and_revalidates_by_its_etag() {
    let tmp = TempDir::new().unwrap();
    let server = serve_standard(tmp.path()).await;
    let token = token_for(&server, &["0"]).await;

    let resp = get(&server, Some(&token), None).await;
    assert_eq!(resp.status(), 200);
    let header = |resp: &reqwest::Response, name: &str| {
        resp.headers()
            .get(name)
            .unwrap_or_else(|| panic!("{name} is sent"))
            .to_str()
            .unwrap()
            .to_string()
    };
    assert_eq!(
        header(&resp, "cache-control"),
        "public, max-age=31536000, immutable"
    );
    assert_eq!(header(&resp, "content-type"), "application/octet-stream");
    let etag = header(&resp, "etag");
    assert!(
        etag.starts_with('"') && etag.ends_with('"') && etag.len() > 2,
        "{etag}"
    );

    for tag in [
        etag.clone(),
        format!("W/{etag}"),
        format!("\"other\", {etag}"),
        "*".to_string(),
    ] {
        let resp = get(&server, Some(&token), Some(&tag)).await;
        assert_eq!(resp.status(), 304, "If-None-Match: {tag}");
        assert_eq!(header(&resp, "etag"), etag);
        assert_eq!(
            header(&resp, "cache-control"),
            "public, max-age=31536000, immutable"
        );
        assert!(resp.bytes().await.unwrap().is_empty());
    }

    let resp = get(&server, Some(&token), Some("\"other\"")).await;
    assert_eq!(resp.status(), 200);
    assert_eq!(header(&resp, "etag"), etag);
    assert!(!resp.bytes().await.unwrap().is_empty());
}

/// The body is wrapped by the viewer plane's CORS layer like every other route on it.
#[tokio::test]
async fn a_listed_origin_may_read_it() {
    let tmp = TempDir::new().unwrap();
    let bundle_root = standard_fixture(tmp.path());
    let server = spawn_server_with_cors(
        &bundle_root,
        &tmp.path().join("cache"),
        &tmp.path().join("wal.log"),
        CorsOrigins::production(&[ORIGIN]),
    )
    .await;
    let resp = server
        .client
        .get(server.viewer_url("/v1/basemaps/world"))
        .bearer_auth(token_for(&server, &["0"]).await)
        .header("Origin", ORIGIN)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers()
            .get("access-control-allow-origin")
            .map(|v| v.to_str().unwrap()),
        Some(ORIGIN)
    );
    let exposed = resp.headers()["access-control-expose-headers"]
        .to_str()
        .unwrap()
        .to_ascii_lowercase();
    assert!(exposed.split(',').any(|h| h.trim() == "etag"), "{exposed}");
}

/// Reads `MBW1` as its encoder writes it, checking each point lies on the globe.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
    points: usize,
}

impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> [u8; N] {
        let out = *self
            .bytes
            .get(self.at..self.at + N)
            .and_then(|s| s.first_chunk::<N>())
            .unwrap_or_else(|| panic!("the body ends at byte {} inside a field", self.bytes.len()));
        self.at += N;
        out
    }

    fn u16(&mut self) -> u16 {
        u16::from_le_bytes(self.take())
    }

    fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.take())
    }

    /// One point run, as longitude and latitude in degrees.
    fn run(&mut self) -> Vec<(f64, f64)> {
        let n = self.u32() as usize;
        assert!(
            self.at + 4 * n <= self.bytes.len(),
            "a run of {n} points overruns the body"
        );
        (0..n)
            .map(|_| {
                let x = i16::from_le_bytes(self.take());
                let y = i16::from_le_bytes(self.take());
                assert!(
                    x != i16::MIN && y != i16::MIN,
                    "a point at ({x}, {y}) is off the globe"
                );
                self.points += 1;
                (
                    f64::from(x) * 180.0 / 32767.0,
                    f64::from(y) * 90.0 / 32767.0,
                )
            })
            .collect()
    }

    fn lines(&mut self) -> usize {
        let n = self.u32() as usize;
        for _ in 0..n {
            assert!(self.run().len() >= 2, "a line has fewer than two points");
        }
        n
    }
}

#[tokio::test]
async fn the_body_is_the_world_in_mbw1() {
    let tmp = TempDir::new().unwrap();
    let server = serve_standard(tmp.path()).await;
    let token = token_for(&server, &["0"]).await;
    let body = get(&server, Some(&token), None).await.bytes().await.unwrap();
    assert_eq!(&body[..4], b"MBW1");

    let mut r = Reader {
        bytes: &body,
        at: 4,
        points: 0,
    };
    let polygons = r.u32() as usize;
    let (mut west, mut east, mut south, mut north) = (180.0f64, -180.0f64, 90.0f64, -90.0f64);
    for _ in 0..polygons {
        let rings = r.u16();
        assert!(rings >= 1, "a polygon has no outer ring");
        for _ in 0..rings {
            let ring = r.run();
            assert!(ring.len() >= 4, "a ring has {} points", ring.len());
            assert_eq!(ring.first(), ring.last(), "a ring is not closed");
            for &(lon, lat) in &ring {
                west = west.min(lon);
                east = east.max(lon);
                south = south.min(lat);
                north = north.max(lat);
            }
        }
    }
    let land_points = r.points;
    let coastlines = r.lines();
    let coast_points = r.points - land_points;
    let borders = r.lines();
    let border_points = r.points - land_points - coast_points;
    assert_eq!(r.at, body.len(), "bytes follow the borders layer");

    // Natural Earth 1:50m has about 1,400 land polygons and coastlines and 400 border lines.
    assert!(
        (1_000..3_000).contains(&polygons),
        "{polygons} land polygons"
    );
    assert!(
        (1_000..3_000).contains(&coastlines),
        "{coastlines} coastlines"
    );
    assert!((200..1_000).contains(&borders), "{borders} border lines");
    for (layer, points) in [
        ("land", land_points),
        ("coastline", coast_points),
        ("borders", border_points),
    ] {
        assert!(
            (10_000..200_000).contains(&points),
            "{layer} has {points} points"
        );
    }
    // The land reaches round the globe and from Antarctica to the Arctic.
    assert!(west < -179.0 && east > 179.0, "land spans {west}..{east}");
    assert!(south < -85.0 && north > 80.0, "land spans {south}..{north}");
}
