//! An open server (`[serve] open`): every request is served as the operator, with no credential,
//! on a server that listens on this machine alone and refuses what a page on another site could
//! send.

mod common;

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use common::{
    N_ITEMS, OPERATOR_CREDENTIAL, TestServer, build_ingest_batch_optional, decode_records,
    decode_viewport, serve_state, write_deployment,
};
use mosaica_config::ConfigError;

/// A deployment over the standard fixture with the operator credential, not open.
fn deployment(tmp: &Path) -> PathBuf {
    write_deployment(tmp, "127.0.0.1:0", "")
}

/// Rewrite `[serve]` in the deployment at `path`, adding `extra` and dropping the operator
/// credential where `credential` is false.
fn rewrite(path: &Path, extra: &str, credential: bool) {
    let text = std::fs::read_to_string(path).unwrap();
    let mut text = text.replace("[catalogue]", &format!("{extra}\n        [catalogue]"));
    if !credential {
        text = text
            .lines()
            .filter(|line| !line.contains("operator_credential_file"))
            .collect::<Vec<_>>()
            .join("\n");
    }
    std::fs::write(path, text).unwrap();
}

async fn serve(path: &Path) -> TestServer {
    let prepared = mosaica_server::prepare(path).expect("the deployment must start");
    serve_state(prepared.state, String::new(), None).await
}

fn refusal(path: &Path) -> ConfigError {
    let Err(e) = mosaica_server::prepare(path) else {
        panic!("the deployment must be refused");
    };
    match e.downcast::<ConfigError>() {
        Ok(e) => *e,
        Err(other) => panic!("a configuration refusal, not {other}"),
    }
}

/// `/v1/meta`, a viewport's tile counts and the ids `/v1/items` lists, as `token` reads them.
async fn reads(
    server: &TestServer,
    token: Option<&str>,
) -> (Value, Vec<(u64, u64, u64)>, Vec<u64>) {
    let with = |r: reqwest::RequestBuilder| match token {
        Some(t) => r.bearer_auth(t),
        None => r,
    };
    let meta = with(server.client.get(server.viewer_url("/v1/meta")))
        .send()
        .await
        .unwrap();
    assert_eq!(meta.status(), 200);
    let meta: Value = meta.json().await.unwrap();
    let viewport = with(server.client.post(server.viewer_url("/v1/viewport")))
        .json(&json!({ "view": "s0", "zoom": 0, "bbox": [0.0, 0.0, 1000.0, 1000.0] }))
        .send()
        .await
        .unwrap();
    assert_eq!(viewport.status(), 200);
    let (tiles, _) = decode_viewport(&viewport.bytes().await.unwrap());
    let items = with(server.client.post(server.viewer_url("/v1/items")))
        .json(&json!({ "view": "s0", "fields": [] }))
        .send()
        .await
        .unwrap();
    assert_eq!(items.status(), 200);
    let records = decode_records(&items.bytes().await.unwrap());
    let ids = records
        .pages
        .iter()
        .flat_map(|(batch, _)| {
            let column = batch
                .column(0)
                .as_any()
                .downcast_ref::<arrow::array::UInt64Array>()
                .expect("mosaica_id is u64")
                .clone();
            column.values().to_vec()
        })
        .collect();
    (meta, tiles, ids)
}

fn visible(tiles: &[(u64, u64, u64)]) -> u64 {
    tiles.iter().map(|(_, visible, _)| visible).sum()
}

