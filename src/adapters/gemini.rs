//! Gemini (Google AI) adapter
//!
//! Implements LlmAdapter for the Google Gemini API.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AxonError;
use crate::protocol::{AgentConfig, AgentId, LlmMessage, MessageContent, MessageType};
use crate::tools::ToolDefinition;
use crate::Result;

use super::LlmAdapter;

const GEMINI_API_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models";
const DEFAULT_MAX_TOKENS: u32 = 4096;

/// Gemini adapter for Google AI API
pub struct GeminiAdapter {
    config: AgentConfig,
    client: Client,
    api_key: String,
    tools: Vec<ToolDefinition>,
}

impl GeminiAdapter {
    /// Create a new Gemini adapter
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
            .unwrap_or("GOOGLE_API_KEY");

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

    /// Build API URL for the model
    fn build_url(&self) -> String {
        format!(
            "{}/{}:generateContent?key={}",
            GEMINI_API_URL, self.config.model, self.api_key
        )
    }

    /// Convert LlmMessage to Gemini request format
    fn to_gemini_request(&self, message: &LlmMessage, history: &[LlmMessage]) -> GeminiRequest {
        let mut contents = Vec::new();

        // Add history
        for msg in history {
            contents.push(self.convert_message(msg));
        }

        // Add current message
        contents.push(self.convert_message(message));

        // Build generation config
        let generation_config = GenerationConfig {
            max_output_tokens: Some(self.config.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS)),
            temperature: self.config.temperature,
            top_p: None,
            top_k: None,
        };

        // Build tools if any
        let tools = if self.tools.is_empty() {
            None
        } else {
            Some(vec![GeminiTools {
                function_declarations: self
                    .tools
                    .iter()
                    .map(|t| FunctionDeclaration {
                        name: t.name.clone(),
                        description: t.description.clone(),
                        parameters: t.parameters.clone(),
                    })
                    .collect(),
            }])
        };

