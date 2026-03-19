//! Claude (Anthropic) adapter
//!
//! Implements LlmAdapter for the Anthropic Claude API.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AxonError;
use crate::protocol::{AgentConfig, AgentId, LlmMessage, MessageContent, MessageType};
use crate::tools::ToolDefinition;
use crate::Result;

use super::LlmAdapter;

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const DEFAULT_MAX_TOKENS: u32 = 4096;

/// Claude adapter for Anthropic API
pub struct ClaudeAdapter {
    config: AgentConfig,
    client: Client,
    api_key: String,
    tools: Vec<ToolDefinition>,
}

impl ClaudeAdapter {
    /// Create a new Claude adapter
    pub fn new(config: AgentConfig) -> Result<Self> {
        let api_key = Self::get_api_key(&config)?;

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|e| AxonError::api(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            config,
            client,
            api_key,
            tools: Vec::new(),
        })
    }

    /// Get API key from environment
    fn get_api_key(config: &AgentConfig) -> Result<String> {
        let env_var = config
            .api_key_env
            .as_deref()
            .unwrap_or("ANTHROPIC_API_KEY");

        let api_key = std::env::var(env_var).map_err(|_| {
            AxonError::config(format!(
                "API key not found. Set {} environment variable.",
                env_var
            ))
        })?;

        if api_key.is_empty() {
            return Err(AxonError::config(format!(
                "API key is empty. Set {} environment variable.",
                env_var
            )));
        }

        Ok(api_key)
    }

    /// Convert LlmMessage to Anthropic request format
    fn to_anthropic_request(&self, message: &LlmMessage, history: &[LlmMessage]) -> AnthropicRequest {
        let mut messages = Vec::new();

        // Add history
        for msg in history {
            messages.push(self.convert_message(msg));
        }

        // Add current message
        messages.push(self.convert_message(message));

        AnthropicRequest {
            model: self.config.model.clone(),
            max_tokens: self.config.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS),
            system: self.config.system_prompt.clone(),
            messages,
            tools: if self.tools.is_empty() {
                None
            } else {
                Some(self.tools.iter().map(|t| AnthropicTool::from(t.clone())).collect())
            },
            temperature: self.config.temperature,
        }
    }

    /// Convert a single LlmMessage to Anthropic message format
    fn convert_message(&self, msg: &LlmMessage) -> AnthropicMessage {
        let role = if msg.from == self.config.id {
            "assistant"
        } else {
            "user"
        };

        let content = match &msg.content {
            MessageContent::Text(text) => vec![AnthropicContent::Text { text: text.clone() }],
            MessageContent::Json(value) => {
                vec![AnthropicContent::Text {
                    text: value.to_string(),
                }]
            }
            MessageContent::Parts(parts) => {
                parts
                    .iter()
                    .map(|p| match p {
                        crate::protocol::ContentPart::Text { text } => {
                            AnthropicContent::Text { text: text.clone() }
                        }
                        crate::protocol::ContentPart::Image { base64, media_type, .. } => {
                            AnthropicContent::Image {
                                source: ImageSource {
                                    r#type: "base64".to_string(),
                                    media_type: media_type.clone().unwrap_or_else(|| "image/png".to_string()),
                                    data: base64.clone().unwrap_or_default(),
                                },
                            }
                        }
                    })
                    .collect()
            }
        };

        AnthropicMessage {
            role: role.to_string(),
            content,
        }
    }

    /// Convert Anthropic response to LlmMessage
    fn parse_response(
        &self,
        response: AnthropicResponse,
        conversation_id: Uuid,
    ) -> Result<LlmMessage> {
        // Check for tool use
        for content in &response.content {
            if let AnthropicContent::ToolUse { id, name, input } = content {
                return Ok(LlmMessage {
                    id: Uuid::new_v4(),
                    from: self.config.id.clone(),
                    to: None,
                    message_type: MessageType::ToolCall {
                        tool: name.clone(),
                        call_id: Uuid::parse_str(id).unwrap_or_else(|_| Uuid::new_v4()),
                    },
                    content: MessageContent::Json(input.clone()),
                    conversation_id,
                    timestamp: chrono::Utc::now(),
                    metadata: None,
                });
            }
        }

        // Extract text content
        let text = response
            .content
            .iter()
            .filter_map(|c| {
                if let AnthropicContent::Text { text } = c {
                    Some(text.as_str())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        Ok(LlmMessage {
            id: Uuid::new_v4(),
            from: self.config.id.clone(),
            to: None,
            message_type: MessageType::Chat,
            content: MessageContent::Text(text),
            conversation_id,
            timestamp: chrono::Utc::now(),
            metadata: Some(serde_json::json!({
                "model": response.model,
                "stop_reason": response.stop_reason,
                "usage": response.usage,
            })),
        })
    }

    /// Call the Anthropic API
    async fn call_api(&self, request: &AnthropicRequest) -> Result<AnthropicResponse> {
        let response = self
            .client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(request)
            .send()
            .await
            .map_err(|e| AxonError::api(format!("Request failed: {}", e)))?;

        let status = response.status();

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse().ok())
                .unwrap_or(60);
            return Err(AxonError::RateLimited {
                retry_after_secs: retry_after,
            });
        }

        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AxonError::api(format!(
                "API error ({}): {}",
                status, error_text
            )));
        }

        response
            .json::<AnthropicResponse>()
            .await
            .map_err(|e| AxonError::api(format!("Failed to parse response: {}", e)))
    }
}

