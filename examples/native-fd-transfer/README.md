# Shared pixels over AF_UNIX SCM_RIGHTS

Three ordinary AArch64 processes run the same C ELF in separate address spaces. The producer sends a read-only file descriptor for a 320 × 240 XRGB8888 image. The display owner maps it and copies each source row into ramfb. A third process exercises numeric descriptor guesses and exit with an unread attachment. The fourth process brokers the AF_UNIX streams.

| Slot | Role | Argument | FIFO priority |
| --- | --- | --- | --- |
| 0 | Display consumer | none | 7 |
| 1 | Pixel producer | `p` | 5 |
| 2 | Unrelated client | `a` | 4 |
| 3 | Socket broker | `7` (client mask) | 1 |

## Build and package

Run these commands from the repository root in the Linux native build environment.

```sh
out=$(mktemp -d)
export CARGO_TARGET_DIR="$out/target"
sh sdk/native-aarch64/build.sh --local-socket examples/native-fd-transfer/main.c "$out/app.elf"
sh sdk/native-aarch64/build-rust.sh services/socket/Cargo.toml "$out/socket.elf"
python3 tools/build_native_el0.py --board virt --policy fifo --out "$out/package" \
    --elf "$out/app.elf" --priority 7 \
    --peer-elf "$out/app.elf" --peer-arg p --peer-priority 5 \
    --peer-elf "$out/app.elf" --peer2-arg a --peer-priority 4 \
    --peer-elf "$out/socket.elf" --peer3-arg 7 --peer-priority 1 \
    --display-service 0 --socket-service 3 --shared-fd \
    --ipc-route 0:3 --ipc-route 1:3 --ipc-route 2:3
```

For interactive display and serial input, start QEMU with the packaged kernel.

```sh
qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -m 128M \
    -device ramfb -serial stdio -monitor none -kernel "$out/package/kernel.elf"
```

## Capture gates

| Gate | Program state | Capture and advance |
| --- | --- | --- |
| Queued grant | Consumer stage 10, producer stage 0, unrelated stage 1. The producer's FDs 96 and 97 now identify the decoy. The receive stream holds `prFD1z`, with an escrow attached to `F` at offset two from the unread head. Consumer FDs 96 and 97 identify its guard object. | Stop and capture RAM, resume and send lowercase `r`. |
| Installed grant | Consumer stage 11, producer stage 0, unrelated stage 1. Consumer FD 98 identifies the original full-size object with read-only authority. The map exists and a writable request was denied. The unread bytes are `D1z`; the escrow has been consumed. | Stop and capture RAM, resume and send lowercase `i`. |
| Initial frame | Consumer and producer at stage 1. Producer holds its original writable mapping and a separate 4-byte decoy. Consumer holds the read-only alias after closing its received FD. The unrelated process has no shared mapping or FD. | Stop via QMP, capture RAM and the entire framebuffer, then continue and send lowercase `u` over PL011. |
| Updated frame | Producer rewrote every pixel through the original mapping. Both participants are at stage 2 with the original permissions. The unrelated process stays at stage 1. | Stop and capture RAM and framebuffer again, then continue and send lowercase `q`. |
| Terminal | Three application witnesses reach stage 3. All shared handles, aliases and queued rights have been released. Expected role exits in slot order are **70, 71, 72, 0**. | Preserve UART and final stopped RAM. An application failure records source line and errno and exits 100. |

Before the producer is allowed to send data, the consumer begins a one-byte rights receive on the empty data stream and issues a raw timed COMMIT. During this transient stage 9, QA can stop the guest to inspect the broker's pending receive. Its two-second guest monotonic deadline returns IPC status −110. The consumer checks that FD 98 is still absent, then releases the producer. One socket carries plain `pr`, tagged `FD1` and plain `z`. A separate control socket acknowledges closure and reuse before the receiver reads. In slot 0, two handles to an independent object occupy 96 and 97 during receipt. The new local FD is 98. An attempted writable map through 98 must return `EACCES`. The accepted read-only mapping remains live after its FD closes. Slots 0 and 1 can use the same numerical mapping address in their own translation tables.

