//! Ollama adapter
//!
//! Implements LlmAdapter for the local Ollama API.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AxonError;
use crate::protocol::{AgentConfig, AgentId, LlmMessage, MessageContent, MessageType};
use crate::tools::ToolDefinition;
use crate::Result;

use super::LlmAdapter;

const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";

/// Ollama adapter for local LLM
pub struct OllamaAdapter {
    config: AgentConfig,
    client: Client,
    base_url: String,
    tools: Vec<ToolDefinition>,
}

impl OllamaAdapter {
    /// Create a new Ollama adapter
    pub fn new(config: AgentConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(300)) // Longer timeout for local models
            .build()
            .map_err(|e| AxonError::api(format!("Failed to create HTTP client: {}", e)))?;

        let base_url = config
            .endpoint
            .clone()
            .unwrap_or_else(|| DEFAULT_OLLAMA_URL.to_string());

        Ok(Self {
            config,
            client,
            base_url,
            tools: Vec::new(),
        })
    }

    /// Build API URL for chat endpoint
    fn build_url(&self) -> String {
        format!("{}/api/chat", self.base_url)
    }

    /// Convert LlmMessage to Ollama request format
    fn to_ollama_request(&self, message: &LlmMessage, history: &[LlmMessage]) -> OllamaRequest {
        let mut messages = Vec::new();

        // Add system prompt if set
        if let Some(system) = &self.config.system_prompt {
            messages.push(OllamaMessage {
                role: "system".to_string(),
                content: system.clone(),
                images: None,
                tool_calls: None,
            });
        }

        // Add history
        for msg in history {
            messages.push(self.convert_message(msg));
        }

        // Add current message
        messages.push(self.convert_message(message));

        // Build options
        let options = OllamaOptions {
            temperature: self.config.temperature,
            num_predict: self.config.max_tokens.map(|t| t as i32),
        };

        // Build tools if any (Ollama supports tools in newer versions)
        let tools = if self.tools.is_empty() {
            None
        } else {
            Some(
                self.tools
                    .iter()
                    .map(|t| OllamaTool {
                        r#type: "function".to_string(),
                        function: OllamaFunction {
                            name: t.name.clone(),
                            description: t.description.clone(),
                            parameters: t.parameters.clone(),
                        },
                    })
                    .collect(),
            )
        };

        OllamaRequest {
            model: self.config.model.clone(),
            messages,
            stream: Some(false),
            options: Some(options),
            tools,
        }
    }

    /// Convert a single LlmMessage to Ollama message format
    fn convert_message(&self, msg: &LlmMessage) -> OllamaMessage {
        let role = if msg.from == self.config.id {
            "assistant"
        } else {
            "user"
        };

        let (content, images) = match &msg.content {
            MessageContent::Text(text) => (text.clone(), None),
            MessageContent::Json(value) => (value.to_string(), None),
            MessageContent::Parts(parts) => {
                let mut text_parts = Vec::new();
                let mut image_parts = Vec::new();

                for part in parts {
                    match part {
                        crate::protocol::ContentPart::Text { text } => {
                            text_parts.push(text.as_str());
                        }
                        crate::protocol::ContentPart::Image { base64, .. } => {
                            if let Some(data) = base64 {
                                image_parts.push(data.clone());
                            }
                        }
                    }
                }

                let content = text_parts.join("\n");
                let images = if image_parts.is_empty() {
                    None
                } else {
                    Some(image_parts)
                };

                (content, images)
            }
        };

        OllamaMessage {
            role: role.to_string(),
            content,
            images,
            tool_calls: None,
        }
    }

    /// Parse Ollama response to LlmMessage
    fn parse_response(
        &self,
        response: OllamaResponse,
        conversation_id: Uuid,
    ) -> Result<LlmMessage> {
        let message = response.message;

        // Check for tool calls
        if let Some(tool_calls) = message.tool_calls {
            if let Some(tc) = tool_calls.into_iter().next() {
                return Ok(LlmMessage {
                    id: Uuid::new_v4(),
                    from: self.config.id.clone(),
                    to: None,
                    message_type: MessageType::ToolCall {
                        tool: tc.function.name,
                        call_id: Uuid::new_v4(),
                    },
                    content: MessageContent::Json(tc.function.arguments),
                    conversation_id,
                    timestamp: chrono::Utc::now(),
                    metadata: None,
                });
            }
        }

        // Extract text content
        let text = message.content;

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
                "done_reason": response.done_reason,
                "total_duration": response.total_duration,
                "eval_count": response.eval_count,
            })),
        })
    }

    /// Call the Ollama API
    async fn call_api(&self, request: &OllamaRequest) -> Result<OllamaResponse> {
        let url = self.build_url();

        let response = self
            .client
            .post(&url)
            .header("content-type", "application/json")
            .json(request)
            .send()
            .await
            .map_err(|e| {
                if e.is_connect() {
                    AxonError::config(format!(
                        "Cannot connect to Ollama at {}. Is Ollama running?",
                        self.base_url
                    ))
                } else {
                    AxonError::api(format!("Request failed: {}", e))
                }
            })?;

        let status = response.status();

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
            .json::<OllamaResponse>()
            .await
            .map_err(|e| AxonError::api(format!("Failed to parse response: {}", e)))
    }
}

