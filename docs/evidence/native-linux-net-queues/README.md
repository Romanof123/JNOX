# EVD-327 · Native virtio-net queue setup and reset / 네이티브 virtio-net 큐 설정·초기화

**REQ-004/011 · DES-002 · IMP-327 · VER-004/011.** Two AArch64 QEMU virt cold boots each configured an empty RX/TX queue pair twice. In each boot slot 0 released the device, reclaimed it in the same live process, and received a fresh queue token. Offline replay matched the guest observations to the device-register trace, execution PCs and stopped RAM. [Results](result.json) · [Queue setup #428](https://github.com/Romanof123/JNOX/issues/428) · [Parent packet-path issue #426](https://github.com/Romanof123/JNOX/issues/426)

AArch64 QEMU virt에서 두 번 콜드 부팅했다. 매 부팅마다 슬롯 0의 같은 프로세스가 빈 RX/TX 큐 쌍을 설정하고 장치를 반환한 뒤, 다시 점유해 새 큐 토큰을 받았다. 저장된 장치 레지스터 접근, 실행 주소와 정지 시점의 RAM을 재생해 두 세대의 결과를 대조했다. [설계와 빌드 명령](../../native-linux-net-queues.en-ko.md)

| Observation in each boot / 부팅 1회당 관측 | Result / 실측 |
| --- | --- |
| Device limits and setup / 장치 상한·설정 | RX index 0 and TX index 1 each offered 1,024 entries. Both were configured with 32 entries and `QueueReady=1` in each generation. |
| Queue backing / 큐 메모리 | QEMU's descriptor, available and used-ring address registers matched the EL1 pool in both generations. RX used `0x4021e000 / 0x4021e200 / 0x4021e248` and TX used `0x4021f000 / 0x4021f200 / 0x4021f248`. |
| Status and reset / 상태·초기화 | Status stayed at 11 (`FEATURES_OK` with ACKNOWLEDGE and DRIVER). Reset returned status and both queue-ready bits to zero before reuse. Zero descriptors were posted. All 8,192 bytes of the pool were zero at guest stop. |
| Ownership / 소유권 | Tokens 1 then 2. Inspection with token 1 returned `-ESTALE` (−116) after release and after reclaim. Slot 1's setup and inspection calls both returned `-EACCES` (−13). |
| Peer progress / 상대 프로세스 진행 | Six monotonic-clock samples across five absolute timed sleeps. Both processes exited with status 0. |
| Executed entries / 실행 진입점 | Two entries into the upstream `virtnet_validate` and four into the J-NOX bus helper `jnox_queue_find_vqs`. Two helper calls rejected an invalid queue count and a missing callback. |

각 큐의 장치 상한은 1,024개였고 두 세대 모두 실제 설정값은 32개였다. 반환 후 장치 상태와 양쪽 준비 비트가 0으로 돌아갔고 재점유 후에도 옛 토큰은 −116으로 거부됐다. 권한이 없는 슬롯 1의 큐 설정·조회는 각각 −13이었고, 슬롯 1은 절대 시각 대기 다섯 번에 걸쳐 단조 시각 여섯 개를 기록했다. 실행 추적에는 업스트림 기능 검증 콜백이 두 번, J-NOX 버스 헬퍼가 네 번 나타났다.

Replay rejected seven altered records per boot covering a nonempty RX descriptor, unauthorized queue setup, acceptance of a stale token, a false queue-ready readback, reuse of a generation token, an incorrect descriptor address and removal of a bus-helper invocation. Original-driver packet TX/RX, DMA revocation with outstanding buffers, IRQ delivery and recovery in a replacement service remain open. [Verifier](../../../tools/verify_linux_source_net_queues.py) · [Packet-path design](../../native-linux-net-packet-design.en-ko.md)

각 부팅의 재생에서 RX 디스크립터, 권한 거부, 옛 토큰 거부, 큐 준비 상태, 세대 토큰, 디스크립터 주소, 버스 헬퍼 실행 기록을 각각 변조한 일곱 사례가 모두 거부됐다. 원본 드라이버의 패킷 송수신, 미완료 버퍼의 DMA 권한 회수, IRQ 전달과 새 서비스 인스턴스에서의 통신 복구는 후속 과제다.

### Regression checks / 기존 프로파일 회귀 검사

Both prior profiles were rebuilt at the queue change's revision and replayed separately by the PM. In the feature-only profile the original validator executed three times and returned `0, -22, -22`. The device trace contained 79 NIC MMIO accesses across the trial. The original PL031 driver delivered its alarm, and CPU 0 acknowledged GIC INTID 34 twice. [Regression summaries](result.json)

같은 리비전에서 기존 두 프로파일을 다시 빌드한 뒤 각각 별도로 재생했다. 기능 협상 전용 프로파일의 원본 콜백은 세 번 실행됐고 반환값은 차례로 0, −22, −22였다. PL031 원본 드라이버가 알람을 전달했고 CPU 0이 GIC INTID 34를 두 번 수락했다.

### Replay both captures / 두 실행 기록 재생

A separate reviewer decoded the raw CPU and device traces, the two linked ELFs, and both process page tables. The independent decoder and receipts are in `qa/queue-independent.py` and `qa/queue-independent.json` inside the download. Two clean output directories produced identical kernel, driver, peer, stock driver object, packet-profile object and promoted callback object bytes. The public archive is 658,076 bytes with SHA-256 `523b1b0dd8e279dd1efce2899b6fd28608f334ffaf6abdab110fded71fba9641`. Both captures passed replay with all seven negative controls from a fresh extraction. [Implementation and review #429](https://github.com/Romanof123/JNOX/pull/429)

독립 검증자는 CPU·장치 기록과 ELF, 각 프로세스의 페이지 테이블을 별도로 판독해 두 세대의 큐 주소와 권한 거부를 확인했다. 커널·서비스·피어와 세 오브젝트는 새 출력 디렉터리에서 다시 빌드했을 때 바이트 단위로 일치했다. 공개 압축 파일을 새 경로에 푼 후 두 부팅 기록을 각각 7개 변조 대조시험과 함께 재검증했다.

From the repository root, use Python 3 with pyelftools and AArch64 GNU binutils. The command extracts the bundle into a new directory and replays each capture with its seven mutations. 저장소 루트에서 다음 명령을 실행하면 새 디렉터리에 압축을 풀고 두 기록을 각각 재검증한다. [Download raw captures and replay inputs / 원본 기록·재생 입력](qemu-replay.tar.xz)

```sh
set -eu
repo=$(pwd -P)
work=$(mktemp -d)
tar -xJf "$repo/docs/evidence/native-linux-net-queues/qemu-replay.tar.xz" -C "$work"
for capture in capture-a capture-b; do
  python3 -B "$repo/tools/verify_linux_source_net_queues.py" \
    --package "$work/native" --driver-manifest "$work/imported/import.json" \
    --out "$work/$capture" --replay --mutation-tests
done
```
