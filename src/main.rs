//! Axon CLI - LLM-to-LLM Communication Framework
//!
//! Entry point for the axon command-line interface.

use axon::adapters::{ClaudeAdapter, LlmAdapter, StreamingAdapter};
use axon::protocol::{
    AgentConfig, Conversation, LlmMessage, MessageContent, MessageType, Provider, TurnPolicy,
};
use axon::router::MessageRouter;
use axon::server::{start_server, ServerConfig, ServerState};
use axon::tools::filesystem::{FilesystemConfig, ListDirTool, ReadFileTool, WriteFileTool};
use axon::tools::minky::register_minky_tools;
use axon::tools::web::{WebConfig, WebFetchTool};
use axon::tools::{MinkyConfig, ToolDefinition, ToolRegistry};
use clap::{Parser, Subcommand};
use futures::StreamExt;
use std::io::{self, Read as IoRead};
use std::path::PathBuf;
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

        /// Enable streaming output
        #[arg(short, long)]
        stream: bool,

        /// Tools to enable (comma-separated: read_file,web_fetch,list_dir,write_file)
        #[arg(long, value_delimiter = ',')]
        tools: Option<Vec<String>>,

        /// Allow write operations for file tools
        #[arg(long)]
        allow_write: bool,

        /// Base directory for file operations (default: current dir)
        #[arg(long)]
        base_dir: Option<PathBuf>,

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

        /// Tools to enable (comma-separated: read_file,web_fetch,list_dir,write_file)
        #[arg(long, value_delimiter = ',')]
        tools: Option<Vec<String>>,

        /// Allow write operations for file tools
        #[arg(long)]
        allow_write: bool,

        /// Base directory for file operations (default: current dir)
        #[arg(long)]
        base_dir: Option<PathBuf>,
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

/// Setup tool registry based on requested tools
async fn setup_tools(
    tool_names: &[String],
    allow_write: bool,
    base_dir: Option<PathBuf>,
) -> anyhow::Result<ToolRegistry> {
    let registry = ToolRegistry::new();

    let fs_config = FilesystemConfig {
        base_dir: base_dir
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))),
        allow_write,
        max_read_size: 1024 * 1024, // 1MB
    };

    for name in tool_names {
        match name.as_str() {
            "read_file" => {
                registry
                    .register(ReadFileTool::new(fs_config.clone()))
                    .await;
            }
            "write_file" => {
                if allow_write {
                    registry
                        .register(WriteFileTool::new(fs_config.clone()))
                        .await;
                } else {
                    eprintln!("Warning: write_file requires --allow-write flag");
                }
            }
            "list_dir" => {
                registry.register(ListDirTool::new(fs_config.clone())).await;
            }
            "web_fetch" => {
                let web_config = WebConfig::default();
                if let Ok(tool) = WebFetchTool::new(web_config) {
                    registry.register(tool).await;
                }
            }
            _ => {
                eprintln!("Warning: Unknown tool '{}', skipping", name);
            }
        }
    }

    Ok(registry)
}

/// Get tool definitions from registry for adapter registration
async fn get_tool_definitions(registry: &ToolRegistry) -> Vec<ToolDefinition> {
    registry.list().await
}

