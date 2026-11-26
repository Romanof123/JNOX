# CHG-330 · Linux virtio-net NAPI receive polling / Linux virtio-net NAPI 수신 폴링

**REQ-004/011 · DES-002 · IMP-330 · VER-004/011.** Two QEMU boots executed the registered Linux v6.18.65 `virtnet_poll` callback 26 times each. In both runs, 80 data frames and a terminal ACK reached a separate J-NOX consumer. The original `virtnet_alloc_queues` ran once per boot; the registered RX poll entry was `0x40ac80`. EVD-329 covers the earlier direct buffer callbacks. [Parent #426](https://github.com/Romanof123/JNOX/issues/426) · [RX issue #432](https://github.com/Romanof123/JNOX/issues/432) · [Buffer baseline EVD-329](evidence/native-linux-net-rx/README.md) · [Measurements EVD-330](evidence/native-linux-net-napi/README.md)

두 QEMU 부팅에서 Linux v6.18.65의 원본 `virtnet_alloc_queues`가 각각 한 번 실행됐다. 등록된 RX 콜백 `virtnet_poll`은 부팅당 26회 실행됐다. 별도 J-NOX 소비자는 네이티브 파이프로 데이터 프레임 80개와 마지막 ACK를 받았다. 이전 단계인 EVD-329에서는 수신 버퍼 직접 등록과 회수를 측정했다.

### Poll budget and ownership / 폴링 예산과 소유권

- The host queued an initial burst of 16 frames while the guest CPU was paused. With a budget of eight, ten poll calls used their entire allowance. The driver suppressed callbacks 26 times and prepared to re-enable them 16 times. All 16 subsequent used-ring checks returned no pending work. A completion deliberately timed between re-enable and the recheck remains untested.
- EL1 owns the split ring and the 32 bounded DMA buffers. The original poll drives refills; the native transport records queue notifications and reads the hardware used ring. The typed compatibility entry `gro_receive_skb` handed off 81 skbs with `GRO_NORMAL`; this mode delivers individual frames without GRO aggregation. The consumer records the frames in its own translated pages.
- The device interrupt-status bit for a used buffer was polled and acknowledged. CPU delivery of the NIC interrupt, complete upstream probe/open, replacement of a live driver process and physical-board operation remain separate verification tasks. [Poll verifier](../tools/verify_linux_source_net_rx_poll.py) · [EL1 ring and callbacks](../kernel/src/native_linux_net_packets.rs)

호스트가 게스트 CPU를 멈춘 동안 첫 프레임 16개를 한꺼번에 보냈다. 한 번에 처리할 수 있는 예산은 8개였고, 10회는 예산 전부를 사용했다. 콜백 억제는 26회, 재활성화 준비는 16회였으며 재확인 때 대기 중인 완료는 모두 0건이었다. 재활성화와 재확인 사이에 완료가 도착하도록 강제한 경합은 후속 검증 대상이다. EL1이 링과 고정 DMA 버퍼를 소유하고, 원본 폴링 함수가 버퍼 보충을 주도했다. 호환 계층의 `gro_receive_skb`는 81개 skb를 `GRO_NORMAL`로 개별 전달했다. 이번 측정은 장치 상태 비트의 폴링·ACK까지이며 CPU 인터럽트 전달, 원본 probe/open 전체 경로, 실행 중인 드라이버 프로세스 교체 및 실기기 동작은 아직 검증하지 않았다.
