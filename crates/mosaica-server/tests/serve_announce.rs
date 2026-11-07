//! The announce line: `serve` binds what the deployment declares, including a TCP address whose
//! port is 0, and writes one JSON line naming the address each plane is served at.
//!
//! A supervisor that started the process reads that line and sends its first request to the
//! address in it, so what these tests pin is the line's exact text, that it is the only thing on
//! the stream, and that the announced viewer address is already answering when it appears.
//!
//! The process writes the line to stdout; `serve_announcing` is the same code with the stream
//! supplied, which is the seam a test can read. That `mosaica serve` passes stdout, and its
//! diagnostics stderr, is in `mosaica`.

mod common;

use std::io::Write;
use std::sync::{Arc, Mutex};

use tempfile::TempDir;

use common::{wait_for, write_deployment, OPERATOR_CREDENTIAL};

/// The announce stream, readable from the test while the server holds it.
#[derive(Clone)]
struct SharedStream(Arc<Mutex<Vec<u8>>>);

impl SharedStream {
    fn new() -> SharedStream {
        SharedStream(Arc::new(Mutex::new(Vec::new())))
    }

    fn text(&self) -> String {
        String::from_utf8(self.0.lock().unwrap().clone()).expect("the announce line is UTF-8")
    }
}

impl Write for SharedStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A server for `deployment`, bound and prepared as `mosaica serve` does, writing its announce
/// line to the stream returned. It stops when the stream's task handle is dropped.
fn serve(deployment: &std::path::Path) -> (SharedStream, AbortOnDrop) {
    let config = mosaica_config::load(deployment).expect("the deployment loads");
    let listeners = mosaica_server::bind(&config).expect("the deployment's addresses bind");
    let prepared = mosaica_server::prepare_config(config).expect("the deployment must start");
    let stream = SharedStream::new();
    let serving = tokio::spawn({
        let stream = stream.clone();
        async move { mosaica_server::serve_announcing(prepared, listeners, stream).await }
    });
    (stream, AbortOnDrop(serving))
}

