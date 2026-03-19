//! Message Router
//!
//! Routes messages between LLM agents.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

use crate::adapters::LlmAdapter;
use crate::error::AxonError;
use crate::protocol::{AgentId, Conversation, LlmMessage};
use crate::Result;

/// Message router for LLM-to-LLM communication
pub struct MessageRouter {
    /// Registered adapters by agent ID
    adapters: Arc<RwLock<HashMap<String, Box<dyn LlmAdapter>>>>,

    /// Active conversations
    conversations: Arc<RwLock<HashMap<uuid::Uuid, Conversation>>>,

    /// Message sender for the internal queue
    sender: mpsc::Sender<RouterMessage>,

    /// Message receiver for processing
    receiver: Arc<RwLock<mpsc::Receiver<RouterMessage>>>,
}

/// Internal message wrapper for the router queue
#[derive(Debug)]
struct RouterMessage {
    message: LlmMessage,
    response_tx: Option<mpsc::Sender<Result<LlmMessage>>>,
}

impl MessageRouter {
    /// Create a new message router
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(100);

        Self {
            adapters: Arc::new(RwLock::new(HashMap::new())),
            conversations: Arc::new(RwLock::new(HashMap::new())),
            sender,
            receiver: Arc::new(RwLock::new(receiver)),
        }
    }

    /// Create a new router with custom queue size
    pub fn with_queue_size(size: usize) -> Self {
        let (sender, receiver) = mpsc::channel(size);

        Self {
            adapters: Arc::new(RwLock::new(HashMap::new())),
            conversations: Arc::new(RwLock::new(HashMap::new())),
            sender,
            receiver: Arc::new(RwLock::new(receiver)),
        }
    }

    /// Register an adapter
    pub async fn register_adapter(&self, adapter: Box<dyn LlmAdapter>) {
        let agent_id = adapter.agent_id().to_string();
        let mut adapters = self.adapters.write().await;
        adapters.insert(agent_id, adapter);
    }

    /// Unregister an adapter
    pub async fn unregister_adapter(&self, agent_id: &str) -> Option<Box<dyn LlmAdapter>> {
        let mut adapters = self.adapters.write().await;
        adapters.remove(agent_id)
    }

    /// Get list of registered agent IDs
    pub async fn list_agents(&self) -> Vec<String> {
        let adapters = self.adapters.read().await;
        adapters.keys().cloned().collect()
    }

    /// Check if an agent is registered
    pub async fn has_agent(&self, agent_id: &str) -> bool {
        let adapters = self.adapters.read().await;
        adapters.contains_key(agent_id)
    }

    /// Start or get a conversation
    pub async fn get_or_create_conversation(&self, id: uuid::Uuid) -> Conversation {
        let mut conversations = self.conversations.write().await;
        conversations
            .entry(id)
            .or_insert_with(|| Conversation::new(vec![]))
            .clone()
    }

    /// Update a conversation
    pub async fn update_conversation(&self, conversation: Conversation) {
        let mut conversations = self.conversations.write().await;
        conversations.insert(conversation.id, conversation);
    }

    /// Send a message and wait for response
    pub async fn send(&self, message: LlmMessage) -> Result<LlmMessage> {
        // Validate sender exists
        if !self.has_agent(message.from.as_str()).await {
            return Err(AxonError::agent(
                message.from.as_str(),
                "Sender agent not registered",
            ));
        }

        // If there's a specific target, send directly
        if let Some(ref to) = message.to {
            return self.send_to_agent(to, &message).await;
        }

        // Broadcast to all agents except sender
        self.broadcast(&message).await
    }

    /// Send a message to a specific agent
    async fn send_to_agent(&self, to: &AgentId, message: &LlmMessage) -> Result<LlmMessage> {
        let adapters = self.adapters.read().await;

        let adapter = adapters
            .get(to.as_str())
            .ok_or_else(|| AxonError::agent(to.as_str(), "Target agent not registered"))?;

        // Get conversation history
        let conversations = self.conversations.read().await;
        let history = conversations
            .get(&message.conversation_id)
            .map(|c| c.messages.as_slice())
            .unwrap_or(&[]);

        adapter.process_with_history(message, history).await
    }

    /// Broadcast a message to all agents except sender
    async fn broadcast(&self, message: &LlmMessage) -> Result<LlmMessage> {
        let adapters = self.adapters.read().await;

        // Find first agent that isn't the sender
        for (agent_id, adapter) in adapters.iter() {
            if agent_id != message.from.as_str() {
                let conversations = self.conversations.read().await;
                let history = conversations
                    .get(&message.conversation_id)
                    .map(|c| c.messages.as_slice())
                    .unwrap_or(&[]);

                return adapter.process_with_history(message, history).await;
            }
        }

        Err(AxonError::Router(
            "No available agents for broadcast".into(),
        ))
    }

    /// Queue a message for async processing
    pub async fn queue(&self, message: LlmMessage) -> Result<()> {
        self.sender
            .send(RouterMessage {
                message,
                response_tx: None,
            })
            .await
            .map_err(|_| AxonError::Router("Failed to queue message".into()))
    }

    /// Queue a message and get a channel for the response
    pub async fn queue_with_response(
        &self,
        message: LlmMessage,
    ) -> Result<mpsc::Receiver<Result<LlmMessage>>> {
        let (tx, rx) = mpsc::channel(1);

        self.sender
            .send(RouterMessage {
                message,
                response_tx: Some(tx),
            })
            .await
            .map_err(|_| AxonError::Router("Failed to queue message".into()))?;

        Ok(rx)
    }

    /// Process the next message in the queue
    pub async fn process_next(&self) -> Option<Result<LlmMessage>> {
        let mut receiver = self.receiver.write().await;

        if let Some(router_msg) = receiver.recv().await {
            let result = self.send(router_msg.message).await;

            // Send response if channel provided
            if let Some(tx) = router_msg.response_tx {
                // Clone the Ok value or convert error to string for the channel
                let channel_result = match &result {
                    Ok(msg) => Ok(msg.clone()),
                    Err(e) => Err(AxonError::Router(e.to_string())),
                };
                let _ = tx.send(channel_result).await;
            }

            Some(result)
        } else {
            None
        }
    }

    /// Run the router loop (blocking)
    pub async fn run(&self) {
        loop {
            if self.process_next().await.is_none() {
                break;
            }
        }
    }

    /// Get router statistics
    pub async fn stats(&self) -> RouterStats {
        let adapters = self.adapters.read().await;
        let conversations = self.conversations.read().await;

        RouterStats {
            registered_agents: adapters.len(),
            active_conversations: conversations.values().filter(|c| c.is_active()).count(),
            total_conversations: conversations.len(),
        }
    }
}

