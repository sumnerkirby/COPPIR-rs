//! COPPIR: a Common Operational Picture for incident response exercises.
//!
//! The crate is a local web server (axum) plus, with the `desktop` feature, a
//! native window (wry) pointed at it. The Svelte frontend in `web/` is built
//! to `web/dist/` and embedded in the binary.

pub mod assets;
pub mod auth;
pub mod config;
#[cfg(feature = "desktop")]
pub mod desktop;
pub mod error;
pub mod exercise;
pub mod guard;
pub mod live;
pub mod room;
pub mod routes;
pub mod state;
pub mod store;
pub mod validate;

use std::{future::Future, io, net::SocketAddr};

use tokio::net::TcpListener;

pub use config::Config;
pub use state::AppState;

/// Bind on loopback. If `port` is taken and `fallback` is set, take any free
/// port instead — a second copy of the app should still start rather than
/// quietly attaching its window to the first copy's server.
pub async fn bind(port: u16, fallback: bool) -> io::Result<TcpListener> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    match TcpListener::bind(addr).await {
        Err(e) if fallback && port != 0 && e.kind() == io::ErrorKind::AddrInUse => {
            tracing::warn!("port {port} is in use; picking a free one");
            TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).await
        }
        other => other,
    }
}

/// Serve until `shutdown` resolves.
pub async fn serve(
    listener: TcpListener,
    config: Config,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> io::Result<()> {
    let store = match &config.database {
        Some(path) => {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            store::Store::open(path)
        }
        None => store::Store::open_in_memory(),
    }
    .map_err(io::Error::other)?;
    let state = AppState::new(config, store);
    let app = routes::router(state.clone());
    let _ = state.port.set(listener.local_addr()?.port());
    let _ = state.router.set(app.clone());
    // Handlers need the peer address: being on this machine is what makes
    // someone the facilitator.
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(shutdown)
        .await
}
