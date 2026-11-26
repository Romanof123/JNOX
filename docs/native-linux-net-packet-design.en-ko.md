# Native Linux virtio-net packet path / Linux 원본 드라이버의 네이티브 패킷 경로

[#426](https://github.com/Romanof123/JNOX/issues/426) · [Current validation profile / 현재 검증 프로파일](native-linux-source-net.en-ko.md) · [Linux stable source / Linux 원본](https://github.com/gregkh/linux/blob/4700e3adf3b24b39dd4929e9c6d62811ba3ccea8/drivers/net/virtio_net.c)

## Evidence and scope / 근거와 범위

The retained `426-full-02/imported` build compiled the complete Linux v6.18.65 translation unit. Its original object SHA-256 is `5767a14f1d391e2419240bde3a0dab3c39457b894a35f2de581edce177c4862b`; the validation service ELF is `3c3ab215f2d7d1a9782ba0fbe8bc36754cea1a80e6c23b50a2501939570b2dd1`. Only validation is linked into that service from the original lifecycle entries. The following counts come from that object's `import.json` and apply before any native header adaptation. An indirect site is a register branch, including possible tail calls or jump-table dispatch. Registration addresses and relocation closures establish possible dependencies; instruction execution requires a runtime witness.

아래 값은 보존된 원본 오브젝트의 정적 측정값이다. 직접 함수 수에는 진입 함수도 포함한다. 직접 함수들의 외부 심볼 수와 데이터 참조까지 따라간 전체 외부 심볼 수를 구분했다. 이미 링크된 검증 서비스의 성공은 패킷 처리의 실행 증거가 아니다. 런타임에는 슬롯에 등록된 실제 콜백과 완료된 패킷을 연결해 기록한다.

| Original entry / 원본 진입점 | Direct functions / 직접 함수 | Direct closure imports / 외부 심볼 | Indirect sites / 간접 분기 | Imports incl. data / 데이터 포함 |
|---|---:|---:|---:|---:|
| `virtnet_validate` | 1 | 4 | 1 | 4 |
| `virtnet_probe` | 14 | 74 | 18 | 198 |
| `virtnet_open` | 7 | 33 | 0 | 33 |
| `start_xmit` | 6 | 29 | 1 | 29 |
| `virtnet_poll` | 26 | 78 | 1 | 78 |
| `virtnet_remove` | 9 | 28 | 3 | 28 |

In this object `R_AARCH64_ABS64` registers validate, probe and remove at offsets 184, 192 and 208 in `virtio_net_driver`. Open and TX appear at offsets 16 and 32 in `virtnet_netdev`. Poll has no equivalent static table entry in the receipt. Trace its callback argument at NAPI registration and its invocation by the service event loop. Keep these as build-relative offsets; use typed fields for registration.

원본 등록 테이블에는 위 오프셋의 함수 포인터가 있다. NAPI poll은 등록 호출에서 전달되므로 같은 형태의 정적 테이블 항목이 없다. 특정 빌드의 오프셋을 네이티브 ABI로 고정하지 말고 타입에 맞는 콜백 등록을 사용한다.

## ARM64 instruction boundary / ARM64 명령 경계

Offsets below are hexadecimal bytes from the named function in `virtio_net.original.o`. They were read using `aarch64-linux-gnu-objdump -dr` and checked against the saved input relocations. Source associations use the pinned C and headers; the table does not claim instruction-level DWARF line attribution.

| Function + offset | Observed instruction and source association / 관측 명령과 소스 대응 |
|---|---|
| `start_xmit+0x50` | `mrs x1,tpidr_el1`; relocations at +0x28/+0x30 reference `softnet_data+0x62`, and +0x58 loads a byte using that per-CPU offset. Corresponds to `netdev_xmit_more()` at the start of TX. This precedes the first packet submission. |
| `start_xmit+0xa8c` | `mrs x1,tpidr_el2`; the pinned `arch/arm64/include/asm/percpu.h` supplies an EL1/EL2 alternative. Presence in the input section does not establish that this instruction ran. |
| `virtnet_poll+0x9c`, `+0x360` | `mrs x0,sp_el0` and `mrs x1,sp_el0`. Each is followed by a load at offset 16. The first is on the TX-cleanup lock path; the second is on the XDP-TX queue selection path near references to `nr_cpu_ids`. The ARM64 `get_current()` header reads SP_EL0 and `raw_smp_processor_id()` consumes `current_thread_info()->cpu`. |
| `virtnet_set_queues+0x19c` | `mrs x1,sp_el0`, followed by load/add/store at +8. This helper in probe's direct closure has the `IFF_UP` / `local_bh_disable()` path; it returns early without a control virtqueue or MQ. No matching system register read was observed inside `virtnet_probe` itself. |
| Optional paths | `xsk_wakeup+0x64`, `virtnet_xdp_xmit+0x4c`, `virtnet_xdp_handler+0x180/+0x3c4` read SP_EL0. The last function also reads TPIDR_EL1 at +0x100 and contains the TPIDR_EL2 alternative at +0x424. |

The stock ELF cannot simply promote TX and run it at EL0. Implement a narrow, versioned build-header port covering the semantics of per-CPU access, current CPU/task, local bottom-half nesting and queue locks. Keep the original C byte-identical. For the single-threaded service, store batching state in a service-owned record, use the pinned CPU identity of its actual execution context, and serialize NAPI work and TX callbacks through one event loop. Code needing real interrupt exclusion must enter an authorized kernel operation. Treat kernel alternatives, static keys, atomic ISA requirements and inline traps as additional code-generation audit items. Inventory and disassemble the complete reachable code after linking, including the registered indirect targets, for the actual Cortex-A72 instruction set. Reject an unresolved register read or unsupported alternative. Record input headers, adaptation diff, source hash, both object hashes and final link map. The adapted callback may have different instruction bytes from the stock build; compare unchanged source and same-build promoted bytes separately.

EL0 서비스는 커널의 TPIDR_EL1/EL2나 SP_EL0 접근에 의존할 수 없다. 전송 경로에는 패킷 제출 전부터 실제 특권 명령이 있다. 원본 C 파일은 그대로 두고, 버전을 고정한 헤더에서 실제 서비스 전용 상태와 CPU 식별, BH 중첩 및 큐 직렬화 의미를 제공한다. 커널의 IRQ 마스킹이 필요한 작업은 권한이 제한된 커널 호출로 구현한다. 권한 상승이나 미정의 명령의 임의 에뮬레이션을 허용하지 않는다. 새 오브젝트에서 직접 경로와 등록된 간접 대상까지 역어셈블하고, Cortex-A72에서 실행 가능한 명령과 대체 명령 처리도 확인한다.

## Implementation order / 구현 순서

1. **Bound the transport contract.** Extend the existing PID-0, single-CPU control path. Calls 156–160 currently claim/release, read two feature banks, read MAC/MTU configuration, commit features and read status. Claim status is 3 and successful commit yields 11. Keep VERSION_1, MAC and optionally MTU. Select one immutable split-ring pair, RX queue 0 and TX queue 1. Reject MQ, control VQ, packed rings, mergeable RX, XDP/XSK, offloads and queue changes in this profile. Validate both queue maxima before allocation. The polling native transport uses eight descriptors; upstream `tx_may_stop()` stops below `MAX_SKB_FRAGS + 2`. Choose a bounded power-of-two queue depth with room above that compiled threshold after a normal submission (candidate 64, subject to both maxima). Preserve the original stop/wake policy. Keep one physical owner of the transport. [Control ABI](../kernel/src/native_linux_net.rs) · [Native queue implementation](../kernel/src/native_virtio_net.rs)

   **장치 계약을 먼저 고정한다.** 기존 호출 156–160과 슬롯 0 권한을 유지하고 RX 0, TX 1의 분할 링 한 쌍만 제공한다. 두 큐의 장치 한도와 실제 빌드의 `MAX_SKB_FRAGS + 2`를 검사해 깊이를 결정한다. 기존 8개 슬롯을 그대로 사용하면 원본 드라이버의 정지·재개 조건을 만족시키지 못할 수 있다. 기능 비트, 큐 번호와 큐 구성이 장치 점유 중 바뀌는 요청은 거부한다.

2. **Run the original lifecycle.** Promote a board-profile registration accessor or the measured callbacks and dispatch the original `virtnet_probe`, `virtnet_open`, TX, poll, close and remove through their actual registration contracts. Implement allocation, queue discovery, registration/unregistration, callback identity, unwind and bounded work cancellation. The Linux probe calls `register_netdevice()` before `virtio_device_ready()`; accept DRIVER_OK (status 15) only after queue memory and notifications are installed and retain the original open/refill sequence. Start receiving only after native RX buffers are posted. Fail setup on an unsupported operation; log which path requested it. Distinguish returned kernel API errors from unimplemented lifecycle behavior. [Original lifecycle source](https://github.com/gregkh/linux/blob/4700e3adf3b24b39dd4929e9c6d62811ba3ccea8/drivers/net/virtio_net.c#L6839)

   **원본 콜백으로 수명주기를 구동한다.** probe의 큐 발견과 등록, 장치 준비, open의 RX 보충을 순서대로 확인한다. NAPI에 등록된 함수와 `ndo_start_xmit`에 연결된 함수가 원본 오브젝트에서 온 것인지 남긴다. 등록 실패 시 부분 할당을 역순으로 회수하고, 미구현 경로를 성공 처리하지 않는다.

3. **Translate skb and scatterlist at the ownership boundary.** Start with linear Ethernet frames of 14–1514 bytes, a single supported MTU, and no fragments, GSO or checksum delegation. Validate `head <= data <= tail <= end`, `len`, `data_len`, headroom, queue mapping zero, and sufficient trailing space for aligned `skb_shared_info`. Support the original `xmit_skb()` header-in-headroom and separate-header cases. Check `skb_to_sgvec`, SG entry count, termination, offsets and cumulative length with overflow-safe arithmetic. Reject SG chaining and out-of-pool pages. On RX, `add_recvbuf_small()` advertises `vi->hdr_len + GOOD_PACKET_LEN`; the latter includes VLAN headroom (1518). Account separately for the Linux allocation, the posted descriptor span, and the permitted delivered frame (1514 maximum). Check used length against the posted span before parsing or trimming. Track the original skb cookie through used-ring completion and free it once. [TX scatterlist creation](https://github.com/gregkh/linux/blob/4700e3adf3b24b39dd4929e9c6d62811ba3ccea8/drivers/net/virtio_net.c#L3332)

   **버퍼 경계를 검증한다.** 첫 구현은 조각이 없는 이더넷 프레임만 받는다. 헤더를 skb 앞에 붙이는 경우와 별도 SG 항목에 두는 경우를 모두 처리한다. 페이지·오프셋·길이의 합, 목록 종료, 버퍼 소유권을 확인한다. RX용 리눅스 할당 크기, 장치에 알려 준 길이, 최종 사용자 프레임 한도는 별도 값으로 검증한다. 완료 시 skb를 중복 해제하지 않는다.

4. **Make the kernel the DMA and IRQ authority.** Use a fixed EL1-owned pool and opaque handles bound to process instance, device epoch, queue, direction and slot. The kernel translates validated user slices into bounded descriptors; descriptor memory, bus addresses, virtio MMIO and the GIC remain kernel-owned. A bounded copy into a fixed device buffer is valid for the first implementation. For TX accept only device-readable data; for RX only authorized device-writable space. Validate free count, immutable head membership, used-ID range, index deltas, generation and duplicate completion. Order descriptor publication, avail index and notify with device-appropriate barriers. Implement `virtqueue_get_buf[_ctx]`, notify suppression/re-enable races and bounded NAPI budgets. Bind physical INTID 79 to the current device epoch. Snapshot and acknowledge the transport interrupt in EL1 and deliver a coalescible queue event; separate the transport ACK from GIC EOI and from releasing a DMA handle. Drain and recheck ring state before rearming. A peer must receive denial for forged, replayed, or foreign handles and unauthorized IRQ consumption.

   **DMA와 IRQ 소유권을 커널에서 강제한다.** 사용자 서비스에는 프로세스 인스턴스·장치 세대·큐·방향·슬롯에 묶인 핸들만 전달한다. EL1에서 길이와 소유자를 검증해 고정 풀의 디스크립터를 작성한다. 완료 인덱스, 완료 ID, 세대 및 이중 완료를 검증하고, RX/TX 게시와 notify 사이의 메모리 순서를 보장한다. IRQ 79의 장치 ACK, GIC EOI, DMA 소유권 반환을 구분한다. NAPI 작업 뒤 큐를 다시 확인해 재무장 중 도착한 완료를 놓치지 않는다.

5. **Retire before restarting.** Stop submissions, mask notifications, cancel work, drain or invalidate outstanding cookies, and gate remove/exit on kernel-observed reset. Reuse the existing bounded reset check for status zero and both queue-ready bits clear. Quarantine the old buffers if reset times out. Only then recycle memory and grant a fresh epoch to the replacement service. Test old handle replay, late completion, an interrupted TX submission, peer progress and a complete second packet exchange. The present control service's release path lacks DMA reclamation responsibility; bind the stronger gate to each new DMA allocation. [Native reset protocol](../kernel/src/native_virtio_reset.rs)

   **재시작 전에 이전 소유권을 종료한다.** 신규 제출과 통지를 멈추고 원본 close/remove를 호출한다. 비정상 종료는 커널이 회수한다. 상태 0과 양쪽 QUEUE_READY 해제를 확인한 후 새 세대를 발급한다. 시간 초과 시 버퍼를 격리한다. 이전 핸들 재사용, 늦은 완료, 중간 종료, 독립 피어의 실행 지속과 교체 후 두 번째 송수신을 검증한다.

6. **Prove the wire path independently.** Capture the QEMU network backend's external PCAP and record the packet generator separately. Send per-run nonce/sequence payloads in both directions, correlate MACs, EtherType, exact lengths and payload hashes against PCAP records, original callback return values, and generation-qualified descriptors. Require enough frames to wrap each ring and exercise stop/wake, RX budget exhaustion, and rearm. Compare ingress/egress bytes before and after replacement. Retain source, object, ELF, map, guest trace and PCAP hashes together. A service-internal packet log alone cannot certify delivery.

   **외부 캡처로 패킷을 입증한다.** 독립 피어의 실행 기록과 QEMU 백엔드 PCAP를 보존하고 양방향 nonce·순번·MAC·EtherType·길이·해시를 게스트 콜백 및 디스크립터 세대와 대조한다. 두 링의 순환, 송신 정지·재개, NAPI 예산 소진과 재무장을 실제로 유발한다. 서비스 교체 전후의 양방향 프레임을 같은 기준으로 검증한다.
