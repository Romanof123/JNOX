# M1 RM·DM 주기 작업 릴리스

한국어 · [English](m1-periodic.en.md)

CHG-009는 POL-004의 구현 IMP-007에 대한 합격 조건과 독립 실행 기록 EVD-007을 정의한다.

| 추적 항목 | 합격 조건 |
| --- | --- |
| REQ-023 / DES-012 / VER-023 | 한 게스트에서 RM(주기가 짧은 순), DM(상대 마감시간이 짧은 순)을 차례로 실행한다. 각 구간은 논리 틱 0~5이다. A의 주기와 상대 마감시간은 각각 2, 2이며 틱 0·2·4에 릴리스한다. B는 각각 3, 1이며 틱 0·3에 릴리스한다. 정적 우선순위는 같다. 두 구간 모두 틱 0에 B를 먼저 큐에 넣는다. 실제 실행 순서는 RM에서 A가 먼저이고 DM에서 B가 먼저이다. |
| REQ-002 / DES-003 / VER-002 | 게스트 물리 타이머의 연속된 절대 만료 시점에서 발생하는 IRQ 12회가 릴리스 없는 틱까지 진행한다. DM 틱 5에서 타이머를 정지한다. |
| REQ-005 / DES-003 / VER-005 | 각 작업은 해당 태스크의 전용 스택에서 진입 표식을 출력하고 SVC #1으로 완료한다. 실행 가능한 작업이 모두 끝나면 컨트롤러는 인터럽트 당시 PC에서 재개한다. |

작업 본문은 유한하며 완료까지 IRQ를 마스킹한다. 작업이 다음 준비 시각까지 실행되면 하드웨어 시계를 검사해 실패로 처리한다. 실행 중인 작업을 새로 준비된 작업이 선점하는 사례는 별도 검증 항목이다.

## 독립 관찰

새 검증자는 소스, 역어셈블리, 예외 로그를 열기 전에 리비전과 첫 실행 판정을 기록한다. 실패한 시도도 기록한다. 저장소 루트의 WSL Bash에서 아래 명령을 한 번 실행한다. 벤치마크 기능을 함께 지정하면 CBS, periodic, 기존 벤치마크 순으로 우선한다. periodic 관찰기의 기본값은 단일 코어 Cortex-A72, GICv2, TCG, 128 MiB 게스트 1회이다.

```sh
set -e -o pipefail
(cd kernel && cargo build --release --locked --features periodic-bench -j1)
export PERIODIC_EVIDENCE=$(mktemp -d)
cp kernel/target/aarch64-unknown-none/release/jnox-kernel "$PERIODIC_EVIDENCE/kernel.elf"
git rev-parse HEAD > "$PERIODIC_EVIDENCE/revision"
cat > "$PERIODIC_EVIDENCE/qemu-int" <<'SH'
#!/bin/sh
exec qemu-system-aarch64 -d int -D "$PERIODIC_EVIDENCE/int.log" "$@"
SH
chmod +x "$PERIODIC_EVIDENCE/qemu-int"
python3 tools/qemu_boot.py --profile periodic --kernel "$PERIODIC_EVIDENCE/kernel.elf" --qemu "$PERIODIC_EVIDENCE/qemu-int" 2>&1 | tee "$PERIODIC_EVIDENCE/runner.log"
```

```text
JNOX M1 BOOT
JNOX M1 READY
RM 0 A
RM 0 B
RM 2 A
RM 3 B
RM 4 A
DM 0 B
DM 0 A
DM 2 A
DM 3 B
DM 4 A
PERIODIC PASS
```

위 13개 줄의 LF 또는 CRLF 종결, 0.1초의 무출력 구간, 호스트의 정상 종료를 확인한다. 같은 게스트의 실행 로그와 ELF, 예외 로그를 보관한다. 첫 판정을 기록한 뒤 IRQ 12회와 완료 SVC #1 10회를 해당 ELF 주소에 대응시킨다. 예외 기록과 링크된 코드로 태스크 진입, 스택 선택, 컨트롤러 복귀 PC, 타이머 정지, 릴리스 생성과 실행 시점의 순위 결정을 확인한다. 호스트에서만 릴리스 한 줄 누락, 틱 0 순서 뒤바뀜, 끝에 바이트 추가를 각각 주입하면 관찰기가 모두 거부해야 한다. 이 계약의 범위는 QEMU에서의 반복 릴리스와 디스패치 순서이다. 보드 타이밍과 분석적 수용성 판정은 별도 범위이다.
