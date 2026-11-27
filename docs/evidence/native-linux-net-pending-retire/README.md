# EVD-333 · Pending RX interrupt across driver replacement / 드라이버 교체 직전 대기 RX 인터럽트

Both QEMU boots recorded an external frame completing against the old owner’s RX grant while CPU0’s local IRQ mask was set. QEMU asserted the virtio line and GIC INTID 79. The old used index advanced **2 → 3**; the pending interrupt caused **zero successor wakes**. The replacement subsequently received its own traffic on epoch 2. [CHG-333](../../native-linux-net-pending-retire.en-ko.md) · [IRQ issue #434](https://github.com/Romanof123/JNOX/issues/434) · [Driver issue #426](https://github.com/Romanof123/JNOX/issues/426)

두 QEMU 부팅 모두 CPU0의 로컬 IRQ가 마스킹된 동안 외부 프레임이 기존 소유자의 RX 버퍼에서 완료됐다. virtio 장치 신호와 GIC INTID 79가 활성화됐고 구세대 used 인덱스는 **2 → 3**으로 증가했다. 이 인터럽트로 인해 교체 드라이버가 깨어난 횟수는 **0회**였다. 이후 새 드라이버는 세대 2에서 자체 트래픽을 수신했다.

| Observation / 관측값 | Pending A | Pending B |
| --- | --- | --- |
| Primary → replacement epoch / 주·교체 세대 | 1 → 2 | 1 → 2 |
| Wire ingress frames / 입력 프레임 | 84 | 84 |
| Prior unreaped ID, third frame ID, distinct outstanding ID / 이전 미회수·세 번째 완료·별도 게시 ID | 1, 2, 3 | 1, 2, 3 |
| Third used length / 세 번째 완료 길이 | 74 B = 12 B virtio + 62 B Ethernet | 74 B = 12 B virtio + 62 B Ethernet |
| Third Ethernet frame SHA-256 | `7a04ff88e64c1b1ad2b1a02c8d9c8289b026401cbf9048588721b70f118a2d7b` | `6d82462f88fa41132119b748420e8f51525da94099c954f577557ab130b40075` |
| PCAP SHA-256 | `a12315139e6185cdf434ae35fd09e2652016246d1abc4776248b6d6911c88ebf` | `9086672c4bb151546156d215e3b964d47750985e47952685161594c239f29d50` |
| Exit statuses, primary / surviving consumer / spare | 86 / 0 / 0 | 86 / 0 / 0 |

### Evidence sequence / 증거 순서

- At WAIT the old RX used index was 2. After the third ingress it was 3. Descriptor 2 was writable, granted to epoch 1, and pointed to the old kernel DMA buffer. Its returned bytes matched the wire and the protected copy. The frozen old user span retained SHA-256 `0dbe3edd6d65c168a81ffcc1751a1c639f1814b9effded769b06ea55a0bec409` in both runs. The two existing release words remained `[0x45584954, 0]` at both snapshots; the later bind used the original `0x42494e44` value.
- Both machine traces placed the transport assertion at offset 4852, GIC assertion at 4853 and pending-register read at 4855. Teardown masked the line at 4864, acknowledged the device at 4866, observed the transport and GIC clear at 4867 and 4868, and wrote the pending clear at 4870. After reset, the raw GIC enable and pending bits and virtio interrupt status read zero. The observer checked for software writes to the GIC set-pending bit during the arrival interval.
- Before successor traffic, the new epoch recorded event sequence 0 and blocked-wake count 0. Across both boots the primary’s `skb_recv_done` executed once per boot; the successor’s executed ten times per boot on its own traffic, with 20 original `virtnet_poll` entries and 81 typed GRO handoffs. [Capture and replay implementation](../../../tools/verify_linux_source_net_restart.py)

WAIT 이후 세 번째 프레임은 구세대의 쓰기 허용 디스크립터 2에 기록됐으며, 반환 바이트가 PCAP 및 보호된 복사본과 일치했다. 두 스냅샷 사이에 기존 게이트 값은 바뀌지 않았다. 장치·GIC 신호 상승과 대기 비트 읽기에 이어 종료 경로의 마스킹, ACK, 신호 하강 및 대기 해제를 확인했다. 리셋 후 GIC 활성·대기 비트와 virtio 인터럽트 상태는 모두 0이었다. 교체 세대는 새 트래픽 전에 이벤트 순번과 대기 해제 횟수가 모두 0이었고, 이후 자체 트래픽에서 부팅당 원본 폴링 20회와 GRO 전달 81회를 기록했다.

### Independent two-boot review / 두 부팅 독립 검증

A separate Python standard-library reader passed both captures. It matched the third ingress frame to descriptor 2 and its protected payload, confirmed the pending GIC and device bits, and found no CPU acknowledgement of INTID 79 in the interval before teardown. It traced the mask, device ACK, GIC clear and queue reset before the replacement queue, then matched all 80 new frames and the 60-byte ACK to the surviving consumer’s private memory. The directed kernel image SHA-256 was `b044f3a99e22a462462a1e7c0ab72404d39390290869c4ff48c817ff39ae0fc7`. Each capture logged exactly two host gate writes, EXIT and BIND. The gate records cannot exclude unlogged external memory writes.

별도 Python 표준 라이브러리 판독기로 두 캡처를 확인했다. 세 번째 입력 프레임을 디스크립터 2의 DMA 및 보호된 바이트 기록과 대조하고, 장치와 GIC의 대기 비트가 올라간 뒤 종료 정리까지 CPU가 INTID 79를 인수한 기록이 없음을 확인했다. 교체 큐 설정 전의 마스킹·장치 ACK·GIC 해제·큐 리셋을 추적했으며, 새 프레임 80개와 60바이트 ACK가 생존 소비자의 전용 메모리와 일치했다. 각 캡처에 기록된 호스트 게이트 쓰기는 EXIT와 BIND 두 건이다. 이 기록만으로는 로그에 남지 않은 외부 메모리 쓰기를 배제할 수 없다.

The five-second deadline and two-second hold are test-only observation windows at exit 86. Both archived captures reproduced after fresh extraction. Each rejected eleven corruptions and reproduced its independent report. The supplement records its ordinary parent’s SHA-256 and includes its replay inputs. Extract the supplement and run `python3 -B replay_pending.py` inside `pending/`.

5초 대기와 2초 유지는 상태 86 종료에 적용한 시험 조건이다. 새 폴더에서 두 캡처의 독립 보고서가 각각 재현됐고, 실행마다 변조 11건을 검출했다. 보충 묶음 SHA-256은 `ec59a2af31ed639168d6d32c8edb138d3da15e5321d9a9084b4c4a318f5ef79d`이다. [Raw archive / 원본](pending-retire.tar.xz) · [Checksums / 체크섬](pending-retire.tar.xz.sha256) · [Results / 결과](result.json) · [Trace / 추적](../../../trace/links.csv)