        GeminiRequest {
            contents,
            generation_config: Some(generation_config),
            system_instruction: self.config.system_prompt.as_ref().map(|s| SystemInstruction {
                parts: vec![Part::Text { text: s.clone() }],
            }),
            tools,
        }
    }

    /// Convert a single LlmMessage to Gemini content format
    fn convert_message(&self, msg: &LlmMessage) -> Content {
        let role = if msg.from == self.config.id {
            "model"
        } else {
            "user"
        };

        let parts = match &msg.content {
            MessageContent::Text(text) => vec![Part::Text { text: text.clone() }],
            MessageContent::Json(value) => {
                vec![Part::Text {
                    text: value.to_string(),
                }]
            }
            MessageContent::Parts(parts) => {
                parts
                    .iter()
                    .map(|p| match p {
                        crate::protocol::ContentPart::Text { text } => Part::Text { text: text.clone() },
                        crate::protocol::ContentPart::Image { base64, media_type, .. } => {
                            Part::InlineData {
                                mime_type: media_type.clone().unwrap_or_else(|| "image/png".to_string()),
                                data: base64.clone().unwrap_or_default(),
                            }
                        }
                    })
                    .collect()
            }
        };

        Content {
            role: role.to_string(),
            parts,
        }
    }

    /// Parse Gemini response to LlmMessage
    fn parse_response(
        &self,
        response: GeminiResponse,
        conversation_id: Uuid,
    ) -> Result<LlmMessage> {
        let candidate = response
            .candidates
            .into_iter()
            .next()
            .ok_or_else(|| AxonError::api("No candidates in response"))?;

        // Check for function calls
        for part in &candidate.content.parts {
            if let Part::FunctionCall { name, args } = part {
                return Ok(LlmMessage {
                    id: Uuid::new_v4(),
                    from: self.config.id.clone(),
                    to: None,
                    message_type: MessageType::ToolCall {
                        tool: name.clone(),
                        call_id: Uuid::new_v4(),
                    },
                    content: MessageContent::Json(args.clone()),
                    conversation_id,
                    timestamp: chrono::Utc::now(),
                    metadata: None,
                });
            }
        }

        // Extract text content
        let text = candidate
            .content
            .parts
            .iter()
            .filter_map(|p| {
                if let Part::Text { text } = p {
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
                "model": self.config.model,
                "finish_reason": candidate.finish_reason,
            })),
        })
    }

    /// Call the Gemini API
    async fn call_api(&self, request: &GeminiRequest) -> Result<GeminiResponse> {
        let url = self.build_url();

        let response = self
            .client
            .post(&url)
            .header("content-type", "application/json")
            .json(request)
            .send()
            .await
            .map_err(|e| AxonError::api(format!("Request failed: {}", e)))?;

        let status = response.status();

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(AxonError::RateLimited {
                retry_after_secs: 60,
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
            .json::<GeminiResponse>()
            .await
            .map_err(|e| AxonError::api(format!("Failed to parse response: {}", e)))
    }
}

#[async_trait]
impl LlmAdapter for GeminiAdapter {
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
        let request = self.to_gemini_request(message, history);
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

// Gemini API types

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiRequest {
    contents: Vec<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generation_config: Option<GenerationConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<SystemInstruction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<GeminiTools>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Content {
    role: String,
    parts: Vec<Part>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum Part {
    Text {
        text: String,
    },
    InlineData {
        #[serde(rename = "mimeType")]
        mime_type: String,
        data: String,
    },
    FunctionCall {
        #[serde(rename = "functionCall")]
        name: String,
        args: serde_json::Value,
    },
    FunctionResponse {
        #[serde(rename = "functionResponse")]
        name: String,
        response: serde_json::Value,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_k: Option<u32>,
}

#[derive(Debug, Serialize)]
struct SystemInstruction {
    parts: Vec<Part>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiTools {
    function_declarations: Vec<FunctionDeclaration>,
}

#[derive(Debug, Serialize)]
struct FunctionDeclaration {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct GeminiResponse {
    candidates: Vec<Candidate>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Candidate {
    content: Content,
    finish_reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::Provider;

    fn test_config() -> AgentConfig {
        AgentConfig::new("gemini-test", Provider::Google, "gemini-1.5-flash")
    }

    #[test]
    fn test_build_url() {
        std::env::set_var("GOOGLE_API_KEY", "test-key");

        let adapter = GeminiAdapter::new(test_config()).unwrap();
        let url = adapter.build_url();

        assert!(url.contains("gemini-1.5-flash"));
        assert!(url.contains("generateContent"));
    }

    #[test]
    fn test_convert_text_message() {
        std::env::set_var("GOOGLE_API_KEY", "test-key");

        let adapter = GeminiAdapter::new(test_config()).unwrap();
        let conv_id = Uuid::new_v4();
        let msg = LlmMessage::chat("user", Some("gemini-test".into()), "Hello", conv_id);

        let content = adapter.convert_message(&msg);

        assert_eq!(content.role, "user");
        assert_eq!(content.parts.len(), 1);
    }

    #[test]
    fn test_request_building() {
        std::env::set_var("GOOGLE_API_KEY", "test-key");

        let adapter = GeminiAdapter::new(test_config()).unwrap();
        let conv_id = Uuid::new_v4();
        let msg = LlmMessage::chat("user", None, "Test message", conv_id);

        let request = adapter.to_gemini_request(&msg, &[]);

        assert_eq!(request.contents.len(), 1);
        assert!(request.generation_config.is_some());
    }

    #[test]
    fn test_with_system_prompt() {
        std::env::set_var("GOOGLE_API_KEY", "test-key");

        let config = test_config().with_system_prompt("You are helpful.");
        let adapter = GeminiAdapter::new(config).unwrap();

        let conv_id = Uuid::new_v4();
        let msg = LlmMessage::chat("user", None, "Hi", conv_id);

        let request = adapter.to_gemini_request(&msg, &[]);

        assert!(request.system_instruction.is_some());
    }

    #[test]
    fn test_tool_registration() {
        std::env::set_var("GOOGLE_API_KEY", "test-key");

        let mut adapter = GeminiAdapter::new(test_config()).unwrap();

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
}
