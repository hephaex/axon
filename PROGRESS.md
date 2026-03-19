# Axon Progress

> LLM-to-LLM Communication Framework

---

## 현재 상태

**Phase**: 2 - Multi-Agent Conversation
**상태**: 🟢 Phase 2.3 완료 (CLI 실제 기능)

---

## 완료된 작업

### 2026-03-10

#### 프로젝트 초기화 ✅
- [x] 프로젝트 디렉토리 생성 (`../axon/`)
- [x] CLAUDE.md 작성 (프로젝트 가이드)
- [x] PLAN.md 작성 (개발 계획)
- [x] PROGRESS.md 작성 (진행 상황)
- [x] Cargo.toml 생성 (clap, tokio, serde, reqwest 등)
- [x] 기본 디렉토리 구조 생성 (cli, protocol, router, adapters, tools, config)
- [x] Error types 정의 (`src/error.rs`)
- [x] Config 시스템 구현 (`src/config/mod.rs`)
- [x] CLI 기본 구조 구현 (serve, send, converse, pipe, agent, tool)
- [x] Git 초기화 및 첫 커밋

#### Phase 1.2: Message Protocol ✅
- [x] `AgentId` 구조체 (`src/protocol/agent.rs`)
- [x] `Provider` enum (Anthropic, Google, OpenAI, Ollama)
- [x] `AgentConfig` 구조체
- [x] `LlmMessage` 구조체 (`src/protocol/message.rs`)
- [x] `MessageType` enum (Chat, ToolCall, ToolResult, Delegate, Complete, Error)
- [x] `MessageContent` enum (Text, Json, Parts)
- [x] Serialization 테스트 (14개 테스트 통과)

---

#### Phase 1.4: Claude Adapter ✅
- [x] ClaudeAdapter 구현 (`src/adapters/claude.rs`)
- [x] LlmAdapter trait 수정 (LlmMessage 사용)
- [x] Anthropic API 클라이언트
- [x] Message 변환 (LlmMessage ↔ Anthropic format)
- [x] Tool calling 지원
- [x] AdapterBuilder 구현

#### Phase 2.1: Conversation Model ✅
- [x] Conversation 구조체 (`src/protocol/conversation.rs`)
- [x] TurnPolicy enum (RoundRobin, Directed, Free, LastSpeakerExcluded)
- [x] ConversationStatus, ConversationEndReason enum
- [x] ConversationBuilder 패턴
- [x] 테스트 9개 통과

#### Phase 2.2: Message Router ✅
- [x] MessageRouter 구현 (`src/router/router.rs`)
- [x] tokio mpsc 메시지 큐
- [x] 라우팅 로직 (send, broadcast)
- [x] Agent 등록/해제/조회
- [x] Conversation 관리
- [x] RouterStats 통계
- [x] 테스트 6개 통과

#### Phase 2.3: CLI 실제 기능 ✅
- [x] `axon send` 실제 기능 구현
  - Claude API 연동
  - 에러 핸들링 (API 키 검증)
- [x] `axon converse` 멀티 에이전트 대화
  - RoundRobin TurnPolicy 적용
  - MessageRouter 통합
  - max_turns 지원

---

## 진행 중인 작업

### Phase 3: Tool Integration (예정)
- [ ] Tool Registry 구현
- [ ] MinKy Adapter 구현

---

## 다음 작업

1. axon pipe 파이프라인 모드 구현
2. Phase 3 Tool Integration
3. 추가 LLM Adapters (Gemini, OpenAI, Ollama)

---

## 세션 로그

### 2026-03-19 Session 3
- Phase 2.1 Conversation Model 구현 완료
- Conversation, TurnPolicy, ConversationStatus 구현
- ConversationBuilder 패턴 적용
- Phase 2.2 MessageRouter 구현 완료
- tokio mpsc 기반 메시지 큐
- Agent 등록/해제/라우팅 로직
- Phase 2.3 CLI 실제 기능 구현 완료
- axon send: Claude API 연동, 에러 핸들링
- axon converse: 멀티 에이전트 대화, RoundRobin
- 41개 테스트 통과

### 2026-03-19 Session 2
- Phase 1.2 Message Protocol 구현 완료
- AgentId, Provider, AgentConfig 구현
- LlmMessage, MessageType, MessageContent 구현
- Phase 1.4 Claude Adapter 구현 완료
- ClaudeAdapter, LlmAdapter trait 구현
- Anthropic API 통합
- 26개 테스트 통과

### 2026-03-10 Session 1
- 프로젝트 생성 및 초기 설정
- MinKy 연동 계획 수립
- 기본 구조 구현

---

*Last updated: 2026-03-19*
