//! The HTTP the identity and write verbs share: where a request goes, one request and its answer,
//! the control plane's credential, and printing an answer.
//!
//! Secrets are never arguments, which other users of the machine can read: the control plane's
//! credential is read from `MOSAICA_CREDENTIAL`.

use std::io::{BufRead, Read, Write};
use std::process::ExitCode;
use std::sync::OnceLock;
use std::time::Duration;

use clap::Args;
use serde_json::Value;

const TIMEOUT: Duration = Duration::from_secs(300);

/// How long one `429` may hold a request, in seconds. The server's own figure is clamped to this.
const MAX_BACKOFF: f64 = 30.0;

/// How many times a request is sent before its `429` is the answer.
const MAX_ATTEMPTS: u32 = 600;

/// Where a request goes: `http://host:port`, or `unix:<path>` for a control plane on a Unix
/// socket.
#[derive(Clone, Debug)]
pub(crate) enum Target {
    Http(String),
    Unix(std::path::PathBuf),
}

impl Target {
    pub(crate) fn parse(raw: &str) -> Result<Target, String> {
        if let Some(path) = raw.strip_prefix("unix:") {
            return Ok(Target::Unix(path.into()));
        }
        if raw.starts_with("http://") || raw.starts_with("https://") {
            return Ok(Target::Http(raw.trim_end_matches('/').to_owned()));
        }
        Err(format!(
            "`{raw}` is not an address; write `http://<host>:<port>`, or `unix:<path>` for a Unix \
             socket"
        ))
    }
}

/// One request's answer.
pub(crate) struct Answer {
    pub(crate) status: u16,
    pub(crate) body: Vec<u8>,
    /// The `retry-after` header, where the answer has one.
    retry_after: Option<String>,
}

impl Answer {
    pub(crate) fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// The body as JSON, or `null` where it is not JSON.
    pub(crate) fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or(Value::Null)
    }

    pub(crate) fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).trim_end().to_owned()
    }
}

/// A request body and its content type.
pub(crate) type Body<'a> = (&'a str, &'a [u8]);

/// One request with a JSON body, or none.
pub(crate) fn send_json(
    target: &Target,
    method: &str,
    path: &str,
    bearer: Option<&str>,
    body: Option<&Value>,
) -> Result<Answer, String> {
    let bytes = body.map(Value::to_string);
    let body = bytes
        .as_deref()
        .map(|bytes| ("application/json", bytes.as_bytes()));
    send(target, method, path, bearer, &[], body)
}

/// One request, sent once.
pub(crate) fn send(
    target: &Target,
    method: &str,
    path: &str,
    bearer: Option<&str>,
    headers: &[(&str, &str)],
    body: Option<Body>,
) -> Result<Answer, String> {
    match target {
        Target::Http(base) => {
            static CLIENT: OnceLock<Result<reqwest::blocking::Client, String>> = OnceLock::new();
            let client = CLIENT
                .get_or_init(|| {
                    reqwest::blocking::Client::builder()
                        .timeout(TIMEOUT)
                        .build()
                        .map_err(|e| format!("starting the HTTP client: {e}"))
                })
                .as_ref()
                .map_err(Clone::clone)?;
            let method = reqwest::Method::from_bytes(method.as_bytes()).expect("a known method");
            let mut req = client.request(method, format!("{base}{path}"));
            if let Some(bearer) = bearer {
                req = req.bearer_auth(bearer);
            }
            for (name, value) in headers {
                req = req.header(*name, *value);
            }
            if let Some((content_type, bytes)) = body {
                req = req
                    .header(reqwest::header::CONTENT_TYPE, content_type)
                    .body(bytes.to_vec());
            }
            let resp = req.send().map_err(|e| format!("{base}{path}: {e}"))?;
            let status = resp.status().as_u16();
            let retry_after = resp
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);
            let bytes = resp.bytes().map_err(|e| format!("{base}{path}: {e}"))?;
            Ok(Answer {
                status,
                body: bytes.to_vec(),
                retry_after,
            })
        }
        Target::Unix(socket) => unix_request(socket, method, path, bearer, headers, body),
    }
}

/// HTTP/1.1 over a Unix socket, one request per connection.
fn unix_request(
    socket: &std::path::Path,
    method: &str,
    path: &str,
    bearer: Option<&str>,
    headers: &[(&str, &str)],
    body: Option<Body>,
) -> Result<Answer, String> {
    let at = || format!("unix:{}", socket.display());
    let mut stream = std::os::unix::net::UnixStream::connect(socket)
        .map_err(|e| format!("connecting to {}: {e}", at()))?;
    stream.set_read_timeout(Some(TIMEOUT)).ok();
    let mut head = format!("{method} {path} HTTP/1.1\r\nhost: localhost\r\nconnection: close\r\n");
    if let Some(bearer) = bearer {
        head.push_str(&format!("authorization: Bearer {bearer}\r\n"));
    }
    for (name, value) in headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    let (content_type, bytes) = body.unwrap_or(("", &[]));
    if !content_type.is_empty() {
        head.push_str(&format!("content-type: {content_type}\r\n"));
    }
    head.push_str(&format!("content-length: {}\r\n\r\n", bytes.len()));
    stream
        .write_all(head.as_bytes())
        .and_then(|()| stream.write_all(bytes))
        .map_err(|e| format!("writing to {}: {e}", at()))?;
    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|e| format!("reading from {}: {e}", at()))?;
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| format!("{} answered without a complete header", at()))?;
    let headers = String::from_utf8_lossy(&raw[..split]).to_ascii_lowercase();
    let status = headers
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("{} answered without a status", at()))?;
    let retry_after = headers
        .lines()
        .find_map(|line| line.strip_prefix("retry-after:"))
        .map(|value| value.trim().to_owned());
    let rest = &raw[split + 4..];
    let body = if headers.contains("transfer-encoding: chunked") {
        dechunk(rest).ok_or_else(|| format!("{} answered a malformed chunked body", at()))?
    } else {
        rest.to_vec()
    };
    Ok(Answer {
        status,
        body,
        retry_after,
    })
}