/// Execute a tool call and return the result
async fn execute_tool_call(
    registry: &ToolRegistry,
    tool_name: &str,
    args: &serde_json::Value,
    verbose: bool,
) -> String {
    if verbose {
        eprintln!("[Tool Call] {} with args: {}", tool_name, args);
    }

    match registry.execute(tool_name, args.clone()).await {
        Ok(result) => {
            if result.success {
                result.content
            } else {
                format!(
                    "Error: {}",
                    result.error.unwrap_or_else(|| "Unknown error".to_string())
                )
            }
        }
        Err(e) => format!("Tool execution failed: {}", e),
    }
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

        Commands::Send {
            from,
            to,
            stream: use_stream,
            tools: tool_names,
            allow_write,
            base_dir,
            message,
        } => {
            let target = to.as_deref().unwrap_or("broadcast");
            tracing::info!("Sending message from {} to {}", from, target);

            // Setup tools if requested
            let tool_registry = if let Some(ref names) = tool_names {
                Some(setup_tools(names, allow_write, base_dir).await?)
            } else {
                None
            };

            // For MVP, we support Claude adapter only
            // TODO: Support other providers via config
            let config = AgentConfig::new(
                from.as_str(),
                Provider::Anthropic,
                "claude-sonnet-4-20250514",
            );

            let mut adapter: ClaudeAdapter = match ClaudeAdapter::new(config) {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    eprintln!("Hint: Set ANTHROPIC_API_KEY environment variable");
                    std::process::exit(1);
                }
            };

            // Register tools with adapter
            if let Some(ref registry) = tool_registry {
                let tool_defs = get_tool_definitions(registry).await;
                if cli.verbose && !tool_defs.is_empty() {
                    eprintln!(
                        "[Tools enabled: {}]",
                        tool_defs
                            .iter()
                            .map(|t| t.name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
                for tool_def in tool_defs {
                    adapter.register_tool(tool_def);
                }
            }

            // Create conversation ID
            let conv_id = Uuid::new_v4();

            // Build message (user → agent)
            let llm_message =
                LlmMessage::chat("user", Some(from.as_str().into()), &message, conv_id);

            if use_stream && tool_registry.is_none() {
                // Streaming mode (only when no tools - tool calls require non-streaming)
                use std::io::Write;

                match adapter.process_stream(&llm_message).await {
                    Ok(mut stream) => {
                        while let Some(result) = stream.next().await {
                            match result {
                                Ok(chunk) => {
                                    if !chunk.delta.is_empty() {
                                        print!("{}", chunk.delta);
                                        // Flush to show output immediately
                                        std::io::stdout().flush().ok();
                                    }
                                    if chunk.is_final {
                                        println!(); // Final newline
                                        if cli.verbose {
                                            if let Some(usage) = chunk.usage {
                                                eprintln!(
                                                    "\n[tokens: {} in, {} out]",
                                                    usage.input_tokens, usage.output_tokens
                                                );
                                            }
                                            if let Some(reason) = chunk.stop_reason {
                                                eprintln!("[stop_reason: {}]", reason);
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!("\nStream error: {}", e);
                                    std::process::exit(1);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                // Non-streaming mode (required for tool use)
                if use_stream && tool_registry.is_some() {
                    eprintln!("Note: Streaming disabled when using tools (tool calls require full response)");
                }

                let mut history: Vec<LlmMessage> = Vec::new();
                let mut current_message = llm_message;
                let max_tool_iterations = 10; // Prevent infinite loops

                for iteration in 0..max_tool_iterations {
                    let response = match adapter
                        .process_with_history(&current_message, &history)
                        .await
                    {
                        Ok(r) => r,
                        Err(e) => {
                            eprintln!("Error: {}", e);
                            std::process::exit(1);
                        }
                    };

                    // Check if this is a tool call
                    if let MessageType::ToolCall { ref tool, call_id } = response.message_type {
                        if let Some(ref registry) = tool_registry {
                            if cli.verbose {
                                eprintln!("[Iteration {}] Tool call: {}", iteration + 1, tool);
                            }

                            // Extract tool arguments
                            let args = match &response.content {
                                MessageContent::Json(v) => v.clone(),
                                MessageContent::Text(t) => {
                                    serde_json::from_str(t).unwrap_or(serde_json::json!({}))
                                }
                                _ => serde_json::json!({}),
                            };

                            // Execute the tool
                            let tool_result =
                                execute_tool_call(registry, tool, &args, cli.verbose).await;

                            if cli.verbose {
                                eprintln!(
                                    "[Tool Result] {}",
                                    &tool_result[..tool_result.len().min(200)]
                                );
                            }

                            // Add assistant's tool call to history
                            history.push(current_message.clone());
                            history.push(response.clone());

                            // Create tool result message
                            current_message = LlmMessage {
                                id: Uuid::new_v4(),
                                from: "tool".into(),
                                to: Some(from.clone().into()),
                                message_type: MessageType::ToolResult {
                                    call_id,
                                    success: true,
                                },
                                content: MessageContent::Text(tool_result),
                                conversation_id: conv_id,
                                timestamp: chrono::Utc::now(),
                                metadata: None,
                            };

                            continue; // Continue the loop to get final response
                        }
                    }

                    // Not a tool call - print the final response
                    println!("{}", response.content.as_text());
                    break;
                }
            }
        }

        Commands::Converse {
            agents,
            topic,
            max_turns,
            tools: tool_names,
            allow_write,
            base_dir,
        } => {
            tracing::info!("Starting conversation with agents: {}", agents);

            // Setup tools if requested
            let tool_registry = if let Some(ref names) = tool_names {
                Some(setup_tools(names, allow_write, base_dir).await?)
            } else {
                None
            };

            // Parse agent list
            let agent_list: Vec<&str> = agents.split(',').map(|s| s.trim()).collect();
            if agent_list.len() < 2 {
                eprintln!("Error: At least 2 agents required for conversation");
                std::process::exit(1);
            }

            // Create router and register adapters
            let router = MessageRouter::new();

            // Get tool definitions if registry exists
            let tool_defs = if let Some(ref registry) = tool_registry {
                let defs = get_tool_definitions(registry).await;
                if cli.verbose && !defs.is_empty() {
                    eprintln!(
                        "[Tools enabled: {}]",
                        defs.iter()
                            .map(|t| t.name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
                defs
            } else {
                Vec::new()
            };

            for agent_name in &agent_list {
                let config =
                    AgentConfig::new(*agent_name, Provider::Anthropic, "claude-sonnet-4-20250514");

                match ClaudeAdapter::new(config) {
                    Ok(mut adapter) => {
                        // Register tools with this adapter
                        for tool_def in &tool_defs {
                            adapter.register_tool(tool_def.clone());
                        }
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
