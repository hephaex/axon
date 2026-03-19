//! Message routing and orchestration
//!
//! Components for routing messages between agents:
//! - `MessageRouter` - Routes messages to appropriate agents
//! - `ConversationOrchestrator` - Manages multi-agent conversations
//! - `MessageQueue` - Async message queue

#[allow(clippy::module_inception)]
pub mod router;

pub use router::{MessageRouter, RouterStats};
