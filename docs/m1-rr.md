# M1 단일 코어 라운드 로빈 스케줄링 벤치

한국어 · [English](m1-rr.en.md)

`CHG-005`는 `IMP-004`의 합격 조건을 정한다. 스레드 A와 B의 고정 우선순위는 서로 같고 LOW보다 높다. 타이머는 A와 B가 각자 작업을 실행하는 도중에 CPU를 회수해야 한다. 공개 UART 판정과 같은 게스트에서 수집한 예외 추적 기록이 일치해야 한다.

독립 검증팀이 한 게스트에서 여섯 번의 예외 발생과 복귀를 확인했다. [EVD-004와 호스트 회귀검사 결과](evidence/evd-004.md)

| 추적 | 합격 조건 |
| --- | --- |
| `REQ-023` → `DES-012` → `IMP-004` → `VER-023` | LOW를 시작하기 전에 정해진 입력으로 정책 검사를 수행한다. 준비 작업의 순위 검사를 통과하면 `POLICY RANK PASS`, CPU 서비스 선택과 사용량 계산 검사를 통과하면 `POLICY SERVICE PASS`를 출력한다. |
| `REQ-023`, `REQ-002` → `DES-012`, `DES-003` → `IMP-004` → `VER-023` | 첫 타이머 IRQ가 높은 우선순위의 A와 B를 준비시킨다. 이어 세 번의 타임 슬라이스 만료 때마다 실행 중인 스레드가 동순위 대기 스레드 뒤로 들어가며 A → B → A → B 순서로 실행된다. LOW는 계속 실행 가능하다. |
| `REQ-005` → `DES-003` → `IMP-004` → `VER-023` | LOW, A, B는 같은 주소 공간에서 서로 겹치지 않는 스택을 쓴다. 각 스레드는 선점 때 저장한 PC와 스택으로 실행을 이어 간다. A와 B가 한 번씩 종료하면 LOW가 재개된다. |

준비 작업의 순위 검사는 주어진 작업 정보로 고정 우선순위, FCFS, SJF, SRTF, HRRN, RM, DM, EDF, LLF의 선택을 확인한다. 서비스 검사에는 실행 예산 차감, 가상 실행 시간 계산, 주어진 추첨값에 따른 티켓 선택이 포함된다. 두 출력은 정해진 입력에 대한 결정 결과를 나타낸다. 이 게스트에서 실제 스레드를 실행하는 경로는 고정 우선순위로 선택하고 동순위 스레드에 라운드 로빈을 적용한다. [스케줄링 정책과 전체 검사 목록](scheduling-policies.md) · [요구사항](requirements.md) · [검증 계획](verification-plan.md)

## 게스트 실행 순서

| 실행 중인 스레드 | 예외 | 기대하는 예외 복귀 지점 |
| --- | --- | --- |
| 작업 루프를 실행하는 LOW | 타이머 IRQ가 A와 B를 준비시킨다 | A의 첫 진입점 |
| 작업 루프를 실행하는 A | 타임 슬라이스 만료 IRQ | B의 첫 진입점 |
| 작업 루프를 실행하는 B | 타임 슬라이스 만료 IRQ | A가 처음 선점될 때 저장된 PC |
| 재개한 A | 타임 슬라이스 만료 IRQ | B가 선점될 때 저장된 PC |
| 재개한 B | `SVC #1`로 B 종료 | A가 마지막으로 선점될 때 저장된 PC |
| 재개한 A | `SVC #1`로 A 종료 | LOW가 처음 선점될 때 저장된 PC |

첫 IRQ에서만 `SCHED TIMER IRQ`를 출력한다. A와 B는 각자 자기 스택에서 CPU를 양보하지 않고 작업해야 한다. 스레드를 처음부터 다시 실행하거나, 자발적 양보로 전환하거나, 예외 처리기가 스레드를 대신해 해당 출력을 내면 실패다. LOW는 저장된 상태를 확인한 뒤 마지막 두 줄을 출력한다.

## 빌드 한 번과 게스트 한 대

저장소 루트에서 고정된 Rust 도구 체인과 `aarch64-unknown-none` 타깃으로 한 번 빌드한다. 독립 검증자에게 후보 리비전과 이 공개 합격 조건, 빌드한 ELF를 전달한다.

