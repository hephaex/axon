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

### 1.3 CLI 기본 구조 ✅
- [x] `axon serve` - 라우터 서버 시작
- [x] `axon send` - 단일 메시지 전송
- [x] `axon agent add/list/remove` - 에이전트 관리
- [x] 실제 기능 구현 (Phase 2.3, 5.3)

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
- [x] Shell command (제한적)

### 3.4 CLI 확장 ✅
- [x] `axon tool add/list/remove`
- [x] `--tools` 옵션으로 대화에 도구 연결

---

## Phase 4: Additional Adapters ✅

### 4.1 Gemini Adapter ✅
- [x] Google AI API 클라이언트
- [x] Function calling 지원
- [x] System instruction 지원

### 4.2 OpenAI Adapter ✅
- [x] GPT API 클라이언트
- [x] Tool use 지원
- [x] Custom endpoint 지원 (Azure 호환)

### 4.3 Ollama Adapter (Local) ✅
- [x] 로컬 Ollama 연동
- [x] 오프라인 사용 지원
- [x] 이미지 지원 (multimodal)

---

## Phase 5: Advanced Features

### 5.1 Streaming ✅
- [x] StreamingAdapter trait 정의
- [x] StreamChunk, StreamUsage 타입
- [x] ClaudeAdapter 스트리밍 (SSE)
- [x] OpenAiAdapter 스트리밍 (SSE)
- [x] GeminiAdapter 스트리밍 (SSE)
- [x] OllamaAdapter 스트리밍 (NDJSON)
- [x] collect_stream 유틸리티 함수

### 5.2 Persistence ✅
- [x] ConversationStore trait 정의
- [x] FileStore (JSON 파일 기반)
- [x] MemoryStore (인메모리, 테스트용)

### 5.3 Server Mode ✅
- [x] WebSocket 서버
- [x] HTTP API 엔드포인트
- [x] ServerState 공유 상태 관리
- [x] CLI `axon serve` 명령 구현

### 5.4 Reliability ✅
- [x] Retry with exponential backoff
- [x] Token bucket rate limiting
- [x] Per-provider rate limiters

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

## 다음 작업 (Optional)

1. ~~Shell command 도구 구현 (Phase 3.3)~~ ✅ 완료
2. ~~`--tools` 옵션으로 대화에 도구 연결 (Phase 3.4)~~ ✅ 완료
3. ~~CLI 스트리밍 출력 (`axon send --stream` 옵션)~~ ✅ 완료
4. ~~프로덕션 배포 준비 (Docker, 설정 파일)~~ ✅ 완료

## 추가 작업 (Future)

1. Kubernetes 배포 (Helm chart)
2. Prometheus metrics 연동
3. OpenTelemetry tracing
4. Agent 자동 등록 (config.toml 기반)

---

## 완료 현황

| Phase | 상태 | 완료일 |
|-------|------|--------|
| Phase 1: Core Protocol & CLI | ✅ 완료 | Week 1 |
| Phase 2: Multi-Agent Conversation | ✅ 완료 | Week 2 |
| Phase 3: Tool Integration | ✅ 완료 | Week 3 |
| Phase 4: Additional Adapters | ✅ 완료 | Week 4 |
| Phase 5: Advanced Features | ✅ 완료 | Week 5 |

---

*Last updated: 2026-03-20*
