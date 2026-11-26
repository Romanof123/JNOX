# CHG-327 · Native virtio-net queue-pair setup / 네이티브 virtio-net 큐 쌍 설정

**REQ-004/011 · DES-002 · IMP-327 · VER-004/011.** EVD-327 is reserved for a guest capture and its replay. The object comparison below is a completed host measurement. [Queue-pair issue #428](https://github.com/Romanof123/JNOX/issues/428) · [Packet-path design](native-linux-net-packet-design.en-ko.md) · [Earlier feature-validation evidence](evidence/native-linux-source-net/README.md)

### Object comparison / 오브젝트 비교

The same unchanged Linux v6.18.65 `drivers/net/virtio_net.c` was compiled twice against the pinned AArch64 headers. The second build added the J-NOX packet-context headers. The table counts instructions in each entry's direct-call closure, in stock/overlay order. Closures can overlap. Indirect callees and final executable paths require separate examination. [Import profile](../drivers/linux-compat/profiles/virtio-net.json) · [Object auditor](../tools/linux_driver_el0_audit.py)

| Entry / 진입점 | SP_EL0 reads / 읽기 | TPIDR_EL1 reads / 읽기 | Exception instructions / 예외 명령 |
| --- | ---: | ---: | ---: |
| `virtnet_validate` | 0/0 | 0/0 | 0/0 |
| `virtnet_probe` | 1/0 | 0/0 | 4/4 |
| `virtnet_open` | 2/0 | 0/0 | 7/7 |
| `start_xmit` | 1/0 | 1/0 | 4/4 |
| `virtnet_poll` | 6/0 | 1/0 | 14/14 |
| `virtnet_remove` | 0/0 | 0/0 | 1/1 |

동일한 원본 C 파일과 고정 AArch64 헤더를 사용했다. 표의 앞 값은 기본 헤더 빌드, 뒤 값은 J-NOX 패킷 컨텍스트 헤더를 추가한 빌드다. 각 진입점에서 직접 호출로 도달하는 함수들을 조사했으며 경로가 겹칠 수 있다. 간접 호출과 최종 실행 경로는 별도 검증 대상이다. SHA-256은 기본 오브젝트가 `5767a14f1d391e2419240bde3a0dab3c39457b894a35f2de581edce177c4862b`, 패킷 오브젝트가 `16e93df7744717a9b24af0ba60ca0f2fdfacae0118db9651a30db143fc717724`이다.

### Guest acceptance / 게스트 인수 기준

1. Trace the upstream `virtnet_validate` entry twice, once per device claim. Trace four entries to the J-NOX bus adapter `jnox_queue_find_vqs` through the Linux `virtio_find_vqs` helper, including rejection of an invalid count and a missing callback. Original Linux probe, open, TX, NAPI poll and remove are outside this service's linked entries. The RX/TX completion counters must stay zero.
2. In each claim, verify QEMU's queue maximum and the programmed descriptor, available and used-ring addresses for RX index 0 and TX index 1. Require 32 entries and `QueueReady=1` for each. Device status must remain 11 (`ACKNOWLEDGE | DRIVER | FEATURES_OK`), with zero posted buffers. Verify the 8 KiB ring pool belongs to EL1 and slot 1 is denied queue setup and inspection.
3. Release, reclaim and repeat in the same live owner. Before ring memory reuse, require device status zero and both queue-ready bits clear. Require a positive second token greater than the first and `-ESTALE` for the first token after release and after reclaim. Capture and replay the device trace, execution PCs and stopped memory before attaching EVD-327. [EL1 queue implementation](../kernel/src/native_linux_net_queues.rs) · [Replay criteria](../tools/verify_linux_source_net_queues.py)

게스트에서는 점유 세대마다 업스트림 `virtnet_validate`가 한 번씩 실행되는지 확인한다. Linux `virtio_find_vqs`가 호출하는 J-NOX 버스 어댑터는 유효한 설정 두 번과 잘못된 큐 개수·누락된 콜백을 합쳐 네 번 진입해야 한다. 원본 probe·open·TX·NAPI poll·remove 실행과 패킷 송수신은 후속 범위다. RX 0과 TX 1이 각각 32개 항목으로 준비됐는지 MMIO에서 확인하고, 등록된 버퍼와 완료 횟수는 0, 장치 상태는 11이어야 한다. 슬롯 1의 큐 접근 거부를 확인한다. 같은 소유자가 반환 후 다시 점유할 때 장치 상태와 양쪽 준비 비트의 초기화, 토큰 증가, 반환 후와 재점유 후 이전 토큰의 `-ESTALE`을 확인한다. EVD-327은 게스트 기록을 재생한 뒤 연결한다.

### Build / 빌드

From the repository root, set `source_c` and `headers` to the pinned source and prepared AArch64 headers. 지정한 원본 소스와 AArch64 헤더로 저장소 루트에서 실행한다. [Input preparation / 입력 준비](evidence/native-linux-source-net/README.md)

```sh
set -eu
work=$(mktemp -d)
python3 -B tools/build_linux_source_virtio_net.py --queue-service --source "$source_c" \
  --headers "$headers" --out "$work/imported"
sh sdk/native-aarch64/build.sh examples/linux-source-net-queues/peer.c "$work/peer.elf"
python3 -B tools/build_native_el0.py --board virt --linux-source-net-queues --policy fifo \
  --priority 8 --peer-priority 8 --elf "$work/imported/service.elf" \
  --peer-elf "$work/peer.elf" --out "$work/native"
```

Use `--packet-object-only` in place of `--queue-service` to reproduce just the static comparison in a fresh output directory. 정적 비교만 재현할 때는 새 출력 디렉터리에 `--packet-object-only`로 임포터를 실행한다.
