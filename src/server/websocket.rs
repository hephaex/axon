//! WebSocket handler for real-time streaming responses

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::protocol::{AgentId, LlmMessage};

use super::state::ServerState;

/// WebSocket request message
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsRequest {
    /// Send a message with streaming response
    SendStream {
        from: String,
        to: String,
        content: String,
        #[serde(default)]
        conversation_id: Option<Uuid>,
    },
    /// Ping to keep connection alive
    Ping,
}

/// WebSocket response message
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsResponse {
    /// Streaming content chunk
    Chunk {
        conversation_id: Uuid,
        delta: String,
        is_final: bool,
    },
    /// Complete response
    Complete {
        conversation_id: Uuid,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        usage: Option<WsUsage>,
    },
    /// Error response
    Error {
        message: String,
    },
    /// Pong response
    Pong,
}

/// Usage information
#[derive(Debug, Serialize)]
pub struct WsUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

/// WebSocket upgrade handler
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<ServerState>,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Handle WebSocket connection
async fn handle_socket(socket: WebSocket, state: ServerState) {
    let (mut sender, mut receiver) = socket.split();

    // Channel for sending messages back to the client
    let (tx, mut rx) = mpsc::channel::<WsResponse>(32);

    // Spawn task to forward messages to WebSocket
    let send_task = tokio::spawn(async move {
        while let Some(response) = rx.recv().await {
            if let Ok(json) = serde_json::to_string(&response) {
                if sender.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
        }
    });

    // Process incoming messages
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                if let Err(e) = handle_message(&text, &state, &tx).await {
                    let _ = tx
                        .send(WsResponse::Error {
                            message: e.to_string(),
                        })
                        .await;
                }
            }
            Ok(Message::Close(_)) => break,
            Err(_) => break,
            _ => {} // Ignore other message types
        }
    }

    // Clean up
    drop(tx);
    let _ = send_task.await;
}

/// Handle a single WebSocket message
async fn handle_message(
    text: &str,
    state: &ServerState,
    tx: &mpsc::Sender<WsResponse>,
) -> crate::Result<()> {
    let request: WsRequest = serde_json::from_str(text).map_err(|e| {
        crate::error::AxonError::config(format!("Invalid WebSocket message: {}", e))
    })?;

    match request {
        WsRequest::SendStream {
            from,
            to,
            content,
            conversation_id,
        } => {
            let conversation_id = conversation_id.unwrap_or_else(Uuid::new_v4);
            let target_id: AgentId = to.clone().into();

            // Get the adapter
            let adapter = state.get_adapter(&target_id).await.ok_or_else(|| {
                crate::error::AxonError::agent(&to, "Agent not found")
            })?;

            // Create the message
            let message =
                LlmMessage::chat(from.clone(), Some(target_id), &content, conversation_id);

            // Try streaming if supported
            let streaming_adapter = adapter.as_streaming();

            if let Some(streaming) = streaming_adapter {
                // Stream the response
                let mut stream = streaming.process_stream(&message).await?;

                let mut full_content = String::new();
                let mut final_usage = None;

                while let Some(chunk_result) = stream.next().await {
                    match chunk_result {
                        Ok(chunk) => {
                            full_content.push_str(&chunk.delta);

                            // Send chunk to client
                            let _ = tx
                                .send(WsResponse::Chunk {
                                    conversation_id,
                                    delta: chunk.delta,
                                    is_final: chunk.is_final,
                                })
                                .await;

                            if let Some(usage) = chunk.usage {
                                final_usage = Some(WsUsage {
                                    input_tokens: usage.input_tokens,
                                    output_tokens: usage.output_tokens,
                                });
                            }
                        }
                        Err(e) => {
                            let _ = tx
                                .send(WsResponse::Error {
                                    message: e.to_string(),
                                })
                                .await;
                            break;
                        }
                    }
                }

                // Send completion
                let _ = tx
                    .send(WsResponse::Complete {
                        conversation_id,
                        content: full_content,
                        usage: final_usage,
                    })
                    .await;
            } else {
                // Fall back to non-streaming
                let response = adapter.process(&message).await?;

                let _ = tx
                    .send(WsResponse::Complete {
                        conversation_id,
                        content: response.content.as_text(),
                        usage: None,
                    })
                    .await;
            }
        }
        WsRequest::Ping => {
            let _ = tx.send(WsResponse::Pong).await;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ws_request_deserialize() {
        let json = r#"{"type": "ping"}"#;
        let req: WsRequest = serde_json::from_str(json).unwrap();
        assert!(matches!(req, WsRequest::Ping));
    }

    #[test]
    fn test_ws_request_send_stream() {
        let json = r#"{
            "type": "send_stream",
            "from": "user",
            "to": "claude",
            "content": "Hello"
        }"#;
        let req: WsRequest = serde_json::from_str(json).unwrap();
        if let WsRequest::SendStream { from, to, content, .. } = req {
            assert_eq!(from, "user");
            assert_eq!(to, "claude");
            assert_eq!(content, "Hello");
        } else {
            panic!("Expected SendStream");
        }
    }

    #[test]
    fn test_ws_response_serialize() {
        let response = WsResponse::Chunk {
            conversation_id: Uuid::nil(),
            delta: "Hello".to_string(),
            is_final: false,
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("chunk"));
        assert!(json.contains("Hello"));
    }
}
