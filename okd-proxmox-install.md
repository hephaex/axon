---
id: okd-proxmox-001
title: OKD on Proxmox 설치 가이드
tags: [okd, proxmox, installation, kubernetes, openshift]
sources:
  - url: https://andrearaponi.it/devops/deploy-okd-on-proxmox/
    title: Deploying OKD Single Node on Proxmox
  - url: https://www.pivert.org/deploy-openshift-okd-on-proxmox-ve-or-bare-metal-tutorial/
    title: Deploy OKD on Proxmox VE (UPI)
  - url: https://github.com/stratokumulus/proxmox-openshift-setup
    title: Proxmox OpenShift Setup (Terraform + Ansible)
  - url: https://docs.okd.io/4.13/installing/installing_platform_agnostic/installing-platform-agnostic.html
    title: OKD Platform Agnostic Installation
created: 2026-03-10
updated: 2026-03-10
---

# OKD on Proxmox 설치 가이드

## 요약

OKD(The Community Distribution of Kubernetes that powers Red Hat OpenShift)를 Proxmox VE에 설치하는 방법을 다룹니다. Single Node (SNO)와 Multi-Node 구성 모두 가능합니다.

---

## 설치 유형

| 유형 | 노드 수 | 용도 | 리소스 |
|------|---------|------|--------|
| **SNO** | 1 | 개발/테스트/홈랩 | 8 vCPU, 32GB RAM |
| **Compact** | 3 | 소규모 프로덕션 | Control+Worker 통합 |
| **Standard** | 3+2 | 프로덕션 | Control 3 + Worker 2+ |

---

## Single Node OKD (SNO) 요구사항

### 하드웨어 (Proxmox 호스트)
- CPU: Intel VT-x 또는 AMD-V 활성화
- RAM: 40GB+ (VM에 32GB 할당)
- Storage: 200GB+ SSD/NVMe

### VM 스펙
| 항목 | 최소 | 권장 |
|------|------|------|
| vCPU | 8 | 16 |
| RAM | 32GB | 64GB |
| Disk | 120GB | 150GB+ |
| Network | 1 NIC | 1 NIC |

---

## Multi-Node 클러스터 요구사항

### 인프라 VM

| 역할 | 수량 | vCPU | RAM | Disk |
|------|------|------|-----|------|
| HAProxy (LB) | 1 | 2 | 4GB | 20GB |
| DNS Server | 1 | 2 | 4GB | 20GB |
| Bootstrap | 1 | 4 | 16GB | 100GB |
| Control Plane | 3 | 4 | 16GB | 100GB |
| Worker | 2+ | 4 | 16GB | 100GB |

### 네트워크 요구사항
- 고정 IP 또는 DHCP 예약
- Forward DNS (A 레코드)
- Reverse DNS (PTR 레코드)
- API/Ingress 로드밸런싱

---

## 아키텍처 (Multi-Node)

```
                    ┌─────────────────┐
                    │   HAProxy       │
                    │   (Load Balancer)│
                    │   :6443, :443   │
                    └────────┬────────┘
                             │
         ┌───────────────────┼───────────────────┐
         │                   │                   │
         ▼                   ▼                   ▼
┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
│  Control Plane  │ │  Control Plane  │ │  Control Plane  │
│     Node 1      │ │     Node 2      │ │     Node 3      │
│  (master)       │ │  (master)       │ │  (master)       │
└─────────────────┘ └─────────────────┘ └─────────────────┘
         │                   │                   │
         └───────────────────┼───────────────────┘
                             │
         ┌───────────────────┼───────────────────┐
         ▼                   ▼                   ▼
┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
│     Worker      │ │     Worker      │ │     Worker      │
│     Node 1      │ │     Node 2      │ │     Node N      │
└─────────────────┘ └─────────────────┘ └─────────────────┘
```

---

## 설치 단계 (SNO)

### 1. 사전 준비

```bash
# 필요 도구 설치
brew install openshift-install oc  # macOS
# 또는
wget https://github.com/okd-project/okd/releases/download/4.14.0-0.okd-2024-01-06-084517/openshift-install-linux-4.14.0-0.okd-2024-01-06-084517.tar.gz

# Pull Secret 준비 (Red Hat 계정 필요 - 무료)
# https://console.redhat.com/openshift/install/pull-secret
```

### 2. install-config.yaml 생성

