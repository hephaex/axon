//! HTTP API handlers for the Axon server

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::protocol::{AgentConfig, AgentId, LlmMessage, Provider};

use super::state::ServerState;
use super::websocket::ws_handler;

/// API response wrapper
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(error: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error.into()),
        }
    }
}

/// Request to send a message
#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    pub from: String,
    #[serde(default)]
    pub to: Option<String>,
    pub content: String,
    #[serde(default)]
    pub conversation_id: Option<Uuid>,
}

/// Response with message
#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub id: Uuid,
    pub from: String,
    pub to: Option<String>,
    pub content: String,
    pub conversation_id: Uuid,
    pub timestamp: String,
}

impl From<LlmMessage> for MessageResponse {
    fn from(msg: LlmMessage) -> Self {
        Self {
            id: msg.id,
            from: msg.from.to_string(),
            to: msg.to.map(|a| a.to_string()),
            content: msg.content.as_text(),
            conversation_id: msg.conversation_id,
            timestamp: msg.timestamp.to_rfc3339(),
        }
    }
}

/// Request to register an agent
#[derive(Debug, Deserialize)]
pub struct RegisterAgentRequest {
    pub id: String,
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
}

/// Agent info response
#[derive(Debug, Serialize)]
pub struct AgentInfo {
    pub id: String,
    pub provider: String,
    pub model: String,
}

/// Router stats response
#[derive(Debug, Serialize)]
pub struct StatsResponse {
    pub registered_agents: usize,
    pub active_conversations: usize,
    pub total_conversations: usize,
}

/// Create the API router
pub fn create_router(state: ServerState) -> Router {
    Router::new()
        // Health check
        .route("/health", get(health_check))
        // Message endpoints
        .route("/api/send", post(send_message))
        // Agent endpoints
        .route("/api/agents", get(list_agents))
        .route("/api/agents", post(register_agent))
        .route("/api/agents/:id", delete(remove_agent))
        // Stats endpoint
        .route("/api/stats", get(get_stats))
        // WebSocket endpoint
        .route("/ws", get(ws_handler))
        .with_state(state)
}

/// Health check endpoint
async fn health_check() -> Json<ApiResponse<&'static str>> {
    Json(ApiResponse::ok("healthy"))
}

/// Send a message to an agent
async fn send_message(
    State(state): State<ServerState>,
    Json(req): Json<SendMessageRequest>,
) -> (StatusCode, Json<ApiResponse<MessageResponse>>) {
    let conversation_id = req.conversation_id.unwrap_or_else(Uuid::new_v4);
    let to: Option<AgentId> = req.to.as_ref().map(|s| s.clone().into());

    let message = LlmMessage::chat(req.from.clone(), to.clone(), &req.content, conversation_id);

    // Get the adapter for the target agent
    let result = if let Some(target_id) = to {
        if let Some(adapter) = state.get_adapter(&target_id).await {
            adapter.process(&message).await
        } else {
            return (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::err(format!(
                    "Agent '{}' not found",
                    target_id
                ))),
            );
        }
    } else {
        // Broadcast - use router
        let router = state.router().await;
        router.send(message.clone()).await
    };

    match result {
        Ok(response) => (StatusCode::OK, Json(ApiResponse::ok(response.into()))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::err(e.to_string())),
        ),
    }
}

/// List all registered agents
async fn list_agents(State(state): State<ServerState>) -> Json<ApiResponse<Vec<AgentInfo>>> {
    let agents = state.list_adapters().await;
    let agent_infos: Vec<AgentInfo> = agents
        .into_iter()
        .map(|id| AgentInfo {
            id: id.to_string(),
            provider: "unknown".to_string(), // Would need to store this info
            model: "unknown".to_string(),
        })
        .collect();

    Json(ApiResponse::ok(agent_infos))
}

/// Register a new agent
async fn register_agent(
    State(state): State<ServerState>,
    Json(req): Json<RegisterAgentRequest>,
) -> (StatusCode, Json<ApiResponse<AgentInfo>>) {
    let provider = match req.provider.to_lowercase().as_str() {
        "anthropic" | "claude" => Provider::Anthropic,
        "google" | "gemini" => Provider::Google,
        "openai" | "gpt" => Provider::OpenAi,
        "ollama" | "local" => Provider::Ollama,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::err(format!(
                    "Unknown provider: {}. Valid: anthropic, google, openai, ollama",
                    req.provider
                ))),
            );
        }
    };

    // Build the adapter
    let config = AgentConfig::new(req.id.clone(), provider, req.model.clone());

    let adapter_result = if let Some(ref api_key) = req.api_key {
        crate::adapters::AdapterBuilder::new(config)
            .system_prompt(req.system_prompt.unwrap_or_default())
            .api_key(api_key)
            .build()
    } else {
        crate::adapters::AdapterBuilder::new(config)
            .system_prompt(req.system_prompt.unwrap_or_default())
            .build()
    };

    match adapter_result {
        Ok(adapter) => {
            let agent_id: AgentId = req.id.clone().into();
            state.register_adapter(agent_id, adapter).await;

            (
                StatusCode::CREATED,
                Json(ApiResponse::ok(AgentInfo {
                    id: req.id,
                    provider: req.provider,
                    model: req.model,
                })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::err(e.to_string())),
        ),
    }
}

/// Remove an agent
async fn remove_agent(
    State(state): State<ServerState>,
    Path(id): Path<String>,
) -> (StatusCode, Json<ApiResponse<()>>) {
    let agent_id: AgentId = id.clone().into();

    if state.remove_adapter(&agent_id).await {
        (StatusCode::OK, Json(ApiResponse::ok(())))
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(ApiResponse::err(format!("Agent '{}' not found", id))),
        )
    }
}

/// Get router stats
async fn get_stats(State(state): State<ServerState>) -> Json<ApiResponse<StatsResponse>> {
    let router = state.router().await;
    let stats = router.stats().await;

    Json(ApiResponse::ok(StatsResponse {
        registered_agents: stats.registered_agents,
        active_conversations: stats.active_conversations,
        total_conversations: stats.total_conversations,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_response_ok() {
        let response: ApiResponse<&str> = ApiResponse::ok("test");
        assert!(response.success);
        assert_eq!(response.data, Some("test"));
        assert!(response.error.is_none());
    }

    #[test]
    fn test_api_response_err() {
        let response: ApiResponse<()> = ApiResponse::err("error message");
        assert!(!response.success);
        assert!(response.data.is_none());
        assert_eq!(response.error, Some("error message".to_string()));
    }
}
