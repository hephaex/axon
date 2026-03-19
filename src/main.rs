//! Axon CLI - LLM-to-LLM Communication Framework
//!
//! Entry point for the axon command-line interface.

use axon::adapters::{ClaudeAdapter, LlmAdapter};
use axon::protocol::{AgentConfig, Conversation, LlmMessage, Provider, TurnPolicy};
use axon::router::MessageRouter;
use axon::server::{start_server, ServerConfig, ServerState};
use axon::tools::minky::register_minky_tools;
use axon::tools::{MinkyConfig, ToolRegistry};
use clap::{Parser, Subcommand};
use std::io::{self, Read as IoRead};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

/// Axon - LLM-to-LLM Communication Framework
#[derive(Parser)]
#[command(name = "axon")]
#[command(author = "Mario Cho <hephaex@gmail.com>")]
#[command(version = "0.1.0")]
#[command(about = "LLM-to-LLM Communication Framework", long_about = None)]
struct Cli {
    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Configuration file path
    #[arg(short, long, global = true)]
    config: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the Axon router server
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value = "8090")]
        port: u16,

        /// Host to bind to
        #[arg(short = 'H', long, default_value = "127.0.0.1")]
        host: String,
    },

    /// Send a message between agents
    Send {
        /// Source agent
        #[arg(short, long)]
        from: String,

        /// Target agent (optional for broadcast)
        #[arg(short, long)]
        to: Option<String>,

        /// Message content
        message: String,
    },

    /// Start a multi-agent conversation
    Converse {
        /// Agents to include (comma-separated)
        #[arg(short, long)]
        agents: String,

        /// Conversation topic
        #[arg(short, long)]
        topic: String,

        /// Maximum turns
        #[arg(short, long, default_value = "10")]
        max_turns: usize,
    },

    /// Pipe input through agent chain
    Pipe {
        /// Agent chain (e.g., "claude:review -> gemini:security")
        #[arg(short, long)]
        chain: String,
    },

    /// Manage agents
    Agent {
        #[command(subcommand)]
        action: AgentAction,
    },

    /// Manage tools
    Tool {
        #[command(subcommand)]
        action: ToolAction,
    },
}

#[derive(Subcommand)]
enum AgentAction {
    /// Add a new agent
    Add {
        /// Agent name
        name: String,

        /// Provider (anthropic, google, openai, ollama)
        #[arg(short, long)]
        provider: String,

        /// Model name
        #[arg(short, long)]
        model: String,

        /// Custom endpoint (for Ollama)
        #[arg(short, long)]
        endpoint: Option<String>,
    },

    /// List registered agents
    List,

    /// Remove an agent
    Remove {
        /// Agent name
        name: String,
    },
}

#[derive(Subcommand)]
enum ToolAction {
    /// Add a new tool
    Add {
        /// Tool name
        name: String,

        /// Tool endpoint
        #[arg(short, long)]
        endpoint: String,
    },

    /// List registered tools
    List,

