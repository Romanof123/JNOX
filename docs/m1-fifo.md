# M1 단일 코어 FIFO 스케줄링 벤치

한국어 · [English](m1-fifo.en.md)

`CHG-003`은 상위 저장소 기준선 `715b02f`에서 `IMP-003`의 합격 조건을 정한다. 우선순위가 같은 두 스레드가 한 번의 타이머 인터럽트에서 준비 상태가 된다. 이 벤치는 어느 스레드가 먼저 실행되는지, CPU를 양보한 스레드가 준비 큐의 어느 위치로 돌아가는지, 선점된 낮은 우선순위 스레드가 언제 재개되는지 확인한다. [선점 기준선](m1-scheduler.md)

| 추적 | 합격 조건 |
| --- | --- |
| `REQ-002` → `DES-003` → `IMP-003` → `VER-002` | 타이머가 LOW를 선점한다. A와 B의 고정 우선순위는 서로 같고 LOW보다 높다. 스케줄러는 준비된 스레드 중 우선순위가 가장 높은 스레드를 선택하며, 우선순위가 같으면 준비 큐에 들어온 순서를 따른다. |
| `REQ-005` → `DES-003` → `IMP-003` → `VER-005` | LOW, A, B는 같은 주소 공간에서 서로 다른 스택을 사용한다. A와 B는 각자 양보한 지점에서 재개하고 한 번씩 종료해 선택 대상에서 빠진다. 이어 LOW가 중단된 작업을 계속한다. |

[요구사항](requirements.md) · [스케줄러 설계](architecture.md) · [검증 계획](verification-plan.md)

## 게스트 실행 순서

1. LOW가 단독으로 실행을 시작해 CPU를 양보하지 않는 작업 구간에 들어간다. 실제 타이머 IRQ가 그 구간을 중단한다. 처리기는 GICv2에서 타이머 인터럽트를 식별하고, 다음 스레드를 선택하기 전에 A와 B 순서로 둘 다 준비 상태로 만든 뒤 인터럽트 처리를 완료한다. **A의 숫자형 스레드 ID는 B보다 크므로** ID 오름차순으로 선택하면 B가 먼저 나온다.
2. 먼저 실행된 A가 양보해 B 뒤의 큐 꼬리로 들어간다. 이어 B가 실행되고 양보해 A 뒤의 큐 꼬리로 들어간다. 두 스레드는 각각 양보한 뒤에도 실행 가능한 상태를 유지한다. A가 양보 지점에서 재개해 종료하고, B도 양보 지점에서 재개해 종료한다. 이 동안 LOW는 계속 실행 가능하다.
3. A와 B가 모두 종료하면 LOW가 중단된 계산을 이어 간다. LOW는 저장해 둔 상태를 확인한 뒤 성공을 출력한다.

## 빌드와 판정

해당 기능을 한 번 빌드하고 그 ELF로 QEMU 게스트를 한 번 기동한다. 구성은 `virt`, GICv2, TCG, Cortex-A72 CPU 1개, RAM 128 MiB다. 고정된 Rust 도구 체인, `aarch64-unknown-none` 타깃, Python 3와 `qemu-system-aarch64`를 사용해 저장소 루트에서 다음 명령을 실행한다.

```sh
cd kernel
cargo build --release --locked --features fifo-bench -j 1
cd ..
python3 tools/qemu_boot.py --profile fifo --kernel kernel/target/aarch64-unknown-none/release/jnox-kernel
```

전체 UART 바이트열은 정확히 아래 열 줄이어야 하며 각 줄은 LF 또는 CRLF로 끝난다. 마지막 출력 뒤 무출력 관찰 구간을 거쳐 호스트가 종료할 때까지 게스트가 실행 중이어야 한다.

```text
JNOX M1 BOOT
JNOX M1 READY
SCHED LOW RUN
SCHED TIMER IRQ
FIFO A FIRST
FIFO B FIRST
FIFO A RESUMED
FIFO B RESUMED
SCHED LOW RESUMED
SCHED PASS
```

독립 검증자는 최초 UART 판정, 원시 바이트, 에뮬레이터 진단, 종료 사유, 후보 리비전과 ELF SHA-256을 보존한다. 첫 실행 두 줄의 순서를 바꾼 입력과 재개 줄을 누락한 입력 등은 호스트에서 재생해 실패 검출을 확인한다. 바이트 누락·중복·순서 변경·추가, 마지막 줄 종결자 누락, 시한 초과, 조기 종료는 실패다. [증거 양식](templates/verification-evidence.md)

동일한 ELF와 소스로 IRQ 복귀 PC가 LOW의 작업 구간 안에 있는지, 준비 큐가 우선순위와 삽입 순서를 지키는지, 세 스택의 범위가 분리되는지, 각 양보 때 저장된 복귀 위치에서 실행이 재개되는지 확인한다. 종료한 스레드가 선택 대상에서 빠지는지, LOW가 선점 전부터 보존한 상태를 검사하는지도 확인한다. 합격에는 UART 순서와 이러한 구조적 근거가 모두 필요하다. 이 사례는 `VER-002`와 `VER-005`의 단일 코어 부분 근거다. SMP, 타임 슬라이스, 실보드의 시간 상한에는 별도 근거가 필요하다.
