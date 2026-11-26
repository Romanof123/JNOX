# Native Ethernet ownership fixture

Build from the repository root using the freestanding C SDK.

~~~sh
app=examples/native-net-dma
sh sdk/native-aarch64/build.sh "$app/primary.c" dma-primary.elf
sh sdk/native-aarch64/build.sh "$app/peer.c" dma-peer.elf
sh sdk/native-aarch64/build.sh "$app/spare.c" dma-spare.elf
~~~

Package the ELFs in that order. Assign initial network ownership to slot 0 and standby authority to slot 2. Configure synchronous IPC route 0:2 and notification route 2:1. Each sleeping thread uses the absolute monotonic clock. Slot 2 first checks that claiming the active primary's NIC returns -11, then waits for a retirement message. Slot 0 sends that message after its first exchange and returns after receiving the reply. Slot 2 sleeps for 15 guest seconds after replying, then claims its epoch.

Both owners exchange 60-byte Ethernet frames. The host uses source MAC 02:00:00:00:00:01 and broadcast destination. Bytes 12–13 are 88 b5. Bytes 14–17 contain ASCII JNDM. Byte 18 is epoch 1 or 2. Byte 19 is opcode 1 for challenge, 2 for reply or 3 for retirement. The host supplies a fresh 16-byte nonce at offsets 20–35. The reply preserves all payload bytes except the opcode and uses guest source MAC 52:54:00:12:34:56 and the host destination MAC. Send retirement only after verifying the matching echo.

Each ELF exports a private unsigned 64-bit dma_phase initialized to zero. For the primary, 1 means ready for challenges and 2 means retirement requested. For the spare, 1 marks the start of its wait after replying to IPC, 2 means it claimed the NIC and observed -114 on a duplicate claim, and 3 means retirement requested. Inspect device state for completed retirement. For the peer, 1 means its heartbeat started and 2 means its five post-notification ticks finished. Each guest exits with status zero on success.

Read the private ELF symbols dma_echo_count and dma_ignored separately for each owner. The spare records dma_spare_preclaim, dma_spare_claim and dma_spare_duplicate. The peer records dma_peer_rx, dma_peer_tx and dma_peer_claim as signed 64-bit results, dma_peer_ticks as a count, dma_peer_ns as the latest monotonic timestamp in nanoseconds and dma_peer_finished as the tick at completion notification.

## Build, boot and replay / 빌드·부팅·원본 재생

From a Linux checkout with the repository's AArch64 build tools, QEMU and GNU AArch64 binutils, build the three ELFs above. The package fixes process identities, broker permissions, the retirement handshake and the completion event.

```sh
python3 -B tools/build_native_el0.py --board virt --policy fifo \
    --elf dma-primary.elf --priority 4 --peer-elf dma-peer.elf --peer-priority 5 \
    --peer-elf dma-spare.elf --peer-priority 6 --ipc-route 1:0 --ipc-route 0:2 \
    --notify-route 2:1 --socket-service 0 --net-service 0 --net-standby 2 --out dma-package
python3 -B tools/native_nic_lifecycle.py dma-capture --kernel dma-package/kernel.elf \
    --lan-announcements --mmio-trace
python3 -B tools/native_nic_lifecycle.py dma-capture --lan-announcements --mmio-trace
rustc --test kernel/src/native_virtio_reset_test.rs -o /tmp/jnox-dma-reset-tests
/tmp/jnox-dma-reset-tests
python3 -B tools/test_native_service_slots.py
```

프레임 API는 Rust와 C 양쪽에서 사용할 수 있다. 시험은 실제 패킷, 게스트별 비공개 변수와 QEMU 장치의 레지스터 읽기·쓰기 기록을 대조한다. `--lan-announcements`는 소유자가 없는 동안 시험 링크에 ARP 세 건을 보낸다. `--mmio-trace`는 첫 장치 리셋 구간의 레지스터 기록을 요구한다. [SDK API](../../sdk/native-aarch64/raw-ethernet.md) · [결과 및 검증 범위](../../docs/evidence/native-net-dma/README.md)
