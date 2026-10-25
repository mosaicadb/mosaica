//! What the CLI's tests of a served bundle share: the binary, a generated deployment built into a
//! bundle, a server process stopped when its test ends, and the request a stand-in server reads.

// Each test binary compiles this module on its own, so a helper one binary does not use is dead
// code there.
#![allow(dead_code)]

use std::collections::HashMap;
use std::io::BufRead;
use std::path::Path;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

pub const OPERATOR_CREDENTIAL: &str = "operator-credential";

/// A built bundle's one shard, in the one partition a build writes.
pub const SHARD: mosaica_store::ShardLayout<'static> =
    mosaica_store::ShardLayout::new("default", mosaica_types::ShardId::ZERO);

pub fn mosaica() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mosaica"))
}

/// A port nothing is listening on, taken from the kernel and released.
pub fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// The ports a deployment's planes listen on; 0 lets the kernel choose when the server starts.
pub struct Ports {
    pub viewer: u16,
    pub session: u16,
    pub control: u16,
}

impl Ports {
    /// Ports nothing listens on now, for a deployment whose addresses are read from its file.
    pub fn free() -> Ports {
        Ports {
            viewer: free_port(),
            session: free_port(),
            control: free_port(),
        }
    }

    /// Ports the kernel chooses as the server binds them, which no other test can be handed.
    pub fn chosen() -> Ports {
        Ports {
            viewer: 0,
            session: 0,
            control: 0,
        }
    }
}

/// A generated corpus of 2,000 items built into a bundle in `dir`, with the viewer on `0.0.0.0`, so
/// that a health check has to reach it on loopback.
pub fn deployment(dir: &Path, ports: &Ports) {
    deployment_with_control(dir, ports, &format!("127.0.0.1:{}", ports.control));
}

/// [`deployment`], with the control plane at `control`, an address or `unix:<path>`.
pub fn deployment_with_control(dir: &Path, ports: &Ports, control: &str) {
    let materialised = mosaica()
        .args(["corpus", "materialise", "--seed", "1", "--n", "2000", "--out"])
        .arg(dir)
        .output()
        .unwrap();
    assert!(materialised.status.success(), "{materialised:?}");
    std::fs::rename(dir.join("corpus-config.toml"), dir.join("schema.toml")).unwrap();
    std::fs::write(dir.join("operator.cred"), OPERATOR_CREDENTIAL).unwrap();
    std::fs::write(
        dir.join("mosaica.toml"),
        format!(
            r#"
[bundle]
path  = "bundle"
cache = "cache"
wal   = "wal.log"

[disclosure]
token_max_lifetime = 3600

[serve]
viewer  = "0.0.0.0:{}"
session = "127.0.0.1:{}"
control = "{}"
operator_credential_file = "operator.cred"

[catalogue]
dir = "catalogue"
"#,
            ports.viewer, ports.session, control
        ),
    )
    .unwrap();
    let built = mosaica()
        .arg("build")
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(built.status.success(), "{built:?}");
}

pub fn healthy(dir: &Path) -> bool {
    mosaica()
        .arg("health")
        .current_dir(dir)
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .success()
}

/// `mosaica serve` over the deployment in `dir`, killed when this drops.
pub struct Server(pub Child);

/// The loopback addresses a server's viewer and session planes bound, as `http://…` URLs.
pub struct Bound {
    pub viewer: String,
    pub session: String,
    /// The control plane's `http://…` URL, or `unix:<path>`.
    pub control: String,
    /// Held open for the life of the server, whose stdout it is.
    _stdout: ChildStdout,
}

impl Server {
    /// Start the server and read the addresses it bound from the line it announces them on.
    pub fn announced(dir: &Path) -> (Server, Bound) {
        Server::announced_with(dir, &[])
    }

    /// [`Server::announced`], with `flags` after `mosaica serve`.
    pub fn announced_with(dir: &Path, flags: &[&str]) -> (Server, Bound) {
        let mut child = mosaica()
            .arg("serve")
            .args(flags)
            .current_dir(dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let server = Server(child);
        let mut reader = std::io::BufReader::new(stdout);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let listening: serde_json::Value = serde_json::from_str(&line)
            .unwrap_or_else(|e| panic!("serve announced {line:?}: {e}"));
        let url = |plane: &str| {
            let addr: std::net::SocketAddr = listening[plane].as_str().unwrap().parse().unwrap();
            format!("http://127.0.0.1:{}", addr.port())
        };
        let control = listening["control"].as_str().unwrap();
        let bound = Bound {
            viewer: url("viewer"),
            session: url("session"),
            control: if control.starts_with("unix:") {
                control.to_owned()
            } else {
                url("control")
            },
            _stdout: reader.into_inner(),
        };
        (server, bound)
    }

    /// Start the server and wait until it answers its health check.
    pub fn start(dir: &Path) -> Server {
        let server = Server(
            mosaica()
                .arg("serve")
                .current_dir(dir)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let started = Instant::now();
        while !healthy(dir) {
            assert!(
                started.elapsed() < Duration::from_secs(60),
                "the server did not become ready within a minute"
            );
            std::thread::sleep(Duration::from_millis(100));
        }
        server
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// One HTTP request as a stand-in server reads it.
pub struct Request {
    pub path: String,
    /// Each header, its name lower-cased.
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl Request {
    /// The header `name`, written lower-case, or empty where the request has none.
    pub fn header(&self, name: &str) -> &str {
        self.headers.get(name).map_or("", String::as_str)
    }
}

/// The next request on `reader`, or `None` where the connection ends before its head does.
pub fn read_request(reader: &mut impl BufRead) -> Option<Request> {
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let path = line.split_whitespace().nth(1)?.to_owned();
    let mut headers = HashMap::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let Some((name, value)) = line.trim_end().split_once(':') else {
            break;
        };
        headers.insert(name.to_ascii_lowercase(), value.trim().to_owned());
    }
    let length = headers
        .get("content-length")
        .map_or(0, |length| length.parse().unwrap_or(0));
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    Some(Request {
        path,
        headers,
        body,
    })
}
