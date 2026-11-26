# EVD-330 · Original Linux NAPI poll / Linux 원본 NAPI 폴링

Two QEMU captures traced one execution of Linux v6.18.65 `virtnet_alloc_queues` and 26 executions of the registered `virtnet_poll` per boot. In each capture a separate EL0 consumer received **80 ordered Ethernet frames totalling 5,912 bytes**, then a final host acknowledgement. The 81 entries to `gro_receive_skb` are calls into the typed J-NOX compatibility adapter. It delivered each skb through the `GRO_NORMAL` path. [Parent #426](https://github.com/Romanof123/JNOX/issues/426) · [RX issue #432](https://github.com/Romanof123/JNOX/issues/432) · [CHG-330](../../native-linux-net-napi.en-ko.md)

두 QEMU 캡처 각각에서 원본 큐 할당 함수 1회, 등록된 원본 NAPI 폴링 함수 26회의 실행을 추적했다. 별도 EL0 소비자는 **총 5,912바이트의 이더넷 프레임 80개**를 순서대로 받은 뒤 호스트의 마지막 ACK를 받았다. `gro_receive_skb` 진입 81회는 J-NOX 호환 어댑터의 실행이며, `GRO_NORMAL`로 skb를 개별 전달했다.

| Per boot / 부팅별 | Capture A | Capture B |
| --- | --- | --- |
| MAC / MTU | `52:54:00:19:33:41` / 1410 | `52:54:00:19:33:42` / 1386 |
| Host nonce / 호스트 nonce | `b4327173d9655a1c` | `142645246cd83d63` |
| Wire PCAP SHA-256 | `861977b4d359c9a2d71bac8afb6ea83aff6692ddce6ab708cc109cc2ffb694d8` | `536c58fcdb5ba2620d1e1114fb273410a11fd3bcc8bb707fdce62d8c6a1744a8` |
| Execution/MMIO trace SHA-256 | `2deb6d2b2d5c11a591ecf7fc7fd047124de02a49850e92c359c0c83498dffe68` | `c2ceccdca1f8e9e92ccdd33fd2102fda77028fd005e62252cf00d1068c780c8e` |
| Poll entries at active / 활성 시점 누적 | 24 | 23 |
| Poll entries at final / 종료 시점 누적 | 26 | 26 |

### What the captures show / 관측 결과

1. **Scheduling.** The registered poll address was `0x40ac80`. The initial 16-frame injection occurred with the guest CPU paused. Both boots exhausted a budget of eight ten times. Callback suppression ran 26 times and enable-prepare ran 16 times. All 16 rechecks found no pending completion. This schedule exercised rearming; the arrival-during-rearm case remains open.
2. **Receive ring.** The available/used indices went from 32/0 at ready to 112/80 at active and 112/81 after the ACK. At that point 31 posted buffers remained device-owned before reset. Each boot issued 11 queue notifications. The native transport polled used-buffer interrupt status `1` and acknowledged it.
3. **Delivery.** Socket/PCAP frame bytes were compared with the separate pipe consumer’s private memory. The skb delivery journal counted 81 handoffs, dequeues and releases, zero drops and zero delivery faults. The reserved unsupported-import fault ID and native NAPI fault word remained zero. The trace and link map identify the original allocation and poll functions separately from the callback-control and skb compatibility routines. [Capture and replay verifier](../../../tools/verify_linux_source_net_rx_poll.py)

1. **스케줄링.** 등록 주소는 `0x40ac80`이었다. 첫 프레임 16개를 CPU 정지 상태에서 주입했다. 두 부팅 모두 예산 8개를 전부 사용한 폴링이 10회였고, 콜백 억제 26회와 재활성화 준비 16회가 기록됐다. 재확인 16회에서 새 완료는 관측되지 않았다. 재활성화 도중 도착하는 경합은 아직 강제로 재현하지 않았다.
2. **수신 링.** 가용/완료 인덱스는 준비 시점 32/0, 활성 시점 112/80, 마지막 ACK 이후 112/81이었다. 리셋 직전에는 게시된 버퍼 중 31개를 장치가 소유했다. 큐 알림은 11회였고 네이티브 전송 계층이 인터럽트 상태 값 1을 폴링해 ACK했다.
3. **전달.** 소켓·PCAP의 프레임 바이트를 파이프로 연결된 별도 소비자의 전용 메모리와 대조했다. skb 전달·회수·해제는 각각 81회였고 드롭과 전달 오류는 0이었다. 미지원 임포트에 예약한 오류 ID와 네이티브 NAPI 오류 값도 0이었다.

An independent reader matched the non-relocated instructions of the two original functions to the packet-profile object, then decoded their executed addresses, the registered poll pointer, 10 exhausted budgets and 81 typed GRO handoffs in each capture. Both external streams matched the isolated recipient's RAM. All 16 rearm checks per run saw an empty pending set. The NIC IRQ status bit was polled. An actual CPU interrupt wake and a completion arriving in the rearm window have separate acceptance criteria. [IRQ design](../../native-linux-net-irq-design.en-ko.md) · [Driver restart plan](../../native-linux-net-restart-design.en-ko.md) · [IRQ issue #434](https://github.com/Romanof123/JNOX/issues/434)

독립 판독으로 원본 오브젝트와 링크된 실행 파일의 명령, 등록된 콜백 주소, 예산 소진, 수신 인계와 별도 앱의 메모리를 대조했다. 이 기록은 GRO 병합을 끈 QEMU 프로파일에 해당한다. CPU 인터럽트 전달, 재활성화 중 들어오는 완료, 원본 probe/open 전체 경로, 프로세스 교체 및 실기기 동작은 후속 검증 대상이다.

Extract the archive into an empty directory and run `sh replay.sh`. Both offline runs rejected eight intentional corruptions each and reproduced the independent report. A second build reproduced the upstream, adapted and promoted objects and the kernel, service and peer ELFs byte for byte. The original buffer-only profile passed a separate regression boot on the same revision. EVD-330 supplies partial evidence for REQ-004/011 through DES-002, IMP-330 and VER-004/011. [Trace links](../../../trace/links.csv) · [Raw replay archive](qemu-replay.tar.xz) · [SHA-256](qemu-replay.tar.xz.sha256) · [Run identities](result.json)

빈 폴더에 압축을 풀고 각 캡처에 심은 8개 오류의 검출과 독립 결과 재현을 확인했다. 원본 오브젝트와 커널·서비스·소비자 실행 파일은 재빌드해도 바이트가 일치했다. 같은 리비전에서 기존 원본 버퍼 프로파일도 별도로 부팅해 통과했다.
