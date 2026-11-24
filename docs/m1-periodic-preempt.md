# M1 RM·DM 실행 중 타이머 선점

한국어 · [English](m1-periodic-preempt.en.md)

CHG-011은 POL-004를 확장한다. IMP-009는 EVD-009의 합격 조건을 정의한다.

| 추적 항목 | 합격 조건 |
| --- | --- |
| REQ-023 / DES-012 / VER-023 | 기존 IRQ 12회와 작업 진입 10회를 유지한다. 각 3틱인 RM 구간과 DM 구간을 순서대로 추가한다. 틱 0에는 A(주기 2, 상대 마감 2)만 실행한다. 틱 1에는 중단된 A를 먼저, 새로 도착한 B(주기 3, 상대 마감 1)를 다음으로 큐에 넣는다. 정적 우선순위는 같다. RM은 A를 즉시 재개하고 A가 끝나면 B를 실행한다. DM은 B를 끝낸 뒤 A를 재개한다. |
| REQ-002 / DES-003 / VER-002 | 두 추가 구간의 틱 1에서 게스트 타이머 IRQ가 자발적으로 양보하지 않고 실행 중인 A를 중단한다. DM 틱 2에서 타이머를 정지한다. 전체 게스트 실행에서 타이머 IRQ는 18회다. |
| REQ-005 / DES-003 / VER-005 | 각 작업은 전용 스택을 쓴다. A는 중단 당시의 레지스터 값과 작업 진행 상태를 보존한 채 정확히 중단된 PC에서 재개한다. 두 작업 모두 각 추가 구간에서 SVC #1로 완료한다. 추가 완료는 4회, 전체 완료는 14회다. |

## 독립 관찰

periodic-preempt-bench로 빌드하면 periodic-bench도 활성화된다. 여러 기능을 켠 경우의 선택 순서는 [스케줄링 카탈로그](scheduling-policies.md)에 있다. WSL Bash의 저장소 루트에서 리비전과 ELF를 보관하고 구현이나 추적 로그를 열기 전에 첫 블랙박스 판정을 실패까지 포함해 기록한다. 기본 관찰 단위는 새 단일 코어 Cortex-A72 게스트 한 대다.

```sh
set -e -o pipefail
(cd kernel && cargo build --release --locked --features periodic-preempt-bench -j1)
export PREEMPT_EVIDENCE=$(mktemp -d)
cp kernel/target/aarch64-unknown-none/release/jnox-kernel "$PREEMPT_EVIDENCE/kernel.elf"
git rev-parse HEAD > "$PREEMPT_EVIDENCE/revision"
cat > "$PREEMPT_EVIDENCE/qemu-int" <<'SH'
#!/bin/sh
exec qemu-system-aarch64 -d int -D "$PREEMPT_EVIDENCE/int.log" "$@"
SH
chmod +x "$PREEMPT_EVIDENCE/qemu-int"
python3 tools/qemu_boot.py --profile periodic-preempt --kernel "$PREEMPT_EVIDENCE/kernel.elf" --qemu "$PREEMPT_EVIDENCE/qemu-int" 2>&1 | tee "$PREEMPT_EVIDENCE/runner.log"
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
RM A START
RM A CONTINUE
RM B RUN
DM A START
DM B RUN
DM A CONTINUE
PREEMPT PASS
```

위 줄 전체가 순서대로 일치하고(LF 또는 CRLF), 0.1초 동안 추가 출력이 없으며 호스트가 게스트를 정상 정지해야 한다. 첫 판정을 기록한 뒤 같은 게스트의 IRQ 18회와 SVC #1 예외 14회를 보관한 ELF 주소에 대응시킨다. 두 틱 1의 중단 PC와 예외 복귀 위치, 작업별 스택, 레지스터·진행 상태 검사, 실행 시 정책 선택, 완료 순서, 마지막 타이머 정지를 확인한다. 호스트에서 재개 행 누락, DM 순서 역전, 끝에 바이트 추가를 각각 관찰기에 입력해 모두 실패하는지 확인한다. 이 판정은 QEMU에서 관찰한 기능 동작에 적용된다. 실기 지연 시간과 일반적인 스케줄 가능성에는 별도 증거가 필요하다.

[EVD-009](evidence/evd-009.md)
