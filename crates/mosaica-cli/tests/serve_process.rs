//! `mosaica serve` as a process, and `mosaica health` asking it whether it is ready: what a
//! container runs and what its health check runs.

mod common;

use std::io::BufRead;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use common::{
    deployment, deployment_with_control, healthy, mosaica, Ports, Server, OPERATOR_CREDENTIAL,
};

#[test]
fn health_follows_the_server_and_sigterm_stops_it_cleanly() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();
    deployment(dir, &Ports::free());

    assert!(!healthy(dir), "nothing is serving yet");

    let mut server = Server::start(dir);

    let signalled = Command::new("kill")
        .args(["-TERM", &server.0.id().to_string()])
        .status()
        .unwrap();
    assert!(signalled.success());
    let stopping = Instant::now();
    let status = loop {
        if let Some(status) = server.0.try_wait().unwrap() {
            break status;
        }
        assert!(
            stopping.elapsed() < Duration::from_secs(10),
            "the server did not exit within ten seconds of SIGTERM"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(status.success(), "SIGTERM ends serve with {status}");
    assert!(!healthy(dir), "nothing is serving after the stop");
}

/// `mosaica serve --deployment <dir>/mosaica.toml` with `flags`, its standard input a pipe held
/// here, once it has announced its addresses.
fn piped(dir: &Path, flags: &[&str]) -> Child {
    piped_with(dir, flags, Stdio::null())
}

/// [`piped`], with standard error going to `stderr`.
fn piped_with(dir: &Path, flags: &[&str], stderr: Stdio) -> Child {
    let mut child = mosaica()
        .arg("serve")
        .arg("--deployment")
        .arg(dir.join("mosaica.toml"))
        .args(flags)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(stderr)
        .spawn()
        .unwrap();
    let mut line = String::new();
    std::io::BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    assert!(line.contains("listening"), "serve announced {line:?}");
    child
}

/// Close the server's standard input, and the status it exits with within ten seconds.
fn close_stdin(mut child: Child) -> ExitStatus {
    drop(child.stdin.take());
    let closing = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        if closing.elapsed() > Duration::from_secs(10) {
            let _ = child.kill();
            panic!("the server did not exit within ten seconds of its standard input closing");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn principal(control: &str, args: &[&str]) -> bool {
    mosaica()
        .arg("principal")
        .args(args)
        .args(["--control", control])
        .env("MOSAICA_CREDENTIAL", OPERATOR_CREDENTIAL)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .success()
}

#[test]
fn the_end_of_stdin_stops_the_server_and_what_it_wrote_survives_a_restart() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();
    let control = format!("unix:{}", dir.join("control.sock").display());
    deployment_with_control(dir, &Ports::chosen(), &control);

    let server = piped(dir, &["--exit-on-stdin-close"]);
    assert!(principal(&control, &["create", "ann", "--kind", "person"]));
    assert!(close_stdin(server).success());
    assert!(
        dir.join("mosaica.toml").exists(),
        "only --remove-on-exit removes the deployment"
    );

    let again = piped(dir, &["--exit-on-stdin-close"]);
    assert!(
        principal(&control, &["show", "ann"]),
        "the principal written before the stop"
    );
    assert!(close_stdin(again).success());
}

#[test]
fn the_server_stops_after_the_process_reading_its_stderr_has_gone() {
    // A starting process that dies closes the server's stderr with its stdin. Writing the stop's
    // note must not stop the stop.
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path();
    deployment(dir, &Ports::chosen());
    let mut server = piped_with(dir, &["--exit-on-stdin-close"], Stdio::piped());
    drop(server.stderr.take());
    assert!(close_stdin(server).success());
}
