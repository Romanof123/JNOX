# Ordinary Rust mutex waiters after a timed donation / 시간 제한 상속 후 남은 뮤텍스 대기자

A producer can finish a protected sensor batch after an urgent reader times out, and two equal-priority reporters can update the report in their actual arrival order. 긴급 판독 요청이 만료된 뒤에도 생산자는 보호된 센서 배치를 완료하고, 같은 우선순위의 보고 작업 둘은 실제 도착 순서대로 보고서를 갱신한다.

Prospective single-CPU QEMU contract on accepted main 179b72e4a8709d2d0e88d0574923800d30dc95d1. CHG-109 / IMP-109 / EVD-109; REQ-002/005/023 → DES-003/012 → VER-002/005/023. [#284](https://github.com/Romanof123/JNOX/issues/284) · [#49](https://github.com/Romanof123/JNOX/issues/49) · [#21](https://github.com/Romanof123/JNOX/issues/21).

## Tasks, locks and admission / 작업·잠금·실제 도착 순서

Use two PI locks, X for publishing the report and Y for producing the sensor batch. Retain the four priority levels of the [timed chain](ordinary-mutex-chain.md). The two reporters take LOW's priority so that they tie on X after withdrawal and the existing MEDIUM work can proceed. The separate [FIFO workload](ordinary-mutex-fifo-plan.en-ko.md) keeps its original priorities.

| Worker / 작업 | Base / 기본 | Ordinary operation / 일반 작업 |
| --- | ---: | --- |
| BASE | 1 | Own Y, complete the original 4 MiB protected read and release Y / Y를 잡고 기존 4 MiB 보호 읽기를 마친 뒤 해제 |
| LOW | 2 | Own X, initialize report seed 4, wait on Y, publish BASE's verified count and sum, release Y then X / X 소유·초깃값 4 설정, Y 대기, 배치 게시 후 역순 해제 |
| A, B | 2, 2 | Acquire X, append digits 1 and 2 respectively under X, release and exit / X 획득 후 각자 1 또는 2를 뒤에 붙이고 해제·종료 |
| MEDIUM | 3 | Remain ready during donation, then perform the existing independent sum / 상속 중 준비 상태를 유지하고 이후 기존 독립 합계 계산 |
| HIGH | 4 | Timed acquire of X at the original 1 ms comparator, receive timeout and exit / 기존 1 ms 기한으로 X를 요청하고 시간 초과로 복귀·종료 |

Initially only BASE runs. After BASE owns Y, admit LOW. With X held by LOW, admit one reporter and voluntarily place LOW behind it, using the existing FIFO activation scheduling point. Let that reporter actually block in acquire(X) before activating the next. In epoch one the arrivals on X are A, B, HIGH. In epoch two they are B, A, HIGH. After both reporters block, LOW requests Y and blocks behind BASE. Admit HIGH and MEDIUM as in #236. HIGH's timed attempt on X raises LOW and BASE to 4 while MEDIUM stays ready. Neither reporter unlocks or exits during this setup. Finish the first epoch before starting the reversed epoch with fresh stacks, frames, locks, report and timer state.

BASE만 실행을 시작한다. BASE가 Y를 잡으면 LOW를 활성화한다. LOW가 X를 소유한 상태에서 보고 작업 하나를 활성화하고 같은 우선순위의 LOW는 준비 큐 뒤로 양보한다. 그 작업이 실제로 X 획득에서 차단된 뒤 다음 보고 작업을 활성화한다. 첫 구간의 X 도착 순서는 A·B·HIGH, 둘째는 B·A·HIGH다. 두 보고 작업이 차단된 뒤 LOW가 Y에서 차단되면 HIGH와 MEDIUM을 기존 연쇄 작업처럼 활성화한다. HIGH의 X 대기로 LOW와 BASE가 모두 4를 상속하고 MEDIUM은 준비 상태에 남는다. 첫 구간의 모든 작업을 끝낸 뒤 새 스택·프레임·잠금·보고서·타이머로 둘째 구간을 시작한다.

## Observable decisions / 관측할 판정

Use the shared LockSet acquire, timeout(HIGH), release and effective priorities, stable ReadyQueue, ordinary Rust SVC frames and assembly return. On the actual PPI30 require BASE inside the original useful work with 0 < completed chunks < 16,384. Remove only HIGH. Show the surviving X waiters in unchanged order, LOW still waiting on Y, and both owners' effective priorities falling 4→2 before selection. HIGH's saved acquire must return timeout 1 without a protected write. MEDIUM completes its independent sum 2,176. BASE returns to the interrupted PC, finishes 4,194,304 reads and sum 35,651,584, then directly hands Y to LOW. LOW resumes its saved acquire with success 0 and publishes that batch under X. At each X release obtain the grantee from the lock policy. Distinguish ownership transfer from the later dispatch and saved acquire return. Each reporter must return 0 on its own stack before reading and writing under X. With two successful appends in each epoch, require the actual writer values 4→41→412 and 4→42→421. Preserve stack/frame identity, IRQ and return PCs, comparator and counter, wait/ready order, effective priorities, ownership and worker-produced values for reconstruction. The existing task identifiers and output formats of the comparison profiles remain specific to their own runs.

공유 LockSet의 획득·timeout(HIGH)·해제·유효 우선순위와 안정 준비 큐, 일반 Rust 작업의 SVC 프레임·어셈블리 복귀를 사용한다. 실제 PPI30 시점에 BASE가 보호 작업 중이고 16,384청크 중 일부만 완료했는지 확인한다. HIGH만 제거한 후 X에 남은 두 작업의 순서, Y에서 대기하는 LOW, 다음 선택 전 두 소유자의 4→2 복원을 확인한다. HIGH는 저장된 획득 호출에서 1을 받고 보호 데이터를 쓰지 않는다. MEDIUM의 합은 2,176이다. BASE는 중단 PC에서 재개해 4,194,304회 읽기와 합 35,651,584를 완료하고 LOW에 Y를 직접 양도한다. LOW는 자기 호출에서 0을 받고 X 보호 아래 배치를 게시한다. X 양도 대상은 실제 잠금 정책이 선택한다. 양도 시점과 이후 실행·복귀 시점을 구분해 각 작업이 자기 스택에서 0으로 복귀한 뒤 쓴 중간값과 최종값 4→41→412, 4→42→421을 확인한다. 원본에서 프레임, IRQ·복귀 PC, 비교값·관측 카운터, 대기·준비 순서, 유효 우선순위, 소유권과 실제 계산 결과를 대조한다.

## First positions and historical controls / 최초 순번 및 기존 대조

Freeze the exact combined source and release ELFs before any scored launch. Use one Cortex-A72 on QEMU virt/GICv2, 128 MiB and the ordinary clock. One new opt-in workload runs both epochs in each first plain and externally traced cold boot. Then run the eight [accepted comparison positions](evidence/evd-110.en.md) in order: chain, FIFO, original PI and original ceiling, each plain then traced. Preserve each original criterion: chain mid-work 1 ms expiry and protected batch, FIFO both arrival orders per boot, PI and ceiling 100 µs comparators and original protected-work qualification. Keep all first captures including failures. Keep the prior combined chain 2/2 and original controls 2/4, with the two traced reason-16 failures, separate from the later accepted 8/8. Retain existing host/selector checks. #49 retains physical maximum blocking and #21 retains product acceptance.

최초 실행 전 통합 소스와 릴리스 ELF를 고정한다. 단일 Cortex-A72 QEMU virt/GICv2, 128 MiB, 기본 시계에서 새 선택 작업의 최초 일반·외부 추적 콜드 부팅 각각에 두 도착 순서를 모두 실행한다. 이후 기존 연쇄·FIFO·PI·천장 순으로 각각 일반과 추적을 실행해 [기존 비교 8회](evidence/evd-110.md)를 보존한다. 연쇄의 작업 중 1 ms 만료와 배치 결과, FIFO의 두 순서, 기존 PI·천장의 100 µs 및 보호 작업 판정을 각각 유지한다. 실패한 최초 원본도 보존한다. 앞선 통합의 연쇄 2/2와 기존 대조 2/4(추적 2건 사유 16), 이후 통과한 8/8은 별도 집단이다. 기존 호스트·선택자 검사를 유지한다. 실물 최대 차단 시간은 #49, 제품 승인은 #21에서 판정한다.

## Build and run / 빌드 및 실행

From the repository root on Linux or WSL, select the two-epoch workload with the ordinary mutex profile. Linux 또는 WSL의 저장소 루트에서 아래와 같이 두 도착 순서를 실행하는 일반 뮤텍스 작업을 선택한다.

~~~sh
run=$(mktemp -d)
CARGO_TARGET_DIR="$run" JNOX_BOARD=virt JNOX_PROFILE=mutex JNOX_APP=ordinary-survivors \
    python3 tools/build_kernel.py --arch aarch64
elf="$run/aarch64-unknown-none/release/jnox-kernel"
qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -accel tcg \
    -m 128M -nodefaults -display none -monitor none -serial stdio -kernel "$elf"
~~~

For an externally traced boot of the same release ELF, add `-d int -D "$run/exceptions.log"` to the QEMU command. 같은 릴리스 ELF의 외부 예외 추적은 QEMU 명령에 해당 옵션을 추가해 기록한다.
