# M1 단일 코어 EDF 디스패치

한국어 · [English](m1-edf.en.md)

`CHG-006`은 `IMP-005`의 합격 조건을 정의한다. 고정 우선순위가 더 높은 LOW가 먼저 준비 큐에 들어가 실행된다. 타이머는 게스트의 동일한 단조 시계 기준으로 절대 마감 시각이 더 이른 URGENT를 실행 가능 상태로 만든다. LOW도 계속 실행 가능하다. 고정 우선순위와 FIFO라면 모두 LOW가 계속 실행된다.

| 요구사항 / 설계 / 검증 | 합격 조건 |
| --- | --- |
| REQ-023 / DES-012 / VER-023 | EDF는 실행 가능한 두 태스크의 절대 마감 시각을 비교하여 LOW가 양보하기 전에 URGENT를 선택한다. |
| REQ-002 / DES-003 / VER-002 | 게스트 타이머 IRQ가 작업 루프 안의 LOW를 중단하고 URGENT의 스택으로 직접 복귀한다. |
| REQ-005 / DES-003 / VER-005 | URGENT가 `SVC #1`로 종료하면 LOW의 중단 시점 PC와 스택을 복원한다. LOW가 저장 상태를 확인하고 완료를 출력한다. |

## 독립 관찰

구현 소스와 디스어셈블리를 검토하기 전에 후보 리비전을 기록하고 공개 계약과 후보 ELF로 첫 실행을 관찰한다. WSL Bash의 저장소 루트에서 한 번 빌드하고 같은 게스트에서 수집한 실행 로그와 예외 추적을 보관한다.

```sh
(cd kernel && cargo build --release --locked --features edf-bench -j1)
ELF=kernel/target/aarch64-unknown-none/release/jnox-kernel
export EDF_EVIDENCE=$(mktemp -d)
cat > "$EDF_EVIDENCE/qemu-int" <<'SH'
#!/bin/sh
exec qemu-system-aarch64 -d int -D "$EDF_EVIDENCE/int.log" "$@"
SH
chmod +x "$EDF_EVIDENCE/qemu-int"
set -o pipefail
python3 tools/qemu_boot.py --profile edf --kernel "$ELF" --qemu "$EDF_EVIDENCE/qemu-int" 2>&1 | tee "$EDF_EVIDENCE/runner.log"
```

EDF 프로필은 기본으로 단일 코어 Cortex-A72, GICv2, TCG, 메모리 128 MiB 게스트 한 대를 새로 부팅한다. 각 줄이 LF 또는 CRLF로 끝나는 다음 일곱 줄을 정확히 출력하고 추가 출력 관찰 구간이 지난 뒤 실행기가 성공 종료해야 한다.

```text
JNOX M1 BOOT
JNOX M1 READY
SCHED LOW RUN
SCHED TIMER IRQ
EDF URGENT RUN
SCHED LOW RESUMED
SCHED PASS
```

`EVD-005`에는 ELF SHA-256, 실행 인자, PID, UART와 같은 실행의 예외 기록을 보관한다. 해당 ELF에서 IRQ의 중단 PC가 LOW, 복귀 PC가 URGENT에 속하는지 확인한다. SVC 명령이 URGENT에 있고 복귀 PC가 LOW의 원래 중단 지점과 일치하는지 확인한다. 소스와 링크된 코드에서 마감 시각 산출·비교, 타이머에 의한 준비 상태 전환, 별도 스택, 상태 검증, 각 마커를 출력한 스레드를 확인한다. 불일치와 첫 실행 실패도 기록한다.

검증 범위는 에뮬레이션된 단일 코어에서 EDF 디스패치와 문맥 복원까지다. FIFO 또는 RR이 포함된 빌드는 기존 프로필을 유지하며 둘 다 활성화하면 RR을 사용한다. 제품의 마감 보장, 최악 지연, 스케줄 가능성 및 다중 코어 동작에는 별도 근거가 필요하다.
