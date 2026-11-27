# CHG-333 · NIC interrupt pending at owner exit / 소유자 종료 시 대기 중인 NIC 인터럽트

**REQ-004/011 · DES-002 · IMP-333 · VER-004/011.** Two directed QEMU boots exercised a third host frame arriving after the primary driver entered its exit-86 path. CPU0 had locally masked IRQs, while the old driver’s virtio transport and GIC line remained enabled. The frame advanced the old RX used index from 2 to 3 and raised INTID 79. Production teardown drained that pending interrupt before the replacement was admitted. [IRQ issue #434](https://github.com/Romanof123/JNOX/issues/434) · [Driver issue #426](https://github.com/Romanof123/JNOX/issues/426) · [EVD-333](evidence/native-linux-net-pending-retire/README.md)

두 QEMU 부팅에서 주 드라이버가 종료 코드 86의 처리 경로에 진입한 뒤 호스트가 구세대 세 번째 프레임을 보냈다. CPU0은 로컬 IRQ를 마스킹한 상태였고 기존 장치와 GIC 라인은 활성 상태였다. 구세대 RX used 인덱스가 2에서 3으로 증가했고 INTID 79가 대기 상태가 됐다. 기존 종료 경로가 해당 인터럽트를 정리한 후에만 교체 드라이버가 기동했다.

### Directed observation / 관측 조건

1. The existing consumer gate words retain their values and their roles. `EXIT` authorizes the primary’s exit; `BIND` authorizes the subsequent bind. The test publishes a read-only WAIT marker after `EXIT` and before masking the distributor line. At WAIT and at the subsequent pending snapshot the two words remain `[EXIT, 0]`. Only an Ethernet frame is injected during this interval.
2. Require one additional used descriptor, the matching ingress bytes in its old DMA buffer, a rising transport line, and a GIC pending readback. Local IRQ masking keeps the old handler from consuming the edge. Verify the normal mask, transport ACK, GIC pending clear, reset and queue readbacks before the successor’s first wait. The successor must still have zero event sequence and zero wakes before new traffic.
3. The test-only WAIT has a five-second guest-counter deadline and the pending snapshot has a two-second guest-counter hold. Failure to observe the required arrival prevents replacement. These windows allow inspection and do not measure production interrupt or restart latency. The directed hook is limited to the primary’s status-86 exit. [Test-only observation](../kernel/src/native_linux_net_pending_retire.rs) · [Production retirement](../kernel/src/native_linux_net_restart.rs)

소비자 게이트 `EXIT`와 `BIND`는 기존과 같은 값과 순서로 사용한다. 두 관측 시점 모두 게이트는 `[EXIT, 0]`이며 호스트는 그 사이에 이더넷 프레임 하나만 주입한다. 이전 DMA 버퍼의 실제 바이트와 완료 항목, 장치 신호 상승, GIC 대기 비트를 대조한다. 마스크·장치 ACK·GIC 대기 해제·리셋을 거친 뒤, 새 트래픽을 보내기 전 교체 프로세스의 이벤트 순번과 깨움 횟수가 모두 0이어야 한다. 종료 코드 86 전용 시험에서 대기 상한 5초와 관측 유지 2초는 게스트 카운터 기준의 검사 구간이다. 운용 환경 지연 측정은 별도 과제다.

A separate offline reader passed both boots, including the third frame’s DMA bytes, the pending interrupt cleared before CPU acknowledgement, and the successor traffic in consumer memory. Each run logged two host gate writes. Coverage of external memory writes is limited to those records. Fresh extraction reproduced both independent reports and rejected all 22 corruption attempts. [Independent reading / 독립 판독](evidence/native-linux-net-pending-retire/README.md)

두 부팅의 독립 판독에서 세 번째 프레임의 DMA 바이트, CPU 인수 전 대기 IRQ 해제, 소비자 메모리의 새 트래픽을 확인했다. 호스트 게이트 쓰기는 부팅당 두 건이 기록됐으며, 로그에 없는 외부 쓰기는 이 기록의 검증 범위 밖이다. 새 폴더에서 두 독립 보고서를 재현하고 총 22건의 변조를 검출했다.
