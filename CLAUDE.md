# Axon - LLM-to-LLM Communication Framework

> **Axon** (축삭): 뉴런에서 다른 뉴런으로 신호를 전달하는 부분

---

## 🚀 Quick Start (Claude Code 세션)

**세션 시작 시 반드시 읽어야 할 파일:**

```
1. PLAN.md      → 무슨 일을 해야 하는지 (할 일 목록)
2. PROGRESS.md  → 어디까지 진행되었는지 (완료된 작업)
```

### 현재 상태 (2026-03-19)

- ✅ Phase 1.1 완료: 프로젝트 초기화
- ✅ Phase 1.2 완료: Message Protocol
- 🟡 Phase 1.4 진행 예정: Claude Adapter
- 📦 테스트 21개 통과, Clippy 통과

### 다음 작업

1. `src/adapters/claude.rs` - Claude API adapter
2. LlmAdapter trait에 LlmMessage 적용
3. `axon send` 실제 기능 구현

### 빌드 & 테스트

```bash
cargo build              # 빌드
cargo test               # 테스트
cargo clippy             # 린트
cargo run -- --help      # CLI 확인
```

### Git Commit Rules

- 작성자: Mario Cho (hephaex@gmail.com) 단독
- Co-Authored-By 추가 금지
- AI 생성 표시 문구 추가 금지

---

## Project Vision

Axon은 여러 LLM 에이전트가 서로 대화하고, 협력하며, 도구를 공유할 수 있는 **CLI 기반 LLM 오케스트레이션 프레임워크**입니다.

### 핵심 철학

```
LLM A ──signal──▶ Axon Router ──signal──▶ LLM B
   ▲                  │                      │
   └──────────────────┴──────────────────────┘
              (bidirectional communication)
```

- **Agent Agnostic**: Claude, Gemini, GPT, Llama 등 모든 LLM 지원
- **CLI First**: 파이프라인 친화적, 스크립트 자동화 가능
- **Tool Sharing**: MCP/Function Calling 통합
- **MinKy Integration**: 지식 검색 도구로 MinKy 연동

---

## Quick Start (목표)

```bash
# 라우터 시작
axon serve

# 에이전트 등록
axon agent add claude --provider anthropic --model claude-sonnet-4-20250514
axon agent add gemini --provider google --model gemini-pro

# 단일 메시지
axon send --from claude --to gemini "이 아키텍처를 검토해 줘"

# 멀티 에이전트 대화
axon converse --agents claude,gemini --topic "MinKy 성능 최적화"

# 파이프라인
cat code.rs | axon pipe --chain "claude:review -> gemini:security"

# MinKy 도구 연결
axon tool add minky --endpoint http://localhost:3000/api
```

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              Axon                                        │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │                        CLI Interface                             │    │
│  │   axon serve | send | converse | pipe | agent | tool            │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                                    │                                     │
│                                    ▼                                     │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │                      Message Router                              │    │
│  │   - Message queuing (tokio mpsc)                                │    │
│  │   - Conversation orchestration                                   │    │
│  │   - Turn management                                              │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│         │                    │                    │                      │
│         ▼                    ▼                    ▼                      │
│  ┌─────────────┐     ┌─────────────┐     ┌─────────────┐               │
│  │   Claude    │     │   Gemini    │     │    GPT      │               │
│  │   Adapter   │     │   Adapter   │     │   Adapter   │               │
│  └─────────────┘     └─────────────┘     └─────────────┘               │
│         │                    │                    │                      │
│         └────────────────────┼────────────────────┘                      │
│                              ▼                                           │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │                      Tool Registry                               │    │
│  │   - MinKy (search, knowledge graph)                             │    │
│  │   - File system                                                  │    │
│  │   - Web fetch                                                    │    │
│  │   - Custom MCP servers                                          │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Tech Stack

- **Language**: Rust
- **CLI**: clap v4
- **Async**: tokio
- **HTTP Client**: reqwest
- **WebSocket**: tokio-tungstenite (선택)
- **Serialization**: serde + serde_json
- **Config**: toml

---

## Directory Structure

```
axon/
├── CLAUDE.md                 # 프로젝트 가이드 (이 파일)
├── PLAN.md                   # 개발 계획
├── PROGRESS.md               # 진행 상황
├── Cargo.toml
├── src/
│   ├── main.rs               # CLI 진입점
│   ├── lib.rs                # 라이브러리 루트
│   ├── cli/
│   │   ├── mod.rs
│   │   ├── serve.rs          # axon serve
│   │   ├── send.rs           # axon send
│   │   ├── converse.rs       # axon converse
│   │   ├── pipe.rs           # axon pipe
│   │   ├── agent.rs          # axon agent add/remove/list
│   │   └── tool.rs           # axon tool add/remove/list
│   ├── protocol/
│   │   ├── mod.rs
│   │   ├── message.rs        # LlmMessage, MessageType
│   │   ├── agent.rs          # AgentId, AgentConfig
│   │   └── conversation.rs   # Conversation, Turn
│   ├── router/
│   │   ├── mod.rs
│   │   ├── router.rs         # MessageRouter
│   │   ├── orchestrator.rs   # ConversationOrchestrator
│   │   └── queue.rs          # MessageQueue
│   ├── adapters/
│   │   ├── mod.rs            # LlmAdapter trait
│   │   ├── claude.rs         # Anthropic API
│   │   ├── gemini.rs         # Google AI API
│   │   ├── openai.rs         # OpenAI API
│   │   └── ollama.rs         # Local Ollama
│   ├── tools/
│   │   ├── mod.rs            # ToolRegistry
│   │   ├── minky.rs          # MinKy integration
│   │   ├── filesystem.rs     # File operations
│   │   └── web.rs            # Web fetch
│   ├── config/
│   │   ├── mod.rs
│   │   └── settings.rs       # Configuration
│   └── error.rs              # Error types
├── tests/
│   ├── integration/
│   └── unit/
└── examples/
    ├── simple_chat.rs
    ├── code_review.rs
    └── research_team.rs
```

