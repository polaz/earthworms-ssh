use std::{
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use axum::{
    Router,
    extract::{
        Path, Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::{Html, IntoResponse},
    routing::get,
};
use serde::Deserialize;
use tokio::sync::mpsc::{self, UnboundedSender};
use tracing::info;

use crate::game::{ColorDepth, Event, GlyphMode};

const INDEX_HTML: &str = include_str!("../static/index.html");

#[derive(Clone)]
struct AppState {
    events: UnboundedSender<Event>,
    next_id: Arc<AtomicU64>,
}

#[derive(Deserialize, Default)]
struct WsQuery {
    cols: Option<u32>,
    rows: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum ClientMsg {
    Resize { cols: u32, rows: u32 },
}

pub async fn run(
    events: UnboundedSender<Event>,
    next_id: Arc<AtomicU64>,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    let state = AppState { events, next_id };
    let app = Router::new()
        .route("/", get(|| async { Html(INDEX_HTML) }))
        .route("/ws/:name", get(ws_handler))
        .with_state(state);
    info!(address = %addr, "web client serving on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(name): Path<String>,
    Query(q): Query<WsQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, name, q, state))
}

async fn handle_socket(mut socket: WebSocket, name: String, q: WsQuery, state: AppState) {
    let id = state.next_id.fetch_add(1, Ordering::Relaxed);
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let cols = q.cols.unwrap_or(100).clamp(20, 400);
    let rows = q.rows.unwrap_or(30).clamp(8, 120);
    if state
        .events
        .send(Event::Join {
            id,
            username: name,
            frames: tx,
            columns: cols,
            rows,
            colors: ColorDepth::TrueColor,
            glyphs: GlyphMode::Ascii,
        })
        .is_err()
    {
        return;
    }

    loop {
        tokio::select! {
            biased;
            maybe_frame = rx.recv() => {
                match maybe_frame {
                    Some(frame) => {
                        if socket.send(Message::Text(frame)).await.is_err() {
                            break;
                        }
                    }
                    None => break,
                }
            }
            maybe_msg = socket.recv() => {
                match maybe_msg {
                    Some(Ok(Message::Text(t))) => {
                        if let Ok(ClientMsg::Resize { cols, rows }) = serde_json::from_str::<ClientMsg>(&t) {
                            let _ = state.events.send(Event::Resize {
                                id,
                                columns: cols.clamp(20, 400),
                                rows: rows.clamp(8, 120),
                            });
                        } else {
                            let _ = state.events.send(Event::Input { id, input: t.into_bytes() });
                        }
                    }
                    Some(Ok(Message::Binary(b))) => {
                        let _ = state.events.send(Event::Input { id, input: b });
                    }
                    Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                    _ => {}
                }
            }
        }
    }
    let _ = state.events.send(Event::Leave(id));
}
