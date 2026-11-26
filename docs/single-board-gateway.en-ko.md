# JNOX requests to the assigned Linux NIC / 단일 보드 장치 요청

**CHG-324 · REQ-004/011 · DES-002 · #422.** A native Rust caller owns the absolute monotonic deadline. A separately scheduled JNOX process must continue while a Linux service handles the request. The original Alpine kernel and virtio_net driver retain sole access to the NIC on the two-CPU Bao board. The previous single-board HTTP test supplies the regression baseline. [Domain allocation](single-board-driver-domains.en-ko.md) · [Previous capture](evidence/single-board-domains/README.md)

| Allocation / 자원 | Contract / 설계 |
| --- | --- |
| Transport | One 4096-byte Bao shared region at machine PA 0x5e000000, guest IPA 0x0b000000 in both domains. No doorbell IRQ. |
| JNOX | FIFO slot 0 may send or receive one copied 24-byte JG v1 record. Slot 1 has its own address space and runs a periodic heartbeat. The shared page is EL1-only, Device-nGnRnE, execute-never. |
| Linux | A trusted launcher validates the DT mapping, maps one page, closes its raw-memory FD and drops privilege. The service uses aligned accesses and a listener on eth0. |
| Mailbox | Single writer per lane. Request starts at byte 0, reply at 64. An aligned little-endian 32-bit sequence precedes six aligned words containing the unchanged wire bytes. Zero is empty. An odd sequence brackets writes and an even nonzero sequence publishes a record. Readers check the same even value before and after copying. Full system barriers order the accesses. JNOX acknowledges consumption at byte 192 after successful copy-out. |
| Device operation | The external TCP peer reads the 24 JG bytes from the Linux NIC and returns its own fresh eight bytes. Both directions are captured on the NIC. The peer has no access to the mailbox. |

**Acceptance.** One QEMU process runs both kernels. Observe a host-generated response in the authorized JNOX task's private address space. Withhold the external device response for the 250 ms request, observe the software output turn off, measure the actual deadline overshoot, and find unrelated task wakes inside the waiting interval. Complete a fresh generation through the NIC. Preserve process denial, original kernel/module hashes, package manifests, UART, native RAM reads and the entire wire capture. Repeat the original single-board HTTP profile as regression. A separate fault trial must subsequently stop or restart the Linux kernel alone, present a retained stale reply, and re-establish service on the same board. [Earlier two-machine fault test](evidence/linux-driver-gateway/README.md)

동일한 가상 보드에서 JNOX 슬롯 0만 공유 페이지를 통해 장치 요청을 보낼 수 있다. 앱이 볼 수 있는 것은 요청·응답 복사 API다. Linux 서비스는 초기화 후 일반 사용자 권한으로 원본 NIC를 사용한다. 장치 응답을 보류한 250ms 동안 다른 JNOX 태스크가 실행되는지, 호출자가 대체 출력을 적용하는지, 이후 새 요청이 성공하는지를 측정한다. Linux 커널 단독 재시작과 이전 응답 폐기는 같은 보드에서 추가로 검증한다. 실물 DMA 차단과 제품별 기능안전 승인은 상위 이슈에서 추적한다. [Issue #422](https://github.com/Romanof123/JNOX/issues/422)

The subsequent same-board trial covers authorized JNOX-originated device calls and a 250 ms device-response timeout. 이후 같은 보드에서 승인된 JNOX 앱의 실제 장치 요청과 응답 기한 만료를 검증했다. [Gateway results / 게이트웨이 실측](evidence/single-board-domains/gateway.en-ko.md)