    /// Remove a tool
    Remove {
        /// Tool name
        name: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "axon=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = Cli::parse();

    if cli.verbose {
        tracing::info!("Verbose mode enabled");
    }

    match cli.command {
        Commands::Serve { port, host } => {
            tracing::info!("Starting Axon server on {}:{}", host, port);

            let config = ServerConfig {
                host,
                port,
                cors_permissive: true,
            };

            let state = ServerState::new();

            if let Err(e) = start_server(config, state).await {
                eprintln!("Server error: {}", e);
                std::process::exit(1);
            }
        }

        Commands::Send { from, to, message } => {
            let target = to.as_deref().unwrap_or("broadcast");
            tracing::info!("Sending message from {} to {}", from, target);

            // For MVP, we support Claude adapter only
            // TODO: Support other providers via config
            let config = AgentConfig::new(
                from.as_str(),
                Provider::Anthropic,
                "claude-sonnet-4-20250514",
            );

            let adapter: ClaudeAdapter = match ClaudeAdapter::new(config) {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    eprintln!("Hint: Set ANTHROPIC_API_KEY environment variable");
                    std::process::exit(1);
                }
            };

            // Create conversation ID
            let conv_id = Uuid::new_v4();

            // Build message (user → agent)
            let llm_message =
                LlmMessage::chat("user", Some(from.as_str().into()), &message, conv_id);

            // Send message and get response
            match adapter.process(&llm_message).await {
                Ok(response) => {
                    println!("{}", response.content.as_text());
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Converse {
            agents,
            topic,
            max_turns,
        } => {
            tracing::info!("Starting conversation with agents: {}", agents);

            // Parse agent list
            let agent_list: Vec<&str> = agents.split(',').map(|s| s.trim()).collect();
            if agent_list.len() < 2 {
                eprintln!("Error: At least 2 agents required for conversation");
                std::process::exit(1);
            }

            // Create router and register adapters
            let router = MessageRouter::new();

            for agent_name in &agent_list {
                let config =
                    AgentConfig::new(*agent_name, Provider::Anthropic, "claude-sonnet-4-20250514");

                match ClaudeAdapter::new(config) {
                    Ok(adapter) => {
                        router.register_adapter(Box::new(adapter)).await;
                    }
                    Err(e) => {
                        eprintln!("Error creating adapter for {}: {}", agent_name, e);
                        eprintln!("Hint: Set ANTHROPIC_API_KEY environment variable");
                        std::process::exit(1);
                    }
                }
            }

            // Create conversation
            let mut conversation =
                Conversation::new(agent_list.iter().map(|s| (*s).into()).collect())
                    .with_topic(&topic)
                    .with_turn_policy(TurnPolicy::RoundRobin)
                    .with_max_turns(max_turns);

            println!("=== Conversation: {} ===\n", topic);

            // Start with the topic as initial context
            let initial_message = LlmMessage::chat(
                "user",
                Some(agent_list[0].into()),
                format!("Let's discuss: {}. Start the conversation.", topic),
                conversation.id,
            );

            // First turn
            match router.send(initial_message.clone()).await {
                Ok(response) => {
                    println!(
                        "[{}]: {}\n",
                        response.from.as_str(),
                        response.content.as_text()
                    );
                    conversation.add_message(response.clone());

                    // Continue conversation loop
                    let mut current_message = response;

                    while conversation.is_active() && conversation.current_turn < max_turns {
                        // Get next speaker
                        let next_speaker = match conversation.get_next_speaker() {
                            Some(speaker) => speaker.clone(),
                            None => break,
                        };

                        // Create message from last response to next speaker
                        let msg = LlmMessage::chat(
                            current_message.from.as_str(),
                            Some(next_speaker.clone()),
                            current_message.content.as_text(),
                            conversation.id,
                        );

                        // Update conversation and get response
                        match router.send(msg).await {
                            Ok(response) => {
                                println!(
                                    "[{}]: {}\n",
                                    response.from.as_str(),
                                    response.content.as_text()
                                );
                                conversation.add_message(response.clone());
                                current_message = response;
                            }
                            Err(e) => {
                                eprintln!("Error: {}", e);
                                break;
                            }
                        }
                    }

                    println!(
                        "=== Conversation ended after {} turns ===",
                        conversation.current_turn
                    );
                }
                Err(e) => {
                    eprintln!("Error starting conversation: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Pipe { chain } => {
            tracing::info!("Piping through chain: {}", chain);

            // Read input from stdin
            let mut input = String::new();
            if let Err(e) = io::stdin().read_to_string(&mut input) {
                eprintln!("Error reading stdin: {}", e);
                std::process::exit(1);
            }

            if input.trim().is_empty() {
                eprintln!("Error: No input provided. Pipe content to stdin.");
                eprintln!("Usage: cat file.txt | axon pipe --chain \"claude:review\"");
                std::process::exit(1);
            }

            // Parse chain: "claude:review -> gemini:security"
            let stages: Vec<(&str, Option<&str>)> = chain
                .split("->")
                .map(|s| s.trim())
                .map(|s| {
                    if let Some((agent, task)) = s.split_once(':') {
                        (agent.trim(), Some(task.trim()))
                    } else {
                        (s, None)
                    }
                })
                .collect();

            if stages.is_empty() {
                eprintln!("Error: No agents specified in chain");
                std::process::exit(1);
            }

            // Create conversation ID
            let conv_id = Uuid::new_v4();
            let mut current_content = input.trim().to_string();

            // Process through each stage
            for (i, (agent_name, task)) in stages.iter().enumerate() {
                let stage_num = i + 1;
                let total_stages = stages.len();

                if cli.verbose {
                    eprintln!(
                        "[{}/{}] Processing with {}...",
                        stage_num, total_stages, agent_name
                    );
                }

                // Create adapter
                let config =
                    AgentConfig::new(*agent_name, Provider::Anthropic, "claude-sonnet-4-20250514");

                let adapter: ClaudeAdapter = match ClaudeAdapter::new(config) {
                    Ok(a) => a,
                    Err(e) => {
                        eprintln!("Error creating adapter for {}: {}", agent_name, e);
                        eprintln!("Hint: Set ANTHROPIC_API_KEY environment variable");
                        std::process::exit(1);
                    }
                };

                // Build prompt with task context
                let prompt = match task {
                    Some(t) => format!(
                        "Task: {}\n\nInput:\n{}\n\nProvide your response:",
                        t, current_content
                    ),
                    None => current_content.clone(),
                };

                // Create and send message
                let message = LlmMessage::chat("user", Some((*agent_name).into()), prompt, conv_id);

                match adapter.process(&message).await {
                    Ok(response) => {
                        current_content = response.content.as_text().to_string();
                    }
                    Err(e) => {
                        eprintln!("Error from {}: {}", agent_name, e);
                        std::process::exit(1);
                    }
                }
            }

            // Output final result
            println!("{}", current_content);
        }

        Commands::Agent { action } => match action {
            AgentAction::Add {
                name,
                provider,
                model,
                endpoint,
            } => {
                tracing::info!("Adding agent: {}", name);
                println!(
                    "Adding agent '{}' (provider: {}, model: {})",
                    name, provider, model
                );
                if let Some(ep) = endpoint {
                    println!("  Endpoint: {}", ep);
                }
                println!("(Not yet implemented)");
            }
            AgentAction::List => {
                println!("Registered agents:");
                println!("  (No agents registered yet)");
            }
            AgentAction::Remove { name } => {
                println!("Removing agent: {}", name);
                println!("(Not yet implemented)");
            }
        },

        Commands::Tool { action } => match action {
            ToolAction::Add { name, endpoint } => {
                tracing::info!("Adding tool: {}", name);

                // Create registry and add the tool
                let registry = ToolRegistry::new();

                match name.as_str() {
                    "minky" => {
                        let config = MinkyConfig::new(&endpoint);
                        if let Err(e) = register_minky_tools(&registry, config).await {
                            eprintln!("Error adding MinKy tools: {}", e);
                            std::process::exit(1);
                        }
                        println!("Added MinKy tools (endpoint: {})", endpoint);
                        println!("  - minky_search: Search knowledge base");
                        println!("  - minky_ask: RAG question answering");
                        println!("  - minky_get: Get document by ID");
                    }
                    _ => {
                        println!("Adding custom tool '{}' at {}", name, endpoint);
                        println!("Note: Custom tools require MCP server at the endpoint");
                        println!("(MCP integration not yet implemented)");
                    }
                }
            }
            ToolAction::List => {
                println!("Available Tools:\n");

                println!("Built-in Tools:");
                println!("  read_file     - Read file contents");
                println!("  write_file    - Write to a file (requires --allow-write)");
                println!("  list_dir      - List directory contents");
                println!("  web_fetch     - Fetch content from URL");
                println!();

                println!("MinKy Tools (requires --minky-endpoint):");
                println!("  minky_search  - Search knowledge base");
                println!("  minky_ask     - RAG question answering");
                println!("  minky_get     - Get document by ID");
                println!();

                println!("Usage:");
                println!("  axon converse --tools read_file,web_fetch ...");
                println!("  axon tool add minky --endpoint http://localhost:3000/api");
            }
            ToolAction::Remove { name } => {
                println!("Removing tool: {}", name);
                println!("Note: Built-in tools cannot be removed.");
                println!("To disable tools, simply don't include them in --tools option.");
            }
        },
    }

    Ok(())
}