#[tokio::test(flavor = "multi_thread")]
async fn an_open_server_serves_what_a_read_all_session_sees_with_no_credential() {
    let tmp = TempDir::new().unwrap();
    let path = deployment(tmp.path());

    let server = serve(&path).await;
    let minted: Value = server
        .client
        .post(server.session_url("/session/authorise"))
        .bearer_auth(OPERATOR_CREDENTIAL)
        .json(&json!({ "read_all": true }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let read_all = reads(&server, Some(minted["token"].as_str().unwrap())).await;
    assert_eq!(visible(&read_all.1), N_ITEMS);
    server.shutdown().await;

    rewrite(&path, "open = true", true);
    let server = serve(&path).await;
    assert_eq!(reads(&server, None).await, read_all);
    // A token is not needed and changes nothing, whatever it is.
    assert_eq!(reads(&server, Some("not-a-token")).await, read_all);
}

#[tokio::test(flavor = "multi_thread")]
async fn writes_and_declarations_need_no_credential_and_survive_a_restart() {
    let tmp = TempDir::new().unwrap();
    let path = deployment(tmp.path());
    rewrite(&path, "open = true", false);
    let server = serve(&path).await;

    let declared = server
        .client
        .put(server.control_url("/control/attributes"))
        .json(&json!({ "name": "sentiment", "type": "f32", "index": true }))
        .send()
        .await
        .unwrap();
    assert!(
        declared.status().is_success(),
        "{}",
        declared.text().await.unwrap()
    );
    let ingested = server
        .client
        .post(server.control_url("/control/ingest?wait=visible"))
        .header("x-mosaica-batch-id", "open-1")
        .header("content-type", "application/vnd.apache.arrow.stream")
        .body(build_ingest_batch_optional(&[(None, 10.0, 10.0, "0")]))
        .send()
        .await
        .unwrap();
    assert_eq!(ingested.status(), 200, "{}", ingested.text().await.unwrap());
    let (_, tiles, _) = reads(&server, None).await;
    assert_eq!(visible(&tiles), N_ITEMS + 1);
    server.shutdown().await;

    let server = serve(&path).await;
    let (meta, tiles, _) = reads(&server, None).await;
    assert_eq!(visible(&tiles), N_ITEMS + 1);
    let declared: Vec<&str> = meta["declared_scalars"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|d| d["name"].as_str())
        .collect();
    assert!(declared.contains(&"sentiment"), "{meta}");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_open_server_refuses_to_start_off_loopback_or_beside_users() {
    let tmp = TempDir::new().unwrap();
    let path = deployment(tmp.path());
    let text = std::fs::read_to_string(&path).unwrap();

    std::fs::write(&path, text.replacen("127.0.0.1:0", "0.0.0.0:0", 1)).unwrap();
    rewrite(&path, "open = true", true);
    assert!(matches!(
        refusal(&path),
        ConfigError::OpenNotLoopback { .. }
    ));

    std::fs::write(&path, &text).unwrap();
    rewrite(
        &path,
        "open = true\ncors_origins = [\"https://app.example\"]",
        true,
    );
    assert!(matches!(refusal(&path), ConfigError::OpenBeside { .. }));

    // A principal, and with it any API key, makes this a server with users.
    std::fs::write(&path, &text).unwrap();
    let server = serve(&path).await;
    let created = server
        .client
        .post(server.control_url("/control/principals"))
        .bearer_auth(OPERATOR_CREDENTIAL)
        .json(&json!({ "name": "ann", "kind": "person" }))
        .send()
        .await
        .unwrap();
    assert!(created.status().is_success());
    server.shutdown().await;
    rewrite(&path, "open = true", true);
    assert!(matches!(refusal(&path), ConfigError::OpenBeside { .. }));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_open_server_adds_no_principal_and_no_provider() {
    let tmp = TempDir::new().unwrap();
    let path = deployment(tmp.path());
    rewrite(&path, "open = true", true);
    let server = serve(&path).await;
    let principal = server
        .client
        .post(server.control_url("/control/principals"))
        .json(&json!({ "name": "ann", "kind": "person" }))
        .send()
        .await
        .unwrap();
    assert_eq!(principal.status(), 403);
    let provider = server
        .client
        .put(server.control_url("/control/providers/idp"))
        .json(&json!({
            "issuer": "https://idp.example",
            "audience": "mosaica",
            "jwks_url": "https://idp.example/jwks"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(provider.status(), 403);
    server.shutdown().await;
    serve(&path).await;
}

/// One HTTP/1.1 request written as given, and the status it is answered with.
async fn raw(addr: SocketAddr, request: &str) -> u16 {
    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut answer = Vec::new();
    stream.read_to_end(&mut answer).await.unwrap();
    let answer = String::from_utf8_lossy(&answer);
    answer
        .split_whitespace()
        .nth(1)
        .and_then(|status| status.parse().ok())
        .unwrap_or_else(|| panic!("no status in {answer:?}"))
}

fn get(target: &str, headers: &str) -> String {
    format!("GET {target} HTTP/1.1\r\n{headers}connection: close\r\n\r\n")
}

#[tokio::test(flavor = "multi_thread")]
async fn an_open_server_answers_only_a_host_that_names_this_machine_on_every_plane() {
    let tmp = TempDir::new().unwrap();
    let path = deployment(tmp.path());
    rewrite(&path, "open = true", false);
    let server = serve(&path).await;
    let planes = [
        (server.viewer_addr, "/healthz"),
        (server.session_addr, "/healthz"),
        (server.control_addr, "/control/status"),
    ];
    for (addr, route) in planes {
        for host in [
            "localhost",
            "localhost:8141",
            "127.0.0.1",
            "127.0.0.2:1",
            "[::1]",
            "[::1]:8141",
            "[0:0:0:0:0:0:0:1]",
            "[::ffff:127.0.0.1]:80",
        ] {
            let status = raw(addr, &get(route, &format!("host: {host}\r\n"))).await;
            assert_eq!(status, 200, "{host} on {route}");
        }
        for host in [
            "evil.example",
            "evil.example:8141",
            "localhost.evil.example",
            "127.0.0.1.evil.example",
            "LOCALHOST",
            "localhost.",
            "127.1",
            "[::ffff:10.0.0.1]",
            "localhost:",
            "localhost:80x",
            "user@localhost",
        ] {
            let status = raw(addr, &get(route, &format!("host: {host}\r\n"))).await;
            assert_eq!(status, 403, "{host} on {route}");
        }
        // The absolute form names the host in the request line, which is what the server reads.
        let line = format!("http://evil.example{route}");
        let status = raw(addr, &get(&line, "host: localhost\r\n")).await;
        assert_eq!(
            status, 403,
            "an absolute-form request to evil.example on {route}"
        );
        let line = format!("http://localhost{route}");
        let status = raw(addr, &get(&line, "host: localhost\r\n")).await;
        assert_eq!(
            status, 200,
            "an absolute-form request to localhost on {route}"
        );
        // No host, and two, are refused, by the guard or by the HTTP parser.
        for headers in [
            "",
            "host: localhost\r\nhost: localhost\r\n",
            "host: localhost\r\nhost: evil.example\r\n",
        ] {
            let status = raw(addr, &get(route, headers)).await;
            assert!(
                (400..500).contains(&status),
                "{headers:?} on {route} answered {status}"
            );
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn an_open_server_refuses_a_write_sent_as_a_type_a_browser_sends_without_asking() {
    let tmp = TempDir::new().unwrap();
    let path = deployment(tmp.path());
    rewrite(&path, "open = true", false);
    let server = serve(&path).await;
    let routes = [
        ("POST", server.control_url("/control/flush")),
        ("POST", server.control_url("/control/compact")),
        ("POST", server.control_url("/control/changes")),
        ("POST", server.control_url("/control/ingest")),
        ("PUT", server.control_url("/control/attributes")),
        (
            "PATCH",
            server.control_url("/control/vocabularies/v/values"),
        ),
        ("DELETE", server.control_url("/control/layers/l")),
        ("POST", server.session_url("/session/authorise")),
        ("POST", server.session_url("/session/revoke")),
        ("POST", server.viewer_url("/v1/logout")),
        ("POST", server.viewer_url("/v1/viewport")),
    ];
    for (method, url) in &routes {
        // No content type, as a page on another site sends a write with no body.
        let status = server
            .client
            .request(method.parse().unwrap(), url)
            .header("x-mosaica-batch-id", "b")
            .send()
            .await
            .unwrap()
            .status();
        assert_eq!(status, 403, "{method} {url} with no content type");
        for content_type in [
            "text/plain",
            "text/plain;charset=UTF-8",
            "TEXT/Plain ; charset=utf-8",
            "application/x-www-form-urlencoded",
            "multipart/form-data; boundary=x",
        ] {
            let status = server
                .client
                .request(method.parse().unwrap(), url)
                .header("content-type", content_type)
                .header("x-mosaica-batch-id", "b")
                .body("{}")
                .send()
                .await
                .unwrap()
                .status();
            assert_eq!(status, 403, "{method} {url} as {content_type}");
        }
    }
    // The content types the clients send are served, a write with no body included.
    let flushed = server
        .client
        .post(server.control_url("/control/flush"))
        .header("content-type", "application/json")
        .send()
        .await
        .unwrap();
    assert_eq!(flushed.status(), 202);
    let authorised = server
        .client
        .post(server.session_url("/session/authorise"))
        .json(&json!({ "read_all": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(authorised.status(), 200);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_open_server_admits_a_loopback_page_to_the_viewer_plane_alone() {
    let tmp = TempDir::new().unwrap();
    let path = deployment(tmp.path());
    rewrite(&path, "open = true", false);
    let server = serve(&path).await;
    let preflight = |url: String, origin: &'static str| {
        let client = server.client.clone();
        async move {
            client
                .request(reqwest::Method::OPTIONS, url)
                .header("origin", origin)
                .header("access-control-request-method", "POST")
                .header("access-control-request-headers", "content-type")
                .send()
                .await
                .unwrap()
                .headers()
                .get("access-control-allow-origin")
                .map(|v| v.to_str().unwrap().to_string())
        }
    };
    assert_eq!(
        preflight(server.viewer_url("/v1/viewport"), "http://localhost:5173").await,
        Some("http://localhost:5173".into())
    );
    assert_eq!(
        preflight(server.viewer_url("/v1/viewport"), "https://evil.example").await,
        None
    );
    for url in [
        server.session_url("/session/authorise"),
        server.control_url("/control/ingest"),
        server.control_url("/control/changes"),
    ] {
        assert_eq!(
            preflight(url.clone(), "http://localhost:5173").await,
            None,
            "{url}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn an_open_server_refuses_a_browser_request_from_a_page_on_another_site() {
    let tmp = TempDir::new().unwrap();
    let path = deployment(tmp.path());
    rewrite(&path, "open = true", false);
    let server = serve(&path).await;
    let send = |url: String, headers: &'static [(&'static str, &'static str)]| {
        let client = server.client.clone();
        async move {
            let mut request = client.get(url);
            for (name, value) in headers {
                request = request.header(*name, *value);
            }
            request.send().await.unwrap().status().as_u16()
        }
    };
    for url in [
        server.viewer_url("/v1/meta"),
        server.session_url("/healthz"),
        server.control_url("/control/status"),
    ] {
        assert_eq!(send(url.clone(), &[]).await, 200, "{url} with no Origin");
        for headers in [
            &[("origin", "https://evil.example")][..],
            &[("origin", "http://localhost.evil.example")][..],
            &[("origin", "null")][..],
            &[("sec-fetch-site", "cross-site")][..],
            &[
                ("origin", "https://evil.example"),
                ("sec-fetch-site", "same-origin"),
            ][..],
        ] {
            assert_eq!(
                send(url.clone(), headers).await,
                403,
                "{url} with {headers:?}"
            );
        }
        // A page on localhost calling 127.0.0.1 is cross-site to the browser, and on this machine.
        let local = &[
            ("origin", "http://localhost:5173"),
            ("sec-fetch-site", "cross-site"),
        ][..];
        assert_eq!(
            send(url.clone(), local).await,
            200,
            "{url} from a loopback page"
        );
    }
    let viewport = server
        .client
        .post(server.viewer_url("/v1/viewport"))
        .header("origin", "https://evil.example")
        .json(&json!({ "view": "s0", "zoom": 0, "bbox": [0.0, 0.0, 1000.0, 1000.0] }))
        .send()
        .await
        .unwrap();
    assert_eq!(viewport.status(), 403);
}
