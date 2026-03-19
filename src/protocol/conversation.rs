//! Conversation management
//!
//! Types for managing multi-agent conversations.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use super::agent::AgentId;
use super::message::LlmMessage;

/// A conversation between multiple agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    /// Unique conversation identifier
    pub id: Uuid,

    /// Topic or title of the conversation
    pub topic: Option<String>,

    /// Participating agents
    pub participants: Vec<AgentId>,

    /// Turn policy for this conversation
    pub turn_policy: TurnPolicy,

    /// Current turn number
    pub current_turn: usize,

    /// Maximum number of turns (None = unlimited)
    pub max_turns: Option<usize>,

    /// Conversation status
    pub status: ConversationStatus,

    /// Message history
    pub messages: Vec<LlmMessage>,

    /// Who should speak next (for Directed policy)
    pub next_speaker: Option<AgentId>,

    /// Conversation metadata
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,

    /// When the conversation started
    pub started_at: DateTime<Utc>,

    /// When the conversation ended (if ended)
    pub ended_at: Option<DateTime<Utc>>,
}

impl Conversation {
    /// Create a new conversation
    pub fn new(participants: Vec<AgentId>) -> Self {
        Self {
            id: Uuid::new_v4(),
            topic: None,
            participants,
            turn_policy: TurnPolicy::default(),
            current_turn: 0,
            max_turns: None,
            status: ConversationStatus::Active,
            messages: Vec::new(),
            next_speaker: None,
            metadata: HashMap::new(),
            started_at: Utc::now(),
            ended_at: None,
        }
    }

    /// Create a new conversation with a topic
    pub fn with_topic(mut self, topic: impl Into<String>) -> Self {
        self.topic = Some(topic.into());
        self
    }

    /// Set the turn policy
    pub fn with_turn_policy(mut self, policy: TurnPolicy) -> Self {
        self.turn_policy = policy;
        self
    }

    /// Set the maximum number of turns
    pub fn with_max_turns(mut self, max: usize) -> Self {
        self.max_turns = Some(max);
        self
    }

    /// Add a message to the conversation
    pub fn add_message(&mut self, message: LlmMessage) {
        self.messages.push(message);
        self.current_turn += 1;

        // Check if we should end the conversation
        if let Some(max) = self.max_turns {
            if self.current_turn >= max {
                self.end(ConversationEndReason::MaxTurnsReached);
            }
        }
    }

    /// Get the next speaker based on turn policy
    pub fn get_next_speaker(&self) -> Option<&AgentId> {
        match &self.turn_policy {
            TurnPolicy::RoundRobin => {
                if self.participants.is_empty() {
                    return None;
                }
                let idx = self.current_turn % self.participants.len();
                Some(&self.participants[idx])
            }
            TurnPolicy::Directed => self.next_speaker.as_ref(),
            TurnPolicy::Free => None, // Anyone can speak
            TurnPolicy::LastSpeakerExcluded => {
                // Get last speaker and return someone else
                if let Some(last_msg) = self.messages.last() {
                    self.participants
                        .iter()
                        .find(|p| *p != &last_msg.from)
                } else {
                    self.participants.first()
                }
            }
        }
    }

    /// Set the next speaker (for Directed policy)
    pub fn set_next_speaker(&mut self, agent: AgentId) {
        self.next_speaker = Some(agent);
    }

    /// Check if an agent can speak
    pub fn can_speak(&self, agent: &AgentId) -> bool {
        if !self.participants.contains(agent) {
            return false;
        }

        match &self.turn_policy {
            TurnPolicy::RoundRobin => {
                self.get_next_speaker() == Some(agent)
            }
            TurnPolicy::Directed => {
                self.next_speaker.as_ref() == Some(agent)
            }
            TurnPolicy::Free => true,
            TurnPolicy::LastSpeakerExcluded => {
                self.messages.last().map(|m| &m.from) != Some(agent)
            }
        }
    }

    /// End the conversation
    pub fn end(&mut self, reason: ConversationEndReason) {
        self.status = ConversationStatus::Ended { reason };
        self.ended_at = Some(Utc::now());
    }

    /// Check if the conversation is active
    pub fn is_active(&self) -> bool {
        matches!(self.status, ConversationStatus::Active)
    }

    /// Get the last message
    pub fn last_message(&self) -> Option<&LlmMessage> {
        self.messages.last()
    }

    /// Get messages from a specific agent
    pub fn messages_from(&self, agent: &AgentId) -> Vec<&LlmMessage> {
        self.messages.iter().filter(|m| &m.from == agent).collect()
    }

    /// Get message count
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Get conversation duration
    pub fn duration(&self) -> chrono::Duration {
        let end = self.ended_at.unwrap_or_else(Utc::now);
        end - self.started_at
    }
}

/// Turn policy for conversations
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TurnPolicy {
    /// Agents speak in order, cycling through the list
    #[default]
    RoundRobin,

    /// Explicitly directed - next speaker is set by the orchestrator
    Directed,

    /// Anyone can speak at any time
    Free,

    /// Anyone except the last speaker can speak
    LastSpeakerExcluded,
}

impl std::fmt::Display for TurnPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RoundRobin => write!(f, "round_robin"),
            Self::Directed => write!(f, "directed"),
            Self::Free => write!(f, "free"),
            Self::LastSpeakerExcluded => write!(f, "last_speaker_excluded"),
        }
    }
}

/// Conversation status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ConversationStatus {
    /// Conversation is active
    Active,

    /// Conversation is paused
    Paused,

    /// Conversation has ended
    Ended {
        reason: ConversationEndReason,
    },
}

