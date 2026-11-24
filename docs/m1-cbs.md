# M1 단일 코어 하드 CBS

한국어 · [English](m1-cbs.en.md)

CHG-008은 IMP-006의 합격 조건과 독립 실행 기록 EVD-006의 수집 절차를 정의한다. POL-008은 예산과 보충 시점을 제한하고 POL-005는 실행 가능한 서버를 절대 마감 시각순으로 정렬한다.

| 추적 항목 | 합격 조건 |
| --- | --- |
| REQ-023 / DES-012 / VER-023 | 논리 틱 0에 예산/주기가 각각 1/3과 1/4인 A와 B를 동시에 해제한다. 두 서버는 관찰 종료 뒤까지 작업이 남아 있다. 슬롯 0~4에서 A, B, 유휴, A, B가 실제로 실행된다. 틱 2에는 둘 다 예산을 소진했고 A는 틱 3, B는 틱 4에 보충된다. 서버 마감은 논리 틱의 절댓값이다. 게스트 물리 타이머의 각 경계는 최초 카운터 값을 기준으로 정한 절대 카운터 값이다. IRQ 처리 중 슬롯 하나를 통째로 놓치면 게스트는 해당 실행을 실패로 처리한다. |
| REQ-002 / DES-003 / VER-002 | 하드웨어 타이머 IRQ 다섯 번이 경계 1~5를 표시한다. 처음 네 번은 자발적 양보 없이 실행 주체를 전환한다. 마지막 IRQ에서 타이머를 멈추고 관찰을 마친다. |
| REQ-005 / DES-003 / VER-005 | A와 B는 각자의 전용 스택에서 저장된 PC로 복귀한다. 각각 레지스터, 스택 값과 진행 상태를 확인한 뒤 재개를 보고한다. |

## 독립 관찰

구현에 참여하지 않은 검증자는 소스와 디스어셈블리를 읽기 전에 리비전과 첫 실행 결과를 기록한다. 실패한 첫 시도도 남긴다. 저장소 루트에서 WSL Bash로 아래 절차를 한 번 실행한다. 벤치 기능을 함께 켜면 CBS가 우선한다. cbs 관찰기는 기본값으로 TCG, 메모리 128 MiB, GICv2를 사용하는 단일 코어 Cortex-A72 게스트 한 개를 실행한다.

```sh
set -e -o pipefail
(cd kernel && cargo build --release --locked --features cbs-bench -j1)
ELF=kernel/target/aarch64-unknown-none/release/jnox-kernel
export CBS_EVIDENCE=$(mktemp -d)
cp "$ELF" "$CBS_EVIDENCE/kernel.elf"
git rev-parse HEAD > "$CBS_EVIDENCE/revision"
cat > "$CBS_EVIDENCE/qemu-int" <<'SH'
#!/bin/sh
exec qemu-system-aarch64 -d int -D "$CBS_EVIDENCE/int.log" "$@"
SH
chmod +x "$CBS_EVIDENCE/qemu-int"
python3 tools/qemu_boot.py --profile cbs --kernel "$CBS_EVIDENCE/kernel.elf" --qemu "$CBS_EVIDENCE/qemu-int" 2>&1 | tee "$CBS_EVIDENCE/runner.log"
```

아래 아홉 줄이 순서대로 출력되어야 한다. 각 줄은 LF 또는 CRLF로 끝나야 하며 0.1초 동안 추가 출력이 없고 호스트가 게스트를 정상 종료해야 한다.

```text
JNOX M1 BOOT
JNOX M1 READY
CBS A FIRST
CBS B FIRST
CBS IDLE
CBS A RESUMED
CBS B RESUMED
CBS TIMER STOP
CBS PASS
```

동일 게스트의 ELF 해시, 실행 인자, PID, UART와 예외 추적을 보존한다. 저장한 ELF에서 다섯 IRQ의 피중단 PC와 복귀 경로를 확인하고 두 서버의 저장 PC 복귀 및 종료 경로를 대조한다. 링크된 코드와 소스에서 유휴 코드와 두 작업의 실제 실행, 전용 스택, 복귀 상태 검사, 타이머 정지, 실행 자격 판정을 확인하고 차이를 기록한다. 호스트 전용 관찰기 대조 시험은 누락, 순서 변경, 추가 출력을 각각 거부해야 한다. 이 시험에는 게스트 실행을 추가하지 않는다.

## 수용 판정 시험

이상적인 선점형 단일 코어에서 마감이 주기와 같은 독립 주기 예약을 대상으로 호스트에서 별도 시험한다. 1/3 + 1/4 및 합이 정확히 1인 1/2 + 1/2를 허용한다. 과부하 2/3 + 1/2, 예산 0, 주기 0, 주기를 넘는 예산을 거부한다. 수용 판정의 중간 계산값을 표현할 수 없어 오버플로 검사가 실패하는 사례도 포함하고, 해당 예약을 보수적으로 거부하는지 확인한다. 이 에뮬레이션의 범위에는 실물 보드 타이밍과 제품 안전성 입증이 포함되지 않는다.
