//! Message types for LLM-to-LLM communication
//!
//! Core message protocol enabling agents to communicate with each other.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::agent::AgentId;

/// Main message structure for LLM communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmMessage {
    /// Unique message identifier
    pub id: Uuid,

    /// Sender agent
    pub from: AgentId,

    /// Target agent (None = broadcast to all)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<AgentId>,

    /// Message type
    pub message_type: MessageType,

    /// Message content
    pub content: MessageContent,

    /// Conversation this message belongs to
    pub conversation_id: Uuid,

    /// Message timestamp
    pub timestamp: DateTime<Utc>,

    /// Optional metadata
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

impl LlmMessage {
    /// Create a new chat message
    pub fn chat(
        from: impl Into<AgentId>,
        to: Option<AgentId>,
        content: impl Into<String>,
        conversation_id: Uuid,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            from: from.into(),
            to,
            message_type: MessageType::Chat,
            content: MessageContent::Text(content.into()),
            conversation_id,
            timestamp: Utc::now(),
            metadata: None,
        }
    }

    /// Create a tool call message
    pub fn tool_call(
        from: impl Into<AgentId>,
        tool: impl Into<String>,
        args: serde_json::Value,
        conversation_id: Uuid,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            from: from.into(),
            to: None,
            message_type: MessageType::ToolCall {
                tool: tool.into(),
                call_id: Uuid::new_v4(),
            },
            content: MessageContent::Json(args),
            conversation_id,
            timestamp: Utc::now(),
            metadata: None,
        }
    }

    /// Create a tool result message
    pub fn tool_result(
        from: impl Into<AgentId>,
        call_id: Uuid,
        result: serde_json::Value,
        success: bool,
        conversation_id: Uuid,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            from: from.into(),
            to: None,
            message_type: MessageType::ToolResult { call_id, success },
            content: MessageContent::Json(result),
            conversation_id,
            timestamp: Utc::now(),
            metadata: None,
        }
    }

    /// Create a delegate message (assign task to another agent)
    pub fn delegate(
        from: impl Into<AgentId>,
        to: impl Into<AgentId>,
        task: impl Into<String>,
        context: serde_json::Value,
        conversation_id: Uuid,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            from: from.into(),
            to: Some(to.into()),
            message_type: MessageType::Delegate {
                task: task.into(),
            },
            content: MessageContent::Json(context),
            conversation_id,
            timestamp: Utc::now(),
            metadata: None,
        }
    }

    /// Create a completion message
    pub fn complete(
        from: impl Into<AgentId>,
        task_id: Uuid,
        summary: impl Into<String>,
        conversation_id: Uuid,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            from: from.into(),
            to: None,
            message_type: MessageType::Complete { task_id },
            content: MessageContent::Text(summary.into()),
            conversation_id,
            timestamp: Utc::now(),
            metadata: None,
        }
    }

    /// Create an error message
    pub fn error(
        from: impl Into<AgentId>,
        code: impl Into<String>,
        message: impl Into<String>,
        conversation_id: Uuid,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            from: from.into(),
            to: None,
            message_type: MessageType::Error {
                code: code.into(),
            },
            content: MessageContent::Text(message.into()),
            conversation_id,
            timestamp: Utc::now(),
            metadata: None,
        }
    }

    /// Check if this is a broadcast message
    pub fn is_broadcast(&self) -> bool {
        self.to.is_none()
    }

    /// Set metadata
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

/// Message type indicating the purpose of the message
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageType {
    /// Regular chat message
    Chat,

    /// Tool/function call request
    ToolCall {
        /// Tool name
        tool: String,
        /// Unique call identifier for matching results
        call_id: Uuid,
    },

    /// Result from a tool call
    ToolResult {
        /// The call_id this result corresponds to
        call_id: Uuid,
        /// Whether the tool call succeeded
        success: bool,
    },

    /// Delegate a task to another agent
    Delegate {
        /// Task description
        task: String,
    },

    /// Task completion notification
    Complete {
        /// The task_id being completed
        task_id: Uuid,
    },

    /// Error message
    Error {
        /// Error code
        code: String,
    },

    /// System message (not from an agent)
    System,
}

impl MessageType {
    /// Check if this is a chat message
    pub fn is_chat(&self) -> bool {
        matches!(self, Self::Chat)
    }

    /// Check if this is a tool-related message
    pub fn is_tool(&self) -> bool {
        matches!(self, Self::ToolCall { .. } | Self::ToolResult { .. })
    }

    /// Check if this is an error
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error { .. })
    }
}

