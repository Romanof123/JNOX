# Native absolute monotonic sleep / 네이티브 단조 시계 절대 대기

Design CHG-113 · Implementation reservation IMP-113 · Evidence reservation EVD-113. Single-process AArch64 native ABI v1, one awake CPU. These identifiers reserve future work. 구현과 실행 증거는 후속 작업이다.

## Starting point / 출발점

At main `47461714`, the [system time plan](system-time-plan.en.md) calls for absolute sleep, but the ordinary SDK and kernel have no `clock_nanosleep`. On `46a87fb7`, `kernel/src/native_el0.rs` dispatches exit (1) and monotonic read (6). Entry masks interrupts and `native_el0_vectors.S` routes only lower-A64 synchronous exceptions to the native handler. Keep the ELF launcher, startup stack, process exit and read-clock ABI. This slice advances #21 for ordinary programs.

Held #178 (`663b598e`, `sdk/native-aarch64/clock.md`) supplies the call-7 contract. Its monitor runs roles A/B with weights 1/2, a mapped completion word at `0x01200000` and three 40 ms waits by B while A works. The ordinary caller has a private deadline and may be the only runnable application. Reuse the ABI and applicable timer/context primitives. The held review retains its own disposition.

기준 main에는 계획만 있고 일반 앱의 대기 호출은 없다. 시계 브랜치의 일반 진입점은 IRQ를 마스킹하며 읽기와 종료만 처리한다. #178의 호출 규약을 재사용하되 일반 앱은 역할, 가중치, 공유 완료 주소, 고정 횟수 없이 자신의 기한을 지정한다. #178의 보류 판정은 독립적이다.

## Caller contract / 호출 규약

C exposes `int clock_nanosleep(clockid_t clock, int flags, const struct timespec *request, struct timespec *remaining)` in `<time.h>`. Use `CLOCK_MONOTONIC` (1) and `TIMER_ABSTIME` (1). Native `svc #0x4a01` takes x8=7, x0=clock, x1=flags, x2=request VA, x3=0 for this mode. Kernel x0 is zero or negative errno. C returns zero or positive errno directly, preserves `errno` and leaves `remaining` untouched. Rust exposes safe `sleep_until(deadline: Duration) -> Result<(), ClockError>` alongside `monotonic_time()`. Convert seconds with checked signed LP64 conversion and map a positive C result directly to `ClockError::Errno(code)`.

Check clock and flags first (`EINVAL`, 22). Copy the readable 16-byte request once (`EFAULT`, 14), decode two signed LP64 values and require nonnegative seconds and nanoseconds in `[0, 10^9)` (`EINVAL`). Use call 6’s captured epoch and counter frequency. For seconds s, nanoseconds n and frequency f, compute `epoch + s*f + ceil(n*f/10^9)` in wide checked arithmetic. Overflow of the representable compare returns `EOVERFLOW` (75), including an unrepresentable Rust argument. Expose these error names in the SDK. Validate before changing task state. An already-due valid target succeeds immediately. A later return is permitted.

C는 오류 번호를 직접 반환하고 errno와 remaining을 보존한다. Rust는 같은 시간축의 Duration을 받는다. 시계·플래그 오류는 EINVAL, 읽을 수 없는 요청은 EFAULT, 잘못된 시각 필드는 EINVAL, 표현 범위 초과는 EOVERFLOW다. 6번 호출의 내림 읽기와 짝을 이루도록 절대 비교값을 올림한다. 이미 지난 기한은 즉시 성공한다.

## Block, idle, resume / 차단·유휴·복귀

1. With local IRQ delivery serialized, compare the physical counter with the converted target. For a future target retain the actual SVC `NativeFrame` on task-owned storage or a pinned kernel stack. Record the task’s address-space and kernel resume context. Change Running → BlockedUntil(target), remove eligibility and program the earliest pending physical compare (`CNTP_CVAL_EL0`). Recheck due work before idle so a target crossed during setup completes normally.
2. Schedule ready work or switch to a separate EL1 idle context. Route the current-EL SPx IRQ vector for idle and any enabled lower-EL IRQ through the port. Enable physical timer PPI 30 and use an IRQ-safe `WFI` idle protocol that handles a pending interrupt before sleeping. Share the port’s interrupt acknowledge, compare, EOI and scheduler selection logic wherever frame ownership agrees. No counter polling in userspace or EL1.
3. At PPI 30 read the counter, keep future waits blocked, and move each due task to Ready once. Clear its wait record, disarm or reprogram the compare and complete the interrupt. Schedule the waiter, restore its address space and native frame, set the saved x0 result to zero and `eret` to the saved post-SVC ELR. Preserve SP_EL0, SPSR, other GPRs and FP/SIMD context. Each subsequent wait takes a fresh request and can reuse that task’s wait slot.

미래 기한이면 실제 SVC 프레임과 커널 복귀 문맥을 보관하고 실행 가능 목록에서 태스크를 뺀다. 비교 타이머를 설정한 뒤 실행할 앱이 없으면 독립 EL1 유휴 스택에서 WFI로 대기한다. PPI 30 처리 시 실제 카운터가 기한에 도달한 태스크만 한 번 준비 상태로 옮긴다. 대기 슬롯을 비우고 스케줄러가 원래 스택·레지스터·주소 공간을 복원해 SVC 다음 명령으로 복귀한다.

## Application acceptance / 앱 인수 조건

Build separate ordinary Rust and C mains for a periodic reporting loop. Read the initial monotonic time, advance the previous target by one reporting period each round, sleep, read the clock, and add the completed round to a stack-local report total. At exit verify the total and each observed return time ≥ its target. Run several rounds on each language image with no peer. Include the just-completed target again to exercise immediate success and then wait on a new future target. In C include an invalid nanosecond field and verify positive EINVAL and preserved errno and remaining. In Rust include a Duration beyond signed seconds and verify the overflow error.

For the future waits retain evidence of the task leaving the runnable set, EL1 entering idle, PPI 30 observing the due counter and the same task continuing its loop through ordinary exit. For the past target verify no blocking transition. Use the model’s running counter and recorded targets. This establishes functional ordering without a wake-latency budget.

Rust와 C 일반 프로그램 각각에서 이전 기한에 주기를 더하며 보고 횟수를 누적한다. 미래 기한의 반복 대기, 직전 기한 재사용, 다음 미래 기한 대기 및 각 언어의 오류 처리를 확인한다. 앱의 기한·복귀 시각과 정상 종료, 커널의 차단·유휴·타이머·재개 기록을 대응시킨다.