```yaml
apiVersion: v1
baseDomain: example.com
metadata:
  name: okd-sno
networking:
  networkType: OVNKubernetes
  clusterNetwork:
  - cidr: 10.128.0.0/14
    hostPrefix: 23
  serviceNetwork:
  - 172.30.0.0/16
compute:
- name: worker
  replicas: 0
controlPlane:
  name: master
  replicas: 1
platform:
  none: {}
pullSecret: '<pull-secret-json>'
sshKey: '<ssh-public-key>'
```

### 3. ISO 생성 및 부팅

```bash
# 설치 파일 생성
openshift-install create single-node-ignition-config --dir=./sno

# ISO 생성
coreos-installer iso customize \
  --dest-device /dev/sda \
  --dest-ignition sno/bootstrap-in-place-for-live-iso.ign \
  fedora-coreos-*.iso

# Proxmox에 ISO 업로드 후 VM 부팅
```

### 4. 설치 모니터링

```bash
# 설치 진행 확인
openshift-install wait-for bootstrap-complete --dir=./sno

# 완료 대기
openshift-install wait-for install-complete --dir=./sno

# kubeconfig 설정
export KUBECONFIG=./sno/auth/kubeconfig
oc get nodes
```

---

## 설치 단계 (Multi-Node)

### 1. DNS 구성

```
# Forward DNS (A 레코드)
api.okd.example.com        -> HAProxy IP
api-int.okd.example.com    -> HAProxy IP
*.apps.okd.example.com     -> HAProxy IP

bootstrap.okd.example.com  -> Bootstrap IP
master0.okd.example.com    -> Control Plane 0 IP
master1.okd.example.com    -> Control Plane 1 IP
master2.okd.example.com    -> Control Plane 2 IP
worker0.okd.example.com    -> Worker 0 IP
worker1.okd.example.com    -> Worker 1 IP

# Reverse DNS (PTR 레코드)
<IP> -> <hostname>.okd.example.com
```

### 2. HAProxy 구성

```
# /etc/haproxy/haproxy.cfg

frontend api
    bind *:6443
    default_backend api-backend

backend api-backend
    balance roundrobin
    server bootstrap bootstrap.okd.example.com:6443 check
    server master0 master0.okd.example.com:6443 check
    server master1 master1.okd.example.com:6443 check
    server master2 master2.okd.example.com:6443 check

frontend ingress-https
    bind *:443
    default_backend ingress-https-backend

backend ingress-https-backend
    balance roundrobin
    server worker0 worker0.okd.example.com:443 check
    server worker1 worker1.okd.example.com:443 check
```

### 3. install-config.yaml (Multi-Node)

```yaml
apiVersion: v1
baseDomain: example.com
metadata:
  name: okd
networking:
  networkType: OVNKubernetes
compute:
- name: worker
  replicas: 2
controlPlane:
  name: master
  replicas: 3
platform:
  none: {}
pullSecret: '<pull-secret>'
sshKey: '<ssh-public-key>'
```

### 4. Ignition 파일 생성

```bash
openshift-install create manifests --dir=./okd
openshift-install create ignition-configs --dir=./okd

# 생성되는 파일:
# - bootstrap.ign
# - master.ign
# - worker.ign
```

### 5. VM 부팅 순서

1. Bootstrap 노드 부팅
2. Control Plane 노드들 부팅
3. Bootstrap 완료 대기
4. Worker 노드들 부팅
5. CSR 승인

```bash
# CSR 승인 (Worker 노드 추가 시)
oc get csr -o name | xargs oc adm certificate approve
```

---

## 자동화 도구

### Terraform + Ansible

```bash
# https://github.com/stratokumulus/proxmox-openshift-setup
git clone https://github.com/stratokumulus/proxmox-openshift-setup
cd proxmox-openshift-setup

# Terraform으로 VM 생성
cd terraform
terraform init
terraform apply

# Ansible로 구성
cd ../ansible
ansible-playbook site.yml
```

---

## 트러블슈팅

### 부팅 실패
- BIOS에서 VT-x/AMD-V 활성화 확인
- VM에서 "host" CPU 타입 사용

### DNS 문제
```bash
# DNS 확인
dig +short api.okd.example.com
dig +short -x <IP>  # Reverse lookup
```

### 인증서 오류
```bash
# 인증서 확인
oc get csr
oc adm certificate approve <csr-name>
```

### 디스크 공간
- 최소 120GB 필요
- 로그 확인: `journalctl -b -f`

---

## 관련 문서

- [Proxmox VE 개요](./proxmox-overview.md)
- [Red Hat OpenShift 개요](./openshift-overview.md)
- [OKD 공식 문서](https://docs.okd.io/)
