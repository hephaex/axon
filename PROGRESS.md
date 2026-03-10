# Axon Progress

> LLM-to-LLM Communication Framework

---

## 현재 상태

**Phase**: 1 - Core Protocol & CLI (MVP)
**상태**: 🟡 초기 설정 진행 중

---

## 완료된 작업

### 2026-03-10

#### 프로젝트 초기화
- [x] 프로젝트 디렉토리 생성 (`../axon/`)
- [x] CLAUDE.md 작성 (프로젝트 가이드)
- [x] PLAN.md 작성 (개발 계획)
- [x] PROGRESS.md 작성 (진행 상황)
- [x] Cargo.toml 생성
- [x] 기본 디렉토리 구조 생성
- [x] Error types 정의

---

## 진행 중인 작업

### Phase 1.2: Message Protocol
- [ ] `LlmMessage` 구조체
- [ ] `MessageType` enum
- [ ] `AgentId` 구조체
- [ ] Serialization 테스트

---

## 다음 작업

1. Message Protocol 구현 (`src/protocol/message.rs`)
2. Agent 모델 구현 (`src/protocol/agent.rs`)
3. CLI 기본 구조 (clap)
4. Claude adapter MVP

---

## 세션 로그

### 2026-03-10 Session 1
- 프로젝트 생성 및 초기 설정
- MinKy 연동 계획 수립
- 기본 구조 구현

---

*Last updated: 2026-03-10*