#[async_trait]
impl LlmAdapter for ClaudeAdapter {
    fn agent_id(&self) -> &AgentId {
        &self.config.id
    }

    fn config(&self) -> &AgentConfig {
        &self.config
    }

    async fn process(&self, message: &LlmMessage) -> Result<LlmMessage> {
        self.process_with_history(message, &[]).await
    }

    async fn process_with_history(
        &self,
        message: &LlmMessage,
        history: &[LlmMessage],
    ) -> Result<LlmMessage> {
        let request = self.to_anthropic_request(message, history);
        let response = self.call_api(&request).await?;
        self.parse_response(response, message.conversation_id)
    }

    fn available_tools(&self) -> Vec<ToolDefinition> {
        self.tools.clone()
    }

    fn register_tool(&mut self, tool: ToolDefinition) {
        self.tools.push(tool);
    }

    async fn health_check(&self) -> Result<bool> {
        // Simple health check - verify API key is set
        if self.api_key.is_empty() {
            return Ok(false);
        }
        Ok(true)
    }
}

// Anthropic API types

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<AnthropicTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: Vec<AnthropicContent>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicContent {
    Text {
        text: String,
    },
    Image {
        source: ImageSource,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct ImageSource {
    r#type: String,
    media_type: String,
    data: String,
}

#[derive(Debug, Serialize)]
struct AnthropicTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

impl From<ToolDefinition> for AnthropicTool {
    fn from(tool: ToolDefinition) -> Self {
        Self {
            name: tool.name,
            description: tool.description,
            input_schema: tool.parameters,
        }
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AnthropicResponse {
    id: String,
    model: String,
    content: Vec<AnthropicContent>,
    stop_reason: Option<String>,
    usage: Usage,
}

#[derive(Debug, Serialize, Deserialize)]
struct Usage {
    input_tokens: u32,
    output_tokens: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::Provider;

    const TEST_MODEL: &str = "claude-sonnet-4-20250514";

    fn test_config() -> AgentConfig {
        AgentConfig::new("claude-test", Provider::Anthropic, TEST_MODEL)
    }

    #[test]
    fn test_convert_text_message() {
        // Set dummy API key for test
        std::env::set_var("ANTHROPIC_API_KEY", "test-key");

        let adapter = ClaudeAdapter::new(test_config()).unwrap();
        let conv_id = Uuid::new_v4();
        let msg = LlmMessage::chat("user", Some("claude-test".into()), "Hello", conv_id);

        let anthropic_msg = adapter.convert_message(&msg);

        assert_eq!(anthropic_msg.role, "user");
        assert_eq!(anthropic_msg.content.len(), 1);
        if let AnthropicContent::Text { text } = &anthropic_msg.content[0] {
            assert_eq!(text, "Hello");
        } else {
            panic!("Expected text content");
        }
    }

    #[test]
    fn test_request_building() {
        std::env::set_var("ANTHROPIC_API_KEY", "test-key");

        let config = test_config();
        let adapter = ClaudeAdapter::new(config).unwrap();

        let conv_id = Uuid::new_v4();
        let msg = LlmMessage::chat("user", None, "Test message", conv_id);

        let request = adapter.to_anthropic_request(&msg, &[]);

        assert_eq!(request.model, TEST_MODEL);
        assert_eq!(request.max_tokens, DEFAULT_MAX_TOKENS);
        assert_eq!(request.messages.len(), 1);
    }

    #[test]
    fn test_with_system_prompt() {
        std::env::set_var("ANTHROPIC_API_KEY", "test-key");

        let config = test_config().with_system_prompt("You are helpful.");
        let adapter = ClaudeAdapter::new(config).unwrap();

        let conv_id = Uuid::new_v4();
        let msg = LlmMessage::chat("user", None, "Hi", conv_id);

        let request = adapter.to_anthropic_request(&msg, &[]);

        assert_eq!(request.system, Some("You are helpful.".to_string()));
    }

    #[test]
    fn test_tool_registration() {
        std::env::set_var("ANTHROPIC_API_KEY", "test-key");

        let mut adapter = ClaudeAdapter::new(test_config()).unwrap();

        let tool = ToolDefinition {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        };

        adapter.register_tool(tool);

        assert_eq!(adapter.available_tools().len(), 1);
        assert_eq!(adapter.available_tools()[0].name, "test_tool");
    }

    #[test]
    fn test_anthropic_tool_conversion() {
        let tool = ToolDefinition {
            name: "search".to_string(),
            description: "Search for information".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" }
                }
            }),
        };

        let anthropic_tool: AnthropicTool = tool.into();

        assert_eq!(anthropic_tool.name, "search");
        assert_eq!(anthropic_tool.description, "Search for information");
    }
}
