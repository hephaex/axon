# Axon

[![CI](https://github.com/hephaex/axon/actions/workflows/ci.yml/badge.svg)](https://github.com/hephaex/axon/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Docker](https://img.shields.io/badge/Docker-Ready-blue.svg)](https://github.com/hephaex/axon)

> LLM-to-LLM Communication Framework

Axon enables multiple LLM agents to communicate, collaborate, and orchestrate complex workflows. Built in Rust for performance and reliability.

## Features

- **Multi-Provider Support** - Claude, GPT, Gemini, Ollama (local)
- **Multi-Agent Conversations** - RoundRobin, Directed, Free turn policies
- **Streaming Responses** - Real-time SSE/WebSocket streaming
- **Tool Integration** - Built-in tools + MCP-compatible extensions
- **Pipeline Mode** - Chain agents for sequential processing
- **HTTP API & WebSocket** - Server mode for integration
- **Persistence** - Save/load conversations
- **Reliability** - Retry, rate limiting, exponential backoff

## Installation

```bash
# Clone the repository
git clone https://github.com/hephaex/axon.git
cd axon

# Build
cargo build --release

# Install globally (optional)
cargo install --path .
```

## Quick Start

### Send a Message

```bash
# Set your API key
export ANTHROPIC_API_KEY="your-key"

# Send a single message
axon send --from user --to claude "Explain quantum computing in simple terms"

# Stream output in real-time
axon send --from user --stream "Write a short poem about coding"
```

### Tool-Enabled Messages

```bash
# Enable file system tools
axon send --from user --tools read_file,list_dir "List the files in the current directory and read README.md"

# Enable multiple tools including web fetch
axon send --from user --tools read_file,web_fetch "Summarize the content from https://example.com"

# Enable shell commands (restricted to safe commands)
axon send --from user --tools shell "Run 'git status' and 'ls -la'"

# Enable write operations (sandboxed to base-dir)
axon send --from user --tools read_file,write_file --allow-write --base-dir ./output "Create a summary file"

# Tools in multi-agent conversations
axon converse --agents "analyst,critic" --tools read_file,shell --topic "Review code in src/"
```

### Multi-Agent Conversation

```bash
# Start a conversation between agents
axon converse --agents "analyst,critic" --topic "Pros and cons of microservices" --max-turns 6
```

### Pipeline Mode

```bash
# Chain agents for sequential processing
cat code.rs | axon pipe --chain "claude:review -> claude:security"

# With task descriptions
echo "Build a REST API" | axon pipe --chain "architect:design -> developer:implement"
```

### Server Mode

```bash
# Start the HTTP/WebSocket server
axon serve --port 8090

# Health check
curl http://localhost:8090/health

# Send message via API
curl -X POST http://localhost:8090/api/send \
  -H "Content-Type: application/json" \
  -d '{"from": "user", "to": "claude", "content": "Hello!"}'
```

## CLI Commands

| Command | Description |
|---------|-------------|
| `axon send` | Send a single message to an agent (supports `--stream`, `--tools`) |
| `axon converse` | Start multi-agent conversation (supports `--tools`) |
| `axon pipe` | Pipeline mode (stdin → agents → stdout) |
| `axon serve` | Start HTTP/WebSocket server |
| `axon agent add/list/remove` | Manage agents |
| `axon tool add/list/remove` | Manage tools |

## API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Health check |
| `/api/send` | POST | Send message to agent |
| `/api/agents` | GET | List registered agents |
| `/api/agents` | POST | Register new agent |
| `/api/agents/:id` | DELETE | Remove agent |
| `/api/stats` | GET | Router statistics |
| `/ws` | GET | WebSocket for streaming |

### WebSocket Protocol

```json
// Request
{"type": "send_stream", "from": "user", "to": "claude", "content": "Hello"}

// Response (streaming)
{"type": "chunk", "conversation_id": "...", "delta": "Hello", "is_final": false}
{"type": "complete", "conversation_id": "...", "content": "Hello! How can I help?"}
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         CLI / API                            │
├─────────────────────────────────────────────────────────────┤
│                      Message Router                          │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐        │
│  │ Claude  │  │  GPT    │  │ Gemini  │  │ Ollama  │        │
│  │ Adapter │  │ Adapter │  │ Adapter │  │ Adapter │        │
│  └─────────┘  └─────────┘  └─────────┘  └─────────┘        │
├─────────────────────────────────────────────────────────────┤
│                     Tool Registry                            │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐    │
│  │ read_file│  │web_fetch │  │  minky   │  │  custom  │    │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘    │
├─────────────────────────────────────────────────────────────┤
│              Persistence / Streaming / Reliability           │
└─────────────────────────────────────────────────────────────┘
```

## Supported Providers

| Provider | Environment Variable | Models |
|----------|---------------------|--------|
| Anthropic | `ANTHROPIC_API_KEY` | claude-sonnet-4-20250514, etc. |
| OpenAI | `OPENAI_API_KEY` | gpt-4o, gpt-4-turbo, etc. |
| Google | `GOOGLE_API_KEY` | gemini-pro, gemini-1.5-pro, etc. |
| Ollama | (none - local) | llama3, mistral, etc. |

## Built-in Tools

| Tool | Description |
|------|-------------|
| `read_file` | Read file contents |
| `write_file` | Write to a file |
| `list_dir` | List directory contents |
| `web_fetch` | Fetch content from URL |
| `shell` | Execute shell commands (restricted to safe commands) |
| `minky_search` | Search MinKy knowledge base |
| `minky_ask` | RAG question answering |

## Configuration

```bash
# Environment variables
export ANTHROPIC_API_KEY="sk-ant-..."
export OPENAI_API_KEY="sk-..."
export GOOGLE_API_KEY="..."

# Ollama (local, no key needed)
ollama serve  # Start Ollama first
```

### Configuration File

Create `~/.axon/config.toml` or `./axon.toml`:

```toml
[server]
port = 8090
host = "127.0.0.1"
log_level = "info"

[agents.claude]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"

[agents.llama]
provider = "ollama"
model = "llama3.2"
endpoint = "http://localhost:11434"
```

See `axon.example.toml` for more options.

## Docker Deployment

### Quick Start with Docker

```bash
# Build image
docker build -t axon .

# Run with API key
docker run -d \
  -p 8090:8090 \
  -e ANTHROPIC_API_KEY="your-key" \
  --name axon \
  axon

# Check health
curl http://localhost:8090/health
```

### Docker Compose

```bash
# Copy environment file
cp .env.example .env
# Edit .env with your API keys

# Start Axon server
docker compose up -d

# Start with local Ollama
docker compose --profile with-ollama up -d

# View logs
docker compose logs -f axon

# Stop
docker compose down
```

### Docker Compose with Custom Config

```bash
# Create config file
cp axon.example.toml axon.toml

# Start with mounted config
docker compose up -d
```

## Development

```bash
# Run tests
cargo test

# Run with verbose logging
RUST_LOG=axon=debug axon send --from user "Hello"

# Check code
cargo clippy
cargo fmt
```

## License

MIT

## Author

Mario Cho ([@hephaex](https://github.com/hephaex))
