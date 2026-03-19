---
id: proxmox-001
title: Proxmox VE 개요
tags: [proxmox, virtualization, kvm, lxc, hypervisor]
sources:
  - url: https://www.proxmox.com/en/products/proxmox-virtual-environment/overview
    title: Proxmox VE Official Overview
  - url: https://www.proxmox.com/en/products/proxmox-virtual-environment/features
    title: Proxmox VE Features
  - url: https://www.starwindsoftware.com/blog/proxmox-vs-esxi-detailed-comparison/
    title: Proxmox vs ESXi Comparison
  - url: https://www.acronis.com/en/blog/posts/proxmox-vs-vmware-a-comprehensive-comparison/
    title: Proxmox vs VMware Comparison
created: 2026-03-10
updated: 2026-03-10
---

# Proxmox VE 개요

## 요약

Proxmox Virtual Environment (PVE)는 오픈소스 서버 가상화 플랫폼으로, KVM 하이퍼바이저와 LXC 컨테이너를 단일 플랫폼에서 통합 관리합니다.

- **라이선스**: GNU AGPL v3 (무료 오픈소스)
- **기반 OS**: Debian 12 (Bookworm) - PVE 8.x
- **최신 버전**: 8.4 (2026년 기준)
- **EOL 예상**: 2026년 8월 (Debian 12 기준)

---

## 아키텍처

```
┌─────────────────────────────────────────────────────────────┐
│                    Proxmox VE 8.x                           │
├─────────────────────────────────────────────────────────────┤
│  Web UI (Port 8006)                                         │
│  - VM/CT 관리, 클러스터 관리, 백업/복구                      │
├─────────────────────────────────────────────────────────────┤
│  Management Layer                                            │
│  - REST API, CLI (pvesh, qm, pct)                           │
│  - High Availability (Corosync + PVE HA Manager)            │
├─────────────────────────────────────────────────────────────┤
│  Virtualization                                              │
│  ┌─────────────────┐  ┌─────────────────┐                   │
│  │  KVM/QEMU       │  │  LXC            │                   │
│  │  (Full VMs)     │  │  (Containers)   │                   │
│  └─────────────────┘  └─────────────────┘                   │
├─────────────────────────────────────────────────────────────┤
│  Storage                                                     │
│  - Local: ZFS, LVM, Directory                               │
│  - Shared: Ceph, NFS, iSCSI, GlusterFS                      │
├─────────────────────────────────────────────────────────────┤
│  Networking                                                  │
│  - Linux Bridge, Open vSwitch, SDN                          │
├─────────────────────────────────────────────────────────────┤
│  Debian 12 (Bookworm) + Linux Kernel 6.x                    │
└─────────────────────────────────────────────────────────────┘
```

---

## 핵심 기능

### 1. 가상화
- **KVM/QEMU**: 완전 가상화 (Windows, Linux, BSD 등)
- **LXC**: 경량 Linux 컨테이너
- **지원 디스크 포맷**: qcow2, raw, vmdk, vdi

### 2. 고가용성 (HA)
- Corosync 기반 클러스터링
- 자동 Failover
- 라이브 마이그레이션 (무중단)

### 3. 스토리지
- **로컬**: ZFS, LVM, Directory
- **공유**: Ceph (내장), NFS, iSCSI, GlusterFS
- **백업**: Proxmox Backup Server 연동

### 4. 네트워킹
- Linux Bridge (기본)
- Open vSwitch (고급)
- Software-Defined Networking (SDN)
- VLAN, 본딩 지원

### 5. 관리
- 웹 UI (포트 8006)
- REST API
- CLI 도구 (pvesh, qm, pct, pvecm)

---

## Proxmox vs VMware ESXi 비교

| 구분 | Proxmox VE | VMware ESXi |
|------|------------|-------------|
| **라이선스** | 오픈소스 (AGPL v3) | 상용 (유료) |
| **비용** | 무료 (Enterprise repo: €115/년) | 수천 달러/년 |
| **성능** | 57개 테스트 중 56개 우위, IOPS 50%↑ | - |
| **가상화** | KVM + LXC | VMware 전용 |
| **디스크 포맷** | qcow2, vdi, vmdk | vmdk만 |
| **컨테이너** | LXC 네이티브 | 미지원 |
| **HA 클러스터** | 무료 포함 | vSphere 라이선스 필요 |
| **관리 UI** | 내장 Web UI | vCenter 별도 |
| **적합 대상** | SMB, 홈랩, 비용 민감 | 대기업, VMware 기존 고객 |

---

## 최신 버전 (8.4) 주요 기능

1. **vGPU 라이브 마이그레이션**
   - NVIDIA vGPU 사용 VM도 온라인 이전 가능
   - 이전에는 VM 종료 필요

2. **백업 플러그인 API**
   - 서드파티 백업 솔루션 통합
   - Proxmox UI/API에서 직접 관리

3. **SDN 개선**
   - Software-Defined Networking 기능 강화
   - EVPN, VXLAN 지원

4. **Ceph 통합 개선**
   - 분산 스토리지 관리 UI 개선
   - 성능 모니터링 강화

---

## 설치 요구사항

### 최소 사양
- CPU: 64비트 (Intel VT-x/AMD-V 필수)
- RAM: 2GB (권장 4GB+)
- Storage: 32GB+ (ZFS 사용 시 더 필요)

### 권장 사양 (프로덕션)
- CPU: 멀티코어 서버급
- RAM: 64GB+
- Storage: SSD/NVMe, RAID 또는 ZFS
- Network: 1Gbps+ (10Gbps 권장)

---

## CLI 명령어 참조

```bash
# VM 관리
qm list                    # VM 목록
qm start <vmid>           # VM 시작
qm stop <vmid>            # VM 중지
qm migrate <vmid> <node>  # VM 마이그레이션

# 컨테이너 관리
pct list                   # CT 목록
pct start <ctid>          # CT 시작
pct stop <ctid>           # CT 중지

# 클러스터 관리
pvecm status              # 클러스터 상태
pvecm nodes               # 노드 목록

# 스토리지 관리
pvesm status              # 스토리지 상태
pvesm list <storage>      # 스토리지 내용

# API 접근
pvesh get /nodes          # REST API 호출
```

---

## 관련 문서

- [OKD on Proxmox 설치](./okd-proxmox-install.md)
- [Proxmox 네트워크 구성](./proxmox-network.md) (예정)
- [Proxmox 스토리지 구성](./proxmox-storage.md) (예정)
