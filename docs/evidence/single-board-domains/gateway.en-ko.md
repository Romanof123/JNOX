# JNOX app to stock Linux NIC on one board / 단일 보드 앱 장치 호출

**EVD-324 · CHG-324 · #422.** In two cold boots, a JNOX Rust EL0 process issued three requests through its shared-page gateway. Linux used Alpine's original `virtio_net.ko` to send the first and third requests to the external TCP peer. Its independently generated eight-byte responses reached the JNOX caller. For the middle request, the external peer stayed silent and the caller applied its software fallback. [Contract](../../single-board-gateway.en-ko.md) · [Raw-derived measurements](gateway-result.json)

JNOX 앱이 요청을 시작했다. 호스트는 Linux NIC에 연결된 외부 장치 역할만 맡았다. 두 게스트 사이의 메시지는 Bao가 지정한 4 KiB 페이지로만 전달됐다. 두 부팅 모두 패킷 원본의 왕복 데이터와 Linux UART, JNOX 앱의 독립 주소 공간에서 읽은 결과가 일치했다.

| Quantity / 측정 항목 | First / 1차 | Repeat / 2차 |
| --- | ---: | ---: |
| Caller deadline / 앱 요청 기한 | 250 ms | 250 ms |
| Request to software fallback / 요청부터 대체 출력까지 | 250.145792 ms | 250.158624 ms |
| Time after nominal deadline / 기한 초과 | 145.792 µs | 158.624 µs |
| Other app wakes strictly within the 250 ms interval / 기한 내 별도 태스크 기상 | 13 | 12 |
| Unrelated app send and receive / 비인가 앱 송신·수신 | −EACCES / −EACCES | −EACCES / −EACCES |
| Granted app null source and destination / 승인 앱의 잘못된 포인터 | −EFAULT / −EFAULT | −EFAULT / −EFAULT |
| Successful wire operations / 실제 장치 성공 | 2 | 2 |

The native SDK preserves the caller deadline and generation checks. Linux validated the DT mapping, closed its raw-memory FD and ran its service as UID/GID 65534 with empty capabilities and `no_new_privs`. A second open of the raw-memory device was denied. The EVD-324 one-board HTTP profile and the EVD-322 two-machine UDP gateway both passed after rebuilding from the same source. The latter repeated a Linux-only stop, restart and rejection of one old-generation reply. [HTTP baseline](README.md) · [Two-machine fault contract](../linux-driver-gateway/README.md)

초기 시험에서 Linux는 올바른 외부 응답을 보냈지만 JNOX SDK가 이전 UDP 진단의 XOR 응답 규칙을 적용해 10초에 만료됐다. Bao 경로는 기기에서 실제로 반환한 8바이트를 상위 호출자에게 전달하도록 수정했다. 기존 UDP 진단 경로의 응답 검사는 유지하고 전체 장애복구 시나리오를 회귀 시험했다.

**Scope.** Linux stayed up during this device-response timeout. Linux-only kernel restart, stale-reply rejection on the single board, physical DMA containment, and product worst-case timing have separate acceptance criteria. 이번 측정의 지연은 QEMU 실행 2회의 관측값이다. 동일 보드의 Linux 커널 재부팅 검증과 실물 기능안전 승인을 계속 추적한다. [Integration issue](https://github.com/Romanof123/JNOX/issues/422)

**Originals.** The local `single-board-gateway-originals.tar.xz` is 1,015,624 bytes, SHA-256 `14f54f418ffeedfdf33375bff6666f89012b92d54a97ef2fef307c59249e8f0e`. It includes two positive raw captures, the first admission failure, both regression captures, private-memory reads, ELF packages, kernel/module identities and the exact verifier sources. From an empty extraction with Python 3 and pyelftools, `sha256sum --quiet -c SHA256SUMS` and `sh replay.sh` reproduced both verdicts and both regressions. Eight altered copies per positive capture failed the expected checks. 원본 해시와 재생 결과를 확인했으며 두 번의 시험마다 패킷·포트·권한·메모리·시간 변경 8건을 검출했다.

## Reproduce the live run / 실시간 시험 재현

With the build dependencies of the [board guide](../../../examples/single-board/bao/README.md), set `intake` to the pinned Alpine APK directory and `bao_source` to the pinned Bao checkout. Run from the repository root.

```sh
set -eu
work=$(mktemp -d)
export CARGO_BUILD_JOBS=1
python3 -B examples/single-board/linux/prepare.py --gateway --intake "$intake" --out "$work/linux"
for role in app peer; do
    sh sdk/native-aarch64/build-rust.sh --bao-gateway "examples/single-board/gateway-$role/Cargo.toml" "$work/$role.elf"
done
python3 -B tools/build_native_el0.py --board virt --bao-partition --bao-gateway --policy fifo \
    --priority 8 --peer-priority 8 --elf "$work/app.elf" --peer-elf "$work/peer.elf" --out "$work/native"
bash examples/single-board/bao/build.sh --gateway "$bao_source" "$work/linux/linux.bin" "$work/native/kernel.elf" "$work/bao"
python3 -B tools/verify_board_driver_gateway.py --bao "$work/bao/bao.bin" --native "$work/native" \
    --linux "$work/linux" --out "$work/capture"
python3 -B tools/test_board_gateway_receipts.py --out "$work/capture" --native "$work/native" --linux "$work/linux"
```
