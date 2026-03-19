//! Persistence module for saving and loading conversations
//!
//! Provides storage backends for conversation history:
//! - `FileStore` - JSON file-based storage
//! - `MemoryStore` - In-memory storage (for testing)

pub mod file_store;
pub mod memory_store;

use async_trait::async_trait;
use uuid::Uuid;

use crate::protocol::{Conversation, LlmMessage};
use crate::Result;

/// Trait for conversation storage backends
#[async_trait]
pub trait ConversationStore: Send + Sync {
    /// Save a conversation
    async fn save_conversation(&self, conversation: &Conversation) -> Result<()>;

    /// Load a conversation by ID
    async fn load_conversation(&self, id: Uuid) -> Result<Option<Conversation>>;

    /// List all conversation IDs
    async fn list_conversations(&self) -> Result<Vec<Uuid>>;

    /// Delete a conversation
    async fn delete_conversation(&self, id: Uuid) -> Result<bool>;

    /// Add a message to a conversation
    async fn add_message(&self, conversation_id: Uuid, message: LlmMessage) -> Result<()>;

    /// Get messages for a conversation
    async fn get_messages(&self, conversation_id: Uuid) -> Result<Vec<LlmMessage>>;

    /// Clear all conversations
    async fn clear(&self) -> Result<()>;
}

pub use file_store::FileStore;
pub use memory_store::MemoryStore;
