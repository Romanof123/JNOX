# EVD-326 · Linux virtio-net feature validation on native J-NOX / J-NOX에서 실행한 Linux virtio-net 기능 검증

**REQ-004/011 · DES-002 · IMP-326 · VER-004/011.** In two cold boots of AArch64 QEMU virt, the original Linux `virtnet_validate` callback executed three times at EL0 in the same native J-NOX service ELF. Changing the NIC's MAC and MTU changed both the modeled register reads and the values collected from the driver's private memory. This establishes upstream feature validation and enforcement of the package's control-register grant. [Recorded results](result.json) · [CHG-326](../../native-linux-source-net.en-ko.md)

두 번의 콜드 부팅에서 J-NOX 커널이 AArch64 QEMU virt에서 직접 실행됐다. 동일한 네이티브 서비스 ELF에 링크한 원본 Linux `virtnet_validate` 콜백이 매회 EL0에서 세 번 실행됐다. NIC의 MAC과 MTU 설정을 바꾸자 모델의 레지스터 읽기와 드라이버의 전용 메모리에서 각각 바뀐 값이 관측됐다. [실측 결과](result.json)

| Observation / 관측 | Capture C | Capture D |
| --- | --- | --- |
| MAC | `52:54:00:19:26:41` | `52:54:00:19:26:42` |
| Configured and observed MTU / 설정·관측 MTU | 1410 | 1386 |
| Entries at `0x4009b0` / 콜백 진입 횟수 | 3 | 3 |
| Dispatches stopped before entry / 실행 전 중단 횟수 | 0 | 1 |
| Callback returns / 콜백 반환값 | `0, -22, -22` | `0, -22, -22` |
| Device MMIO accesses / 장치 MMIO 접근 횟수 | 79 | 79 |
| Rejected capture mutations / 거부한 변조 기록 수 | 6 | 7 |

### Callback and device observations / 콜백·장치 관측

QEMU traced the linked call target at `0x4009b0`. An independent reader matched the entry instructions to the service ELF and counted three executed entries per boot. In D, QEMU also logged a dispatch stopped before its translation block ran. The matching stop record accounts for that fourth dispatch. In both boots the original callback accepted the offered configuration, rejected a missing configuration-read operation, and rejected `CTRL_RX` without a control virtqueue. The accepted feature banks were `0x28` and `0x1` (MAC, MTU and VERSION_1). QEMU observed device status advance from 3 to 11, setting `FEATURES_OK`. [Guest](../../../examples/linux-source-net/driver.c) · [Replay checks](../../../tools/verify_linux_source_net.py)

독립 판독에서는 실행 ELF의 함수 진입 명령과 QEMU의 실행 주소를 대조했다. D의 기록에는 번역 블록 실행 직전에 중단된 디스패치가 한 건 더 있었고, 이에 대응하는 중단 기록을 확인해 실제 콜백 진입 세 번을 식별했다. 정상 구성은 승인됐다. 설정 읽기 함수가 없는 경우와 제어 virtqueue 없이 `CTRL_RX`를 요청한 경우는 각각 `-EINVAL`이었다. 두 부팅 모두 장치 상태가 3에서 11로 바뀌면서 `FEATURES_OK`가 설정됐다.

Slot 0 held the control-register grant for the 512-byte transport at `0x0a003e00`. The verifier matched slot 1's saved syscall return PCs and RAM results to its own ELF and placed all five denials between slot 0's claim and release. Claim, feature read, configuration read, feature commit and status each returned `-EACCES`. Slot 1 recorded six monotonic timestamps with five absolute sleeps. Slot 0 released and reclaimed the device in the same live process.

슬롯 0이 장치를 점유한 동안 슬롯 1의 장치 호출 다섯 가지가 모두 `-EACCES`로 거부됐다. 각 프로세스의 시스템 호출 복귀 주소, ELF와 RAM 기록을 대조해 점유·반환 사이에 거부가 일어난 순서를 확인했다. 슬롯 1은 절대 시각 대기 다섯 번에 걸쳐 단조 시각 여섯 개를 남겼다. 슬롯 0은 같은 프로세스에서 장치를 반환하고 다시 점유했다.