impl Default for MessageRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// Router statistics
#[derive(Debug, Clone)]
pub struct RouterStats {
    pub registered_agents: usize,
    pub active_conversations: usize,
    pub total_conversations: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{AgentConfig, MessageContent, MessageType, Provider};
    use async_trait::async_trait;

    // Mock adapter for testing
    struct MockAdapter {
        id: AgentId,
        response: String,
    }

    impl MockAdapter {
        fn new(id: impl Into<AgentId>, response: impl Into<String>) -> Self {
            Self {
                id: id.into(),
                response: response.into(),
            }
        }
    }

    #[async_trait]
    impl LlmAdapter for MockAdapter {
        fn agent_id(&self) -> &AgentId {
            &self.id
        }

        fn config(&self) -> &AgentConfig {
            // Return a dummy config for testing
            Box::leak(Box::new(AgentConfig::new(
                self.id.clone(),
                Provider::Anthropic,
                "mock-model",
            )))
        }

        async fn process(&self, message: &LlmMessage) -> Result<LlmMessage> {
            Ok(LlmMessage {
                id: uuid::Uuid::new_v4(),
                from: self.id.clone(),
                to: Some(message.from.clone()),
                message_type: MessageType::Chat,
                content: MessageContent::Text(self.response.clone()),
                conversation_id: message.conversation_id,
                timestamp: chrono::Utc::now(),
                metadata: None,
            })
        }
    }

    #[tokio::test]
    async fn test_router_creation() {
        let router = MessageRouter::new();
        let stats = router.stats().await;

        assert_eq!(stats.registered_agents, 0);
        assert_eq!(stats.active_conversations, 0);
    }

    #[tokio::test]
    async fn test_register_adapter() {
        let router = MessageRouter::new();
        let adapter = MockAdapter::new("claude", "Hello!");

        router.register_adapter(Box::new(adapter)).await;

        assert!(router.has_agent("claude").await);
        assert!(!router.has_agent("gemini").await);
    }

    #[tokio::test]
    async fn test_list_agents() {
        let router = MessageRouter::new();
        router
            .register_adapter(Box::new(MockAdapter::new("claude", "Hi")))
            .await;
        router
            .register_adapter(Box::new(MockAdapter::new("gemini", "Hello")))
            .await;

        let agents = router.list_agents().await;
        assert_eq!(agents.len(), 2);
        assert!(agents.contains(&"claude".to_string()));
        assert!(agents.contains(&"gemini".to_string()));
    }

    #[tokio::test]
    async fn test_send_to_specific_agent() {
        let router = MessageRouter::new();
        router
            .register_adapter(Box::new(MockAdapter::new("user", "I'm user")))
            .await;
        router
            .register_adapter(Box::new(MockAdapter::new("claude", "I'm Claude!")))
            .await;

        let conv_id = uuid::Uuid::new_v4();
        let message = LlmMessage::chat("user", Some("claude".into()), "Hello", conv_id);

        let response = router.send(message).await.unwrap();

        assert_eq!(response.from.as_str(), "claude");
        assert_eq!(response.content.as_text(), "I'm Claude!");
    }

    #[tokio::test]
    async fn test_unregistered_sender() {
        let router = MessageRouter::new();

        let conv_id = uuid::Uuid::new_v4();
        let message = LlmMessage::chat("unknown", None, "Hello", conv_id);

        let result = router.send(message).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_unregister_adapter() {
        let router = MessageRouter::new();
        router
            .register_adapter(Box::new(MockAdapter::new("claude", "Hi")))
            .await;

        assert!(router.has_agent("claude").await);

        router.unregister_adapter("claude").await;

        assert!(!router.has_agent("claude").await);
    }
}
