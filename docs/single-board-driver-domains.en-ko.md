# Two driver domains on one AArch64 board / 단일 AArch64 보드의 드라이버 도메인

**CHG-324 · REQ-004/011 · DES-002 · VER-004/011.** Exercise both guest kernels on one QEMU virt machine with the Bao v2.0.0 EL2 static partitioner. Fix two emulated Cortex-A72 CPUs, 512 MiB RAM and GICv2 for this diagnostic deployment. JNOX remains a Rust EL1 microkernel with an EL0 application. Linux runs the original pinned Alpine Image, module and dependencies from #416. [One-board acceptance #422](https://github.com/Romanof123/JNOX/issues/422) · [Previous gateway and cold restart](evidence/linux-driver-gateway/README.md) · [Bao source revision](https://github.com/bao-project/bao-hypervisor/tree/0af4a1ab558ad60c9658af4f43756c4536dd3141)

| Allocation / 할당 | JNOX | Linux |
| --- | --- | --- |
| Guest CPU / 보드 CPU | One vCPU on CPU 1 | One vCPU on CPU 0 |
| Guest RAM → machine RAM | 128 MiB at 0x40000000 → 0x48000000 | 192 MiB at 0x50000000 → 0x50000000 |
| Entry | EL1h, kernel at guest 0x40200000 | EL1 shim at 0x50000000 supplies guest DTB in x0 to stock Image |
| Time and interrupt controller | Virtual timer PPI 27, virtual GICv2 | Virtual timer PPI 27, virtual GICv2 |
| I/O | Bounded kernel RAM journal and private EL0 heartbeat | PL011 after boot (INTID 33), modern virtio-net at 0x0a003e00 (INTID 79) |

Linux RAM is placed at matching guest and machine addresses so the original NIC driver can submit device-visible descriptors. This board profile has no IOMMU protecting other domains from a misbehaving DMA master. The Linux NIC occupies a 4 KiB stage-2 page with seven vacant transport slots. The monitor uses the PL011 during startup. Independent checks must identify the Linux owner, actual IRQ and traffic for device claims.

**First acceptance.** Keep the same QEMU PID while observing both guest identities. Read a JNOX timer interrupt counter and the EL0 task's private count/timestamp through its linked translation tables. Interleave samples with fresh Linux NIC HTTP exchanges. Verify original module hashes, live guest DTB, external packet payloads and elapsed times. This establishes simultaneous boot and ordinary device service.

**Remaining acceptance.** A bounded inter-domain transport must return the real NIC result to a JNOX caller. A Linux-only stop and cold restart must show JNOX fallback, continuing unrelated tasks, prior generation rejection and a fresh exchange on this topology. Target-board DMA confinement and product fault-response timing need separate evidence.

검증 배치에서는 한 보드의 두 가상 CPU에 JNOX와 Linux를 하나씩 배치한다. JNOX의 EL0 태스크와 타이머가 진행하는 동안 Linux 원본 NIC로 실제 HTTP 왕복을 반복한다. 자원 표, 두 게스트 식별값, 패킷, 실행 시각과 장치 정보를 함께 보관한다. 다음 단계는 JNOX 앱에서 보내는 장치 메시지와 Linux 단독 재부팅이다. 실물 장치의 DMA 제한과 제품 시간 보장은 실측해야 한다.
