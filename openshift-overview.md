---
id: openshift-001
title: Red Hat OpenShift 개요
tags: [openshift, kubernetes, enterprise, container-platform]
sources:
  - url: https://www.redhat.com/en/technologies/cloud-computing/openshift
    title: Red Hat OpenShift Official
  - url: https://thenewstack.io/red-hat-openshift-4-20-boosts-ai-security-hybrid-cloud/
    title: OpenShift 4.20 Features
  - url: https://www.portainer.io/blog/openshift-vs-kubernetes
    title: OpenShift vs Kubernetes
  - url: https://www.redhat.com/en/technologies/cloud-computing/openshift/red-hat-openshift-kubernetes
    title: OpenShift vs Kubernetes (Red Hat)
created: 2026-03-10
updated: 2026-03-10
---

# Red Hat OpenShift 개요

## 요약

Red Hat OpenShift는 Kubernetes 기반의 엔터프라이즈급 컨테이너 오케스트레이션 플랫폼입니다. 순수 Kubernetes에 보안, 개발자 도구, 운영 기능을 추가한 "Kubernetes 배포판"입니다.

- **개발사**: Red Hat (IBM 자회사)
- **라이선스**: 상용 (구독 기반)
- **기반**: Kubernetes + Red Hat CoreOS
- **최신 버전**: 4.21 (2026년 기준)

---

## 아키텍처

```
┌─────────────────────────────────────────────────────────────┐
│                    OpenShift Platform                        │
├─────────────────────────────────────────────────────────────┤
│  Developer Experience                                        │
│  - Web Console, oc CLI, IDE Plugins                         │
│  - Source-to-Image (S2I), Tekton CI/CD                      │
├─────────────────────────────────────────────────────────────┤
│  Platform Services                                           │
│  - Operator Lifecycle Manager (OLM)                         │
│  - Service Mesh (Istio), GitOps (ArgoCD)                    │
│  - Logging, Monitoring, Observability                       │
├─────────────────────────────────────────────────────────────┤
│  Security & Policy                                           │
│  - Security Context Constraints (SCC)                       │
│  - OAuth, RBAC, Network Policy                              │
├─────────────────────────────────────────────────────────────┤
│  Kubernetes (Core Engine)                                    │
├─────────────────────────────────────────────────────────────┤
│  Red Hat CoreOS (RHCOS) - Immutable OS                      │
└─────────────────────────────────────────────────────────────┘
```

---

## OpenShift vs Kubernetes 비교

| 구분 | Kubernetes | OpenShift |
|------|------------|-----------|
| **설치** | 수동 설정 필요 (kubeadm, kops) | 자동화된 IPI/UPI 설치 |
| **OS** | 선택 자유 | RHCOS (Control Plane 필수) |
| **CLI** | kubectl | oc (kubectl 확장) |
| **웹 콘솔** | 기본 Dashboard (별도 설치) | 풍부한 내장 콘솔 |
| **CI/CD** | 별도 구성 | Tekton Pipelines 내장 |
| **보안** | 기본 RBAC | SCC + 강화된 정책 |
| **Operator** | 별도 설치 | OLM 내장 |
| **지원** | 커뮤니티 | Red Hat 엔터프라이즈 |
| **비용** | 무료 (인프라 비용만) | 구독 기반 (코어당 과금) |

---

## 주요 컴포넌트

### 1. Operator Lifecycle Manager (OLM)
- Operator 설치, 업데이트, 라이프사이클 관리
- OperatorHub에서 검증된 Operator 제공
- GitOps 원칙 지원 (OLM 1.0)

### 2. OpenShift GitOps
- ArgoCD 기반 선언적 배포
- Git 저장소를 단일 진실 소스로 사용
- 멀티 클러스터 관리

### 3. OpenShift Service Mesh
- Istio 기반 서비스 메시
- 트래픽 관리, mTLS 보안
- 분산 트레이싱

### 4. OpenShift Pipelines
- Tekton 기반 클라우드 네이티브 CI/CD
- Kubernetes 네이티브 파이프라인
- 서버리스 빌드

### 5. OpenShift Virtualization
- KubeVirt 기반 VM 실행
- VM과 컨테이너 통합 관리
- VMware 마이그레이션 지원

---

## 최신 버전 (4.20/4.21) 기능

### AI/ML 워크로드
- LeaderWorkerSet GA (분산 학습)
- Gateway API 추론 확장
- OCI 이미지 볼륨 소스

### 가상화 개선
- CPU 부하 인식 리밸런싱
- 빠른 라이브 마이그레이션
- ARM 아키텍처 지원 강화

### 보안 강화
- 외부 IdP 연동 (BYOIDP)
- User Namespaces 지원
- BGP 네트워킹 (온프레미스)

### 관측성 통합
- 메트릭, 로그, 트레이스, 네트워크 텔레메트리 통합
- 단일 대시보드 경험

---

## 배포 옵션

| 옵션 | 설명 | 관리 주체 |
|------|------|-----------|
| **Self-managed** | 온프레미스/클라우드 직접 설치 | 고객 |
| **ROSA** | AWS 관리형 OpenShift | Red Hat + AWS |
| **ARO** | Azure 관리형 OpenShift | Red Hat + Azure |
| **RHOIC** | IBM Cloud 관리형 | Red Hat + IBM |
| **Dedicated** | Red Hat 관리형 (GCP/AWS) | Red Hat |

---

## 가격 정책

- **모델**: 구독 기반, 코어(vCPU)당 과금
- **지원 레벨**: Standard / Premium
- **예시**: 3노드 클러스터 약 $560-580/월 (구성별 변동)

정확한 가격은 Red Hat 영업팀 문의 필요

---

## CLI 명령어 참조

```bash
# 로그인
oc login https://api.cluster.example.com:6443

# 프로젝트 관리
oc new-project myproject
oc project myproject
oc projects

# 애플리케이션 배포
oc new-app nginx
oc expose svc/nginx

# 리소스 관리
oc get pods
oc get deployments
oc describe pod <name>
oc logs <pod>

# 빌드
oc start-build <buildconfig>
oc logs -f bc/<buildconfig>

# 라우트
oc get routes
oc expose svc/<service>
```

---

## 관련 문서

- [OKD (오픈소스 버전)](./okd-overview.md) (예정)
- [OpenShift vs OKD 비교](./openshift-vs-okd.md) (예정)
- [OKD on Proxmox 설치](./okd-proxmox-install.md)
