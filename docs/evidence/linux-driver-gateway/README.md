# Linux driver gateway on AArch64 / AArch64 Linux 드라이버 게이트웨이 시험

**EVD-322 · #417.** A JNOX Rust process completed an exchange through Alpine’s original `virtio_net.ko`. During a controlled stop of Linux, the JNOX process applied its software fallback and an unrelated task continued to run. After a cold Linux boot, JNOX discarded a retained first-generation reply and accepted the new device result. Two runs used the same JNOX image and Linux operands. [Acceptance criteria](../../linux-driver-gateway.en-ko.md) · [Build and replay](../../../examples/linux-driver-gateway/README.md)

JNOX 앱에서 장치 요청을 시작해 원본 Linux NIC의 송수신과 앱의 최종 반환값을 대조했다. Linux 종료 중에도 JNOX가 소프트웨어 출력을 비활성화했고, 재부팅 시에는 같은 요청 ID와 데이터에 대한 이전 세대 응답 1건을 폐기한 뒤 새 응답을 처리했다.

| Measurement / 실측 | First run / 1차 | Repeat / 2차 |
| --- | ---: | ---: |
| Outage request deadline on JNOX / JNOX 요청 기한 | 250 ms | 250 ms |
| Request start to fallback / 요청부터 대체 출력까지 | 250.186448 ms | 251.937248 ms |
| Time past deadline / 기한 초과분 | 0.186448 ms | 1.937248 ms |
| Peer ticks strictly inside outage request interval / 응답 대기 중 별도 태스크 진행 | 5 | 5 |
| Peer ticks across recovery / 복구 중 진행 | 27 | 35 |
| Peer ticks between Linux startup and ready observation reads / Linux 기동·준비 관측 사이 진행 | Unmeasured · 미측정 | 31 (16 → 47) |
| Linux process exit to next ready message, host clock / 호스트에서 관측한 종료~준비 | 1.600855410 s | 2.048715203 s |
| Stale replies rejected / 이전 응답 폐기 | 1 | 1 |

The healthy requests used 10 s and the offline request used 250 ms. Both fallback observations met the fixture’s 300 ms acceptance limit. Each Linux instance used an independent emulated NIC. Its packet trace records the exact request and the eight reply bytes independently of the test adapter’s serial path. The unauthorized JNOX peer received `-EACCES`. A separate run repeated the Linux-only reference exchange. Two packet parser tests passed, and the observer rejected nine altered copies of the repeat capture.

두 AArch64 QEMU 머신에서 JNOX와 Linux를 실행했다. Linux 원본 커널, 설정, 모듈 4개는 단독 기준 시험과 일치했고 장치 요청을 받는 사용자 공간 어댑터를 initramfs에 추가했다. 성공 요청 기한은 10초, 중단 요청 기한은 250ms다. 300ms는 이 진단 시험의 관측 기준이다. Linux를 강제로 중단한 동안 JNOX 쪽 브로커와 시계는 계속 동작했다. 실물 출력, 동일 SoC의 DMA·IRQ 격리, 최악 응답시간 및 제품 기능안전 승인은 별도 검증 대상이다. [Parent integration issue / 상위 이슈](https://github.com/Romanof123/JNOX/issues/3)

## Records / 시험 기록

- [Measured times and image hashes / 계측값·이미지 해시](gateway-results.json) · [Host event sequence / 호스트 사건 순서](event-sequence.json) · [UART observations and packet hashes / UART 기록·패킷 해시](guest-records.json) · [Linux-only regression / Linux 단독 재검증](reference-control.json)
- [Guest runtime source](https://github.com/Romanof123/JNOX/commit/36d893bffbd18f2a19813ac36e62c23cee4ceada) · [First observer](https://github.com/Romanof123/JNOX/commit/40653834733d0157da5fc8e2806d47f95c7225f5) · [Repeat observer with startup samples](https://github.com/Romanof123/JNOX/commit/7a41b01f6f2bcbc73d23f1344669ee6e05ef34a5). The runtime manifest reports local Git commit `3c8d188d99678498b8f7d4a758e7d8852dc26332`. Its Git tree and the public runtime commit both equal `f41d3995b1fa99ba979be17a7b60e8fc8dc9fb54`.
- Retained local archive `linux-driver-gateway-originals.tar.xz`, 137252 bytes, SHA-256 `d43a31ffb428f217f9e4ffb48a3d3d2f7c5360654d56edd761c35d3efbdd4ad5`. It holds both sets of packet captures, UART logs, sampled task memory and host events. The first run completed its guest operations, then the observer failed to find the tracked Linux reference in a sparse checkout. Restoring that file and replaying the original bytes produced the first-column verdict. The second run passed during initial collection and offline replay.
- The bounded JNOX scheduler journal reported 9,955 dropped entries in the repeat. Peer progress was measured with private timestamped samples and live counters. The startup count spans the two host reads bracketing the ready message. The exact last guest tick before Linux announced readiness was not measured. / 2차 시험에서 스케줄러 로그 9,955건이 용량 한도로 빠져 태스크 진행은 별도 메모리 기록으로 판정했다. 부팅 구간의 31회는 두 관측 사이의 차이다. 1차 시험은 검증 폴더에서 기준 파일을 복원한 뒤 동일 원본 재생으로 판정했다.

CHG-322 → REQ-004/011 → DES-002 → IMP-322 → VER-004/011 → EVD-322. [Trace edges / 추적 관계](../../../trace/links.csv) · [Integration PR / 통합 PR](https://github.com/Romanof123/JNOX/pull/419)
