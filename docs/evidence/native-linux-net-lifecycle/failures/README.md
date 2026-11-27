# EVD-334 · Upstream driver error recovery / 원본 드라이버 오류 회수

Two AArch64 QEMU cold boots injected four one-shot failures in JNOX providers used by the unchanged Linux driver. All four calls returned −ENOMEM at the original probe or open boundary. After the fourth cleanup, both boots established a new binding and delivered a fresh external frame and ACK byte for byte into the separate peer process. [Issue #438](https://github.com/Romanof123/JNOX/issues/438) · [Successful lifecycle](../README.md)

| Injected failure | Original result | Actual ownership on return |
|---|---|---|
| Netdevice allocation | Probe −12 | Zero device allocations, registrations and queue tokens. |
| Netdevice registration under RTNL | Probe −12 | The allocated device was freed. The interface had never been published. |
| Addition of the second CPU notifier | Probe −12 | The first notifier was removed before the interface was unregistered and freed. |
| XDP PAGE_SHARED memory model during open | Open −12 | The interface stayed registered and down. Its 32 posted RX buffers and 49 page references remained live until remove. |

The restored binding used token 4. The three retired queue tokens returned −116. Each boot recorded two hardware RX interrupts for the two 60-byte recovery frames. Both driver and peer exited zero. Fifteen stopped checkpoints per boot cover the four injection points and normal recovery. The independent reader checks the source pin, ELF addresses, protected queue state, ownership, MMIO and external packet bytes. [Per-boot results](result.json)

두 번 부팅해 장치 할당, 등록, 두 번째 CPU 알림 등록, XDP 수신 메모리 모델 등록에서 각각 한 번씩 실패를 주입했다. 첫 세 경우는 원본 probe가 −12를 반환했다. 마지막 경우 원본 open이 −12를 반환했고 장치는 등록된 DOWN 상태였다. 정상 소유 중인 RX 버퍼 32개와 페이지 참조 49개는 remove에서 회수됐다. 새 토큰 4로 복구한 후 외부 프레임과 ACK 모두 별도 소비자 메모리에서 일치했다.

Extract the archive into an empty directory and run `python3 -B replay.py` on Linux. Fresh extraction checked 1,459 members, reproduced both independent reports and rejected four mutations per boot. The first capture used a verifier that incorrectly expected the first NAPI poll to follow ingress. The archive reproduces this error and records the corrected offline replay of the same raw capture. The second capture used the corrected observer throughout. Launch recordings include the command line. The QEMU executable digest is available from the board reference only. [Replay archive](lifecycle-failures.tar.xz) · [Checksum](lifecycle-failures.tar.xz.sha256)

SHA-256 `125fdd6bb8eee58bff81a6f4c44b438f1c2d3fcb9e8dd57ef7d9ed932199f9f0`. 첫 기록은 open 직후의 빈 폴링을 잘못 분류한 초기 판독기와 수정 후의 판독 결과를 함께 보존한다. 두 번째 기록은 수정된 판독기로 캡처했다. 시험 대상은 지정 위치에서 주입한 실패다. 실보드와 시스템 전체 메모리 고갈은 별도 검증 대상이다.
