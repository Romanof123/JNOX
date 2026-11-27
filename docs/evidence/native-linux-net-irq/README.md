# EVD-331 · Native virtio-net RX interrupt / 네이티브 virtio-net 수신 인터럽트

On one AArch64 QEMU virt CPU with GICv2 and FIFO scheduling, ingress on virtio-net queue 0 woke a blocked J-NOX Linux-source service. Each of three boots delivered 80 distinct data frames and one terminal ACK to the isolated native consumer. The first two used ordinary delivery. The third used a separate image to admit a frame during callback re-enable and to exercise an interrupt with no device work. EVD-331 supplies partial guest evidence for REQ-004/011, DES-002, IMP-331 and VER-004/011. [Change #434](https://github.com/Romanof123/JNOX/issues/434) · [Integration #436](https://github.com/Romanof123/JNOX/pull/436) · [Design](../../native-linux-net-irq-design.en-ko.md)

단일 코어 QEMU virt의 GICv2·FIFO 구성에서 virtio-net 큐 0의 수신 인터럽트가 대기 중인 J-NOX 서비스를 깨웠다. 세 번의 부팅에서 각각 서로 다른 데이터 프레임 80개와 마지막 ACK 한 개가 격리된 네이티브 소비자에게 전달됐다. 일반 이미지로 두 번 실행했고 별도 시험 이미지로 콜백 재활성화 중 수신과 장치 작업이 없는 인터럽트를 확인했다.

| Capture / 실행 | Device IRQs / NIC IRQ | CPU IAR/EOI pairs | Blocked wakes from idle / idle 중 대기 해제 | Queue callbacks / 큐 콜백 | Original NAPI polls / 원본 폴링 | Full budgets of 8 / 예산 8개 소진 | Timer IRQs / 타이머 IRQ |
|---|---:|---:|---:|---:|---:|---:|---:|
| Ordinary A · `capture-a` | 19 | 19 | 10 | 10 | 20 | 10 | 4 |
| Ordinary B · `capture-b` | 14 | 14 | 10 | 10 | 20 | 10 | 4 |
| Directed · `directed-final` | 14 | 15 | 11 | 11 | 21 | 9 | 28 |

**Interrupt and ownership.** QEMU’s NIC assertion, GIC input, CPU acknowledgment of INTID 79, virtio status/ACK and EOI were recorded together. The service invoked the registered upstream `skb_recv_done` callback and the scheduled `virtnet_poll`. All three boots recorded zero catch-up wakes, 81 `gro_receive_skb` entries and one retirement each. The peer’s wait and metric reads returned −13, and the old owner token returned −116 after retirement. The ordinary peer continued on its absolute timer during the quiet interval while the RX service stayed blocked. [Capture checks](../../../tools/verify_linux_source_net_irq.py) · [Owner and generation checks](../../../kernel/src/native_linux_net_irq.rs)

장치 IRQ부터 GIC 입력, CPU의 INTID 79 수락, virtio 상태·ACK, EOI까지 대조했다. 원본 큐 콜백과 NAPI 폴링이 실행됐고 실행마다 `gro_receive_skb` 진입 81회, 장치 회수 1회, IRQ 없이 누적 완료를 처리한 깨우기 0회였다. 일반 피어가 대기와 계측값 읽기를 요청하면 −13, 회수된 세대의 토큰은 −116을 반환했다. 입력이 없는 동안 RX 서비스가 대기하는 사이 피어의 절대시각 타이머는 계속 진행했다.

**Re-enable race.** The directed image first software-pended INTID 79 with zero virtio status. The CPU acknowledged it and issued EOI without dispatching an RX callback. During real ingress, the paused snapshots showed queue 0’s used index move from 1 to 2 after notification re-enable. The original pending check scheduled another poll, which delivered the second frame before another queue callback. The directed boot therefore has 15 CPU acknowledgments for 14 device IRQs. [Callback and recheck code](../../../drivers/linux-compat/packet/rx_virtqueue.c)

지정 시험 이미지는 먼저 virtio 상태가 0인 상태에서 GIC에 소프트웨어로 INTID 79를 걸었다. CPU는 수락과 EOI를 수행했고 RX 콜백은 호출하지 않았다. 실제 프레임을 넣은 두 정지 지점의 큐 0 완료 인덱스는 1과 2였다. 콜백을 다시 켠 뒤 원본 코드가 새 완료를 찾아 다음 큐 콜백 전에 폴링을 재실행해 두 번째 프레임을 전달했다.

**Build history and comparison.** The first package check stopped on `missing queue-service implementation: jnox_net_setup_queues`. The IRQ profile uses upstream `virtnet_find_vqs`. A later routing check expected the target byte to read back 1. On this uniprocessor GICv2 that field reads as zero and ignores writes (RAZ/WI). The revised check accepts the fixed CPU0 route when `GICD_TYPER` identifies one interface and the IRQ is implemented. All three guests reported `GICD_TYPER = 8` and target byte 0. A separate polling-only regression from source `6c9c5926ec09ee97bea2bffcaeb4cb73d3e5f2bb` delivered 80 data frames and the ACK, with ten exhausted budgets. [Implementation history and first build digest](../../native-linux-net-irq-design.en-ko.md) · [Earlier polling evidence](../native-linux-net-napi/README.md) · [Same-source polling regression](poll-regression.tar.xz)

최초 패키징은 이전 큐 등록 함수의 심볼 검사에서 중단됐다. IRQ 프로필의 검사 대상을 원본 등록 함수로 수정했다. 이후 단일 CPU에서 GIC 대상 바이트가 0으로 읽히는 동작을 반영했다. 세 부팅 모두 GICD_TYPER 값 8과 대상 바이트 0을 기록했다. 같은 소스의 폴링 전용 회귀 시험에서도 데이터 80개와 ACK 한 개를 전달했고 예산 소진은 10회였다.

**Capture identities / 캡처 식별값**

| Capture | Nonce | SHA-256 of original PCAP |
|---|---|---|
| `capture-a` | `6b37f1f86972e2c9` | `d0bdf6cf2edabe18cc6e36becac908e2cfbdea2b052456dc8f3964790c61b764` |
| `capture-b` | `0f693082bbeb8d40` | `e7b137ca53f63b488f7c5e13f95b2690228665f6c59aa7cac93e76639238720d` |
| `directed-final` | `22cd7789f92458d3` | `6aef5ea177f2802c9fd4e63c99bb4ebf9068cee1ff7d9860b64dc1d985754e95` |

Ordinary kernel ELF SHA-256 `c01936a662f30da44716dc284c27eff986a3b21cf2bb1fa62ce12706d69f2ec9`, observer `08e492586020588cb4afcde631c3d3bf356ca109`. Directed kernel ELF SHA-256 `6a9dd4613170d7e0c98decee69502dc64dda2772ce023b0956708d6d6a37d129`, observer `9559913e98abc7a1a2033e69dc90d1071cbc2a7b`.

An independent reader confirmed the device IRQ, original callback, NAPI poll and GRO counts in all three captures. It checked the received frames through the peer’s page-table root at `0x40679000`. The driver root was `0x40264000`. A clean extraction reproduced all three independent JSON reports. The offline verifier rejected all 40 attempted mutations, with 13 each for ordinary A and B and 14 for the directed case. [Fresh replay output](fresh-replay.txt) · [Capture manifest](result.json)

독립 판독으로 세 실행의 NIC IRQ·원본 콜백·NAPI 폴링·GRO 횟수를 확인했다. 수신 프레임은 드라이버와 별개인 소비자의 페이지 테이블 루트로 판독했다. 빈 디렉터리에 압축을 풀어 재실행했을 때 독립 판독 JSON 세 건이 모두 일치했고 일반 실행별 13건, 경합 실행 14건으로 총 40건의 변조 시도를 모두 검출했다.

To replay, verify SHA-256 `7e0709dca87ba1de995825bcbd55057ff78ad49a9a385e587035d7b97baae59f`, extract the IRQ archive into an empty directory and run `python3 -B replay.py` from the extraction root with Python 3.10+ and AArch64 binutils. 위 SHA-256을 확인한 뒤 빈 폴더에 압축을 풀고 해당 폴더에서 재판독 명령을 실행한다. [Raw IRQ archive](qemu-replay.tar.xz) · [Checksum](qemu-replay.tar.xz.sha256)

**Retirement and open criteria.** Each captured boot retired the owner once after the terminal ACK, with quiet teardown and rejection of the retired token. A real NIC interrupt already pending at retirement and the resulting deferred work across the generation boundary remain to be exercised under #434. #426 tracks the successor driver process and an exchange while DMA is in flight. Physical interrupt routing, DMA isolation, worst-case timing and ASIL qualification remain open. [IRQ retirement #434](https://github.com/Romanof123/JNOX/issues/434) · [Successor driver #426](https://github.com/Romanof123/JNOX/issues/426) · [Driver resources #5](https://github.com/Romanof123/JNOX/issues/5) · [Restart criteria](../../native-linux-net-restart-design.en-ko.md)

세 실행 모두 마지막 ACK 이후 새 IRQ가 없는 상태에서 소유자를 한 번 회수했고, 회수한 토큰의 접근을 거부했다. 회수 시점에 실제 NIC IRQ가 이미 대기 중인 경우와 그 지연 작업이 다음 세대에 미치는 영향은 #434의 남은 검증 항목이다. 새 드라이버 프로세스로 소유권을 넘기고 DMA 진행 중 데이터를 교환하는 시나리오는 #426에서 추적한다. 실보드 IRQ 라우팅·DMA 격리, 최악 시간 한계, ASIL 적격성은 별도 검증 대상이다. [Trace links](../../../trace/links.csv)

The polling regression supplement contains its own booted ELFs and raw capture. A clean extraction reproduced the saved result and rejected eight mutations. The original polling wrapper recorded the booted binaries. Its observer module hashes were measured at packaging time. [Polling manifest](poll-regression.json) · [Polling checksum](poll-regression.tar.xz.sha256)

폴링 회귀 묶음에는 해당 부팅의 ELF와 원본 캡처가 들어 있다. 새 디렉터리에서 결과가 재현됐고 변조 8건을 검출했다. 폴링 관측 도구의 모듈 해시는 묶음을 제작할 때 기록했다.
