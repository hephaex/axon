//! In-memory conversation store
//!
//! Useful for testing and short-lived sessions.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::protocol::{Conversation, LlmMessage};
use crate::Result;

use super::ConversationStore;

/// In-memory conversation store
#[derive(Debug, Default)]
pub struct MemoryStore {
    conversations: Arc<RwLock<HashMap<Uuid, Conversation>>>,
    messages: Arc<RwLock<HashMap<Uuid, Vec<LlmMessage>>>>,
}

impl MemoryStore {
    /// Create a new memory store
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the number of stored conversations
    pub async fn len(&self) -> usize {
        self.conversations.read().await.len()
    }

    /// Check if the store is empty
    pub async fn is_empty(&self) -> bool {
        self.conversations.read().await.is_empty()
    }
}

#[async_trait]
impl ConversationStore for MemoryStore {
    async fn save_conversation(&self, conversation: &Conversation) -> Result<()> {
        let mut convs = self.conversations.write().await;
        convs.insert(conversation.id, conversation.clone());
        Ok(())
    }

    async fn load_conversation(&self, id: Uuid) -> Result<Option<Conversation>> {
        let convs = self.conversations.read().await;
        Ok(convs.get(&id).cloned())
    }

    async fn list_conversations(&self) -> Result<Vec<Uuid>> {
        let convs = self.conversations.read().await;
        Ok(convs.keys().copied().collect())
    }

    async fn delete_conversation(&self, id: Uuid) -> Result<bool> {
        let mut convs = self.conversations.write().await;
        let mut msgs = self.messages.write().await;
        msgs.remove(&id);
        Ok(convs.remove(&id).is_some())
    }

    async fn add_message(&self, conversation_id: Uuid, message: LlmMessage) -> Result<()> {
        let mut msgs = self.messages.write().await;
        msgs.entry(conversation_id).or_default().push(message);
        Ok(())
    }

    async fn get_messages(&self, conversation_id: Uuid) -> Result<Vec<LlmMessage>> {
        let msgs = self.messages.read().await;
        Ok(msgs.get(&conversation_id).cloned().unwrap_or_default())
    }

    async fn clear(&self) -> Result<()> {
        let mut convs = self.conversations.write().await;
        let mut msgs = self.messages.write().await;
        convs.clear();
        msgs.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::ConversationBuilder;

    fn test_conversation() -> Conversation {
        ConversationBuilder::new()
            .participant("agent1")
            .participant("agent2")
            .build()
    }

    fn test_message(conv_id: Uuid) -> LlmMessage {
        LlmMessage::chat("user", Some("agent1".into()), "Hello", conv_id)
    }

    #[tokio::test]
    async fn test_save_and_load() {
        let store = MemoryStore::new();
        let conv = test_conversation();
        let id = conv.id;

        store.save_conversation(&conv).await.unwrap();

        let loaded = store.load_conversation(id).await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().id, id);
    }

    #[tokio::test]
    async fn test_list_conversations() {
        let store = MemoryStore::new();

        let conv1 = test_conversation();
        let conv2 = test_conversation();

        store.save_conversation(&conv1).await.unwrap();
        store.save_conversation(&conv2).await.unwrap();

        let ids = store.list_conversations().await.unwrap();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&conv1.id));
        assert!(ids.contains(&conv2.id));
    }

    #[tokio::test]
    async fn test_delete_conversation() {
        let store = MemoryStore::new();
        let conv = test_conversation();
        let id = conv.id;

        store.save_conversation(&conv).await.unwrap();
        assert!(store.load_conversation(id).await.unwrap().is_some());

        let deleted = store.delete_conversation(id).await.unwrap();
        assert!(deleted);

        assert!(store.load_conversation(id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_messages() {
        let store = MemoryStore::new();
        let conv = test_conversation();
        let conv_id = conv.id;

        store.save_conversation(&conv).await.unwrap();

        // Add messages
        let msg1 = test_message(conv_id);
        let msg2 = test_message(conv_id);

        store.add_message(conv_id, msg1.clone()).await.unwrap();
        store.add_message(conv_id, msg2.clone()).await.unwrap();

        // Get messages
        let messages = store.get_messages(conv_id).await.unwrap();
        assert_eq!(messages.len(), 2);
    }

    #[tokio::test]
    async fn test_clear() {
        let store = MemoryStore::new();

        store.save_conversation(&test_conversation()).await.unwrap();
        store.save_conversation(&test_conversation()).await.unwrap();

        assert_eq!(store.len().await, 2);

        store.clear().await.unwrap();

        assert!(store.is_empty().await);
    }

    #[tokio::test]
    async fn test_load_nonexistent() {
        let store = MemoryStore::new();
        let result = store.load_conversation(Uuid::new_v4()).await.unwrap();
        assert!(result.is_none());
    }
}
