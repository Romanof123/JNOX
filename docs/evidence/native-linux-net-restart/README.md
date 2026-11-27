# EVD-332 · Linux-source NIC driver handoff / Linux 원본 NIC 드라이버 인계

In two single-core AArch64 QEMU virt FIFO boots, the primary driver exited with a completed RX buffer it had not harvested and a second writable buffer still posted to the device. After verified reset, a separately linked driver in another address space took ownership once. The same consumer received 80 fresh data frames and one terminal ACK from the replacement. The primary capture verifier reproduced both handoffs. EVD-332 supplies partial evidence for REQ-004/011 through DES-002, IMP-332 and VER-004/011. [Implementation #437](https://github.com/Romanof123/JNOX/pull/437) · [Linux-source driver #426](https://github.com/Romanof123/JNOX/issues/426) · [Design and acceptance](../../native-linux-net-restart-design.en-ko.md)

단일 코어 QEMU virt FIFO에서 두 번 부팅했다. 주 드라이버가 회수하지 않은 수신 완료 버퍼 한 개와 장치에 계속 게시된 쓰기 가능 버퍼 한 개가 남은 상태에서 종료됐다. 리셋을 확인한 뒤 다른 주소 공간의 별도 드라이버가 소유권을 한 번 인수했다. 기존 소비자는 새 드라이버를 통해 데이터 프레임 80개와 마지막 ACK 한 개를 받았다. 두 인계 모두 최초 캡처 검증기로 재현했다.

| Role / 역할 | Slot | TTBR0 page-table base / 페이지 테이블 주소 | ASID | UART exit / 종료 코드 |
|---|---:|---|---:|---:|
| Primary / 주 드라이버 | 0 | `0x40284000` | 1 | 86 |
| Surviving consumer / 기존 소비자 | 1 | `0x40699000` | 2 | 0 |
| Replacement / 새 드라이버 | 2 | `0x40aae000` | 3 | 0 |

**Old generation.** Of two ingress frames addressed to the primary, one reached the consumer. The other was in the used ring at ID 1. Its bytes matched the old frame on the wire and its posted token-1 grant. ID 2 still had a distinct, writable 1,530-byte grant with no used entry. The watched private span of the old process retained SHA-256 `0dbe3edd6d65c168a81ffcc1751a1c639f1814b9effded769b06ea55a0bec409` through the handoff. [Grant receipts](../../../kernel/src/native_linux_net_restart.rs) · [Ring and frame checks](../../../tools/verify_linux_source_net_restart.py)

이전 세대의 입력 두 개 중 첫 프레임은 소비자에게 도착했다. 두 번째는 완료 링의 ID 1에 남았고, 바이트와 게시된 버퍼 권한이 각각 캡처한 프레임과 토큰 1에 일치했다. 별도 ID 2는 완료 항목 없이 1,530바이트 쓰기 가능 버퍼로 장치에 게시돼 있었다. 인계 전후 이전 프로세스의 감시 대상 메모리 해시는 같았다.

**Reset and new generation.** Exit 86 triggered retirement. Device Status and both QueueReady registers read back zero. The old grants were cleared before slot 2 acquired token 2. Six stale-token calls returned −116. The same slot 1 continued its timed work and received only the fresh generation’s data and ACK after handoff. Each PCAP contained 83 ingress records, comprising two old frames, 80 fresh frames and a 60-byte ACK. Queue discovery ran once in each driver. Per boot, the primary entered `skb_recv_done`, `virtnet_poll` and `gro_receive_skb` once each. The replacement entered them 10, 20 and 81 times respectively. The host wrote only two test release words in the consumer. The observed grants, rings, frame data and journals were read-only to the capture procedure.

종료 코드 86으로 장치 회수를 시작했다. 장치 Status와 두 QueueReady 레지스터가 모두 0으로 읽혔고 이전 권한이 정리된 뒤 슬롯 2가 토큰 2를 얻었다. 폐기된 토큰으로 한 여섯 호출은 모두 −116을 반환했다. 슬롯 1의 타이머 작업은 이어졌고, 인계 후에는 새 세대 데이터와 ACK만 전달됐다. 실행마다 PCAP의 입력 83개는 이전 프레임 두 개, 새 데이터 80개, 60바이트 ACK 한 개였다. 호스트가 쓴 메모리는 소비자의 시험 진행 워드 두 개뿐이다.

| Capture / 실행 | SHA-256 of original PCAP / 원본 PCAP 해시 |
|---|---|
| `handoff-a` | `e4cb25aa89b3b6e706a8248418d27b261d82bc94aa00065fa420850b244fa76b` |
| `handoff-b` | `1002ea5767e4e25e8309a11d65c1ef96faa988242c97bb51e215776bf9bbb3f3` |

An independent reader reproduced both handoffs from the embedded ELFs, page tables, five halted RAM checkpoints, device trace, PCAP and UART. It counted INTID 79 deliveries with used-buffer status by driver generation, 2 for the primary and 11 for the spare in A, and 2 and 10 in B. It confirmed six stale-token returns of −116, three consumer authority refusals of −13 and the uninterrupted original consumer. Fresh extraction verified 1,358 checksummed members and reproduced both independent JSON reports exactly. The primary verifier rejected seven corrupted copies per capture, 14 attempts in total.

독립 판독자는 실행 파일, 페이지 테이블, 다섯 정지 지점의 RAM, 장치 추적·PCAP·UART를 대조해 두 인계를 재현했다. used-buffer 상태가 동반된 INTID 79는 A에서 주·예비 드라이버별 2회·11회, B에서 2회·10회였다. 폐기 토큰 호출 6회는 −116, 기존 소비자가 장치 권한을 요청한 3회는 −13이었다. 빈 디렉터리에 압축을 풀어 1,358개 파일의 체크섬을 검증했고, 두 독립 JSON 보고서가 정확히 재현됐다. 실행별 일곱 건씩 총 14건의 변조 시도를 모두 검출했다.

Verify SHA-256 `c780039bcda1abc3d2e4b086457edc71daed2193bcadb525309a35beb575caaa`, extract into an empty directory and run `python3 -B replay.py` from the extraction root with Python 3.10+ and AArch64 binutils. 위 해시를 확인한 후 빈 디렉터리에 압축을 풀고 해당 폴더에서 재실행한다. [Replay archive](replacement.tar.xz) · [Checksum](replacement.tar.xz.sha256) · [Result manifest](result.json)

At primary retirement, both the GIC pending bit and the virtio interrupt-status bits were zero. EVD-333 reports the separate directed #434 captures with a pending device interrupt at retirement. The selected RX callbacks ran with a successful reset. Full upstream `probe/open`, failed reset or stuck QueueReady, physical IRQ/DMA isolation, target timing bounds and product safety qualification require separate evidence. Quarantine ordering on reset failure was inspected in source only. [Directed retirement EVD-333](../native-linux-net-pending-retire/README.md) · [IRQ retirement #434](https://github.com/Romanof123/JNOX/issues/434) · [Driver resources #5](https://github.com/Romanof123/JNOX/issues/5) · [Trace links](../../../trace/links.csv)

주 드라이버 회수 시 GIC 대기 비트와 virtio 인터럽트 상태 비트는 모두 0이었다. 회수 시점에 장치 IRQ가 이미 대기 중인 #434의 후속 지정 시험 결과는 EVD-333에 수록했다. 이번 관측은 리셋이 성공한 지정 RX 경로에 해당한다. 원본 `probe/open` 전체, 리셋 미응답이나 QueueReady 고착, 실보드 IRQ·DMA 격리, 시간 상한과 제품 안전 적격성은 별도 검증이 필요하다. 리셋 실패 때의 격리 처리 순서는 소스에서만 확인했다.
