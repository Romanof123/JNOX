# Native network DMA retirement / 네이티브 NIC DMA 회수

**CHG-323 · REQ-015 · DES-009 · IMP-323 · VER-015.** The AArch64 QEMU virt profile has one emulated modern MMIO NIC, one CPU and two kernel-owned split queues with eight descriptors each. The unprivileged service submits Ethernet frames through its assigned kernel calls. A service exit must retire the device even when the package has no replacement. [Issue #420](https://github.com/Romanof123/JNOX/issues/420) · [Native driver requirement](requirements.en.md) · [Verification plan](verification-plan.en.md)

## Ownership and recovery / 소유권과 복구

1. At the scheduler’s owner-exit boundary, revoke permission for new RX/TX submissions. The kernel serializes device access with ordinary CPU interrupts masked on this profile.
2. Request a full virtio device reset. Bound the number of status reads. Observe status zero and both RX and TX queues disabled, with device-memory ordering, before scrubbing or reusing any previously posted storage. With a missing acknowledgement or a queue still enabled, quarantine the pool and reject subsequent grants. A fixed read count bounds polling work; its duration on hardware remains to be measured. [Virtio 1.2, device reset and MMIO transport](https://docs.oasis-open.org/virtio/virtio/v1.2/virtio-v1.2.html)
3. A package may prepare one distinct standby. After retirement, the designated process can explicitly claim a new device epoch (SVC 76). The new grant covers raw frame calls 38 and 39. Socket broker identity and IPC routes remain those declared at packaging. A package without a standby ends the ownership sequence at retirement.
4. Verify normal operation before retirement and after the new claim with distinct challenges on the isolated emulated wire. Record device status and ring state, retained memory across the retired interval, and progress of a separate process. Record a separate host register-model control for delayed or absent acknowledgement and for a queue still enabled. Keep source, configuration, observations and limits with the verdict.

커널은 서비스 종료 시 송수신 권한을 먼저 회수한다. 장치 리셋 상태와 두 큐 정지를 확인한 후에만 이전 DMA 저장 영역을 비우고 재사용한다. 확인에 실패하면 버퍼를 격리하고 이후 요청을 거절한다. 패키지에서 지정한 예비 프로세스 한 개만 다음 세대의 송수신 권한을 요청할 수 있다. 기존 소켓 브로커 및 IPC 권한은 패키지 설정을 따른다.

가상 NIC에서 이전·새 소유자의 실제 프레임, 정지 중 큐 데이터와 동료 프로세스 진행을 관측한다. 응답 지연·누락 시험은 같은 리셋 절차를 사용하는 호스트 레지스터 모형에서 별도로 수행한다. 실제 보드의 IOMMU, 캐시 일관성, DMA 범위와 최악 정지 시간은 제품별 검증 항목이다. [Native driver integration / 상위 이슈](https://github.com/Romanof123/JNOX/issues/5)

[EVD-323 results and source / 실측·시험 원본](evidence/native-net-dma/README.md) · [SDK usage / SDK 사용 예](../sdk/native-aarch64/raw-ethernet.md)