fn dechunk(mut rest: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let end = rest.windows(2).position(|w| w == b"\r\n")?;
        let size = std::str::from_utf8(&rest[..end]).ok()?;
        let size = usize::from_str_radix(size.split(';').next()?.trim(), 16).ok()?;
        rest = &rest[end + 2..];
        if size == 0 {
            return Some(out);
        }
        out.extend_from_slice(rest.get(..size)?);
        rest = rest.get(size + 2..)?;
    }
}

/// A path segment, percent-encoded so a name holding `/` or a space reaches the route whole.
pub(crate) fn segment(raw: &str) -> String {
    raw.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

/// Prints the answer, or the refusal, and exits accordingly.
pub(crate) fn finish(verb: &str, answer: Result<Answer, String>) -> ExitCode {
    match answer {
        Ok(answer) if answer.ok() => {
            if !answer.body.is_empty() {
                println!("{}", answer.text());
            }
            ExitCode::SUCCESS
        }
        Ok(answer) => {
            eprintln!(
                "mosaica {verb}: the server answered {}: {}",
                answer.status,
                answer.text()
            );
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("mosaica {verb}: {e}");
            ExitCode::FAILURE
        }
    }
}

pub(crate) fn env_secret(name: &str, what: &str) -> Result<String, String> {
    std::env::var(name)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .map(|v| v.trim().to_owned())
        .ok_or_else(|| format!("no {what}: set {name}"))
}

/// The first line of stdin, trimmed of its line ending.
pub(crate) fn stdin_secret(what: &str) -> Result<String, String> {
    let mut line = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut line)
        .map_err(|e| format!("reading the {what} from stdin: {e}"))?;
    let line = line.trim_end_matches(['\r', '\n']).to_owned();
    if line.is_empty() {
        return Err(format!("no {what}: write it on stdin"));
    }
    Ok(line)
}

/// The control plane's address. The credential is read from `MOSAICA_CREDENTIAL`: the operator
/// credential, an API key or an OIDC access token.
#[derive(Args)]
pub(crate) struct Control {
    /// The control plane's address: `http://<host>:<port>`, or `unix:<path>` for a Unix socket.
    #[arg(long, value_name = "ADDRESS")]
    control: String,
}

impl Control {
    pub(crate) fn plane(&self) -> Result<Plane, String> {
        Plane::connect(&self.control)
    }

    /// One request with a JSON body, or none.
    pub(crate) fn call(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
    ) -> Result<Answer, String> {
        self.plane()?.call(method, path, body.as_ref())
    }
}

/// The control plane, with its credential read.
pub(crate) struct Plane {
    target: Target,
    credential: String,
}

impl Plane {
    pub(crate) fn connect(address: &str) -> Result<Plane, String> {
        Ok(Plane {
            target: Target::parse(address)?,
            credential: env_secret("MOSAICA_CREDENTIAL", "control-plane credential")?,
        })
    }

    /// One request with a JSON body, or none, sent again after a `429`.
    pub(crate) fn call(
        &self,
        method: &str,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Answer, String> {
        let bytes = body.map(Value::to_string);
        let body = bytes
            .as_deref()
            .map(|bytes| ("application/json", bytes.as_bytes()));
        self.send(method, path, &[], body)
    }

    /// One request, sent again with the same headers and bytes after a `429`, once the wait it
    /// names has passed: at most 30 seconds a wait and 600 attempts in all.
    pub(crate) fn send(
        &self,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: Option<Body>,
    ) -> Result<Answer, String> {
        let mut attempts = 1;
        loop {
            let answer = send(
                &self.target,
                method,
                path,
                Some(&self.credential),
                headers,
                body,
            )?;
            if answer.status != 429 || attempts >= MAX_ATTEMPTS {
                return Ok(answer);
            }
            attempts += 1;
            let wait = answer
                .retry_after
                .as_deref()
                .and_then(|s| s.parse::<f64>().ok())
                .or_else(|| answer.json()["retry_after_s"].as_f64())
                .unwrap_or(1.0)
                .clamp(0.0, MAX_BACKOFF);
            std::thread::sleep(Duration::from_secs_f64(wait));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chunked_body_is_joined_and_a_cut_one_refused() {
        assert_eq!(
            dechunk(b"3\r\nabc\r\n2;x=y\r\nde\r\n0\r\n\r\n").as_deref(),
            Some(&b"abcde"[..])
        );
        assert_eq!(dechunk(b"3\r\nab"), None);
    }

    #[test]
    fn a_segment_keeps_unreserved_bytes_and_encodes_the_rest() {
        assert_eq!(segment("ann.b-c_d~"), "ann.b-c_d~");
        assert_eq!(segment("a/b c"), "a%2Fb%20c");
    }
}
