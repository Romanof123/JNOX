# 단일 코어 CPU 버스트 디스패치

[English](m1-batch.en.md)

CHG-015 · POL-003 · REQ-002/005/023 → DES-003/012 → IMP-013 → VER-002/005/023 → EVD-013 · [#39](https://github.com/Romanof123/JNOX/issues/39)

용량이 고정된 스케줄러가 단조 증가하는 논리 틱으로 실행 중인 작업의 CPU 사용량을 차감한다. 도착은 큐에 등록하고 종료는 명시적으로 처리한다. 같은 시각의 도착을 모두 반영한 뒤 다음 작업을 선택한다. SJF·HRRN은 실행 중인 작업을 끝까지 유지한다. SRTF는 도착 시 잔여량을 비교하며 동점이면 실행 중인 작업을 유지한다. 대기 작업 동점은 큐 순서, 선점된 작업은 동점인 대기 작업보다 앞선다. 추정 시간을 초과한 작업도 종료 전까지 잔여 추정량 1을 유지한다. 잘못된 요청은 시각·작업·큐를 보존한다.

| 정책 | 작업 조건과 합격 순서 |
| --- | --- |
| SJF | 시각 0에 추정량 5인 A가 실행된다. 시각 1의 타이머가 추정량 2인 B를 등록한다. 잔여량 4인 A가 계속 실행돼 5에 종료하고 B가 7에 종료한다. |
| SRTF | 같은 도착에서 B가 A를 선점한다. B가 3에 종료하면 A가 중단된 바로 그 명령으로 복귀해 7에 종료한다. |
| HRRN | A가 0에 시작하고 추정량 5인 B가 0부터 대기한다. A가 추정량 2를 넘긴다. 시각 10의 IRQ가 추정량 1인 C를 등록한다. A가 계속 실행돼 종료하면 응답비 3인 B가 응답비 1인 C보다 먼저 실행된다. |

kernel 디렉터리에서 `cargo build --release --locked --features batch-bench -j1`로 빌드하고 리비전·ELF 해시·QEMU 버전을 보존한다. 구현과 예외 기록을 보기 전에 저장소 루트에서 `python3 tools/qemu_boot.py --profile batch --kernel "$ELF"`를 한 번 실행한다. 같은 게스트의 예외를 남기려면 `--qemu`에 `-d int -D "$TRACE"`를 붙이는 래퍼를 전달한다. 첫 판정·UART 원본·종료 코드를 기록한 뒤 그 실행의 예외와 ELF를 대조한다. 아래 UART 순서, 이후 0.1초 무출력, 물리 타이머 IRQ 3회와 SVC #1 종료 7회를 확인한다. 중단과 복귀 주소, 완료 작업, 스택·재개 상태를 확인한다. 호스트에서는 누락·순서 오류·추가 바이트를 각각 거부해야 한다.

```text
JNOX M1 BOOT
JNOX M1 READY
BATCH SJF A FIRST
BATCH SJF IRQ B READY
BATCH SJF A RETAINED
BATCH SJF B FIRST
BATCH SRTF A FIRST
BATCH SRTF IRQ B READY
BATCH SRTF B FIRST
BATCH SRTF A RESUMED
BATCH HRRN A FIRST
BATCH HRRN IRQ C READY
BATCH HRRN A RETAINED
BATCH HRRN B FIRST
BATCH HRRN C FIRST
BATCH PASS
```

독립 호스트 오라클로 세 정책의 작은 작업 집합을 대조한다. 동점, 과소 추정, 용량 0, 잘못된 도착·중복·종료, 시각 역행, ID 재사용, 표현 가능한 최대 시각을 포함한다. 표의 완료 시각은 정책에 주는 논리 입력이다. 실제 IRQ 세 번으로 비동기 도착과 커널 문맥 선택을 확인한다. 보드 WCET·물리 지연과 제품 승인은 별도 항목이다.

[EVD-013](evidence/evd-013.md)
