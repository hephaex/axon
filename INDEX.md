# OCP References Index

> 조사 자료 인덱스 - 에이전트/스킬이 참조할 수 있는 기술 문서 목록

---

## 사용법

### 에이전트에서 참조
```
1. INDEX.md 읽기 → 필요한 문서 찾기
2. 해당 문서 읽기 → 컨텍스트 확보
3. 작업 수행
```

### 스킬에서 참조
```bash
/ocp-ref list              # 참조 목록
/ocp-ref search <keyword>  # 키워드 검색
/ocp-ref read <id>         # 문서 읽기
```

---

## 문서 목록

| ID | 제목 | 파일 | 태그 | 업데이트 |
|----|------|------|------|----------|
| proxmox-001 | Proxmox VE 개요 | [proxmox-overview.md](./proxmox-overview.md) | `proxmox`, `virtualization`, `kvm`, `lxc` | 2026-03-10 |
| openshift-001 | Red Hat OpenShift 개요 | [openshift-overview.md](./openshift-overview.md) | `openshift`, `kubernetes`, `enterprise` | 2026-03-10 |
| okd-proxmox-001 | OKD on Proxmox 설치 가이드 | [okd-proxmox-install.md](./okd-proxmox-install.md) | `okd`, `proxmox`, `installation` | 2026-03-10 |

---

## 태그별 분류

### Infrastructure
- `proxmox` - Proxmox VE 관련
- `virtualization` - 가상화 기술
- `storage` - 스토리지 관련
- `network` - 네트워크 관련

### Container Platform
- `kubernetes` - Kubernetes 관련
- `openshift` - Red Hat OpenShift
- `okd` - OKD (OpenShift 오픈소스)

### Operations
- `installation` - 설치 가이드
- `configuration` - 설정 가이드
- `troubleshooting` - 문제 해결

---

## 문서 포맷

각 참조 문서는 다음 YAML frontmatter를 포함:

```yaml
---
id: <unique-id>
title: <문서 제목>
tags: [tag1, tag2, ...]
sources:
  - url: <출처 URL>
    title: <출처 제목>
created: YYYY-MM-DD
updated: YYYY-MM-DD
---
```

---

## 추가 예정

- [ ] OKD 상세 조사
- [ ] Proxmox 네트워크 구성
- [ ] Proxmox 스토리지 구성
- [ ] OpenShift vs OKD 비교