/// Message content variants
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    /// Plain text content
    Text(String),

    /// Structured JSON content
    Json(serde_json::Value),

    /// Multiple content parts (for multimodal)
    Parts(Vec<ContentPart>),
}

impl MessageContent {
    /// Get content as text (converting JSON to string if needed)
    pub fn as_text(&self) -> String {
        match self {
            Self::Text(s) => s.clone(),
            Self::Json(v) => v.to_string(),
            Self::Parts(parts) => parts
                .iter()
                .filter_map(|p| {
                    if let ContentPart::Text { text } = p {
                        Some(text.as_str())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }

    /// Try to get content as JSON
    pub fn as_json(&self) -> Option<&serde_json::Value> {
        match self {
            Self::Json(v) => Some(v),
            _ => None,
        }
    }
}

impl From<String> for MessageContent {
    fn from(s: String) -> Self {
        Self::Text(s)
    }
}

impl From<&str> for MessageContent {
    fn from(s: &str) -> Self {
        Self::Text(s.to_string())
    }
}

impl From<serde_json::Value> for MessageContent {
    fn from(v: serde_json::Value) -> Self {
        Self::Json(v)
    }
}

/// Content part for multimodal messages
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    /// Text content
    Text { text: String },

    /// Image content (base64 or URL)
    Image {
        #[serde(skip_serializing_if = "Option::is_none")]
        url: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        base64: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        media_type: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_message_creation() {
        let conv_id = Uuid::new_v4();
        let msg = LlmMessage::chat("claude", Some("gemini".into()), "Hello!", conv_id);

        assert_eq!(msg.from.as_str(), "claude");
        assert_eq!(msg.to.as_ref().unwrap().as_str(), "gemini");
        assert!(matches!(msg.message_type, MessageType::Chat));
        assert_eq!(msg.content.as_text(), "Hello!");
        assert_eq!(msg.conversation_id, conv_id);
    }

    #[test]
    fn test_broadcast_message() {
        let conv_id = Uuid::new_v4();
        let msg = LlmMessage::chat("claude", None, "Broadcast", conv_id);

        assert!(msg.is_broadcast());
        assert!(msg.to.is_none());
    }

    #[test]
    fn test_tool_call_message() {
        let conv_id = Uuid::new_v4();
        let args = serde_json::json!({
            "query": "rust async"
        });
        let msg = LlmMessage::tool_call("claude", "minky_search", args, conv_id);

        assert!(msg.message_type.is_tool());
        if let MessageType::ToolCall { tool, .. } = &msg.message_type {
            assert_eq!(tool, "minky_search");
        }
    }

    #[test]
    fn test_delegate_message() {
        let conv_id = Uuid::new_v4();
        let context = serde_json::json!({
            "code": "fn main() {}",
            "language": "rust"
        });
        let msg = LlmMessage::delegate("claude", "gemini", "Review this code", context, conv_id);

        if let MessageType::Delegate { task } = &msg.message_type {
            assert_eq!(task, "Review this code");
        }
        assert_eq!(msg.to.as_ref().unwrap().as_str(), "gemini");
    }

    #[test]
    fn test_error_message() {
        let conv_id = Uuid::new_v4();
        let msg = LlmMessage::error("claude", "TIMEOUT", "Request timed out", conv_id);

        assert!(msg.message_type.is_error());
        if let MessageType::Error { code } = &msg.message_type {
            assert_eq!(code, "TIMEOUT");
        }
    }

    #[test]
    fn test_message_serialization() {
        let conv_id = Uuid::new_v4();
        let msg = LlmMessage::chat("claude", None, "Test", conv_id);

        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: LlmMessage = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.from.as_str(), "claude");
        assert_eq!(deserialized.content.as_text(), "Test");
    }

    #[test]
    fn test_message_type_serialization() {
        let tool_call = MessageType::ToolCall {
            tool: "test_tool".to_string(),
            call_id: Uuid::new_v4(),
        };

        let json = serde_json::to_string(&tool_call).unwrap();
        assert!(json.contains("tool_call"));
        assert!(json.contains("test_tool"));
    }

    #[test]
    fn test_content_variants() {
        let text = MessageContent::Text("hello".to_string());
        assert_eq!(text.as_text(), "hello");

        let json = MessageContent::Json(serde_json::json!({"key": "value"}));
        assert!(json.as_json().is_some());

        let parts = MessageContent::Parts(vec![
            ContentPart::Text { text: "part1".to_string() },
            ContentPart::Text { text: "part2".to_string() },
        ]);
        assert_eq!(parts.as_text(), "part1\npart2");
    }
}
