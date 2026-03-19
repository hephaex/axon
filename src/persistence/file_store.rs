//! File-based conversation store
//!
//! Stores conversations as JSON files in a directory.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use tokio::fs;
use uuid::Uuid;

use crate::error::AxonError;
use crate::protocol::{Conversation, LlmMessage};
use crate::Result;

use super::ConversationStore;

/// File-based conversation store
///
/// Each conversation is stored as a separate JSON file.
/// Messages are stored in a subdirectory per conversation.
#[derive(Debug, Clone)]
pub struct FileStore {
    /// Base directory for storage
    base_dir: PathBuf,
}

impl FileStore {
    /// Create a new file store
    ///
    /// Creates the directory if it doesn't exist.
    pub async fn new(base_dir: impl AsRef<Path>) -> Result<Self> {
        let base_dir = base_dir.as_ref().to_path_buf();

        // Create directories
        fs::create_dir_all(&base_dir).await.map_err(|e| {
            AxonError::Io(std::io::Error::other(format!(
                "Failed to create store directory: {}",
                e
            )))
        })?;

        let messages_dir = base_dir.join("messages");
        fs::create_dir_all(&messages_dir).await.map_err(|e| {
            AxonError::Io(std::io::Error::other(format!(
                "Failed to create messages directory: {}",
                e
            )))
        })?;

        Ok(Self { base_dir })
    }

    /// Create a file store in the default location
    ///
    /// Uses `~/.axon/conversations/`
    pub async fn default_location() -> Result<Self> {
        let home = dirs::home_dir().ok_or_else(|| AxonError::config("Cannot find home directory"))?;
        let path = home.join(".axon").join("conversations");
        Self::new(path).await
    }

    /// Get path for a conversation file
    fn conversation_path(&self, id: Uuid) -> PathBuf {
        self.base_dir.join(format!("{}.json", id))
    }

    /// Get path for a conversation's messages directory
    fn messages_dir(&self, id: Uuid) -> PathBuf {
        self.base_dir.join("messages").join(id.to_string())
    }

    /// Get path for a specific message file
    fn message_path(&self, conversation_id: Uuid, message_id: Uuid) -> PathBuf {
        self.messages_dir(conversation_id)
            .join(format!("{}.json", message_id))
    }
}

#[async_trait]
impl ConversationStore for FileStore {
    async fn save_conversation(&self, conversation: &Conversation) -> Result<()> {
        let path = self.conversation_path(conversation.id);
        let json = serde_json::to_string_pretty(conversation)?;
        fs::write(&path, json).await?;
        Ok(())
    }

    async fn load_conversation(&self, id: Uuid) -> Result<Option<Conversation>> {
        let path = self.conversation_path(id);

        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&path).await?;
        let conversation: Conversation = serde_json::from_str(&content)?;
        Ok(Some(conversation))
    }

    async fn list_conversations(&self) -> Result<Vec<Uuid>> {
        let mut ids = Vec::new();

        let mut entries = fs::read_dir(&self.base_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            if path.is_file() && path.extension().is_some_and(|ext| ext == "json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Ok(id) = Uuid::parse_str(stem) {
                        ids.push(id);
                    }
                }
            }
        }

        Ok(ids)
    }

    async fn delete_conversation(&self, id: Uuid) -> Result<bool> {
        let conv_path = self.conversation_path(id);
        let msgs_dir = self.messages_dir(id);

        let mut deleted = false;

        // Delete conversation file
        if conv_path.exists() {
            fs::remove_file(&conv_path).await?;
            deleted = true;
        }

        // Delete messages directory
        if msgs_dir.exists() {
            fs::remove_dir_all(&msgs_dir).await?;
        }

        Ok(deleted)
    }

    async fn add_message(&self, conversation_id: Uuid, message: LlmMessage) -> Result<()> {
        let msgs_dir = self.messages_dir(conversation_id);
        fs::create_dir_all(&msgs_dir).await?;

        let path = self.message_path(conversation_id, message.id);
        let json = serde_json::to_string_pretty(&message)?;
        fs::write(&path, json).await?;

        Ok(())
    }

    async fn get_messages(&self, conversation_id: Uuid) -> Result<Vec<LlmMessage>> {
        let msgs_dir = self.messages_dir(conversation_id);

        if !msgs_dir.exists() {
            return Ok(Vec::new());
        }

        let mut messages = Vec::new();
        let mut entries = fs::read_dir(&msgs_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            if path.is_file() && path.extension().is_some_and(|ext| ext == "json") {
                let content = fs::read_to_string(&path).await?;
                if let Ok(message) = serde_json::from_str::<LlmMessage>(&content) {
                    messages.push(message);
                }
            }
        }

        // Sort by timestamp
        messages.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        Ok(messages)
    }

    async fn clear(&self) -> Result<()> {
        // Remove all files and subdirectories
        let mut entries = fs::read_dir(&self.base_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_file() {
                fs::remove_file(&path).await?;
            } else if path.is_dir() {
                fs::remove_dir_all(&path).await?;
            }
        }

        // Recreate messages directory
        fs::create_dir_all(self.base_dir.join("messages")).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::ConversationBuilder;
    use tempfile::TempDir;

    async fn test_store() -> (FileStore, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let store = FileStore::new(temp_dir.path()).await.unwrap();
        (store, temp_dir)
    }

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
        let (store, _temp) = test_store().await;
        let conv = test_conversation();
        let id = conv.id;

        store.save_conversation(&conv).await.unwrap();

        let loaded = store.load_conversation(id).await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().id, id);
    }

    #[tokio::test]
    async fn test_list_conversations() {
        let (store, _temp) = test_store().await;

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
        let (store, _temp) = test_store().await;
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
        let (store, _temp) = test_store().await;
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
        let (store, _temp) = test_store().await;

        store.save_conversation(&test_conversation()).await.unwrap();
        store.save_conversation(&test_conversation()).await.unwrap();

        let ids = store.list_conversations().await.unwrap();
        assert_eq!(ids.len(), 2);

        store.clear().await.unwrap();

        let ids_after = store.list_conversations().await.unwrap();
        assert!(ids_after.is_empty());
    }

    #[tokio::test]
    async fn test_load_nonexistent() {
        let (store, _temp) = test_store().await;
        let result = store.load_conversation(Uuid::new_v4()).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_messages_sorted_by_timestamp() {
        let (store, _temp) = test_store().await;
        let conv = test_conversation();
        let conv_id = conv.id;

        store.save_conversation(&conv).await.unwrap();

        // Add messages with different timestamps
        let mut msg1 = test_message(conv_id);
        let mut msg2 = test_message(conv_id);

        // msg2 should come after msg1
        msg1.timestamp = chrono::Utc::now() - chrono::Duration::seconds(10);
        msg2.timestamp = chrono::Utc::now();

        // Add in reverse order
        store.add_message(conv_id, msg2.clone()).await.unwrap();
        store.add_message(conv_id, msg1.clone()).await.unwrap();

        // Should be sorted by timestamp
        let messages = store.get_messages(conv_id).await.unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].id, msg1.id);
        assert_eq!(messages[1].id, msg2.id);
    }
}
