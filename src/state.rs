//! Shared application state handed to every handler, and small helpers.

use std::sync::Arc;

#[derive(Clone)]
pub struct AppState(Arc<Inner>);

pub struct Inner {
    pub config: crate::config::Config,
    pub http: reqwest::Client,
    pub live: crate::live::Live,
    pub rooms: crate::room::Rooms,
    pub maps: Arc<crate::maps::Maps>,
    /// The assembled app, so the room can serve it on LAN listeners too.
    pub router: std::sync::OnceLock<axum::Router>,
    /// The port the loopback listener is on; LAN listeners try the same one.
    pub port: std::sync::OnceLock<u16>,
}

impl AppState {
    pub fn new(config: crate::config::Config, store: crate::store::Store) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(crate::config::USER_AGENT)
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("the HTTP client builds with static settings");
        let maps = Arc::new(crate::maps::Maps::new(config.maps_dir.clone(), config.tiles_url.clone()));
        Self(Arc::new(Inner {
            maps,
            config,
            http,
            live: crate::live::Live::new(store),
            rooms: Default::default(),
            router: Default::default(),
            port: Default::default(),
        }))
    }
}

impl std::ops::Deref for AppState {
    type Target = Inner;
    fn deref(&self) -> &Inner {
        &self.0
    }
}

/// The current local time, ISO 8601 with an offset.
pub fn now() -> String {
    chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, false)
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}
