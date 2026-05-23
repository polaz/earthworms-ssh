use std::{
    collections::HashSet,
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use axum::{
    Router,
    extract::{
        Path, Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::header,
    response::{Html, IntoResponse},
    routing::get,
};
use serde::Deserialize;
use tokio::sync::{
    Mutex,
    mpsc::{self, UnboundedSender},
};
use tracing::info;

use crate::game::{ColorDepth, Event, GlyphMode, TICK_RATE};

const INDEX_HTML: &str = include_str!("../static/index.html");
const PULSE_MS: u64 = 1000 / TICK_RATE;

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
#[serde(untagged)]
enum ClientMsg {
    Down { down: String },
    Up { up: String },
    Press { press: String },
    Resize { resize: ResizeMsg },
}

#[derive(Deserialize)]
struct ResizeMsg {
    cols: u32,
    rows: u32,
}

pub async fn run(
    events: UnboundedSender<Event>,
    next_id: Arc<AtomicU64>,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    let state = AppState { events, next_id };
    let app = Router::new()
        .route(
            "/",
            get(|| async { ([(header::CACHE_CONTROL, "no-store")], Html(INDEX_HTML)) }),
        )
        .route("/ws/:name", get(ws_handler))
        .layer(tower_http::compression::CompressionLayer::new())
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
    if socket
        .send(Message::Text("\x1b[?25l\x1b[2J\x1b[H".to_string()))
        .await
        .is_err()
    {
        return;
    }
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

    let held: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    let pulse_held = held.clone();
    let pulse_events = state.events.clone();
    let pulse = tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_millis(PULSE_MS));
        loop {
            tick.tick().await;
            let snapshot: Vec<String> = {
                let guard = pulse_held.lock().await;
                guard.iter().cloned().collect()
            };
            if snapshot.is_empty() {
                continue;
            }
            let mut bytes = Vec::with_capacity(snapshot.iter().map(|s| s.len()).sum());
            for key in snapshot {
                bytes.extend_from_slice(key.as_bytes());
            }
            if pulse_events
                .send(Event::Input { id, input: bytes })
                .is_err()
            {
                break;
            }
        }
    });

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
                        if let Ok(msg) = serde_json::from_str::<ClientMsg>(&t) {
                            match msg {
                                ClientMsg::Down { down } => {
                                    let _ = state.events.send(Event::Input {
                                        id,
                                        input: down.as_bytes().to_vec(),
                                    });
                                    held.lock().await.insert(down);
                                }
                                ClientMsg::Up { up } => {
                                    held.lock().await.remove(&up);
                                }
                                ClientMsg::Press { press } => {
                                    let _ = state.events.send(Event::Input {
                                        id,
                                        input: press.into_bytes(),
                                    });
                                }
                                ClientMsg::Resize { resize } => {
                                    let _ = state.events.send(Event::Resize {
                                        id,
                                        columns: resize.cols.clamp(20, 400),
                                        rows: resize.rows.clamp(8, 120),
                                    });
                                }
                            }
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
    pulse.abort();
    let _ = state.events.send(Event::Leave(id));
}
