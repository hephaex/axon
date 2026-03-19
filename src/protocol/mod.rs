//! Message protocol definitions
//!
//! Core protocol types for LLM-to-LLM communication:
//! - `LlmMessage` - The main message structure
//! - `MessageType` - Chat, ToolCall, ToolResult, Delegate, etc.
//! - `AgentId` - Agent identifier
//! - `Conversation` - Conversation state

pub mod agent;
pub mod conversation;
pub mod message;

// Re-exports for convenience
pub use agent::{AgentConfig, AgentId, Provider};
pub use conversation::{
    Conversation, ConversationBuilder, ConversationEndReason, ConversationStatus, TurnPolicy,
};
pub use message::{ContentPart, LlmMessage, MessageContent, MessageType};
