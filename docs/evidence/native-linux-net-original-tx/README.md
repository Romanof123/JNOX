# EVD-328 · Original Linux virtio-net TX / Linux 원본 virtio-net 송신

Two QEMU runs connected a native J-NOX guest to a host Unix socket. In each run the linked Linux v6.18.65 `start_xmit` executed 80 times. The hardware used ring supplied 80 completions, and the host received all 80 Ethernet frames in sequence. An independent offline reader confirmed the wire bytes, execution count and ring-index progression in both captures. [Parent #426](https://github.com/Romanof123/JNOX/issues/426) · [TX issue #430](https://github.com/Romanof123/JNOX/issues/430) · [CHG-328](../../native-linux-net-original-tx.en-ko.md) · [Queue baseline EVD-327](../native-linux-net-queues/README.md)

두 QEMU 실행에서 네이티브 J-NOX 게스트를 호스트 Unix 소켓에 연결했다. 두 실행 모두 원본 Linux `start_xmit` 진입 80회, 장치 used 링의 송신 완료 80건, 호스트의 이더넷 프레임 80개 순차 수신을 확인했다. 독립 판독으로 양쪽 캡처의 프레임 바이트, 함수 실행 횟수, 링 인덱스 진행을 재확인했다.

### Capture identities / 캡처 식별값

| Capture / 캡처 | Guest MAC | MTU | Host nonce (hex) | Ethernet TX bytes / 송신 바이트 | Socket stream bytes / 소켓 스트림 바이트 |
| --- | --- | ---: | --- | ---: | ---: |
| A | `52:54:00:19:30:41` | 1410 | `ccd7f106c7e67f27` | 5,912 | 6,232 |
| B | `52:54:00:19:30:42` | 1386 | `70f13cfe1a1c5c62` | 5,912 | 6,232 |

The socket adds a four-byte length prefix to each of 80 guest-to-host records, accounting for the extra 320 bytes. The two host-to-guest frames are counted separately. 소켓 스트림에는 송신 프레임마다 길이 필드 4바이트가 붙어 총 320바이트가 추가된다. 호스트에서 게스트로 보낸 프레임 두 개는 별도로 집계했다.

| Capture / 캡처 | `wire.pcap` SHA-256 | `machine.log` SHA-256 |
| --- | --- | --- |
| A | `1e00e87f930debf9c01b56c42b91a8e5fbf6823b23aac17d0bcf9a8bad820d78` | `38434241ba9ac278ef38482c31bfffb9d6ea0e4d7eff2caa932d704dfa270373` |
| B | `ec0f9774de46f073128d8455906368f0bae9f9a0ce47db653a8d09c3fc4ed615` | `d22292b3accfbdde5643be85329e651f76bd03db08972729d32610e4ca0ebfa9` |

### Independent reading / 독립 판독

The offline `read_wire.py` reader used the linked ELF, raw socket streams, per-boot nonce, PCAP, execution/MMIO trace and physical RAM snapshots. It checked each sequence, length and payload directly. In both captures it counted 80 executions at the linked `start_xmit` address `0x4038b0` and 80 queue-1 notifications. TX available/used indices progressed from 0/0 to 80/80 and stayed at 80/80 in the terminal snapshot. RX indices progressed from 32/0 through 33/1 to 34/2. The two wire observations share a single QEMU link; the used index provides an aggregate completion count.

독립 판독기는 통합 검증 결과나 게스트 성공 카운터를 입력으로 쓰지 않고, ELF·소켓 스트림·nonce·PCAP·실행 및 MMIO 추적·물리 RAM을 읽었다. 두 캡처에서 프레임의 순번·길이·내용을 대조하고, 링크 주소 `0x4038b0` 진입과 TX 큐 알림을 각각 80회 집계했다. TX 가용/완료 인덱스는 0/0에서 80/80으로 진행해 종료 스냅샷에도 유지됐다. RX는 32/0 → 33/1 → 34/2였다. 두 관측은 같은 QEMU 링크에서 얻었으며, 완료 인덱스는 장치 완료의 총량을 보여 준다.

The integration verifier supplies the source/object/link-map comparison, SG counters, device-status and IRQ-ACK sequence, stage snapshots, peer denial, stale-token and reset checks. Individual skb retirement relies on the cookie and ownership checks in the TX adapter. Capture A reported dropped fixed scheduler events on UART, so the UART list is incomplete. The independent reader's SHA-256 is `8b2b1a6d26661d3a436710af25770667300db7136be5871751ad7ef8af16d555`; its `result.json` SHA-256 is `a3a5d2d2a9056a7d5a2d650ac7a4482c7bf9000accdc653e0f6016b0a5b3177c`. [Integration verifier](../../../tools/verify_linux_source_net_packets.py) · [skb ownership and retirement](../../../drivers/linux-compat/packet/transmit.c)

원본·오브젝트·링크 맵 대조, SG 구성, 장치 상태와 IRQ ACK 순서, 단계별 스냅샷, 동료 슬롯 접근 거부, 폐기 토큰 및 리셋은 통합 검증기가 확인했다. 개별 skb 해제는 TX 어댑터의 쿠키와 소유권 검사에 의존한다. 캡처 A에서는 고정 스케줄러 이벤트 일부가 누락됐다고 UART가 보고했으므로 해당 목록만으로 모든 이벤트의 발생 여부를 판단할 수 없다.

### Results per capture / 캡처별 결과

| Observer / 관측 지점 | Result in both runs / 두 실행에서 확인한 결과 |
| --- | --- |
| Linked ELF and execution trace / ELF·실행 추적 | 80 entries to the original `start_xmit` / 원본 함수 진입 80회 |
| skb and scatterlist / 송신 버퍼 | 40 single-entry submissions with the header in headroom, 40 two-entry submissions with a separate header / 헤더를 앞 여유 공간에 넣은 1항목 SG 40건, 헤더를 분리한 2항목 SG 40건 |
| Device rings / 장치 링 | 80 TX used entries across 32 positions, two full turns plus 16 entries / 32칸 링을 두 바퀴 돌고 16칸 더 진행 |
| Host socket, PCAP and native RX / 호스트 소켓·PCAP·네이티브 수신 | Host sent the challenge at ready, checked the nonce, sequence and bytes of each TX, then sent the acknowledgement at active. Native RX completed those two frames. / 준비 단계에서 challenge를 보낸 뒤 송신 프레임마다 nonce·순번·바이트를 확인하고 활성 단계에서 ACK를 보냈다. 네이티브 RX가 이 두 프레임을 수신했다. |
| Virtio MMIO / 장치 레지스터 | Status writes `0 → 1 → 3 → 11 → 15 → 0`; observed interrupt status `1` by polling and acknowledged that bit / 상태 전이와 인터럽트 상태 비트 1의 폴링·ACK 확인 |
| Access and retirement / 권한·반환 | Slot 1 received `-EACCES` for all seven packet operations (SVC 163–169). Reset cleared both queue-ready bits; two old-token calls returned `-ESTALE`. Slots 0 and 1 exited with status 0. / 슬롯 1의 패킷 연산 7개 모두 거부됐다. 장치 반환 후 두 큐의 준비 비트가 0이 됐고 이전 토큰 호출 2건이 거부됐다. 두 슬롯 모두 종료 상태 0을 기록했다. |

J-NOX EL1 owns the descriptor rings and bounded DMA buffers. The TX adapter retains each linear skb until its device completion returns the original cookie. The native receive path supplied the challenge and acknowledgement. Upstream `virtnet_poll`/NAPI, upstream `virtnet_probe`, CPU interrupt delivery and a restarted packet exchange remain separate verification tasks. ASIL certification is outside this evidence record. [TX adapter](../../../drivers/linux-compat/packet/transmit.c) · [DMA implementation](../../../kernel/src/native_linux_net_packets.rs)

J-NOX EL1이 디스크립터 링과 크기가 제한된 DMA 버퍼를 소유한다. TX 어댑터는 장치 완료로 원래 쿠키를 회수할 때까지 선형 skb를 유지한다. challenge와 ACK 수신에는 네이티브 RX를 사용했다. 업스트림 NAPI 폴링과 probe, CPU 인터럽트 전달, 장치 재시작 후 패킷 재교환은 별도 검증 대상이다. ASIL 인증도 이 기록의 범위 밖이다.

### Replay and regression / 재현과 회귀 시험

Extract the [replay archive](qemu-replay.tar.xz) in an empty directory and run `sh replay.sh`. A fresh extraction reproduced both captures and the independent report. Six targeted changes per capture were rejected, covering the PCAP, the external socket, an executed callback, the hardware used index, an SG counter and a peer access denial. The archive contains 1,085 files and is 2,100,436 bytes. [Archive SHA-256](qemu-replay.tar.xz.sha256) · [Run identities](result.json) · [Integration verifier](../../../tools/verify_linux_source_net_packets.py)

빈 디렉터리에 압축 파일을 풀고 `sh replay.sh`를 실행해 두 캡처와 독립 판독 결과를 재현했다. PCAP, 소켓, 원본 콜백 실행, 장치 완료 인덱스, SG 카운터, 권한 거부 기록을 각각 바꾸면 두 캡처 모두 해당 검사가 실패했다.

A second clean build reproduced the stock, adapted and promoted Linux objects, service ELF, kernel ELF and peer ELF byte for byte. On this revision the earlier queue profile and device-validation profile each passed a separate QEMU regression boot. / 클린 빌드에서 원본·어댑터용·링크용 Linux 오브젝트와 서비스·커널·동료 ELF의 바이트가 모두 일치했다. 같은 리비전에서 기존 큐 시험과 장치 설정 시험을 각각 다시 빌드하고 별도 QEMU 부팅으로 통과를 확인했다.
