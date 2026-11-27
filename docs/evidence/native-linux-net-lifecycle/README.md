# EVD-334 · Original virtio-net device lifecycle / 원본 네트워크 드라이버 등록·개방·해제

The unchanged Linux v6.18.65 virtio-net driver ran as a native user service on single-core AArch64 QEMU virt. In both cold boots its registered probe created an Ethernet interface and reached DRIVER_OK with an empty RX ring. Its registered open then posted 32 distinct writable buffers. The original service closed, reopened and removed the interface before probing again under a fresh device token. The same peer received all 83 external Ethernet frames in each boot. [Requirement and change](../../native-linux-net-probe-open-design.en-ko.md) · [Issue #438](https://github.com/Romanof123/JNOX/issues/438) · [Implementation PR #439](https://github.com/Romanof123/JNOX/pull/439)

단일 코어 AArch64 QEMU에서 Linux 원본 드라이버가 JNOX 사용자 프로세스로 실행됐다. 원본이 등록한 probe가 끝났을 때 장치는 준비 상태였고 수신 링은 비어 있었다. 등록된 open을 호출한 뒤 처음으로 버퍼 32개가 게시됐다. 같은 서비스에서 닫기·재개방·제거·다시 등록하기까지 수행했고, 새 토큰에서도 동일한 소비자 앱이 패킷을 받았다.

| Capture / 부팅 | MAC | MTU | Hardware RX IRQ / 장치 수신 인터럽트 | Original probe / open / close / remove |
|---|---|---:|---:|---|
| full-02 | 52:54:00:19:26:41 | 1410 | 12 | 2 / 3 / 3 / 2 |
| full-03 | 52:54:00:19:26:52 | 1490 | 13 | 2 / 3 / 3 / 2 |

Each first bind received 80 data frames (5,912 bytes) and one 60-byte ACK. Each second bind received a fresh 60-byte data frame and its 60-byte ACK. The independent reader compared the external stream and NIC capture to the bytes in the consumer's separate address space, checked both tokens, the old-token rejection and three denied device operations in each generation. The primary reader also matched relocated instruction bodies, original ops-table entries, runtime dispatch and real queue/interrupt traces. It distinguishes the empty poll scheduled by open from the poll following an actual device IRQ. [Raw replay results](result.json)

독립 검증자가 외부 스트림과 NIC 캡처의 패킷을 소비자 전용 메모리의 바이트와 대조했다. 첫 등록에서는 데이터 80개와 ACK, 재등록에서는 새로운 데이터 한 개와 ACK가 일치했다. 이전 토큰은 거부됐고 소비자가 장치 권한을 요청한 세 가지 연산도 각 세대에서 거부됐다. 독립 판독의 범위는 저장된 실행 파일·장치 추적·원본 패킷·게스트 상태이며, 컴파일러 재현은 별도다.

## Replay / 재현

Check the SHA-256, extract the archive in an empty Linux directory and run `python3 -B replay.py`. The archive includes the original C translation unit, compiler inputs, linked service and peer, kernel, capture-time observer, independent reader, eight stopped checkpoints per boot, MMIO/IRQ logs, PCAP and UART. Fresh extraction verified 1,391 checksummed files, reproduced both independent reports and rejected four mutations per capture. The build and observer Git objects are recorded separately in the archive. [Download the archive](lifecycle.tar.xz) · [Checksum](lifecycle.tar.xz.sha256)

SHA-256 `1a6bd292ea61c5c65ef591158e3deb0a860a69591c9bd89cf0d153ab6c5a603e`. 빈 디렉터리에 압축을 풀고 위 명령으로 재판독한다. 두 부팅에서 각각 여덟 정지 지점을 대조하고 원본 콜백 포인터, 장치 등록, 명령 추적, 소비자 패킷의 변조 네 종류를 모두 검출했다.

These runs cover successful lifecycle and ingress with acknowledgments sent by the external host. The combined original TX/RX workload is tracked in #426. Forced allocation, registration, notifier and XDP memory-model errors are tracked separately in #438. A driver assertion that parks the guest has a dedicated containment test under #440. Physical MMIO/DMA isolation and product safety approval require target evidence. [Combined transmit/receive #426](https://github.com/Romanof123/JNOX/issues/426) · [Driver assertion #440](https://github.com/Romanof123/JNOX/issues/440)

이 기록은 정상 수명주기와 외부에서 들어오는 패킷을 검증한다. 단일 워크로드에서의 원본 양방향 송수신, 네 가지 오류 주입, 단언 실패 격리 및 실보드 적격성은 각 항목의 후속 증거로 추적한다.
