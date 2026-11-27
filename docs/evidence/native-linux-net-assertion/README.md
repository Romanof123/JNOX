# EVD-335 · Source-driver assertion containment / 원본 드라이버 서비스의 단언 실패 격리

Two independent AArch64 QEMU cold boots recorded the same process-local fault. The unchanged Linux driver had registered and opened its NIC and delivered an external frame to the consumer. With 31 writable RX grants still active, the JNOX test application around the driver executed `BRK #0x440` at `0x400110`. EL1 captured syndrome `0xf2000440` and the driver's TTBR0/ASID before retirement. The device and both queues acknowledged reset. All protected grants cleared, the driver terminated with status −5, and the scheduler selected the original consumer, PID 1. [Design and kernel implementation](../../native-linux-net-assertion.en-ko.md) · [Issue #440](https://github.com/Romanof123/JNOX/issues/440) · [PR #442](https://github.com/Romanof123/JNOX/pull/442)

드라이버 서비스가 실제 패킷 한 개를 전달한 뒤 RX 권한 31개를 가진 상태에서 시험 앱의 단언 명령을 실행했다. 두 번 모두 커널이 명령 주소와 원인을 기록하고 장치 상태 및 두 큐의 리셋을 확인했다. 기존 소비자 앱은 정확한 프레임을 보유한 채 파이프 EOF를 받았고, 지정 타이머를 기다린 뒤 장치 접근 거부 세 건을 확인하고 코드 0으로 종료했다.

| Boot | MAC | MTU | Fault handling tick / EL1 처리 시각 | Peer monotonic elapsed / 소비자 경과 시간 |
|---|---|---:|---:|---:|
| cold-02 | 52:54:00:19:26:41 | 1410 | 130812211 | 100.375088 ms |
| cold-03 | 52:54:00:19:26:52 | 1490 | 129799812 | 100.458496 ms |

Both independent replays joined the same instruction address in the linked driver-service ELF to the exception record, the protected scheduler decision (reason 256, PID 0 → 1), halted physical DMA state, queue reset readbacks, UART and private peer state. Both ordinary scheduler logs retained all 119 events. The selected architecture uses one CPU and one emulated NIC. The observed 100 ms intervals are waits requested by the survivor, not measurements of recovery latency. The original Linux source is unchanged; the deliberate breakpoint belongs to the test application.

독립 판독에서 실행 파일의 명령, 커널이 보관한 예외 레지스터, 스케줄러 결정, 장치 레지스터, DMA 및 소비자 전용 메모리가 서로 일치했다. 두 시험에서 일반 스케줄러 이벤트 119개도 빠짐없이 남았다. 위 시간은 소비자가 요청한 100 ms 대기 후의 경과값이다. 실보드 DMA 격리, 다른 예외 종류 및 제품 시간 상한은 별도 검증 대상이다.

## Regression and reproduction / 회귀와 재현

The same kernel source completed the ordinary full lifecycle workload without the assertion fixture, including 83 incoming frames and both native processes exiting zero. Five Rust register-model controls exercised timely and missing reset acknowledgement, stuck queue-ready bits, and safe admission after retirement. QEMU supplied a successful reset in both assertion boots. The failed-reset observations come from the register model.

초기 통합에서는 FIFO 예외 벡터가 보존한 사용자 x2 값을 스케줄러 시각으로 해석해 전체 게스트가 정지했다. 수정 후 EL1에서 하드웨어 카운터를 직접 읽고, 실제 프로세스 선택을 커널 전용 레코드에 남긴다. 정상 수명주기 회귀와 레지스터 모델 시험 결과를 함께 기록한다.

Verify SHA-256 `d77e3a22214d002707715569574b53aa922443441a32d8e9695235d5dd4636a5`. Extract into an empty Linux directory and run `python3 -B replay.py` from the root. A fresh extraction verified 1,435 checksummed members, reproduced both independent reports, rejected 12 directed mutations per assertion capture and four in the ordinary control. The prior five host-model test results are authenticated against the archived inputs. [Archive](assertion.tar.xz) · [Checksum](assertion.tar.xz.sha256) · [Results](result.json)

빈 폴더에 압축을 풀어 재판독한다. 두 단언 부팅은 각각 변조 12건을, 정상 회귀는 네 건을 검출했다. 독립 보고서 두 건은 새로 추출한 패키지에서 재현됐다.