---

## Core Concepts

### 1. Message Protocol

```rust
pub struct LlmMessage {
    pub id: Uuid,
    pub from: AgentId,
    pub to: Option<AgentId>,      // None = broadcast
    pub message_type: MessageType,
    pub content: MessageContent,
    pub conversation_id: Uuid,
    pub timestamp: DateTime<Utc>,
}

pub enum MessageType {
    Chat,                         // 일반 대화
    ToolCall { tool, args },      // 도구 호출
    ToolResult { call_id, result }, // 도구 결과
    Delegate { task, context },   // 작업 위임
    Complete { task_id, summary }, // 작업 완료
    Error { code, message },      // 에러
}
```

### 2. Agent Adapters

```rust
#[async_trait]
pub trait LlmAdapter: Send + Sync {
    fn agent_id(&self) -> &AgentId;
    async fn process(&self, msg: LlmMessage) -> Result<LlmMessage>;
    fn available_tools(&self) -> Vec<ToolDefinition>;
    async fn process_stream(&self, msg: LlmMessage) -> impl Stream<Item = Chunk>;
}
```

### 3. Conversation Orchestration

```rust
pub struct ConversationOrchestrator {
    agents: Vec<AgentId>,
    turn_policy: TurnPolicy,      // RoundRobin, Directed, Free
    max_turns: Option<usize>,
    timeout: Duration,
}

pub enum TurnPolicy {
    RoundRobin,                   // 순서대로 발언
    Directed,                     // 메시지 수신자만 응답
    Free,                         // 자유 발언 (동시 가능)
}
```

---

## MinKy Integration

### 도구 연동

```rust
// axon/src/tools/minky.rs

pub struct MinkyTool {
    endpoint: String,
    api_key: Option<String>,
}

impl MinkyTool {
    pub async fn search(&self, query: &str, mode: SearchMode) -> Vec<SearchResult>;
    pub async fn get_document(&self, id: Uuid) -> Document;
    pub async fn ask(&self, question: &str) -> RagResponse;
}

// 도구 정의 (LLM에게 노출)
pub fn minky_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "minky_search",
            description: "Search MinKy knowledge base",
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "mode": { "enum": ["keyword", "vector", "hybrid", "deep"] }
                }
            }),
        },
        ToolDefinition {
            name: "minky_ask",
            description: "Ask a question to MinKy RAG system",
            parameters: json!({
                "type": "object",
                "properties": {
                    "question": { "type": "string" }
                }
            }),
        },
    ]
}
```

### 사용 예시

```bash
# MinKy 도구 등록
axon tool add minky --endpoint http://localhost:3000/api

# 대화에서 MinKy 활용
axon converse \
  --agents claude,gemini \
  --tools minky \
  --topic "우리 팀 지식 베이스에서 인증 관련 문서를 찾아 분석해 줘"
```

---

## Configuration

### ~/.axon/config.toml

```toml
[server]
port = 8090
log_level = "info"

[agents.claude]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"

[agents.gemini]
provider = "google"
model = "gemini-pro"
api_key_env = "GOOGLE_API_KEY"

[agents.local]
provider = "ollama"
model = "llama2"
endpoint = "http://localhost:11434"

[tools.minky]
endpoint = "http://localhost:3000/api"
```

---

## Development Guidelines

### Commit Convention

```
feat: 새 기능
fix: 버그 수정
refactor: 리팩토링
docs: 문서
test: 테스트
```

### 코드 품질

- `cargo clippy` 통과 필수
- `cargo fmt` 적용
- 테스트 커버리지 80% 목표

---

## Implementation Phases

### Phase 1: Core Protocol & CLI (MVP)
- [ ] Message protocol 정의
- [ ] CLI 기본 구조 (clap)
- [ ] Claude adapter 구현
- [ ] 단일 메시지 send 기능

### Phase 2: Multi-Agent Conversation
- [ ] Conversation orchestrator
- [ ] Turn management
- [ ] Broadcast messaging

### Phase 3: Tool Integration
- [ ] Tool registry
- [ ] MinKy adapter
- [ ] File system tools

### Phase 4: Advanced Features
- [ ] Streaming responses
- [ ] WebSocket server mode
- [ ] Conversation persistence
- [ ] Rate limiting & retry

---

## Related Projects

- **MinKy** (`../minky/`) - 지식 관리 플랫폼, 검색 도구 제공
- **MCP** - Model Context Protocol (Claude Desktop 통합)

---

## References

- [Anthropic API Docs](https://docs.anthropic.com/)
- [Google AI API](https://ai.google.dev/)
- [OpenAI API](https://platform.openai.com/docs/)
- [MCP Specification](https://modelcontextprotocol.io/)

---

*Last updated: 2026-03-10*
