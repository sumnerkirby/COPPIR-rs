//! The websocket every client holds open for live updates.
//!
//! The server only pushes. On connect it sends `full_state`, then every event
//! as it happens. A client that falls too far behind is handed a fresh
//! `full_state` rather than silently missing events.

use axum::{
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::Response,
};
use tokio::sync::broadcast::error::RecvError;

use crate::state::AppState;

pub async fn upgrade(ws: WebSocketUpgrade, State(app): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| client(socket, app))
}

async fn client(mut socket: WebSocket, app: AppState) {
    let (snapshot, mut rx) = app.hub.subscribe();
    if socket.send(Message::Text(snapshot)).await.is_err() {
        return;
    }
    tracing::debug!(clients = app.hub.client_count(), "websocket connected");

    loop {
        tokio::select! {
            event = rx.recv() => {
                let text = match event {
                    Ok(text) => text,
                    Err(RecvError::Lagged(skipped)) => {
                        tracing::warn!(skipped, "websocket client fell behind; resyncing");
                        let (snapshot, fresh) = app.hub.subscribe();
                        rx = fresh;
                        snapshot
                    }
                    Err(RecvError::Closed) => break,
                };
                if socket.send(Message::Text(text)).await.is_err() {
                    break;
                }
            }
            incoming = socket.recv() => match incoming {
                // Anything the client sends is ignored; we only watch for it leaving.
                Some(Ok(Message::Close(_)) | Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
        }
    }
    tracing::debug!("websocket disconnected");
}
