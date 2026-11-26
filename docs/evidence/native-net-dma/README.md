# Device reset, DMA reclaim and standby handoff / 네이티브 NIC DMA 회수와 소유권 교체

**EVD-323 · AArch64 QEMU virt.** A native NIC owner exited with eight RX buffers posted. The kernel revoked its frame authority, reset the device and checked both queues before reclaiming the 32 KiB DMA pool. A separately packaged standby subsequently claimed the NIC once and returned a new challenge. An independent process kept executing between the two ownership epochs. [Design and acceptance](../../native-net-dma-retire.en-ko.md) · [Build, boot and replay](../../../examples/native-net-dma/README.md)

| Device observation / 장치 실측 | Original owner / 기존 | Retired / 회수 직후 | During retirement / 회수 중 | Standby / 예비 | Final retirement / 최종 회수 |
| --- | ---: | ---: | ---: | ---: | ---: |
| MMIO status | 15 | 0 | 0 | 15 | 0 |
| RX available / used index | 9 / 1 | 0 / 0 | 0 / 0 | 9 / 1 | 0 / 0 |
| All 32,768 pool bytes zero / 전체 바이트가 0 | No | Yes | Yes | No | Yes |
| Independent task count / 별도 태스크 누적 횟수 | 1 | 1 | 2 | 16 | 22 |

The table corresponds to the C SDK trial. The earlier quiet-link and ARP trials produced the same values. In the fourth run QEMU’s device trace captured the guest writing status zero, reading zero, selecting RX and reading QueueReady zero, then selecting TX and reading QueueReady zero. This QEMU version lacks per-queue QMP diagnostics. The bus trace supplies both independent queue readbacks; the production code and register-model test establish their order relative to the subsequent memory scrub. Two offline controls changed individual register observations and the verifier rejected each copy. [Per-run times, queue hashes, packets and task receipts](measurements.json) · [Ordered bus transactions](register-readback.json)

The isolated wire carried two distinct 60-byte challenge/reply exchanges. Three of the four runs also sent three ordinary ARP announcements between two retired-state observations. In the register-traced run the device-side packet capture recorded nine frames, including all three announcements. The unrelated peer’s RX, TX and claim calls returned `-13`. The standby recorded `-11` while the first owner was active, `0` on its new claim and `-114` on a duplicate. All three guest processes exited with zero.

기존 소유자와 예비 소유자가 실제 이더넷 프레임에 각각 응답했다. 두 소유자 사이에 정상 ARP 패킷이 들어온 시험에서도 RX·TX 큐와 버퍼 32KiB가 두 관측 시점 모두 0이었다. 동료 앱은 이 구간에도 실행됐고 송수신·재할당 권한 검사는 모두 거부됐다. 레지스터 원본에서 리셋 확인, RX 정지 확인, TX 정지 확인 순서를 대조했다. 이 가상 NIC는 폴링 방식이다. 물리 장치의 DMA 주소 격리, 캐시 일관성 및 최악 정지 시간은 실제 보드에서 검증해야 한다.

## SDK and controls / SDK와 대조 시험

- The C SDK drove the guest ownership fixture. The Rust Ethernet SDK drove the existing IPv4 service in separate FIFO and RR boots. Each exchanged TCP and UDP traffic with external peers and passed the process-private result checks. These configurations had no standby. [Socket results](socket-regression.json) · [Rust and C interfaces](../../../sdk/native-aarch64/raw-ethernet.md)
- Five host tests of the production reset procedure cover delayed/absent acknowledgement, either queue staying enabled, and restart admission after recovery or quarantine. Seven package tests passed, including standby configuration checks. The 1,024-read limit bounds polling operations. It is a hardware-dependent duration. / 레지스터 모형 5건과 패키지 시험 7건이 통과했다. 기한 내 장치 응답이 없으면 버퍼를 그대로 격리하며 이후 응답이 돌아와도 재사용을 거절한다.
- During bring-up the first C fixture used an unavailable SDK typedef. The first guest boot then exposed the observer’s reliance on stdout from a headless service. The final observer reads linked private receipts through each process’s actual page tables. The first FIFO capture needed the local `pyelftools` module for offline decoding; the original capture passed after the dependency was supplied. / 최초 시험 실패 기록과 수정 후 성공 기록을 함께 보관했다.

## Provenance / 원본과 추적

QEMU 6.2.0, AArch64 Cortex-A72, one vCPU, 128 MiB, modern virtio MMIO, two kernel-owned split queues. Each app has its own address space. Wire transport was restricted to the loopback test peer. CHG-323 → REQ-015 → DES-009 → IMP-323 → VER-015 → EVD-323. [Trace edges](../../../trace/links.csv) · [Native driver parent #5](https://github.com/Romanof123/JNOX/issues/5) · [Trial #420](https://github.com/Romanof123/JNOX/issues/420) · [PR #421](https://github.com/Romanof123/JNOX/pull/421)

| Retained local original / 로컬 원본 | Bytes | SHA-256 |
| --- | ---: | --- |
| `native-net-dma-originals.tar.gz` (three NIC captures, SDK TCP/UDP FIFO/RR captures and initial logs) | 1,967,841 | `6eb16720cd8308ce031bf449df228a4f72ab4f40614d5112b9e113b4a6ae16f7` |
| `native-net-dma-registers.tar.gz` (fourth NIC capture with device register trace) | 52,421 | `346b4bbee9c5cdb6f74f026443c410df2de3ee2c9fabd845703c9d219a6c565d` |

The extracted archives reproduced the four guest verdicts and the two socket memory decodes. The final C SDK trial’s JNOX kernel SHA-256 is `ec74817ac1b147c6705fc7a0480d2d80d528e87dd7cfc633847820ca36724a35`. The recorded SDK source commit is `2e64cd0` in the build workspace and has the same source tree as the public SDK commit. [SDK and guest source](https://github.com/Romanof123/JNOX/commit/c7fea8b7870580a55ede6bc564a283b66369c0ac) · [Register observer](https://github.com/Romanof123/JNOX/commit/ab33f73db0e498ae6ca8043369219502dc9f4e21) · [Single-board Linux domain follow-up #422](https://github.com/Romanof123/JNOX/issues/422)
