# EVD-329 · Original Linux virtio-net receive buffers / Linux 원본 virtio-net 수신 버퍼

In two QEMU boots, the J-NOX EL0 service executed Linux v6.18.65 `add_recvbuf_small` and `virtnet_rq_get_buf`. In each boot, a separate EL0 consumer received **80 ordered Ethernet frames totalling 5,912 bytes**, followed by a host ACK. The independent reader matched each data frame to the consumer’s private RAM and the ACK to its separate destination. [Parent #426](https://github.com/Romanof123/JNOX/issues/426) · [RX buffer issue #432](https://github.com/Romanof123/JNOX/issues/432) · [CHG-329 and acceptance criteria](../../native-linux-net-rx.en-ko.md)

두 QEMU 부팅에서 J-NOX EL0 서비스가 Linux v6.18.65의 원본 수신 버퍼 등록·회수 함수를 실행했다. 각 부팅에서 별도 EL0 소비자가 순서대로 받은 데이터는 **이더넷 프레임 80개, 총 5,912바이트**였고, 그 뒤에 호스트 ACK를 받았다. 독립 판독기는 수신 프레임 각각을 소비자 전용 메모리와 대조하고, ACK가 별도 수신 영역에 기록됐음을 확인했다.

| Per boot / 부팅별 | Capture A | Capture B |
| --- | --- | --- |
| MAC / MTU | `52:54:00:19:32:41` / 1410 | `52:54:00:19:32:42` / 1386 |
| Host nonce / 호스트 nonce | `b055370a716f4b80` | `b772559d3c44ecc6` |
| `add_recvbuf_small` entries / 진입 횟수 | 113 | 113 |
| `virtnet_rq_get_buf` entries, including empty attempts / 빈 회수 시도 포함 | 104 | 102 |
| Device RX used entries, including final ACK / ACK 포함 장치 완료 | 81 | 81 |
| RX queue notifications / RX 큐 알림 | 82 | 82 |
| `wire.pcap` SHA-256 | `fad93cb0cc0e2307e3be5486fe72b488c78b44242e7cd10fb03c6d8dde4101c6` | `a6fd7e2584d64009c73101c9ae849a67f9da7ec582f9f1bbe177eb88c07846d5` |
| `machine.log` SHA-256 | `75d665fae03e5889b7e391f4bc0072ad55a31ccf5269f397b9a68011d2fb49d7` | `61eaf3a08b94cd4bc560f6f3266130677cd365c024c71c20801ac9499c5cf8c3` |

### Independent observations / 독립 판독

The standalone reader used the frozen socket transcripts, PCAP, ELF and packet object, trace, and physical RAM snapshots. At ready, the consumer frame array was empty. At active, all 80 entries matched the socket and PCAP byte for byte, and the ACK destination was still zero. At final, the ACK destination matched the incoming wire ACK. The outbound socket stream was empty. The three snapshots showed RX available/used indices **32/0 → 112/80 → 113/81**. Both indices crossed the 32-slot ring boundary twice after ready. Active DMA memory retained the last 32 frames; final used entries retained the last 32 completion IDs and lengths.

독립 판독기는 동결된 소켓 기록·PCAP·ELF·패킷 오브젝트·실행 추적·물리 RAM을 사용했다. 준비 시점에는 소비자 프레임 배열이 비어 있었다. 활성 시점에는 80개 항목 모두 소켓 및 PCAP의 바이트와 일치했고, ACK 영역은 아직 0이었다. 종료 시점에는 해당 영역이 호스트가 마지막에 보낸 ACK와 일치했다. 게스트에서 호스트로 나간 소켓 데이터는 없었다. RX 가용/완료 인덱스는 **32/0 → 112/80 → 113/81**로 진행했다. 준비 이후 양쪽 인덱스가 32칸 링의 경계를 두 번 넘었다.

The reader matched non-relocated instructions from both packet-object sections to the linked service, counted executions at `0x402e60` and `0x402c50`, checked privileged access at the NIC MMIO address, and translated the consumer’s distinct frame pages. Its checks cover those addresses and capture points. Reset readback showed device status zero and both queues idle. The trace covers the two buffer callbacks; upstream `virtnet_poll`, skb delivery and GRO execution remain outside this measurement.

판독기는 두 함수에서 재배치 대상 명령을 제외한 오브젝트 코드를 링크된 서비스와 대조하고 두 진입 주소의 실행 횟수를 집계했다. 지정한 NIC MMIO 주소의 특권 접근 제한과 소비자 프레임의 별도 물리 페이지를 확인했다. 검증 범위는 관측한 주소와 캡처 시점이다. 리셋 뒤 장치 상태와 양쪽 큐의 준비 비트는 0이었다. 원본 `virtnet_poll`·skb 전달·GRO 실행은 이번 측정 범위 밖이다.

### Grant and interrupt checks / 버퍼 권한·인터럽트 검사

The integration verifier checked 32 explicit 1,530-byte writable grants before device start, rejected an overlapping grant with `-EBUSY`, and observed `-EACCES` for all three RX operations from the consumer. Two calls using the retired token returned `-ESTALE`. The guest polled virtio MMIO interrupt status and acknowledged bit 0 (numeric value `1`, used-buffer notification). CPU interrupt delivery remains unverified. Grant errors and ownership checks come from the integration verifier; the independent reader checked the wire, callback executions, selected mappings and ring progress. [Integration verifier](../../../tools/verify_linux_source_net_rx.py) · [EL1 grant implementation](../../../kernel/src/native_linux_net_packets.rs)

통합 검증기는 장치 시작 전 쓰기 가능한 1,530바이트 버퍼 32개의 등록을 확인했다. 중첩 등록은 `-EBUSY`, 소비자 슬롯에서 호출한 세 RX 연산은 모두 `-EACCES`, 반환된 토큰으로 시도한 두 호출은 `-ESTALE`이었다. 게스트는 virtio MMIO 상태를 폴링해 값 1인 used-buffer 알림을 확인하고 ACK했다. CPU 인터럽트 전달은 검증 대기 상태다. 이 권한 오류는 통합 검증 결과이며, 독립 판독은 프레임 바이트·함수 실행·지정된 매핑·링 진행을 확인했다.

EVD-329 supplies partial evidence for REQ-004 and REQ-011 through DES-002, IMP-329 and VER-004/011. EVD-329는 두 요구사항의 부분 근거로 추적한다. [Requirements / 요구사항](../../requirements.en.md) · [Trace / 추적 관계](../../../trace/links.csv)

Extract the archive into an empty directory and run `sh replay.sh`. Fresh extraction reproduced both captures and the independent reader's report. Each capture rejected all eight altered inputs. The archive contains 1,095 files in 2,162,268 bytes. 빈 폴더에서 압축을 풀고 두 부팅을 재현했다. 각 부팅에서 원본 기록에 심은 8개 오류를 전부 검출했고 독립 판독 결과도 일치했다. [Replay archive / 재생 아카이브](qemu-replay.tar.xz) · [SHA-256](qemu-replay.tar.xz.sha256) · [Run identities / 시험 식별값](result.json)