struct AbortOnDrop(tokio::task::JoinHandle<Result<(), mosaica_server::BoxError>>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// `deployment` rewritten with each `[serve]` line that starts with one of `keys` left out, and
/// `extra` added under `[serve]`.
fn rewrite_serve(deployment: &std::path::Path, keys: &[&str], extra: &str) {
    let text = std::fs::read_to_string(deployment).unwrap();
    let mut out = String::new();
    for line in text.lines() {
        let key = line.trim_start().split([' ', '=']).next().unwrap_or("");
        if keys.contains(&key) {
            continue;
        }
        out.push_str(line);
        out.push('\n');
        if line.trim() == "[serve]" {
            out.push_str(extra);
            out.push('\n');
        }
    }
    std::fs::write(deployment, out).unwrap();
}

/// The first complete line on the stream, waited for rather than raced against.
async fn announce_line(stream: &SharedStream) -> String {
    wait_for(
        "the server announcing its addresses",
        std::time::Duration::from_secs(60),
        async || {
            let text = stream.text();
            match text.split_once('\n') {
                Some((line, _)) => Ok(line.to_string()),
                None => Err(format!("the stream holds {text:?}")),
            }
        },
    )
    .await
}

/// Port 0 on all three planes: each is bound to a port the kernel chose, the line names the ports
/// actually bound, and the viewer plane answers at the address it names.
#[tokio::test(flavor = "multi_thread")]
async fn port_zero_planes_announce_the_ports_the_kernel_chose() {
    let tmp = TempDir::new().unwrap();
    let deployment = write_deployment(tmp.path(), "127.0.0.1:0", "");
    let stream = serve(&deployment);

    let line = announce_line(&stream.0).await;
    let announced: serde_json::Value = serde_json::from_str(&line)
        .unwrap_or_else(|e| panic!("the line must be JSON: {e}, {line}"));
    let viewer = announced["viewer"]
        .as_str()
        .expect("a viewer address")
        .to_string();
    let session = announced["session"]
        .as_str()
        .expect("a session address")
        .to_string();
    let control = announced["control"]
        .as_str()
        .expect("a control address")
        .to_string();

    // The exact text a supervisor parses: one object, these four keys, this order.
    assert_eq!(
        line,
        format!(
            r#"{{"event":"listening","viewer":"{viewer}","session":"{session}","control":"{control}"}}"#
        )
    );
    for (what, addr) in [
        ("viewer", &viewer),
        ("session", &session),
        ("control", &control),
    ] {
        let addr: std::net::SocketAddr = addr.parse().expect("an announced address parses");
        assert_ne!(addr.port(), 0, "the {what} port must be the bound one");
    }

    let client = reqwest::Client::new();
    let health = client
        .get(format!("http://{viewer}/healthz"))
        .send()
        .await
        .expect("the announced viewer address must accept a request");
    assert_eq!(health.status(), 200);
    // The viewer router, not just a socket: `/v1/meta` costs a session token, so an unauthenticated
    // request is refused by the handler rather than by the absence of a route.
    let meta = client
        .get(format!("http://{viewer}/v1/meta"))
        .send()
        .await
        .expect("the viewer plane must answer");
    assert_eq!(meta.status(), 401);
    for addr in [&session, &control] {
        tokio::net::TcpStream::connect(addr)
            .await
            .unwrap_or_else(|e| panic!("the announced address {addr} must be listening: {e}"));
    }

    // One line, and nothing after it: the supervisor reads the first line of the stream and stops.
    assert_eq!(
        stream.0.text(),
        format!("{line}\n"),
        "the announce stream carries the one line"
    );
}

/// A unix-socket control plane is announced as `unix:` and its path, so the supervisor has the
/// one address per plane whatever the plane is listening on.
#[tokio::test(flavor = "multi_thread")]
async fn a_unix_control_plane_is_announced_by_its_path() {
    let tmp = TempDir::new().unwrap();
    let socket = tmp.path().join("control.sock");
    let deployment = write_deployment(tmp.path(), &format!("unix:{}", socket.display()), "");
    let stream = serve(&deployment);

    let line = announce_line(&stream.0).await;
    let announced: serde_json::Value = serde_json::from_str(&line).expect("the line must be JSON");
    assert_eq!(
        announced["control"].as_str(),
        Some(format!("unix:{}", socket.display()).as_str())
    );
    tokio::net::UnixStream::connect(&socket)
        .await
        .expect("the announced socket must be listening");
}

/// A deployment that names only the viewer's address serves all three planes there. Each plane's
/// routes keep their own rules: the viewer plane admits a loopback page, and the session and
/// control planes admit no page and take only their own credentials.
#[tokio::test(flavor = "multi_thread")]
async fn planes_with_no_address_of_their_own_share_the_viewers() {
    let tmp = TempDir::new().unwrap();
    let deployment = write_deployment(tmp.path(), "127.0.0.1:0", "");
    rewrite_serve(&deployment, &["session", "control"], "cors_loopback = true");
    let stream = serve(&deployment);

    let line = announce_line(&stream.0).await;
    let announced: serde_json::Value = serde_json::from_str(&line).expect("the line must be JSON");
    let at = announced["viewer"].as_str().expect("a viewer address").to_string();
    assert_eq!(announced["session"].as_str(), Some(at.as_str()));
    assert_eq!(announced["control"].as_str(), Some(at.as_str()));

    let client = reqwest::Client::new();
    let url = |path: &str| format!("http://{at}{path}");
    assert_eq!(client.get(url("/healthz")).send().await.unwrap().status(), 200);
    assert_eq!(client.get(url("/v1/meta")).send().await.unwrap().status(), 401);
    assert_eq!(client.get(url("/control/status")).send().await.unwrap().status(), 401);
    assert_eq!(client.get(url("/control/nonesuch")).send().await.unwrap().status(), 401);
    let unauthorised = client
        .post(url("/session/authorise"))
        .json(&serde_json::json!({"read_all": true}))
        .send()
        .await
        .unwrap();
    assert_eq!(unauthorised.status(), 401);

    let status = client
        .get(url("/control/status"))
        .bearer_auth(OPERATOR_CREDENTIAL)
        .send()
        .await
        .unwrap();
    assert_eq!(status.status(), 200);
    let minted: serde_json::Value = client
        .post(url("/session/authorise"))
        .bearer_auth(OPERATOR_CREDENTIAL)
        .json(&serde_json::json!({"read_all": true}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let token = minted["token"].as_str().expect("a session token");
    let meta = client.get(url("/v1/meta")).bearer_auth(token).send().await.unwrap();
    assert_eq!(meta.status(), 200);

    let preflight = |path: &str| {
        client
            .request(reqwest::Method::OPTIONS, url(path))
            .header("origin", "http://localhost:8888")
            .header("access-control-request-method", "POST")
            .header("access-control-request-headers", "authorization,content-type")
            .send()
    };
    let admitted = |response: &reqwest::Response| {
        response.headers().get("access-control-allow-origin").is_some()
    };
    assert!(admitted(&preflight("/v1/viewport").await.unwrap()));
    assert!(!admitted(&preflight("/session/authorise").await.unwrap()));
    assert!(!admitted(&preflight("/control/ingest").await.unwrap()));
}

/// A taken port is refused, unless `port_retries` lets the server move up to a free one, which
/// the announce line names.
#[tokio::test(flavor = "multi_thread")]
async fn a_taken_port_moves_up_only_as_far_as_port_retries_allows() {
    let held = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let taken = held.local_addr().unwrap();
    let tmp = TempDir::new().unwrap();
    let deployment = write_deployment(tmp.path(), "127.0.0.1:0", "");
    rewrite_serve(&deployment, &["viewer", "session", "control"], &format!("viewer = \"{taken}\""));

    let config = mosaica_config::load(&deployment).unwrap();
    assert!(mosaica_server::bind(&config).is_err(), "a taken port with no retries is refused");

    rewrite_serve(&deployment, &[], "port_retries = 20");
    let stream = serve(&deployment);
    let line = announce_line(&stream.0).await;
    let announced: serde_json::Value = serde_json::from_str(&line).expect("the line must be JSON");
    let viewer: std::net::SocketAddr = announced["viewer"].as_str().unwrap().parse().unwrap();
    assert_eq!(viewer.ip(), taken.ip());
    assert!(viewer.port() > taken.port() && viewer.port() <= taken.port() + 20);
    assert_eq!(announced["session"], announced["viewer"]);
    let health = reqwest::get(format!("http://{viewer}/healthz")).await.unwrap();
    assert_eq!(health.status(), 200);
    drop(held);
}