Before proceeding at either live gate, wait for a complete stable frame and the corresponding witness stage. For independent reconstruction, let `s=0` before the update and `s=1` afterward. At pixel `(x,y)` the 8-bit channels are `R=(x+32+128*s)&255`, `G=(y+64+48*s)&255`, and `B=(((x>>4)^(y>>4)^s)&1) ? 0xd8 : 0x38`. Each fourth byte in the 32-bit source pixel is zero. Read the producer and consumer page tables through their respective roots. Check the physical address and bytes of every mapped page, the EL0 write permission on each alias, and the different physical page behind the producer's decoy. Compare all 76,800 visible pixels in each saved framebuffer.

After `q`, two tagged bytes each hold the sole live reference to a distinct object. With two additional objects held by the consumer, another create must fail with `ENOSPC`. The consumer truncates the first control message, reads the second through plain `recv`, and can then create both replacement objects. It checks every word of all four recycled full-size objects for zero. Finally, it sends a sole reference to slot 2 and verifies pool exhaustion. Slot 2 exits without reading that byte. The consumer observes EOF on a separate gate socket and can allocate the released capacity. A second unmap of the former image alias must return `EINVAL`.

## Private result layout

Each application exports `fd_transfer_witness[32]`, a private array of signed little-endian 64-bit words. Resolve its ELF symbol separately through each slot's page tables. Positive errno values denote observed failures. Unused words start at zero. The rolling hash starts at 2166136261 and advances once per row-major 32-bit pixel as `(hash ^ pixel) * 16777619` modulo 2³².

| Words | Observations |
| --- | --- |
| 0–3 | Role, stage, source FD 96, and role-local FD (consumer 98, producer replacement 96, unrelated client's own 96). |
| 4–7 | Main mapping VA, producer decoy mapping VA, stale or unrelated map errno 9, consumer writable-map errno 13 or unrelated size errno 9. Slot 2's recorded VA has already been unmapped at the live gates. |
| 8–13 | Initial and updated hashes, initial and updated first pixels, 307200-byte image size, 4-byte decoy or independent object size. |
| 14–18 | Consumer's prefix `0x7072`, suffix `0x44317a`, actual truncated message flags (`MSG_CTRUNC`), control length 0 and byte `R` (82) consumed by plain read. |
| 19–23 | Consumer's three pool exhaustion errors (28), four recycled objects checked for zero, repeated unmap errno 22. |
| 24–28 | Unrelated invalid descriptor send errno 9, consumer's gate EOF (1), consumer/producer explicit cleanup (1), original read-only descriptor (97), producer's read-only descriptor for the new decoy (97). |
| 29–31 | Raw timed COMMIT's IPC status (−110 in slot 0), failure source line and failure errno. A completed run leaves the last two words zero. |

## 한국어

슬롯 1은 320 × 240 픽셀 버퍼를 쓰기 가능한 상태로 매핑한 뒤 읽기 전용 FD 97을 만들어 `SCM_RIGHTS`로 전송한다. 전송 직후 원본 FD 96과 전송 FD 97을 닫고 같은 번호를 다른 4바이트 객체에 재사용한다. 슬롯 0은 독립된 객체의 FD 두 개를 먼저 확보해 새 FD 98을 받는다. 쓰기 가능한 매핑이 거부되는지 확인한 후 읽기 전용으로 매핑하고 FD를 닫는다. 슬롯 2는 다른 프로세스의 FD 번호를 알아도 원본을 매핑하거나 전달할 수 없는지 확인한다.

슬롯 0은 빈 스트림에 대한 수신 COMMIT이 제한 시간으로 취소되는지 먼저 확인한다. 단계 10에서는 큐에 전송 권한이 남아 있는지 QMP로 RAM을 저장한 뒤 `r`을 입력한다. 단계 11에서는 실제 설치된 읽기 전용 FD와 매핑을 저장하고 `i`를 입력한다. 첫 화면이 완성되고 슬롯 0·1의 단계가 1이 되면 다시 정지해 RAM과 전체 화면을 저장한다. 실행을 재개하고 시리얼로 `u`를 보내면 슬롯 1이 기존 매핑을 통해 두 번째 무늬를 쓴다. 단계 2에서 같은 물리 페이지, 서로 다른 쓰기 권한, 바뀐 전체 픽셀을 대조한다. 이후 `q`를 보내면 짧은 제어 버퍼, 일반 바이트 수신, 읽지 않은 객체를 보유한 수신자의 종료에 따른 자원 회수를 순서대로 확인한다. 최종 UART 종료 코드는 슬롯 순서대로 70, 71, 72, 0이다. 마지막 RAM 캡처에서 공유 매핑 해제와 각 프로세스의 단계 3 기록을 확인한다.