#[async_trait]
impl LlmAdapter for OllamaAdapter {
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
        let request = self.to_ollama_request(message, history);
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
        // Check if Ollama is running by hitting the tags endpoint
        let url = format!("{}/api/tags", self.base_url);

        match self.client.get(&url).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false),
        }
    }
}

// Ollama API types

#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OllamaTool>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    images: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    tool_calls: Option<Vec<OllamaToolCall>>,
}

#[derive(Debug, Serialize)]
struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<i32>,
}

#[derive(Debug, Serialize)]
struct OllamaTool {
    r#type: String,
    function: OllamaFunction,
}

#[derive(Debug, Serialize)]
struct OllamaFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaToolCall {
    function: OllamaFunctionCall,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaFunctionCall {
    name: String,
    arguments: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    model: String,
    message: OllamaMessage,
    done_reason: Option<String>,
    total_duration: Option<u64>,
    eval_count: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::Provider;

    fn test_config() -> AgentConfig {
        AgentConfig::new("ollama-test", Provider::Ollama, "llama2")
    }

    #[test]
    fn test_build_url() {
        let adapter = OllamaAdapter::new(test_config()).unwrap();
        let url = adapter.build_url();

        assert!(url.contains("localhost:11434"));
        assert!(url.contains("/api/chat"));
    }

    #[test]
    fn test_custom_endpoint() {
        let config = test_config().with_endpoint("http://192.168.1.100:11434");
        let adapter = OllamaAdapter::new(config).unwrap();

        assert_eq!(adapter.base_url, "http://192.168.1.100:11434");
    }

    #[test]
    fn test_convert_text_message() {
        let adapter = OllamaAdapter::new(test_config()).unwrap();
        let conv_id = Uuid::new_v4();
        let msg = LlmMessage::chat("user", Some("ollama-test".into()), "Hello", conv_id);

        let ollama_msg = adapter.convert_message(&msg);

        assert_eq!(ollama_msg.role, "user");
        assert_eq!(ollama_msg.content, "Hello");
        assert!(ollama_msg.images.is_none());
    }

    #[test]
    fn test_request_building() {
        let adapter = OllamaAdapter::new(test_config()).unwrap();
        let conv_id = Uuid::new_v4();
        let msg = LlmMessage::chat("user", None, "Test message", conv_id);

        let request = adapter.to_ollama_request(&msg, &[]);

        assert_eq!(request.model, "llama2");
        assert_eq!(request.messages.len(), 1);
        assert_eq!(request.stream, Some(false));
    }

    #[test]
    fn test_with_system_prompt() {
        let config = test_config().with_system_prompt("You are helpful.");
        let adapter = OllamaAdapter::new(config).unwrap();

        let conv_id = Uuid::new_v4();
        let msg = LlmMessage::chat("user", None, "Hi", conv_id);

        let request = adapter.to_ollama_request(&msg, &[]);

        // Should have system message + user message
        assert_eq!(request.messages.len(), 2);
        assert_eq!(request.messages[0].role, "system");
    }

    #[test]
    fn test_tool_registration() {
        let mut adapter = OllamaAdapter::new(test_config()).unwrap();

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
    fn test_with_options() {
        let mut config = test_config();
        config.temperature = Some(0.8);
        config.max_tokens = Some(2048);
        let adapter = OllamaAdapter::new(config).unwrap();

        let conv_id = Uuid::new_v4();
        let msg = LlmMessage::chat("user", None, "Hi", conv_id);

        let request = adapter.to_ollama_request(&msg, &[]);

        let options = request.options.unwrap();
        assert_eq!(options.temperature, Some(0.8));
        assert_eq!(options.num_predict, Some(2048));
    }
}
