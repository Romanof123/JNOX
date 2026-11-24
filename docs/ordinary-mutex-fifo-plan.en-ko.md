# Ordinary Rust FIFO handoff between equal-priority mutex waiters

CHG-105 · IMP-105 · EVD-105 · REQ-002/005/023 · DES-003/012 · VER-002/005/023. [#275](https://github.com/Romanof123/JNOX/issues/275) implements the arrival-order clause of [#49](https://github.com/Romanof123/JNOX/issues/49) under [#21](https://github.com/Romanof123/JNOX/issues/21). PM publishes this prospective contract before implementation. Development implements the profile, Integration freezes the actual combined source and binaries, and a separate verifier owns the first launches and independent replay.

## Workload and dispatch

Start from main fb3f4e04e5ba68c4bb9c517036abd9e4fd9d37ea. The main mutex route in kernel/build.rs and kernel/src/main.rs selects the ordinary Rust workload. Its acquire/release path uses LockSet, the stable ready queue, saved SVC frames, private stacks and assembly restore. Reuse these mechanisms. The held [#236](https://github.com/Romanof123/JNOX/pull/236) adds a distinct timed-chain application on its own branch. This case requires no chain source or timing change.

Use one new opt-in application, JNOX_BOARD=virt, JNOX_PROFILE=mutex, JNOX_APP=ordinary-fifo. Build release/locked with the repository toolchain. Use a single Cortex-A72 on QEMU virt/GICv2, 128 MiB, default clock. One ELF runs both epochs. Each epoch starts with fresh locks, ready queues, task frames, counters and protected data. Finish all workers in the first epoch before creating the second. Preserve the first epoch’s observations. Only admission order changes.

| Fixed input | Value |
| --- | --- |
| Task IDs and base priorities | Owner 0 at 2, A 1 at 7, B 2 at 7, idle 3 at 0 |
| Resource | X, PI, declared users 0/1/2, maximum nesting one |
| Initial state | X free, no waiters, only owner runnable among the workers, protected journal count and value zero |
| Work | Owner acquires X and initializes the journal with seed 4. A appends digit 1 and B digit 2. Each reads the previous count/value under X, writes value = 10 × previous + digit and increments count. |
| Epochs | First admit A then B, second B then A. Expected per-writer before/after values are 4→41→412 and 4→42→421, counts 0→1→2 both times. |

The owner opens one worker at a time via an explicit activation scheduling point. Enqueue the newly runnable worker first and the voluntarily yielding owner at the tail. After the first block the owner resumes at inherited priority 7 and activates the second peer by the same rule. Each worker executes an ordinary Rust acquire, appends only after its saved acquire returns successfully and unlocks before exiting. Owner unlocks only after both have attempted X and blocked. The runtime may validate setup but must select the grantee from the shared lock policy using actual acquire history. Admission code cannot supply a winner to unlock. Record handoff and actual execution separately since equal-priority dispatch can return first to an unlocking worker.

## Independent decision

Freeze the source, ELF/hash, map or symbols, argv, emulator/toolchain versions, public selector and expected predicates before launching. Preserve the first plain cold boot and the first separate external exception-traced cold boot of the same ELF, in that order. Use a 15-second wall timeout per FIFO boot, require terminal output and all records, then stop after 100 ms silence. The timeout bounds collection only. Score both epochs separately in each boot; all four must pass. Retain incomplete evidence, timeout, overflow, wrong ownership, stale epoch or early execution as a failed position. Never replace a first capture.

For each epoch independently replay the two blocked acquires on X while task 0 owns it, with both workers at 7, both saved frames blocked and waiting, and neither worker runnable or having written. Verify donation 2→7 on first block and restoration to 2 on owner release. Verify owner→first arrival→second arrival→free, with the remaining waiter still blocked after the first handoff. Correlate each returned acquire's zero result, saved/selected frame addresses, stack bounds, return PC and actual post-return worker observation, then reconcile ordered protected reads/writes, completion of all three workers, empty wait set and restored priorities. Retain raw transitions with epoch/task IDs, ready/wait masks and priority snapshots, handoff targets, restore records, worker observations, UART, process outcome and external exception log where enabled. Match exception returns to ELF addresses in the traced run. A terminal success line alone cannot establish the result.

Use the existing host mutex tests plus one reversed-arrival check at the shared release boundary if existing tests do not cover both orders with fixed IDs. Against frozen raw records, require the independent checker to reject a fixed-ID winner in the reversed epoch, an acquire return before grant, and an incorrect intermediate protected value even with the expected final value. Retain these as constructed controls. Reject the FIFO selector on Pi 4 and reject an unknown mutex app before launch. On the same joint source run the previous ordinary-pi and ordinary-ceiling profiles once plain and once externally traced, with their original compare, protected-work and scoring rules. Report these four first control outcomes separately and hold integration on any miss. Preserve all historical cohorts. Physical blocking, product limits, chain timeout integration and policy-family release remain with their respective owners.

## 한국어 계약

PM은 구현 전에 이 계약을 공개한다. 개발은 일반 Rust 작업과 선택자를 구현하고 통합은 실제 결합 소스와 ELF를 고정한다. 독립 검증자가 최초 실행과 원본 재생을 맡는다. 본선의 기존 LockSet·준비 큐·SVC 프레임과 복귀 경로를 사용한다. #236의 시간 만료 연쇄 분기는 의존성에 포함하지 않는다.

단일 Cortex-A72 virt/GICv2, 128 MiB, 기본 클록에서 같은 ordinary-fifo ELF로 두 구간을 순서대로 실행한다. 각 구간에서 락·작업 프레임·준비 큐·데이터를 초기화한다. 소유자 0의 기본 우선순위는 2, A(1)와 B(2)는 7, 유휴 작업 3은 0이다. X는 사용자 0/1/2만 허용하는 단일 PI 락이다. 소유자만 준비된 상태에서 X를 획득하고 값 4와 개수 0을 기록한다. 소유자는 첫 구간에서 A와 B, 다음 구간에서 B와 A를 차례로 활성화한다. 활성화 시 새 작업을 먼저 준비시키고 소유자는 자발적으로 큐 뒤에 선다. 각 작업은 X 획득을 호출해 차단된다. 첫 차단 후 소유자는 7을 상속한다. 두 작업 모두 대기하고 보호 쓰기를 하지 않은 상태에서 소유자가 X를 해제한다.

실제 락 정책이 첫 도착자에게 소유권을 넘기고 소유자의 우선순위를 2로 복구해야 한다. 다음 대기자는 첫 도착자가 해제할 때까지 차단된다. 실제 저장 프레임의 획득 호출이 0으로 돌아온 뒤 A는 숫자 1, B는 2를 보호 값 뒤에 붙인다. 중간값과 최종값은 각각 4→41→412, 4→42→421이고 개수는 둘 다 0→1→2이다. 소유권 이전과 작업 실행은 별도로 기록한다. 작업 세 개의 완료, 빈 락·대기 집합, 기본 우선순위 복원을 확인한다.

실행 전에 소스·ELF 해시·심볼·명령·도구 버전·판정식을 고정한다. 동일 ELF의 일반 최초 부팅 후 외부 예외 추적 최초 부팅을 실시한다. FIFO 실행당 호스트 제한은 15초, 종료 출력 후 무출력 100 ms이다. 각 부팅의 두 구간 모두 통과해야 한다. 원본 UART, 종료 상태, 상태 전이, 마스크·우선순위·소유권, 스택·프레임·복귀 PC, 실제 작업별 쓰기와 외부 예외 기록을 보존한다. 누락·초과·미완료·조기 쓰기는 해당 위치 실패로 기록한다. 인위적 음성 대조에서 역순 구간의 고정 ID 양도, 양도 전 획득 복귀, 최종값만 맞는 중간값 변조를 각각 거부해야 한다.

기존 호스트 뮤텍스 시험을 유지하고 양방향 고정 ID 조건이 빠져 있으면 한 시험을 추가한다. Pi 4 선택 및 알 수 없는 앱 이름은 부팅 전에 거부한다. 결합 소스에서 기존 PI·천장 앱을 각각 일반·추적 최초 1회씩 원래 비교 시점과 보호 연산 기준으로 판정한다. 네 대조 중 미달이 있으면 통합을 보류한다. 이전 결과, 실보드 최대 차단 시간, 제품 한도 및 정책군 출시는 별도 추적한다.
