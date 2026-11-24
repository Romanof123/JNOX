# M1 단일 코어 고정 우선순위 선점 벤치

한국어 · [English](m1-scheduler.en.md)

검증팀은 동일한 부팅의 UART와 예외 주소를 대조했다. [EVD-002 검증 결과](evidence/evd-002.md)

`CHG-002`는 부팅 기준선 `d17052a`에 선택형 Cargo 기능 `sched-bench`를 추가한다. `IMP-002`의 설계 질문은 한 가지다. 낮은 우선순위 스레드(LOW)가 CPU를 계속 사용하는 동안 타이머 IRQ로 높은 우선순위 스레드(HIGH)가 준비되면, HIGH가 먼저 실행을 마치고 LOW가 중단된 계산을 이어 가는가? 기본 빌드의 부팅 회귀 판정은 기존 두 줄 UART 계약을 따른다. [M1 부팅 벤치](m1-boot.md)

| 추적 | 이번 벤치에서 관찰할 동작 |
| --- | --- |
| `REQ-002` → `DES-003` → `IMP-002` · `VER-002` | 타이머가 LOW 실행을 중단하고, 두 스레드가 모두 실행 가능할 때 스케줄러가 고정 우선순위에 따라 HIGH를 선택한다. |
| `REQ-005` → `DES-003` → `IMP-002` · `VER-005` | 같은 주소 공간에서 각 스레드가 자기 스택으로 실행되고, HIGH가 종료되면 LOW의 저장 문맥으로 복귀한다. |

[시스템 요구사항](requirements.md) · [스케줄러 설계](architecture.md) · [검증 계획](verification-plan.md)

## 실행 순서와 관찰 근거

QEMU `virt`의 TCG, GICv2, Cortex-A72 CPU 1개, RAM 128 MiB를 사용한다. 게스트 커널 ELF를 직접 기동하고 첫 UART를 수집한다. 고정 우선순위가 서로 다른 두 커널 스레드는 정적으로 준비할 수 있다. 처음에는 LOW만 실행 가능하고 HIGH는 타이머를 기다린다.

1. **LOW가 CPU를 점유한다.** LOW는 자기 스택에서 시작해 `SCHED LOW RUN`을 출력한 뒤 CPU를 양보하지 않는 작업 구간에 들어간다. 이 구간에서는 스케줄러 호출, 대기 요청, HIGH 직접 호출 없이 계산을 계속한다. IRQ 발생 시점은 이 구간에 속해야 한다.
2. **IRQ가 HIGH를 준비 상태로 만든다.** 예외 벡터는 LOW의 재개에 필요한 일반 레지스터, 스택 포인터, 복귀 PC와 프로세서 상태를 보존한다. IRQ 경로는 GIC에서 예상한 타이머 인터럽트를 식별한 후 `SCHED TIMER IRQ`를 출력한다. 타이머의 추가 발생을 막고 GIC에 처리를 완료한다. LOW도 실행 가능한 상태에서 스케줄러가 더 높은 우선순위의 HIGH를 선택하며, 예외 복귀로 HIGH의 문맥에 진입한다.
3. **HIGH가 자기 스택에서 실행을 마친다.** HIGH의 스레드 본문에서 `SCHED HIGH RUN`을 출력한다. HIGH의 작업이 완료되면 실행 대상에서 제외하고 스케줄러가 LOW의 저장 문맥을 복원한다.
4. **LOW가 중단된 계산을 이어 간다.** LOW는 복원된 PC와 스택에서 실행을 재개한다. HIGH의 완료와 LOW가 중단 전에 남긴 상태가 보존됐음을 확인한 뒤 LOW에서 `SCHED LOW RESUMED`를 출력한다. `SCHED PASS`는 이 전이를 모두 확인한 후에만 허용한다.

독립 검토자는 출력 위치를 위 동작과 대조한다. IRQ 복귀 주소가 LOW의 비양보 구간 안에 있었는지, 선택 시 두 스레드가 실행 가능했고 우선순위 비교로 HIGH가 선택됐는지 확인한다. 두 스택의 범위가 겹치지 않는지, HIGH가 자기 스택에서 실행됐는지, LOW의 원래 스택과 중단 전 실행 상태가 복원됐는지 확인한다. 후보에서 사용할 수 있는 근거로는 예외 프레임과 스택 경계 검사, 중단 전후에 읽는 스택 지역값 또는 레지스터 값, 그리고 이 검사들이 실패하면 합격 출력을 차단하는 제어 흐름이 있다. IRQ 처리기가 직접 HIGH 본문을 호출하거나 LOW를 처음부터 다시 시작해 같은 문자열을 출력하는 구현은 실패다.

## 빌드와 UART 판정

저장소 루트에서 고정된 Rust 도구 체인과 `aarch64-unknown-none` 타깃, Python 3, `qemu-system-aarch64`를 사용한다. 기본 실행 단위는 QEMU 게스트 1개이며 반복 기동은 순차로 수행한다.

```sh
cd kernel
cargo build --release --locked --features sched-bench -j 1
cd ..
python3 tools/qemu_boot.py --profile scheduler --kernel kernel/target/aarch64-unknown-none/release/jnox-kernel
```

한 번의 정상 기동에서 게스트 UART 전체 바이트열은 다음 일곱 줄이어야 한다. 각 줄은 LF 또는 CRLF로 끝난다. `READY`는 기존 Rust 부팅 검사가 완료된 지점을 뜻한다.

```text
JNOX M1 BOOT
JNOX M1 READY
SCHED LOW RUN
SCHED TIMER IRQ
SCHED HIGH RUN
SCHED LOW RESUMED
SCHED PASS
```

누락, 역순, 중복, 추가 바이트, 마지막 줄 종결자 누락, 호스트 시한 초과, 관찰 구간 중 게스트 조기 종료는 실패다. 검증자는 UART 원문, 별도로 수집한 에뮬레이터 진단, 종료 사유, 소스 리비전, 설정과 ELF SHA-256을 기록한다. 실패 검출 시험에는 `EVD-001`에 보존한 기본 부팅 UART와 이번 기록에서 복귀 줄을 뺀 입력을 사용한다. 호스트에서 두 입력을 스케줄러 판정기에 재생해 한 번의 게스트 부팅으로 여러 조건을 확인한다. [증거 양식](templates/verification-evidence.md)

## 독립 판정과 범위

새 검증자가 공개 계약과 식별된 후보를 받아 최초 블랙박스 판정과 원시 로그를 보존한 뒤 구현과 구조적 근거를 검토한다. UART 순서 및 문맥 전환 근거를 모두 확인해야 이 선점 사례를 합격으로 기록할 수 있다. 근거가 부족하면 미확인 항목과 그 이유를 남긴다. [개발 수명주기](lifecycle.md)

이 벤치는 단일 코어에서 한 차례의 선점과 복귀에 대해 `VER-002`와 `VER-005`의 부분 근거를 정의한다. 같은 우선순위 FIFO, 일반적인 스레드 수명주기, SMP, 예산 초과 대응에는 별도 시험이 필요하다. 호스트 시한은 시험의 종료 조건이다. IRQ 지연, 최악 응답 시간, 실보드 기한 준수와 제품 안전 승인은 각각 별도 근거가 필요하다.
