# Axon Development Plan

> LLM-to-LLM Communication Framework

---

## Phase 1: Core Protocol & CLI (MVP)

### 1.1 프로젝트 초기화 ✅
- [x] Cargo.toml 설정 (clap, tokio, serde, reqwest)
- [x] 기본 디렉토리 구조 생성
- [x] Error types 정의

### 1.2 Message Protocol ✅
- [x] `LlmMessage` 구조체
- [x] `MessageType` enum (Chat, ToolCall, ToolResult, Delegate, Complete, Error)
- [x] `AgentId` 구조체
- [x] Serialization 테스트 (14개 테스트)

### 1.3 CLI 기본 구조 (스켈레톤 완료)
- [x] `axon serve` - 라우터 서버 시작 (스켈레톤)
- [x] `axon send` - 단일 메시지 전송 (스켈레톤)
- [x] `axon agent add/list/remove` - 에이전트 관리 (스켈레톤)
- [ ] 실제 기능 구현

### 1.4 Claude Adapter ✅
- [x] Anthropic API 클라이언트
- [x] Message 변환 (LlmMessage ↔ Anthropic format)
- [x] Tool calling 지원
- [x] 기본 테스트 (5개)

---

## Phase 2: Multi-Agent Conversation

### 2.1 Conversation Model ✅
- [x] Conversation 상태 관리
- [x] Turn policy (RoundRobin, Directed, Free, LastSpeakerExcluded)
- [x] 대화 종료 조건 (MaxTurns, AgentRequested, Timeout 등)
- [x] ConversationBuilder 패턴
- [x] 테스트 9개

### 2.2 Message Router ✅
- [x] 메시지 큐 (tokio mpsc)
- [x] 라우팅 로직 (send, broadcast)
- [x] Agent 등록/해제
- [x] Conversation 관리
- [x] RouterStats 통계
- [x] 테스트 6개

### 2.3 CLI 실제 기능 ✅
- [x] `axon send` - 단일 메시지 전송 (Claude API)
- [x] `axon converse` - 멀티 에이전트 대화 (RoundRobin)
- [x] `axon pipe` - 파이프라인 모드

---

## Phase 3: Tool Integration

### 3.1 Tool Registry ✅
- [x] ToolDefinition 구조체
- [x] Tool 등록/조회/실행
- [x] MCP 호환 포맷

### 3.2 MinKy Adapter ✅
- [x] HTTP 클라이언트
- [x] `minky_search` 도구
- [x] `minky_ask` 도구
- [x] `minky_get` 도구

### 3.3 Built-in Tools ✅
- [x] File system (read, write, list)
- [x] Web fetch
- [ ] Shell command (제한적)

### 3.4 CLI 확장 ✅
- [x] `axon tool add/list/remove`
- [ ] `--tools` 옵션으로 대화에 도구 연결 (Phase 4에서 구현)

---

## Phase 4: Additional Adapters

### 4.1 Gemini Adapter
- [ ] Google AI API 클라이언트
- [ ] Function calling 지원

### 4.2 OpenAI Adapter
- [ ] GPT API 클라이언트
- [ ] Tool use 지원

### 4.3 Ollama Adapter (Local)
- [ ] 로컬 Ollama 연동
- [ ] 오프라인 사용 지원

---

## Phase 5: Advanced Features

### 5.1 Streaming
- [ ] 스트리밍 응답 지원
- [ ] 실시간 출력

### 5.2 Persistence
- [ ] 대화 기록 저장
- [ ] 세션 복구

### 5.3 Server Mode
- [ ] WebSocket 서버
- [ ] HTTP API 엔드포인트

### 5.4 Reliability
- [ ] Rate limiting
- [ ] Retry with backoff
- [ ] 에러 복구

---

## 우선순위

| 단계 | 우선순위 | 예상 일정 |
|------|----------|-----------|
| Phase 1 | 🔴 Critical | Week 1 |
| Phase 2 | 🔴 Critical | Week 2 |
| Phase 3 | 🟡 High | Week 3 |
| Phase 4 | 🟢 Medium | Week 4 |
| Phase 5 | 🟢 Medium | Week 5+ |

---

## 다음 작업

1. `axon pipe` CLI 파이프라인 모드 구현
2. Phase 3 Tool Integration 시작
3. 추가 LLM Adapters (Gemini, OpenAI, Ollama)

---

*Last updated: 2026-03-19*
