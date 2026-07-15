use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State},
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use tauri::async_runtime::JoinHandle;
use tokio::sync::broadcast;

const GLOBAL_CSS: &str = include_str!("../../src/assets/global.css");
const OVERLAY_CSS: &str = include_str!("../../src/assets/overlay-window.css");
const OVERLAY_HTML_TEMPLATE: &str = include_str!("../web-overlay/index.html");

async fn index() -> impl IntoResponse {
    let css = format!("{}\n{}", GLOBAL_CSS, OVERLAY_CSS);
    let html = OVERLAY_HTML_TEMPLATE.replacen("<!--STYLES-->", &css, 1);
    Html(html)
}

#[derive(Clone, Serialize)]
#[serde(tag = "status", content = "message")]
pub enum OverlayBrowserStatus {
    Stopped,
    Starting,
    Running,
    Error(String),
}

#[derive(Clone)]
pub struct OverlayBroadcast {
    tx: broadcast::Sender<String>,
    last: Arc<Mutex<Option<String>>>,
    handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    status: Arc<Mutex<OverlayBrowserStatus>>,
    app: AppHandle,
}

impl OverlayBroadcast {
    pub fn new(app: AppHandle) -> Self {
        let (tx, _rx) = broadcast::channel(32);
        Self {
            tx,
            last: Arc::new(Mutex::new(None)),
            handle: Arc::new(Mutex::new(None)),
            status: Arc::new(Mutex::new(OverlayBrowserStatus::Stopped)),
            app,
        }
    }

    pub fn push(&self, json: String) {
        *self.last.lock().unwrap() = Some(json.clone());
        let _ = self.tx.send(json);
    }

    pub fn get_status(&self) -> OverlayBrowserStatus {
        self.status.lock().unwrap().clone()
    }

    fn set_status(&self, status: OverlayBrowserStatus) {
        *self.status.lock().unwrap() = status.clone();
        let _ = self.app.emit("overlay-browser-status", status);
    }

    pub fn stop(&self) {
        if let Some(handle) = self.handle.lock().unwrap().take() {
            handle.abort();
        }
        self.set_status(OverlayBrowserStatus::Stopped);
    }

    pub fn start_at(&self, addr: String) {
        if let Some(h) = self.handle.lock().unwrap().take() {
            h.abort();
        }

        self.set_status(OverlayBrowserStatus::Starting);

        let state = self.clone();
        let jh = tauri::async_runtime::spawn(async move {
            let router = Router::new()
                .route("/", get(index))
                .route("/ws", get(ws_handler))
                .with_state(state.clone());

            let listener = match tokio::net::TcpListener::bind(&addr).await {
                Ok(l) => l,
                Err(e) => {
                    state.set_status(OverlayBrowserStatus::Error(e.to_string()));
                    return;
                }
            };

            state.set_status(OverlayBrowserStatus::Running);
            let _ = axum::serve(listener, router).await;
            state.set_status(OverlayBrowserStatus::Stopped);
        });
        *self.handle.lock().unwrap() = Some(jh);
    }
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<OverlayBroadcast>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: OverlayBroadcast) {
    let initial_snap: Option<String> = {
        let guard = state.last.lock().unwrap();
        guard.clone()
    };

    if let Some(snap) = initial_snap {
        let _ = socket.send(Message::Text(snap)).await;
    }

    let mut rx = state.tx.subscribe();
    while let Ok(msg) = rx.recv().await {
        if socket.send(Message::Text(msg)).await.is_err() {
            break;
        }
    }
}