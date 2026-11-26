# #434 Native NIC interrupt to blocked RX worker / 네이티브 NIC 인터럽트와 RX 작업자 깨우기

Design for the native FIFO profile, based on the #432 source-net-poll checkout. The intended observation is an Ethernet frame arriving while the service is blocked, followed by an EL1 NIC interrupt, one FIFO wake, upstream RX callback dispatch, and delivery to the independent consumer. No IRQ run was performed for this design.

네이티브 FIFO 프로필에서 서비스가 대기 중일 때 외부 프레임이 도착하도록 한다. EL1 NIC 인터럽트, FIFO 깨우기, 원본 RX 콜백 호출, 독립 소비자 전달까지 같은 실행에서 추적한다. 이 문서는 구현 계획이다.

### Board and interrupt ownership / 보드와 IRQ 소유권

The current service selects MMIO base `0x0a003e00` in `kernel/src/native_linux_net.rs:9`. For the conventional QEMU virt layout this is transport slot 31 (base 0x0a000000, stride 0x200); the expected GIC SPI number is 47, corresponding to architectural INTID 79. An independent DTB export for this QEMU board and NIC configuration confirms the mapping below. Save the actual machine command, DTB and decoded node: match its reg, interrupt-parent, SPI number and trigger flags; read back virtio magic, version and network device ID at that same base. Confirm GIC distributor group, target CPU0, priority, trigger and enable with the active CPU interface. Use the DT trigger setting. The UART SPI initialization at `kernel/src/native_uart_rx.rs:38` illustrates register access and CPU targeting; its line and trigger belong to UART. The native timer uses nonsecure physical PPI 30 (`kernel/src/context.rs:5`).

동일 QEMU 구성에서 출력한 DTB로 슬롯 31의 SPI 47과 INTID 79를 확인했다. 실제 실행의 DTB에서 주소·부모 인터럽트 컨트롤러·번호·트리거를 확인하고 같은 주소의 장치 종류와 GIC 설정을 읽어 남긴다. UART나 다른 플랫폼의 트리거 설정을 복사하지 않는다. 일반 피어의 타이머 기준은 이 네이티브 빌드의 PPI 30이다.

### Blocking and dispatch / 대기와 전달

1. Extend the existing epoch-checked EL1 packet service with a wait taking the active token and last observed event sequence. Serialize wait installation, used-index inspection and interrupt publication on the existing single CPU with local IRQ masked. If a valid pending event or held RX completion exists, return its reason immediately. Otherwise save the syscall frame, mark the caller blocked on this token, and dispatch the ordinary FIFO. Extend the idle eligibility test to include a device waiter: currently only a sleeper prevents terminal park. On wake write the result to the saved frame and enqueue the waiter once. An IRQ while a peer runs preserves that peer's FIFO turn; an IRQ while idle must cause the idle continuation to rescan the ready queue. [FIFO state, timer and dispatch](../kernel/src/native_el0.rs#L186)

   현재 세대와 마지막 이벤트 순번으로 대기한다. 로컬 IRQ를 막은 짧은 구간에서 완료 확인과 대기 등록을 원자적으로 수행한다. 대기 프로세스의 프레임을 저장하고 다른 FIFO 작업을 실행한다. 장치 대기만 남아도 idle에서 IRQ를 받을 수 있게 하며, 깨어난 프로세스는 준비 큐에 한 번만 넣는다.

