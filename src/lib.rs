//! # Axon - LLM-to-LLM Communication Framework
//!
//! Axon (축삭) enables multiple LLM agents to communicate, collaborate,
//! and share tools through a CLI-based orchestration framework.
//!
//! ## Features
//!
//! - **Multi-Agent Communication**: Claude, Gemini, GPT, and local LLMs
//! - **Tool Sharing**: MCP-compatible function calling
//! - **CLI First**: Pipeline-friendly, scriptable interface
//! - **MinKy Integration**: Knowledge search via MinKy platform
//!
//! ## Quick Start
//!
//! ```bash
//! # Start router
//! axon serve
//!
//! # Register agents
//! axon agent add claude --provider anthropic --model claude-sonnet-4-20250514
//!
//! # Send message
//! axon send --from claude --to gemini "Review this code"
//! ```

pub mod error;
pub mod config;
pub mod adapters;
pub mod tools;

// Re-exports
pub use error::{AxonError, Result};