```sh
(cd kernel && cargo build --release --locked --features rr-bench -j1)
```

WSL Bash에서 후보 저장소 루트를 기준으로 최초 실행을 기록한다. 관찰기는 리눅스 기본 임시 디렉터리를 쓴다. 래퍼는 관찰기의 인자를 그대로 전달하고 같은 게스트의 예외를 QEMU 로그에 기록한다. `rr` 프로필의 기본 구성은 게스트 1대, `virt,gic-version=2`, Cortex-A72 CPU 한 개, TCG, RAM 128 MiB다.

```sh
unset TMPDIR TMP TEMP
export RR_EVIDENCE
RR_EVIDENCE=$(mktemp -d)
ELF=kernel/target/aarch64-unknown-none/release/jnox-kernel
cat > "$RR_EVIDENCE/qemu-int" <<'SH'
#!/bin/sh
exec qemu-system-aarch64 -d int -D "$RR_EVIDENCE/int.log" "$@"
SH
chmod +x "$RR_EVIDENCE/qemu-int"
set -o pipefail
python3 tools/qemu_boot.py --profile rr --qemu "$RR_EVIDENCE/qemu-int" --kernel "$ELF" 2>&1 | tee "$RR_EVIDENCE/runner.log"
```

전체 UART 바이트열은 정확히 아래 열두 줄이며 각 줄은 LF 또는 CRLF로 끝나야 한다.

```text
JNOX M1 BOOT
JNOX M1 READY
POLICY RANK PASS
POLICY SERVICE PASS
SCHED LOW RUN
SCHED TIMER IRQ
RR A FIRST
RR B FIRST
RR A RESUMED
RR B RESUMED
SCHED LOW RESUMED
SCHED PASS
```

커널 소스, 디스어셈블리와 인터럽트 로그를 **열기 전에** 최초 블랙박스 판정을 기록한다. 원시 UART, 커널 리비전, ELF SHA-256, 관찰기 명령, 진단 메시지와 종료 사유를 보존한다. 완전히 수집한 UART는 관찰기 로그의 바이트 리터럴을 파이썬 `ast.literal_eval`로 복원해 길이가 보고된 바이트 수와 같은지 확인한 뒤 저장한다. 바이트 누락·중복·순서 변경·추가, 마지막 줄 종결자 누락, 5초 시한 초과, 호스트가 정지시키기 전 게스트 종료는 실패다. 관찰기는 마지막 줄 뒤 0.1초 동안 무출력을 확인하고 종료 직후 바이트를 다시 검사한다. 호스트에서 첫 진입 두 줄의 순서를 바꾼 입력, 재개 한 줄을 누락한 입력, 기존 FIFO 기록을 재생해 거부되는지 확인한다.

## 예외 추적과 ELF 대조

전달받은 ELF에 `aarch64-linux-gnu-nm -n -S -C "$ELF"`와 `aarch64-linux-gnu-objdump -d -C "$ELF"`를 실행한다. 각 IRQ에서 저장한 복귀 주소(ELR)가 선점된 스레드의 작업 구간에 속하는지 확인한다. 각 `ERET` 목적지가 위 표의 진입점이나 저장된 주소와 일치하는지 확인한다. 두 SVC의 즉시값을 해독하고 각 명령의 주소를 해당 스레드의 종료 경로와 대조한다. IRQ 처리기가 GICv2에서 타이머 IRQ를 식별하고 시간 만료를 처리한 뒤 인터럽트 처리를 완료하는지 확인한다. 세 스레드의 스택 범위와 저장 상태의 검사값, 각 스레드가 종료 후 선택 대상에서 빠지는지도 확인한다. 합격에는 한 게스트의 UART, 예외 추적과 해당 ELF의 코드가 모두 이 전환을 뒷받침해야 한다.

이 사례의 범위는 에뮬레이션한 한 코어에서 지정한 작업을 실행할 때의 사건 순서다. 실제 EDF 디스패치, 타이머 주파수·정밀도 측정, 최악 응답 시간, SMP, 실보드 타이밍에는 각각 별도 근거가 필요하다. [증거 양식](templates/verification-evidence.md)