2. Branch on the verified INTID in `native_sleep_irq` after its single IAR read. Read virtio INTERRUPT_STATUS at +0x60, retain the observed VRING/config bits separately, ACK those bits at +0x64, order the MMIO writes, and return the full IAR token through the common EOI tail. Move ownership of ACK away from polling SVC 168 in this profile so it cannot consume the evidence. A VRING status bit covers both queues: compare RX used progress or buffered completions before reporting queue 0, and account for config-only/TX-only/spurious interrupts independently. Publish the event to its epoch and wake a matching waiter without consuming the descriptor or calling EL0 code in the ISR. [IRQ entry and EOI](../kernel/src/native_el0.rs#L606) · [Current status/ACK service and rearm operations](../kernel/src/native_linux_net_packets.rs#L220)

   IAR는 한 번 읽고 장치 상태를 보존한 뒤 관측 비트만 ACK한다. 배리어 후 전체 IAR 토큰으로 EOI한다. 큐 공통 VRING 비트와 RX 사용 인덱스를 함께 확인한다. ISR은 세대별 이벤트를 게시하고 작업자를 깨우며 디스크립터 회수와 원본 함수 실행은 EL0에서 처리한다.

3. During queue discovery capture the actual queue-0 function pointer supplied by upstream `virtnet_find_vqs` at line 6557. Verify its identity against linked `skb_recv_done`. On an RX event dispatch that stored callback with its own virtqueue. It increments `rq->calls` and schedules RX NAPI; execute only scheduled work with a positive budget. Preserve upstream `virtqueue_napi_complete` enable/snapshot/recheck and continue immediately on full budget or missed work. Change the #432 worker that can start a poll whenever the consumer queue is empty. A wait that sees held work without a matching IRQ is a separate catch-up result. [Original callback](https://github.com/gregkh/linux/blob/4700e3adf3b24b39dd4929e9c6d62811ba3ccea8/drivers/net/virtio_net.c#L2871) · [Callback registration](https://github.com/gregkh/linux/blob/4700e3adf3b24b39dd4929e9c6d62811ba3ccea8/drivers/net/virtio_net.c#L6557) · [Rearm and recheck](https://github.com/gregkh/linux/blob/4700e3adf3b24b39dd4929e9c6d62811ba3ccea8/drivers/net/virtio_net.c#L758)

   큐 발견 시 원본이 넘긴 큐 0의 함수 포인터를 보관하고 링크된 심볼과 대조한다. RX 통지 후 해당 큐로 콜백을 호출하고 실제 예약된 NAPI 작업만 실행한다. 예산 소진 시 계속 처리하고 재무장 뒤 도착한 완료도 재검사한다. IRQ 없이 이미 쌓인 완료를 처리한 경우는 따로 집계한다.

### Retirement and acceptance / 회수와 합격 증거

On remove, owner death or generation change, disable new waits, mask the verified SPI, cancel the old wait with a terminal error, detach its callback and prevent publication to the next epoch. Drain observed status, retire the GIC pending state with the correct bank, confirm device reset and both queue-ready bits clear, and invalidate held completions before recycling DMA or granting a new owner. Keep a failed reset quarantined. Exercise both a stale-token wait and an interrupt already pending at teardown; neither may wake the replacement. [Queue reset](../kernel/src/native_linux_net_queues.rs#L61) · [Packet ownership retirement](../kernel/src/native_linux_net_packets.rs#L62)

소유자 종료 시 신규 대기를 막고 SPI를 마스킹한 뒤 이전 대기와 콜백을 취소한다. 장치 상태·GIC pending과 잔여 완료를 정리하고 리셋 및 양쪽 QUEUE_READY 해제를 확인한 후 새 세대에 할당한다. 실패한 자원은 격리한다. 이전 토큰과 종료 시점의 지연 IRQ가 새 프로세스를 깨우지 않는지 확인한다.

Use host-controlled release points with independent packet injection: (a) after the worker actually blocks, (b) after rearm publishes callback enable and before the used-index recheck, and (c) while notifications are suppressed with a burst above the poll budget. Record the hook and actual PCAP arrival, baseline/observed used indices, epoch, wait/IRQ/wake sequence, IAR/ACK/EOI, registered callback PC, poll work and consumer nonce. A case counts only if its recorded ordering realizes the intended window. Require once-only delivery across ring wrap and zero RX polls during a quiet blocked interval. Run an ordinary peer on absolute timer deadlines throughout, including a NIC-silent control, and report timer PPI count, peer progress and deadline lateness separately from NIC wakes. Coalesced packets can share one hardware interrupt. Preserve a second run without race hooks for ordinary delivery.

외부 송신자가 실제 대기 직후, 재무장 게시와 사용 인덱스 재검사 사이, 통지 억제 중 예산을 넘는 버스트에 각각 도전값을 보낸다. 실행 순서가 해당 창에 들어갔는지 PCAP·인덱스·세대·IRQ·콜백·소비자 기록으로 대조한다. 조용한 구간의 RX poll 수는 0이어야 하며, 링 순환 후에도 각 패킷은 한 번 전달되어야 한다. 절대시각 타이머를 쓰는 일반 피어의 진행과 지연을 NIC가 조용한 대조군과 비교한다. 경주 제어 훅 없이도 일반 전달 경로를 별도 검증한다.

### QEMU device tree / QEMU 장치 트리

Using the executable identified by the NAPI captures, a board-only device-tree export for `virt,gic-version=2` gave the node `/virtio_mmio@a003e00`. Its register span is 512 bytes. The three interrupt cells are `<0 47 1>` on `/intc@8000000`, which selects SPI 47 and architectural INTID 79. Hardware IRQ assertions, GIC acknowledgment and driver wakeups require the forthcoming runtime capture. The SHA-256 covers the full exported DTB bytes. [Board export](evidence/native-linux-net-irq-board/board.json) · [Compressed DTB](evidence/native-linux-net-irq-board/virt-net.dtb.gz) · [IRQ acceptance #434](https://github.com/Romanof123/JNOX/issues/434)

두 NAPI 시험에 사용한 것과 동일한 QEMU 실행 파일로 장치 트리를 별도 출력했다. 해당 MMIO 슬롯의 인터럽트 셀은 `<0 47 1>`이고, GIC INTID는 79다. 실제 NIC 요청·CPU 수락·서비스 깨우기는 후속 실행 시험에서 대조한다.
