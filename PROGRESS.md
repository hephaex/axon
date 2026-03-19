# Axon Progress

> LLM-to-LLM Communication Framework

---

## 현재 상태

**Phase**: 1 - Core Protocol & CLI (MVP)
**상태**: 🟢 Phase 1.2 완료, Phase 1.4 진행 예정

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

## 진행 중인 작업

### Phase 1.4: Claude Adapter
- [ ] Anthropic API 클라이언트
- [ ] Message 변환 (LlmMessage ↔ Anthropic format)
- [ ] Tool calling 지원

---

## 다음 작업

1. Claude adapter 구현 (`src/adapters/claude.rs`)
2. LlmAdapter trait 수정 (LlmMessage 사용)
3. axon send 실제 기능 구현
4. 통합 테스트

---

## 세션 로그

### 2026-03-19 Session 2
- Phase 1.2 Message Protocol 구현 완료
- AgentId, Provider, AgentConfig 구현
- LlmMessage, MessageType, MessageContent 구현
- 21개 테스트 통과

### 2026-03-10 Session 1
- 프로젝트 생성 및 초기 설정
- MinKy 연동 계획 수립
- 기본 구조 구현

---

*Last updated: 2026-03-10*