### Source and build identity / 원본·빌드 식별

The importer checked the 196,513-byte upstream Linux stable v6.18.65 `drivers/net/virtio_net.c` against SHA-256 `05e676f7fc4f91783c961c860b930fae33462348064887eea95be3a440282a29`. GCC 11.4 compiled the whole file using 9,403 pinned AArch64 header/configuration files and warned that it ignored the `counted_by` attribute. The 179,640-byte relocatable object had 217 undefined external symbols. Exposing the local callback symbol in a copy of that object preserved its 712-byte function section. The linker retained the validation callback and the adapter functions it needed in the native ELF. [Upstream source](https://github.com/gregkh/linux/blob/4700e3adf3b24b39dd4929e9c6d62811ba3ccea8/drivers/net/virtio_net.c) · [Import profile](../../../drivers/linux-compat/profiles/virtio-net.json)

Two builds produced identical bytes for the original and promoted objects, service ELF, peer ELF and kernel. Independent input controls rejected a changed source file, a changed Alpine header package and a changed header in the extracted tree. Replay controls rejected changes to the peer's exit or authority, recorded MTU, MMIO write and callback trace. D also rejected deletion or misidentification of the stop-before-execution record. [Measurements](result.json) · [Build and link checks](../../../tools/build_linux_source_virtio_net.py)

GCC 11.4가 수정 없는 전체 C 파일을 컴파일했고, `counted_by` 속성을 무시한다는 경고가 남았다. 원본 재배치 오브젝트에는 외부 심볼 217개가 남아 있었다. 복사본에서 콜백 심볼을 외부 링크용으로 바꾼 전후에 712바이트 함수 섹션이 일치했다. 두 빌드에서 원본·파생 오브젝트, 서비스·상대 프로세스 ELF와 커널이 바이트 단위로 같았다. 원본 파일, Alpine 패키지, 압축을 푼 헤더를 각각 바꾼 세 입력도 거부됐다.

The measured scope ends at feature negotiation. Packet TX/RX through the original driver, virtqueue DMA bounds, device IRQ delivery and packet recovery after a fresh service starts remain open. These two QEMU runs cover the emulated device and trusted package. Physical hardware and product safety require their own evidence. [Original-driver packet-path design](../../native-linux-net-packet-design.en-ko.md)

이번 실측 범위는 기능 협상까지다. 원본 드라이버의 패킷 송수신, virtqueue DMA 범위 강제, 장치 IRQ 전달, 새 서비스 기동 후 패킷 교환 복구는 후속 검증 대상이다. 실보드와 제품 안전의 인수에는 각각 별도 증거가 필요하다. [원본 드라이버 패킷 경로 설계](../../native-linux-net-packet-design.en-ko.md)

### Replay the published captures / 공개 실행 기록 재검증

The public archive is 606,612 bytes, SHA-256 `edb5ed0fd6db23123009f7b369ba64a7f0a8a167e84fc101ccc7f2a09f87c0be`. Replay from a fresh extraction passed in both directions. From the repository root, use Python 3 with pyelftools 0.32 and AArch64 GNU binutils to replay both boots offline. [Download both raw captures and replay inputs](qemu-replay.tar.xz)

공개 압축 파일에는 두 부팅의 CPU·MMIO·RAM 원본 기록과 재생용 ELF가 들어 있다. 새 디렉터리에 압축을 풀어 C와 D를 각각 기준으로 재생했고 양쪽 모두 통과했다. 저장소 루트에서 다음 명령을 실행한다. [원본 기록과 재생 입력 다운로드](qemu-replay.tar.xz)

```sh
set -eu
repo=$(pwd)
archive="$repo/docs/evidence/native-linux-source-net/qemu-replay.tar.xz"
work=$(mktemp -d)
printf '%s  %s\n' edb5ed0fd6db23123009f7b369ba64a7f0a8a167e84fc101ccc7f2a09f87c0be "$archive" | sha256sum -c -
tar -xJf "$archive" -C "$work"
(cd "$work" && sha256sum -c SHA256SUMS)
verify() {
  python3 -B "$repo/tools/verify_linux_source_net.py" --package "$work/native" \
    --driver-manifest "$work/imported/import.json" "$@"
}
verify --out "$work/capture-c" --replay --require-exec --compare "$work/capture-d" --mutation-tests
verify --out "$work/capture-d" --replay --require-exec --compare "$work/capture-c" --mutation-tests
```

### Rebuild and boot the guest / 재빌드·게스트 실행

The local full rebuild archive, `native-linux-source-virtio-net-control.tar.xz`, is 23,061,400 bytes, SHA-256 `eb377d3ee93833bf694b50d04300c47b556beedd20be397e5d31b16b94706e12`. It contains the GPL-licensed upstream file, original Alpine APK and signed index, build receipts, independent checks and vendored pyelftools. Its `sh replay.sh` passed after fresh extraction. That script restores the compiler's recorded `__FILE__` prefix for relocated headers and compares the original and promoted objects and service ELF byte for byte.

For fresh guest runs, use a Linux filesystem, AArch64 GNU GCC/binutils, clang/LLD, the repository's Rust toolchain, QEMU AArch64 with TCG, OpenSSL and Python 3. Run from the repository root with `packet` set to the absolute path of the extracted full rebuild archive. The builder verifies the pinned source and entire header tree.

로컬 전체 재빌드 아카이브는 GPL 원본 소스, Alpine APK·서명된 인덱스, 빌드 기록, 독립 판독 도구와 Python 의존성을 포함한다. 새로 압축을 푼 뒤 `sh replay.sh`를 실행해 헤더 경로의 `__FILE__` 문자열을 복원하고 오브젝트·서비스 ELF가 바이트 단위로 일치하는지 확인했다. 새 게스트를 빌드하려면 Linux 파일시스템에서 저장소 루트를 작업 디렉터리로 삼고, `packet`에 전체 재빌드 아카이브를 푼 절대 경로를 지정한다.

```sh
set -eu
: "${packet:?set packet to the absolute path of the extracted evidence archive}"
work=$(mktemp -d)
export LC_ALL=C CARGO_BUILD_JOBS=1 PYTHONDONTWRITEBYTECODE=1 PYTHONPATH="$packet/vendor"
python3 -B "$packet/intake/prepare_headers.py" "$packet/intake" "$work/signed"
python3 -B tools/prepare_linux_virtio_net_headers.py \
  --apk "$packet/intake/linux-virt-dev-6.18.65-r0.apk" --out "$work/headers"
python3 -B tools/build_linux_source_virtio_net.py --source "$packet/upstream/virtio_net.c" \
  --headers "$work/headers" --out "$work/imported"
sh sdk/native-aarch64/build.sh examples/linux-source-net/peer.c "$work/peer.elf"
python3 -B tools/build_native_el0.py --board virt --linux-source-net --policy fifo \
  --priority 8 --peer-priority 8 --elf "$work/imported/service.elf" \
  --peer-elf "$work/peer.elf" --out "$work/native"
verify() {
  python3 -B tools/verify_linux_source_net.py --package "$work/native" \
    --driver-manifest "$work/imported/import.json" "$@"
}
verify --out "$work/capture-c" --mtu 1410 --mac 52:54:00:19:26:41
verify --out "$work/capture-d" --mtu 1386 --mac 52:54:00:19:26:42
verify --out "$work/capture-c" --replay --require-exec --mutation-tests --compare "$work/capture-d"
verify --out "$work/capture-d" --replay --require-exec --mutation-tests --compare "$work/capture-c"
```