/// Reason for conversation ending
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ConversationEndReason {
    /// Reached maximum turns
    MaxTurnsReached,

    /// An agent requested to end
    AgentRequested,

    /// Timeout
    Timeout,

    /// Task completed
    TaskCompleted,

    /// Error occurred
    Error,

    /// Manually stopped
    ManualStop,
}

/// Builder for creating conversations
#[derive(Debug, Default)]
pub struct ConversationBuilder {
    topic: Option<String>,
    participants: Vec<AgentId>,
    turn_policy: TurnPolicy,
    max_turns: Option<usize>,
}

impl ConversationBuilder {
    /// Create a new conversation builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the conversation topic
    pub fn topic(mut self, topic: impl Into<String>) -> Self {
        self.topic = Some(topic.into());
        self
    }

    /// Add a participant
    pub fn participant(mut self, agent: impl Into<AgentId>) -> Self {
        self.participants.push(agent.into());
        self
    }

    /// Add multiple participants
    pub fn participants(mut self, agents: impl IntoIterator<Item = impl Into<AgentId>>) -> Self {
        self.participants.extend(agents.into_iter().map(Into::into));
        self
    }

    /// Set the turn policy
    pub fn turn_policy(mut self, policy: TurnPolicy) -> Self {
        self.turn_policy = policy;
        self
    }

    /// Set the maximum number of turns
    pub fn max_turns(mut self, max: usize) -> Self {
        self.max_turns = Some(max);
        self
    }

    /// Build the conversation
    pub fn build(self) -> Conversation {
        let mut conv = Conversation::new(self.participants)
            .with_turn_policy(self.turn_policy);

        if let Some(topic) = self.topic {
            conv = conv.with_topic(topic);
        }

        if let Some(max) = self.max_turns {
            conv = conv.with_max_turns(max);
        }

        conv
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conversation_creation() {
        let agents = vec![AgentId::new("claude"), AgentId::new("gemini")];
        let conv = Conversation::new(agents.clone());

        assert_eq!(conv.participants.len(), 2);
        assert!(conv.is_active());
        assert_eq!(conv.current_turn, 0);
    }

    #[test]
    fn test_conversation_with_topic() {
        let conv = Conversation::new(vec![AgentId::new("claude")])
            .with_topic("Code Review");

        assert_eq!(conv.topic, Some("Code Review".to_string()));
    }

    #[test]
    fn test_round_robin_policy() {
        let agents = vec![
            AgentId::new("agent1"),
            AgentId::new("agent2"),
            AgentId::new("agent3"),
        ];
        let conv = Conversation::new(agents);

        // First turn should be agent1
        assert_eq!(conv.get_next_speaker().unwrap().as_str(), "agent1");

        // After adding a message, should be agent2
        let mut conv = conv;
        let msg = LlmMessage::chat("agent1", None, "Hello", conv.id);
        conv.add_message(msg);
        assert_eq!(conv.get_next_speaker().unwrap().as_str(), "agent2");
    }

    #[test]
    fn test_directed_policy() {
        let agents = vec![AgentId::new("claude"), AgentId::new("gemini")];
        let mut conv = Conversation::new(agents)
            .with_turn_policy(TurnPolicy::Directed);

        // No next speaker initially
        assert!(conv.get_next_speaker().is_none());

        // Set next speaker
        conv.set_next_speaker(AgentId::new("gemini"));
        assert_eq!(conv.get_next_speaker().unwrap().as_str(), "gemini");
    }

    #[test]
    fn test_free_policy() {
        let agents = vec![AgentId::new("claude"), AgentId::new("gemini")];
        let conv = Conversation::new(agents.clone())
            .with_turn_policy(TurnPolicy::Free);

        // Anyone can speak
        assert!(conv.can_speak(&agents[0]));
        assert!(conv.can_speak(&agents[1]));
    }

    #[test]
    fn test_max_turns() {
        let agents = vec![AgentId::new("claude")];
        let mut conv = Conversation::new(agents.clone())
            .with_max_turns(2);

        let msg1 = LlmMessage::chat("claude", None, "Message 1", conv.id);
        conv.add_message(msg1);
        assert!(conv.is_active());

        let msg2 = LlmMessage::chat("claude", None, "Message 2", conv.id);
        conv.add_message(msg2);
        assert!(!conv.is_active());
        assert!(matches!(
            conv.status,
            ConversationStatus::Ended { reason: ConversationEndReason::MaxTurnsReached }
        ));
    }

    #[test]
    fn test_conversation_builder() {
        let conv = ConversationBuilder::new()
            .topic("Architecture Discussion")
            .participant("claude")
            .participant("gemini")
            .turn_policy(TurnPolicy::RoundRobin)
            .max_turns(10)
            .build();

        assert_eq!(conv.topic, Some("Architecture Discussion".to_string()));
        assert_eq!(conv.participants.len(), 2);
        assert_eq!(conv.turn_policy, TurnPolicy::RoundRobin);
        assert_eq!(conv.max_turns, Some(10));
    }

    #[test]
    fn test_messages_from_agent() {
        let mut conv = Conversation::new(vec![
            AgentId::new("claude"),
            AgentId::new("gemini"),
        ]);

        conv.add_message(LlmMessage::chat("claude", None, "Hello", conv.id));
        conv.add_message(LlmMessage::chat("gemini", None, "Hi", conv.id));
        conv.add_message(LlmMessage::chat("claude", None, "How are you?", conv.id));

        let claude_msgs = conv.messages_from(&AgentId::new("claude"));
        assert_eq!(claude_msgs.len(), 2);
    }

    #[test]
    fn test_conversation_serialization() {
        let conv = ConversationBuilder::new()
            .topic("Test")
            .participant("claude")
            .build();

        let json = serde_json::to_string(&conv).unwrap();
        let deserialized: Conversation = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.topic, conv.topic);
        assert_eq!(deserialized.participants.len(), 1);
    }
}
