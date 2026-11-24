# EDF와 최소 여유 시간 스케줄링

[English](m1-deadline.en.md)

CHG-017 · POL-005 · REQ-023/002/005 → DES-012/003 → IMP-015 → VER-023/002/005 → EVD-015. [#43](https://github.com/Romanof123/JNOX/issues/43)

EDF는 절대 기한이 가장 이른 작업을 선택한다. LLF는 부호 있는 `기한 − 현재 시각 − 잔여 추정량`이 가장 작은 작업을 선택한다. 같은 시각의 도착을 모두 등록한 뒤 결정한다. 실행 중인 작업에 경과 시간을 차감하며 명시적 완료 전까지 잔여 추정량을 최소 1로 유지한다. 동점이면 기존 작업을 유지하고 대기 작업끼리는 큐 순서를 지킨다. 음수 여유도 선택 대상이며 남은 시간에 추정 실행량을 마칠 수 없음을 나타낸다.

양의 간격 q와 시작 시각 s가 결정 경계를 정한다. 실행 중인 작업은 각 경계의 첫 선택에서만 선점될 수 있다. 경계 밖에서 호출하면 경과 시간만 차감하고 실행 중인 작업을 유지한다. 완료 직후 등 CPU가 비면 즉시 다음 작업을 실행한다. s ≤ a ≤ b일 때 (a,b] 안의 비자발적 전환 횟수 상한은 `floor((b−s)/q) − floor((a−s)/q)`다. 완료·자발적 전환에 드는 시간은 별도 예산이 필요하다. 지나간 경계의 전환을 뒤늦게 실행하지 않는다.

시각 0에 A(기한 8, 추정량 1)와 B(10, 8)를 함께 등록한다. EDF는 A를 실행한다. 시각 1의 타이머가 C(3, 3)를 등록하면 C가 A를 선점해 1에 완료하고, A가 중단 PC로 복귀해 2에 완료한다. 이후 B가 3에 완료한다. LLF는 B를 먼저 실행한다. 시각 5에는 A와 B의 여유가 2로 같고, 6에는 A가 1, B가 2다. A가 B를 선점해 6에 완료하면 B가 재개된다. 7에 C(기한 8, 추정량 3, 여유 −2)가 도착해 B를 선점한다. C는 7에, B는 8에 완료한다. 디스패처는 고정된 기준 카운터와 간격으로 물리 타이머 비교값을 설정하고 마지막을 제외한 LLF IRQ에서 다음 절대 경계를 설정한다. 작업은 검사할 상태를 갖춘 뒤 IRQ를 허용한다. 한 주기 안에 해당 경계를 처리하지 못하면 이 시험은 실패한다. 완료 시각과 서비스 추정량은 논리 입력이다.

kernel 디렉터리에서 `cargo build --release --locked --features deadline-bench -j1`로 빌드한다. 저장소 루트에서 `python3 tools/qemu_boot.py --profile deadline --kernel "$ELF"`를 실행한다. 새 검증자는 소스·예외 기록을 보기 전에 리비전, ELF 해시, 첫 UART 판정과 종료 코드를 보존한다. `--qemu`에 `-d int -D "$TRACE"`를 붙이는 래퍼를 지정한다. 아래 문구와 이후 0.1초 무출력, 타이머 IRQ 8회, 완료 SVC #1 6회를 확인한다. 같은 실행의 동점·여유 역전·음수 여유·최종 완료 시점의 중단과 복귀 주소를 ELF와 대조한다. 관찰기는 UART 누락·순서 오류·추가 출력을 모두 거부해야 한다.

```text
JNOX M1 BOOT
JNOX M1 READY
DEADLINE EDF A FIRST
DEADLINE EDF IRQ C READY
DEADLINE EDF C FIRST
DEADLINE EDF A RESUMED
DEADLINE EDF B FIRST
DEADLINE LLF B FIRST
DEADLINE LLF B TIE RETAINED
DEADLINE LLF T6 CROSSOVER
DEADLINE LLF A FIRST
DEADLINE LLF B RESUMED T6
DEADLINE LLF IRQ C READY
DEADLINE LLF C FIRST
DEADLINE LLF B RESUMED T7
DEADLINE PASS
```

호스트에서는 두 정책의 제한된 작업 조합, 동점, 부호 있는 산술, 선택 경계, 실행량 차감과 잘못된 요청을 대조한다. 예외 주소는 실제 태스크 전환을 확인한다. 물리 기한, WCET 및 제품 과부하 대응은 별도 부하·보드 측정·기능안전 승인이 필요하다.

카운터 값을 별도로 대조하려면 게스트가 완료 후 대기할 때 종료 전에 ELF 심볼 `deadline_clock_evidence`의 160바이트를 읽는다. 각 에포크의 리틀엔디언 u64 10개는 카운터 주파수·기준·간격·IRQ 관측값 7개 순서다. EDF는 첫 IRQ 필드만 사용한다. 각 관측값이 설정한 절대 구간 안에 있는지 확인한다. 이 수치는 게스트 물리 카운터 값이다.
