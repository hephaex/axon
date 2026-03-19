//! OpenAI (GPT) adapter
//!
//! Implements LlmAdapter for the OpenAI API.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AxonError;
use crate::protocol::{AgentConfig, AgentId, LlmMessage, MessageContent, MessageType};
use crate::tools::ToolDefinition;
use crate::Result;

use super::LlmAdapter;

const OPENAI_API_URL: &str = "https://api.openai.com/v1/chat/completions";
const DEFAULT_MAX_TOKENS: u32 = 4096;

/// OpenAI adapter for GPT API
pub struct OpenAiAdapter {
    pub(crate) config: AgentConfig,
    pub(crate) client: Client,
    pub(crate) api_key: String,
    tools: Vec<ToolDefinition>,
}

impl OpenAiAdapter {
    /// Create a new OpenAI adapter
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
            .unwrap_or("OPENAI_API_KEY");

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

    /// Get API URL (supports custom endpoint for Azure, etc.)
    pub(crate) fn get_api_url(&self) -> &str {
        self.config
            .endpoint
            .as_deref()
            .unwrap_or(OPENAI_API_URL)
    }

    /// Convert LlmMessage to OpenAI request format
    pub(crate) fn to_openai_request(&self, message: &LlmMessage, history: &[LlmMessage]) -> OpenAiRequest {
        let mut messages = Vec::new();

        // Add system prompt if set
        if let Some(system) = &self.config.system_prompt {
            messages.push(ChatMessage {
                role: "system".to_string(),
                content: Some(system.clone()),
                tool_calls: None,
                tool_call_id: None,
            });
        }

        // Add history
        for msg in history {
            messages.push(self.convert_message(msg));
        }

        // Add current message
        messages.push(self.convert_message(message));

        // Build tools if any
        let tools = if self.tools.is_empty() {
            None
        } else {
            Some(
                self.tools
                    .iter()
                    .map(|t| OpenAiTool {
                        r#type: "function".to_string(),
                        function: FunctionDef {
                            name: t.name.clone(),
                            description: Some(t.description.clone()),
                            parameters: t.parameters.clone(),
                        },
                    })
                    .collect(),
            )
        };

        OpenAiRequest {
            model: self.config.model.clone(),
            messages,
            max_tokens: Some(self.config.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS)),
            temperature: self.config.temperature,
            tools,
            tool_choice: if self.tools.is_empty() {
                None
            } else {
                Some("auto".to_string())
            },
        }
    }

    /// Convert a single LlmMessage to OpenAI message format
    fn convert_message(&self, msg: &LlmMessage) -> ChatMessage {
        let role = if msg.from == self.config.id {
            "assistant"
        } else {
            "user"
        };

        let content = match &msg.content {
            MessageContent::Text(text) => Some(text.clone()),
            MessageContent::Json(value) => Some(value.to_string()),
            MessageContent::Parts(parts) => {
                // OpenAI doesn't support parts in chat format directly
                // Concatenate text parts
                let text: String = parts
                    .iter()
                    .filter_map(|p| match p {
                        crate::protocol::ContentPart::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                Some(text)
            }
        };

        ChatMessage {
            role: role.to_string(),
            content,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Parse OpenAI response to LlmMessage
    fn parse_response(
        &self,
        response: OpenAiResponse,
        conversation_id: Uuid,
    ) -> Result<LlmMessage> {
        let choice = response
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| AxonError::api("No choices in response"))?;

        // Check for tool calls
        if let Some(tool_calls) = choice.message.tool_calls {
            if let Some(tc) = tool_calls.into_iter().next() {
                let args: serde_json::Value = serde_json::from_str(&tc.function.arguments)
                    .unwrap_or_else(|_| serde_json::json!({}));

                return Ok(LlmMessage {
                    id: Uuid::new_v4(),
                    from: self.config.id.clone(),
                    to: None,
                    message_type: MessageType::ToolCall {
                        tool: tc.function.name,
                        call_id: Uuid::parse_str(&tc.id).unwrap_or_else(|_| Uuid::new_v4()),
                    },
                    content: MessageContent::Json(args),
                    conversation_id,
                    timestamp: chrono::Utc::now(),
                    metadata: None,
                });
            }
        }

        // Extract text content
        let text = choice.message.content.unwrap_or_default();

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
                "finish_reason": choice.finish_reason,
                "usage": response.usage,
            })),
        })
    }

    /// Call the OpenAI API
    async fn call_api(&self, request: &OpenAiRequest) -> Result<OpenAiResponse> {
        let url = self.get_api_url();

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.api_key))
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
            .json::<OpenAiResponse>()
            .await
            .map_err(|e| AxonError::api(format!("Failed to parse response: {}", e)))
    }
}

#[async_trait]
impl LlmAdapter for OpenAiAdapter {
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
        let request = self.to_openai_request(message, history);
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
        if self.api_key.is_empty() {
            return Ok(false);
        }
        Ok(true)
    }
}

// OpenAI API types

#[derive(Debug, Serialize)]
pub(crate) struct OpenAiRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenAiTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct OpenAiTool {
    r#type: String,
    function: FunctionDef,
}

#[derive(Debug, Serialize)]
struct FunctionDef {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    parameters: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct ToolCall {
    id: String,
    function: FunctionCall,
}

#[derive(Debug, Serialize, Deserialize)]
struct FunctionCall {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponse {
    model: String,
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ChatMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::Provider;

    fn test_config() -> AgentConfig {
        AgentConfig::new("openai-test", Provider::OpenAi, "gpt-4o")
    }

    #[test]
    fn test_convert_text_message() {
        std::env::set_var("OPENAI_API_KEY", "test-key");

        let adapter = OpenAiAdapter::new(test_config()).unwrap();
        let conv_id = Uuid::new_v4();
        let msg = LlmMessage::chat("user", Some("openai-test".into()), "Hello", conv_id);

        let chat_msg = adapter.convert_message(&msg);

        assert_eq!(chat_msg.role, "user");
        assert_eq!(chat_msg.content, Some("Hello".to_string()));
    }

    #[test]
    fn test_request_building() {
        std::env::set_var("OPENAI_API_KEY", "test-key");

        let adapter = OpenAiAdapter::new(test_config()).unwrap();
        let conv_id = Uuid::new_v4();
        let msg = LlmMessage::chat("user", None, "Test message", conv_id);

        let request = adapter.to_openai_request(&msg, &[]);

        assert_eq!(request.model, "gpt-4o");
        assert_eq!(request.messages.len(), 1);
    }

    #[test]
    fn test_with_system_prompt() {
        std::env::set_var("OPENAI_API_KEY", "test-key");

        let config = test_config().with_system_prompt("You are helpful.");
        let adapter = OpenAiAdapter::new(config).unwrap();

        let conv_id = Uuid::new_v4();
        let msg = LlmMessage::chat("user", None, "Hi", conv_id);

        let request = adapter.to_openai_request(&msg, &[]);

        // Should have system message + user message
        assert_eq!(request.messages.len(), 2);
        assert_eq!(request.messages[0].role, "system");
    }

    #[test]
    fn test_tool_registration() {
        std::env::set_var("OPENAI_API_KEY", "test-key");

        let mut adapter = OpenAiAdapter::new(test_config()).unwrap();

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
    }

    #[test]
    fn test_custom_endpoint() {
        std::env::set_var("OPENAI_API_KEY", "test-key");

        let config = test_config().with_endpoint("https://custom.api.com/v1/chat");
        let adapter = OpenAiAdapter::new(config).unwrap();

        assert_eq!(adapter.get_api_url(), "https://custom.api.com/v1/chat");
    }
}
