# M1 협력형 스케줄링과 이벤트 디스패치

한국어 · [English](m1-cooperative.en.md)

CHG-014(이슈 #37)는 POL-001의 구현 IMP-012와 독립 실행 기록 EVD-012에 대한 합격 조건을 정의한다.

| 추적 항목 | 합격 조건 |
| --- | --- |
| REQ-023 / DES-012 / VER-023 | 협력형 FCFS에서 스레드 ID 1인 A가 먼저 실행된다. A 실행 중 스레드 ID 0인 B가 준비 상태가 되어도 A는 양보할 때까지 계속 실행한다. 이후 A가 대기 중인 B를 깨울 때도 A가 계속 실행한다. |
| REQ-002 / DES-003 / VER-002 | 게스트 물리 타이머 IRQ가 정확히 한 번 발생해 B를 준비 상태로 만든다. 예외 복귀 주소는 인터럽트 당시 A의 PC와 일치한다. |
| REQ-005 / DES-003 / VER-005 | A가 SVC #2로 양보해 B가 실행되고 B가 #3으로 대기해 A가 재개한다. A가 #4로 B를 깨우면 A 자신에게 복귀한다. A가 #1로 종료하면 B가 재개하고 B도 #1로 종료하면 이벤트 디스패처로 넘어간다. 각 예외의 복귀 주소를 태스크 문맥에 대응시키고 재개 시 전용 스택과 보존 레지스터의 검증값을 확인한다. |

이벤트 큐의 용량은 2이다. 1과 2를 차례로 넣은 뒤 가득 찬 상태에서 추가 삽입이 거부되고 기존 항목이 보존되는지 확인한다. 디스패처가 1을 꺼내면 핸들러 1이 3을 추가한다. 핸들러 1이 반환한 뒤에 핸들러 2를 호출하며 핸들러 3은 마지막에 실행한다. 핸들러 실행 중 다른 핸들러가 중첩 호출되지 않는지 확인한다.

## 독립 관찰

저장소 루트의 WSL Bash에서 실행한다. 검증자는 리비전과 실패를 포함한 첫 실행 판정을 기록한 뒤 소스, 역어셈블리, 예외 추적을 조사한다. 관찰기는 기본적으로 QEMU virt, GICv2, TCG, 128 MiB 구성에서 단일 코어 Cortex-A72 게스트를 한 번 실행한다.

```sh
set -e -o pipefail
(cd kernel && cargo build --release --locked --features coop-bench -j1)
export COOP_EVIDENCE=$(mktemp -d)
cp kernel/target/aarch64-unknown-none/release/jnox-kernel "$COOP_EVIDENCE/kernel.elf"
git rev-parse HEAD > "$COOP_EVIDENCE/revision"
cat > "$COOP_EVIDENCE/qemu-int" <<'SH'
#!/bin/sh
exec qemu-system-aarch64 -d int -D "$COOP_EVIDENCE/int.log" "$@"
SH
chmod +x "$COOP_EVIDENCE/qemu-int"
python3 tools/qemu_boot.py --profile coop --kernel "$COOP_EVIDENCE/kernel.elf" --qemu "$COOP_EVIDENCE/qemu-int" 2>&1 | tee "$COOP_EVIDENCE/runner.log"
```

```text
JNOX M1 BOOT
JNOX M1 READY
COOP A FIRST
COOP IRQ B READY
COOP A RETAINED
COOP B FIRST
COOP A RESUMED
COOP B WOKEN
COOP B RESUMED
EVENT 1
EVENT 2
EVENT 3
COOP PASS
```

위 13개 줄이 LF 또는 CRLF로 끝나고, 0.1초 동안 추가 출력이 없으며 호스트가 정상 종료하는지 확인한다. 같은 실행의 ELF 해시, 인수, PID, UART, 예외 로그를 보관한다. IRQ 1회와 SVC 5회를 보관한 ELF 주소에 대응시키고 인터럽트 당시 PC와 복귀 PC, 태스크 문맥, 깨우는 대상, 큐 연산, 핸들러 복귀 경로를 확인한다. UART 출력만으로는 이 전이를 입증할 수 없다. 호스트 관찰기는 줄 누락, 순서 변경, 추가 출력을 각각 거부해야 한다. EVD-012는 이 게스트의 기능 동작을 다루며 실물 보드 실행과 시간 상한은 별도 검증 대상이다.
