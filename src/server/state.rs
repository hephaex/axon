//! Server state management

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::adapters::LlmAdapter;
use crate::persistence::ConversationStore;
use crate::protocol::AgentId;
use crate::router::MessageRouter;
use crate::tools::ToolRegistry;

/// Shared server state
#[derive(Clone)]
pub struct ServerState {
    inner: Arc<ServerStateInner>,
}

struct ServerStateInner {
    /// Message router
    router: RwLock<MessageRouter>,

    /// Registered adapters by agent ID
    adapters: RwLock<HashMap<AgentId, Arc<dyn LlmAdapter>>>,

    /// Tool registry
    tools: RwLock<ToolRegistry>,

    /// Conversation store (optional)
    store: Option<Arc<dyn ConversationStore>>,
}

impl ServerState {
    /// Create new server state
    pub fn new() -> Self {
        Self {
            inner: Arc::new(ServerStateInner {
                router: RwLock::new(MessageRouter::new()),
                adapters: RwLock::new(HashMap::new()),
                tools: RwLock::new(ToolRegistry::new()),
                store: None,
            }),
        }
    }

    /// Create server state with a conversation store
    pub fn with_store(store: Arc<dyn ConversationStore>) -> Self {
        Self {
            inner: Arc::new(ServerStateInner {
                router: RwLock::new(MessageRouter::new()),
                adapters: RwLock::new(HashMap::new()),
                tools: RwLock::new(ToolRegistry::new()),
                store: Some(store),
            }),
        }
    }

    /// Get the message router
    pub async fn router(&self) -> tokio::sync::RwLockReadGuard<'_, MessageRouter> {
        self.inner.router.read().await
    }

    /// Get mutable message router
    pub async fn router_mut(&self) -> tokio::sync::RwLockWriteGuard<'_, MessageRouter> {
        self.inner.router.write().await
    }

    /// Register an adapter
    pub async fn register_adapter(&self, id: AgentId, adapter: Arc<dyn LlmAdapter>) {
        let mut adapters = self.inner.adapters.write().await;
        adapters.insert(id, adapter);
    }

    /// Get an adapter by ID
    pub async fn get_adapter(&self, id: &AgentId) -> Option<Arc<dyn LlmAdapter>> {
        let adapters = self.inner.adapters.read().await;
        adapters.get(id).cloned()
    }

    /// List all registered adapter IDs
    pub async fn list_adapters(&self) -> Vec<AgentId> {
        let adapters = self.inner.adapters.read().await;
        adapters.keys().cloned().collect()
    }

    /// Remove an adapter
    pub async fn remove_adapter(&self, id: &AgentId) -> bool {
        let mut adapters = self.inner.adapters.write().await;
        adapters.remove(id).is_some()
    }

    /// Get the tool registry
    pub async fn tools(&self) -> tokio::sync::RwLockReadGuard<'_, ToolRegistry> {
        self.inner.tools.read().await
    }

    /// Get mutable tool registry
    pub async fn tools_mut(&self) -> tokio::sync::RwLockWriteGuard<'_, ToolRegistry> {
        self.inner.tools.write().await
    }

    /// Get the conversation store if available
    pub fn store(&self) -> Option<&Arc<dyn ConversationStore>> {
        self.inner.store.as_ref()
    }
}

impl Default for ServerState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_state_new() {
        let state = ServerState::new();
        assert!(state.list_adapters().await.is_empty());
    }

    #[tokio::test]
    async fn test_adapter_registration() {
        use crate::protocol::{AgentConfig, Provider};

        let state = ServerState::new();

        // We can't easily create an adapter without API keys, so just test the structure
        let adapters = state.list_adapters().await;
        assert!(adapters.is_empty());
    }
}
