use axum::{
    extract::{
        ws::{Message, WebSocket},
        Path, State, WebSocketUpgrade,
    },
    response::Response,
    routing::get,
    Router,
};
use futures::{SinkExt, StreamExt};
use uuid::Uuid;

use crate::server::AppState;

/// WebSocket handler for collaboration
async fn ws_handler(
    State(state): State<AppState>,
    Path(workbook_id): Path<Uuid>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state, workbook_id))
}

async fn handle_socket(socket: WebSocket, state: AppState, workbook_id: Uuid) {
    let (mut sender, mut receiver) = socket.split();

    // Get or create the document
    let doc = state.docs.get_or_create(workbook_id).await;

    // Send initial state to the client
    {
        let doc_guard = doc.read().await;
        let initial_state = doc_guard.encode_state();
        if let Err(e) = sender.send(Message::Binary(initial_state.into())).await {
            tracing::error!("Failed to send initial state: {}", e);
            return;
        }
    }

    // Subscribe to updates from other clients
    let mut update_rx = {
        let doc_guard = doc.read().await;
        doc_guard.subscribe()
    };

    // Spawn task to forward updates to this client
    let doc_clone = doc.clone();
    let mut send_task = tokio::spawn(async move {
        while let Ok(update) = update_rx.recv().await {
            if sender.send(Message::Binary(update.into())).await.is_err() {
                break;
            }
        }
    });

    // Handle incoming messages from this client
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Binary(data) => {
                    let doc_guard = doc_clone.read().await;

                    // Apply the update
                    if let Err(e) = doc_guard.apply_update(&data) {
                        tracing::error!("Failed to apply update: {}", e);
                        continue;
                    }

                    // Broadcast to other clients
                    doc_guard.broadcast(data.to_vec());
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // Wait for either task to complete
    tokio::select! {
        _ = &mut send_task => {
            recv_task.abort();
        }
        _ = &mut recv_task => {
            send_task.abort();
        }
    }

    tracing::debug!("WebSocket connection closed for workbook {}", workbook_id);
}

pub fn router() -> Router<AppState> {
    Router::new().route("/ws/{workbook_id}", get(ws_handler))
}
