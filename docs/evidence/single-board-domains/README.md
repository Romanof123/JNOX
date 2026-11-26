# One board, two guest kernels and the stock Linux NIC / 단일 보드 동시 구동

**EVD-324 · #422.** Two cold boots of one AArch64 QEMU machine each completed four fresh HTTP echo requests through Alpine's original `virtio_net.ko`. Between exchanges, the Rust JNOX EL0 task woke on its own timer, updated its private memory and went back to sleep. One QEMU process, two vCPUs and one physical device model participated in each boot. [Board contract](../../single-board-driver-domains.en-ko.md) · [Measured operands and IRQ counts](result.json)

한 QEMU 보드에서 JNOX와 Linux를 함께 부팅했다. 각 부팅에서 외부 시험기가 네트워크를 통해 보낸 난수 4개와 Linux의 응답을 원본 패킷에서 대조했다. 요청 사이에는 JNOX 앱의 페이지 테이블을 따라 태스크 카운터와 시각을 읽고, 커널의 타이머 인터럽트 카운터도 별도로 확인했다.

| Observation / 관측 | First cold boot / 첫 부팅 | Repeat / 재부팅 |
| --- | --- | --- |
| Linux GIC INTID 79 at ready, DTB response, then 4 echoes / 시점별 누적 IRQ | 2 → 8 → 22 → 34 → 46 → 57 | 2 → 6 → 20 → 32 → 44 → 55 |
| Native EL0 wakes / JNOX 앱 타이머 기상 | 16 → 18 → 20 | 19 → 20 → 22 |
| Native PPI 27 interrupts / JNOX 타이머 IRQ | 16 → 18 → 20 | 19 → 20 → 22 |
| HTTP captures / 패킷 원본 대조 | DTB + 4 echo pairs | DTB + 4 echo pairs |

The Linux Image, configuration and four module hashes matched the pinned driver intake. The guest-FDT read over TCP matched its packaged bytes. Device ownership is static in this profile. Linux receives the NIC register page and INTID 79, JNOX owns its private heartbeat memory, and Bao supplies both guests' stage-2 translations. Both successful boots used the same Bao and guest binaries. [Intake](../linux-virtio-net/INTAKE.md) · [Board map and boot command](../../../examples/single-board/bao/README.md)

**Failure and correction / 실패 재현과 수정.** The first bring-up exposed a reserved MPIDR bit in the secondary CPU's PSCI startup argument. After masking to affinity fields, both guests booted. With the original SPI group assignment the NIC then asserted its interrupt, all five Linux INTID 79 snapshots stayed at zero, and the DTB request timed out. On this firmware-free GICv2 board Bao enabled Group 0 while assigning shared interrupts to Group 1. Assigning SPIs to the enabled group delivered the NIC interrupt with the same Linux kernel, driver, DTB, CPU target and IRQ trigger. The first successful trace recorded 65 writes to the virtio ISR acknowledge register. The second boot ran without device tracing.

처음에는 보조 CPU 기동 인자의 예약 비트를 제거해야 했다. 이후에도 NIC가 인터럽트 신호를 올렸지만 Linux가 받지 못했다. 이 가상 보드에 한해 Bao의 물리 GIC 그룹 배정을 고친 뒤 Linux가 장치 인터럽트를 받고 실제 패킷에 응답했다. 변경은 이 보드의 모니터 빌드에만 적용했다. [Board-specific build inputs](../../../examples/single-board/bao/build.sh)

**Scope / 검증 범위.** The host originated the HTTP requests. The native task demonstrates independent progress. A bounded request originating in a JNOX app, Linux-only reset, expiry fallback, and stale-generation rejection were previously observed on two separate machines. Repeating that sequence on this board is the next acceptance item. The current profile supplies identity-addressed DMA in Linux RAM. A physical IOMMU, DMA revocation, worst-case fault response and product safety approval require target evidence. [Earlier gateway](../linux-driver-gateway/README.md) · [Open same-board work](https://github.com/Romanof123/JNOX/issues/422)

이번에는 외부 시험기가 요청을 시작했다. 같은 보드에서 JNOX 앱이 게이트웨이를 호출하고 Linux만 재부팅했을 때의 만료 처리·이전 응답 폐기·복구는 후속 시험이다. 실물 DMA 격리와 제품별 기능안전 승인은 별도로 추적한다.

## Replay / 재생

The retained local archive `single-board-driver-domains.tar.xz` is 946,428 bytes, SHA-256 `372b2cad274df1f1e81a229eac7cd8674662f8acba66ad33680ea8fc97bac623`. It includes both positive captures, the IRQ-group failure control, the assigned-IRQ trace, native binaries, the live Linux DTB, and the decoder sources. Extract into an empty directory with Python 3 and pyelftools available. The following checks ran in a fresh extraction.

```sh
sha256sum --quiet -c SHA256SUMS
sh replay.sh
```

원본 묶음에는 UART·PCAP·태스크 메모리 관측과 재생 도구가 들어 있다. 위 두 명령으로 원본 해시와 각 부팅의 판정을 다시 확인할 수 있다. [Trace edges](../../../trace/links.csv) · [Implementation and review #423](https://github.com/Romanof123/JNOX/pull/423)

The subsequent same-board trial covers authorized JNOX-originated device calls and a 250 ms device-response timeout. 이후 같은 보드에서 승인된 JNOX 앱의 실제 장치 요청과 응답 기한 만료를 검증했다. [Gateway results / 게이트웨이 실측](gateway.en-ko.md)
