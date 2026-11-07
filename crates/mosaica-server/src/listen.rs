//! The listeners `mosaica serve` binds. A plane with no address of its own is served on the
//! viewer's listener, and planes that name one address share one listener, so a deployment
//! declares one port, two or three. A port of 0 is a fresh port each time it is named.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;

use mosaica_config::{Config, ControlListen};

use crate::state::AppState;
use crate::BoxError;

/// The planes one listener serves.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Planes {
    pub viewer: bool,
    pub session: bool,
    pub control: bool,
}

impl Planes {
    fn names(self) -> String {
        let names: Vec<&str> = [
            (self.viewer, "viewer"),
            (self.session, "session"),
            (self.control, "control"),
        ]
        .into_iter()
        .filter_map(|(on, name)| on.then_some(name))
        .collect();
        match names.as_slice() {
            [one] => (*one).to_string(),
            [init @ .., last] => format!("{} and {last}", init.join(", ")),
            [] => String::new(),
        }
    }
}

pub(crate) enum Socket {
    Tcp(std::net::TcpListener),
    Unix(std::os::unix::net::UnixListener, PathBuf),
}

impl Socket {
    /// The address as the announce line names it.
    pub(crate) fn announced(&self) -> std::io::Result<String> {
        match self {
            Socket::Tcp(listener) => Ok(listener.local_addr()?.to_string()),
            Socket::Unix(_, path) => Ok(format!("unix:{}", path.display())),
        }
    }
}

/// Every listener a deployment declares, bound and not yet serving.
pub struct Listeners {
    pub(crate) bound: Vec<(Socket, Planes)>,
}

impl Listeners {
    /// The announced address of the listener serving the plane `on` picks.
    pub(crate) fn address_of(&self, on: impl Fn(Planes) -> bool) -> std::io::Result<String> {
        let (socket, _) = self
            .bound
            .iter()
            .find(|(_, planes)| on(*planes))
            .expect("every plane is on a listener");
        socket.announced()
    }
}

#[derive(PartialEq)]
enum Target {
    Tcp(SocketAddr),
    Unix(PathBuf),
}

/// The refusal for a deployment that names no viewer address.
pub(crate) fn no_viewer_address() -> BoxError {
    "this deployment declares no `viewer` address; add one under `[serve]`, such as \
     `viewer = \"127.0.0.1:9141\"`"
        .into()
}

/// Binds every address `config` declares, before anything slow runs, so a taken port refuses
/// at once. A taken port is tried again `port_retries` ports up, one at a time.
pub fn bind(config: &Config) -> Result<Listeners, BoxError> {
    let viewer = config.viewer_addr.ok_or_else(no_viewer_address)?;
    let mut wanted: Vec<(Target, Planes)> = vec![(
        Target::Tcp(viewer),
        Planes {
            viewer: true,
            ..Planes::default()
        },
    )];
    let session = config.session_addr.map(Target::Tcp);
    let control = config.control_listen.clone().map(|listen| match listen {
        ControlListen::Tcp(addr) => Target::Tcp(addr),
        ControlListen::Unix(path) => Target::Unix(path),
    });
    on(&mut wanted, session).session = true;
    on(&mut wanted, control).control = true;

    let mut bound = Vec::with_capacity(wanted.len());
    for (target, planes) in wanted {
        let socket = match target {
            Target::Tcp(addr) => Socket::Tcp(bind_tcp(addr, config.port_retries, planes)?),
            Target::Unix(path) => {
                let _ = std::fs::remove_file(&path);
                let listener = std::os::unix::net::UnixListener::bind(&path)?;
                listener.set_nonblocking(true)?;
                Socket::Unix(listener, path)
            }
        };
        bound.push((socket, planes));
    }
    Ok(Listeners { bound })
}

/// The planes of the listener for `target`: the viewer's when it is `None`, an earlier one naming
/// the same address and port, or a new one. A port of 0 is never shared.
fn on(wanted: &mut Vec<(Target, Planes)>, target: Option<Target>) -> &mut Planes {
    let at = match target {
        None => 0,
        Some(target) => {
            let shared = match &target {
                Target::Tcp(addr) if addr.port() == 0 => None,
                _ => wanted.iter().position(|(t, _)| *t == target),
            };
            shared.unwrap_or_else(|| {
                wanted.push((target, Planes::default()));
                wanted.len() - 1
            })
        }
    };
    &mut wanted[at].1
}

fn bind_tcp(
    addr: SocketAddr,
    retries: u16,
    planes: Planes,
) -> Result<std::net::TcpListener, BoxError> {
    let tries = if addr.port() == 0 { 0 } else { retries };
    let mut taken = None;
    for step in 0..=tries {
        let Some(port) = addr.port().checked_add(step) else {
            break;
        };
        match std::net::TcpListener::bind(SocketAddr::new(addr.ip(), port)) {
            Ok(listener) => {
                listener.set_nonblocking(true)?;
                return Ok(listener);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                taken.get_or_insert(e);
            }
            Err(e) => {
                return Err(format!(
                    "the {} address {addr} cannot be bound: {e}",
                    planes.names()
                )
                .into())
            }
        }
    }
    let what = planes.names();
    let e = taken.expect("a port was tried");
    Err(if tries == 0 {
        format!(
            "the {what} address {addr} cannot be bound: {e}; stop what is listening there, name \
             another port under `[serve]`, or set `port_retries` to try the ports above it"
        )
    } else {
        format!(
            "the {what} address {addr} cannot be bound: {e}, nor can any port up to {tries} above \
             it; stop what is listening there, name another port under `[serve]`, or raise \
             `port_retries`"
        )
    }
    .into())
}

/// The router one listener serves: each of its planes' routes, each behind its own plane's
/// credential, CORS and logging layers.
pub(crate) fn router(state: &Arc<AppState>, planes: Planes) -> Router {
    let mut router = Router::new();
    if planes.viewer {
        router = router.merge(crate::viewer::router(Arc::clone(state)));
    }
    if planes.session {
        // The viewer's router already answers `/healthz` and `/readyz` on a shared listener.
        router = router.merge(crate::session::routes(Arc::clone(state), !planes.viewer));
    }
    if planes.control {
        router = router.merge(crate::control::router(Arc::clone(state)));
    }
    router
}
