# CHG-054 · Ordinary Rust reservations / 일반 Rust 예약 작업

[#123](https://github.com/Romanof123/JNOX/issues/123) · [#21](https://github.com/Romanof123/JNOX/issues/21) · IMP-053 · [EVD-053](evidence/evd-053.en.md) · [한국어](evidence/evd-053.md)

## English

**Configuration.** One Cortex-A72 on QEMU virt/GICv2/TCG, 128 MiB, default clock. Use absolute E+tS, S=50 ms, CNTFRQ divisible by 1000. One EDF arbitration per slot; ascending ID breaks equal absolute deadlines, idle if none eligible. A=0, B=1. Each grant debits one whole slot entitlement; an early SVC forfeits its tail to idle. No reclaim. Record actual counter time separately. [Aligned reservation implementation](../kernel/src/service_policy.rs) · [CBS and exact admission](../kernel/src/temporal_policy.rs) · [EVD-006](evidence/evd-006.en.md) · [EVD-029](evidence/evd-029.en.md)

| Manifest | Inputs and horizon | Required independent observations |
| --- | --- | --- |
| W, wake and excess demand | A=(Q=2,P=8, offset 0), B=(1,4,offset 1). A completes one-grant bursts at first service for releases 0,4,5,13,22, then demands 100 grants from 23. B demands 100 from 1. Observe slots [0,34). | Five real block SVCs and five resumes of A on its saved stack. Releasing pending work at 5 may leave A throttled until 8; a blocked client cannot run before release. B receives one grant in **each** complete 4-slot period from 1 despite excess A demand; require positive worker progress at each grant. Unspent tail or exhausted servers select idle. |
| S, exactly saturated | Two clients initially ready, A=B=(1,2,offset 0); four granted units each, no new arrivals. Score [0,10). | At 0–7 owners alternate A,B. Each final unit performs an actual exit SVC (A at 6, B at 7), slots 8–9 idle, no redispatch of exited frame. Both complete even when A continuously consumes its permitted windows. |
| R, refused | Before creating ready contexts, add C=(1,8) to S: total 9/8. Also zero/invalid Q,P and unrepresentable checked admission. | Explicit reason; no operational compare armed or worker frame dispatched, no worker progress or partial admission. Independently verify ≤1 accepted and >1 rejected with exact rational arithmetic. |

Freeze both server types for W and S. The aligned model replaces q with Q at start+kP even while blocked, has deadline next aligned boundary and never accumulates missed credits. B's alignment starts at 1 in W. Wake inside a period retains its current remainder. Hard CBS replenishes an active depleted server at d to (Q,d+P); on inactive wake at t renew to (Q,t+P) if t≥d or qP>Q(d−t), otherwise retain including equality and q=0. Process releases/eligibility and due credits before EDF and debit. In W/CBS specifically expect equality at 4, zero at 5, excessive density at 13, expiry at 22 and lower density at 23. B grants are 1,5,9,13,17,21,25,29,33 in CBS; compute aligned owners separately (at 13 aligned A has deadline 16, before B at 17). Independently enumerate period boundaries, release, ready/blocked/exited, residual q and d before/after each grant, with duplicate grant and exit-after-exit controls. Do not apply the CBS wake test to aligned reservations.

**Physical execution within the virtual slot.** Use normal linked Rust workers on distinct EL1 stacks with live register, stack-local accumulator and iteration counter. Long grants perform wrapping x=3x+1 in bounded observable chunks until asynchronous cutoff; short bursts perform 256 updates then block/exit. In S use the long path for the first three grants per owner and the finite exit body on its fourth. A resumes at the saved PC after grant expiry or the post-SVC continuation after blocking. B cannot update A's progress. Confirm actual runnable identity and progress via linked PC ranges, stack/frame, independent exception return records and recomputed checksums. No loop that advances a slot on behalf of a worker.

Arm the cutoff for one millisecond **before** the nominal end of each full grant, then leave the tail idle; release the next owner only at its absolute boundary. This reserves a simulator timing margin while preserving the full-slot **entitlement debit**. Preserve actual start/stop brackets, cutoff target/IRQ entry, next dispatch and interrupted PC on the same architectural counter. Qualify a normal run only with cutoff lateness ≤1 ms **and** latest possible outgoing execution ≤ its nominal end, first eligible dispatch within 1 ms of slot opening, ≥1 actual update per grant, elapsed worker upper bound ≤S per grant and ≤Q·S per allocation. Count IRQ/scheduler/idle and uncertainty separately; never erase elapsed execution after the programmed cutoff. A delayed or masked IRQ cannot guarantee a physical instant stop. Independently inject bounded masking in a long grant through nominal end+2 ms; account observed excess to outgoing owner, inhibit further grants at first available trap and record lost neighbor opportunity and actual stop. This fault run requires a nonqualifying normal timing verdict; no replenishment/replay can repair it. Equal/one-tick-over controls exercise both cutoff bounds, plus incorrect owner, missing return, counter discontinuity, synthetic grant without work and dropped overrun. Use conservative brackets; ambiguous limits do not qualify. Admission at 100% is a result for integer entitlement units. Setup, guards, kernel and IRQ overhead reduce the execution actually available; no 100%-CPU product service promise follows.

**Cold sequence.** Freeze toolchain, source/ELF hashes, build flags, QEMU/options, S, trace capacity and case manifest. Keep ELF, stdout, traces and metadata on native Linux /tmp until exit. For each policy run W and S once plain and once with external IRQ/SVC/ERET tracing, counterbalance order (8 boots). Add one over-capacity and one masked-cutoff boot per policy (4), total 12, 15 s each. Keep the first attempt including prelaunch/timeout. Record absolute counter events in bounded RAM in both arms with terminal dump; the plain arm supplies internal return samples, the traced arm independently identifies actual exception returns. Compare independently scored verdicts and distributions per arm; do not transfer an observed return or latency between images/runs. Host controls cover malformed admissions and oracle edges. All normal grants, throttling, wakes, exits, refusal and isolation cases must match before claiming this virtual profile. REQ-002/005/023 → DES-003/012 → VER-002/005/023. Product admission requires the actual task model, qualified execution/interference bounds and board evidence. No product WCET, memory isolation or physical timing claim is assigned here.

## 한국어

단일 Cortex-A72 QEMU virt/GICv2/TCG, 128 MiB, 기본 시계에서 절대 E+tS, S=50 ms(F는 1000의 배수)를 사용한다. 실제 Rust 작업 A(0)·B(1)를 슬롯마다 EDF로 선택하고 동률이면 낮은 ID를 택한다. 실행 자격은 한 슬롯씩 차감하며 조기 SVC 뒤 남은 시간은 유휴다. 실측 시간은 별도 집계한다. [EVD-006](evidence/evd-006.md) · [EVD-029](evidence/evd-029.md)

- **W**: A=(2,8,위상 0), B=(1,4,위상 1). A는 0·4·5·13·22에 한 단위씩, 23에 100단위, B는 1에 100단위를 요청한다. [0,34)에서 A의 실제 SVC 차단 5회와 재개 5회, B의 완전한 4슬롯 구간별 한 번의 실행 및 양의 진행을 요구한다. CBS의 B 실행은 1·5·9·13·17·21·25·29·33이다. 정렬형 예약은 별도로 계산한다.
- **S**: 두 작업 모두 (1,2,위상 0)로 전체 수용률 1이다. 각 4번 실행하고 [0,10)을 관측한다. 0~7은 A/B 교대, 6·7에 각각 실제 종료 SVC, 8·9는 유휴다. 재실행된 종료 프레임은 실패다. **R**은 S에 (1,8)을 추가해 9/8로 시작 전 거부한다. 잘못된 Q·P와 산술 범위 초과도 거부한다. 운영 타이머·작업 디스패치·부분 수용이 없어야 한다.
- 정렬형은 최초 위상+kP에서 q를 Q로 교체하고 마감은 다음 정렬 경계다. 차단 중에도 이 규칙을 유지하고 이월·차용하지 않는다. CBS는 활성 고갈 시 d에 (Q,d+P)로 보충한다. 비활성 작업의 t 재개 시 t≥d 또는 qP>Q(d−t)이면 (Q,t+P)로 갱신하고 그 외에는 0과 등호도 그대로 유지한다. W에서 4의 등호, 5의 잔여 0, 13의 높은 밀도, 22의 만료와 23의 낮은 밀도를 확인한다. 독립 오라클은 릴리스·보충 후 EDF 선택과 차감, 매 슬롯 q/d, 준비·차단·종료 및 유휴를 재구성한다.
- 독립 EL1 스택, 저장 레지스터, 스택 누산기·반복 수를 확인한다. 긴 실행은 순환 x=3x+1을 관측 가능한 유한 묶음으로 반복하고 IRQ로 중단한다. 짧은 버스트는 256회 갱신 후 차단/종료한다. S는 각 첫 세 번 긴 실행, 네 번째 유한 종료다. 링크된 PC, 스택·프레임, 실제 예외 복귀와 체크섬을 대조한다.
- 정상 긴 창 종료 1 ms 전에 타이머를 설정해 남은 구간을 유휴로 두고 다음 소유자는 절대 경계부터 실행한다. 차감은 1슬롯 그대로다. 동일 카운터로 마감 설정, IRQ 진입, 실제 중단·시작 범위와 중단 PC를 기록한다. 컷오프 지연 ≤1 ms, 실제 중단 상한 ≤명목 창 끝, 적격 디스패치 ≤창 시작 후 1 ms, 매 실행 양의 진행, 실행량 상한은 슬롯별 S·할당별 Q·S여야 한다. 커널·IRQ·유휴·불확실성과 실제 초과 실행을 따로 집계한다. 별도 마스킹 결함으로 명목 경계+2 ms까지 진행시켜 실제 과금을 유지하고 첫 트랩에서 이후 실행을 억제한다. 이웃의 잃은 기회와 실측 중단을 남기고 정상 판정 실패를 요구한다. 한계·한 틱 초과, 잘못된 소유자·누락 복귀·불연속 시계·가짜 진행·초과 누락을 검사한다. 불확실한 경계는 합격시키지 않는다.
- 양 정책의 W/S를 각각 평상·외부 예외 추적으로 한 번씩 교차 순서로 실행(8회)하고 정책별 과부하 거부·컷오프 마스킹 각 1회(4회)를 더해 총 12회, 회당 15초다. ELF·기록·메타데이터를 Linux /tmp에 두고 첫 시도와 시간초과도 보존한다. 양쪽 RAM 기록은 같게 유지하고 외부 IRQ/SVC/ERET는 해당 실행에만 귀속한다. 가상 자격 수용률 1은 실제 CPU 전체 시간 보장이 아니다. 제품 수용·WCET·보드 간섭·격리에는 별도 근거가 필요하다.
## Simulator fault injection and arithmetic / 시뮬레이터 주입 및 산술

Only the separately built simulator diagnostic fixture may delay delivery of the already-programmed cutoff IRQ. Ordinary clients expose no mask, delay or injection operation. Preserve the nominal slot end, original cutoff compare value and pending interrupt; record the fixture’s delay interval, actual trap and outgoing worker’s observed execution through enforcement. Do not reprogram the compare to the injection endpoint. Separate any fixture CPU overhead from the client and retain the conservative outgoing-service bracket. At the actual late trap fail-stop further grants, preserve the original ledger and explicitly record excess and the next owner’s lost opportunity. Qualify normal operation only in the unmodified default-clock guest arms. The diagnostic guest demonstrates late enforcement and fail-stop. Host arithmetic and corrupted-trace controls establish oracle behavior only.

Before arming the first timer or enabling a client, use checked arithmetic to validate the epoch through the entire observation horizon, all absolute targets/cutoffs, every required replenishment and scheduling deadline (including a deadline beyond the final observed boundary), and worst representable cumulative elapsed/charged counters for the declared run, including diagnostic lateness. On every live accumulation check both monotonic subtraction and addition before committing the ledger. Overflow, wrap, an unrepresentable deadline or unbounded observation interval fails admission or, if encountered live, freezes the last valid ledger and fail-stops. Host controls include exactly representable maxima and one unit beyond each horizon, deadline and cumulative-accounting limit.

이미 설정한 컷오프 IRQ의 전달 지연은 별도 시뮬레이터 진단 구성에서만 제어한다. 일반 클라이언트에는 마스킹·지연·결함 주입 연산이 없다. 명목 슬롯 끝, 원래 비교 레지스터 값과 대기 IRQ를 유지하고 진단 지연 구간, 실제 트랩과 중단까지의 나가는 작업 실행량을 기록한다. 비교값을 주입 종료 시각으로 바꾸지 않는다. 진단 코드 자체 비용을 분리하고 불확실하면 나가는 작업의 보수적 실행 범위를 남긴다. 늦은 트랩에서 실제 초과와 다음 소유자의 서비스 손실을 원장에 보존하고 후속 할당을 중단한다. 정상 적격 판정은 수정 없는 기본 시계 게스트 실행으로만 수행한다. 진단 게스트는 늦은 집행과 실패 정지를 관측하고, 호스트의 산술·기록 변조 대조는 판정기 동작만 검증한다.

첫 타이머 설정·작업 활성화 전에 기준 시각부터 관측 끝까지, 모든 절대 목표·컷오프, 관측 끝을 넘는 마지막 마감까지 포함한 보충·스케줄링 마감, 진단 지연을 포함한 누적 경과·과금의 최대 필요값을 검사 산술로 검증한다. 실행 중에도 단조 시각 차와 각 누적 덧셈을 원장 변경 전에 확인한다. 오버플로·순환·표현 불가능한 마감·상한 없는 관측 구간은 시작 전 거부하며 실행 중 검출 시 마지막 유효 원장을 보존하고 정지한다. 호스트 대조군은 관측 끝·마감·누적량 각각의 표현 가능 최댓값과 한 단위 초과를 검사한다.

## IMP-053 build and capture / 빌드 및 기록

From the repository's `kernel` directory, choose one application for each build. Use a fresh native Linux capture directory for each cold attempt. The board selector is `virt`, the profile is `reservation`, and the application is the policy and case joined by a hyphen. Build without Cargo benchmark features. Both plain and externally traced runs use the same copied ELF for a given application. The first independent boot results are recorded in EVD-053 v1.

저장소의 `kernel` 디렉터리에서 벤치마크 Cargo 기능 없이 애플리케이션 하나씩 빌드한다. 매 콜드 실행에는 Linux 기본 파일시스템의 새 디렉터리를 사용한다. 같은 애플리케이션의 일반·외부 예외 추적 실행은 해시가 같은 ELF를 쓴다. 최초 독립 실행 기록은 EVD-053 v1에 있다.

| Policy / 정책 | Application values / 애플리케이션 값 |
| --- | --- |
| Aligned | `aligned-wake`, `aligned-saturated`, `aligned-refused`, `aligned-late` |
| Hard CBS | `cbs-wake`, `cbs-saturated`, `cbs-refused`, `cbs-late` |

~~~bash
export JNOX_BOARD=virt JNOX_PROFILE=reservation JNOX_APP=aligned-wake
run_dir=$(mktemp -d /tmp/jnox-reservation.XXXXXXXX)
cargo build --release --locked &&
  cp target/aarch64-unknown-none/release/jnox-kernel "$run_dir/kernel.elf" || exit 1
sha256sum "$run_dir/kernel.elf" > "$run_dir/kernel.sha256"
qemu=(qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1
      -accel tcg -m 128M -nodefaults -display none -monitor none -serial stdio
      -kernel "$run_dir/kernel.elf")
# Traced arm only / 외부 추적 실행에서만:
# qemu+=(-d int -D "$run_dir/exceptions.log")
printf '%q ' "${qemu[@]}" > "$run_dir/argv"
timeout 15s "${qemu[@]}" > "$run_dir/uart" 2> "$run_dir/stderr"
printf '%s\n' "$?" > "$run_dir/host-status"
~~~

Keep first-launch errors, incomplete records, host status, QEMU and toolchain versions with each attempt. A parked guest may outlive its terminal record; independently check completeness, exception attribution and the frozen case verdict. The two late applications select only the simulator fault fixture. Run refused cases separately from live service. [Requirements and design](requirements.en.md) · [Verification](verification-plan.en.md)

최초 실행 오류·불완전 기록·호스트 종료 코드·QEMU 및 도구 버전을 함께 보관한다. 게스트가 최종 기록 뒤 대기할 수 있어 기록 완결성·예외 귀속·동결한 사례 판정을 독립 확인한다. late 두 구성은 시뮬레이터 결함 주입용이며 refused는 실제 서비스와 분리한다. [요구사항](requirements.md) · [검증 계획](verification-plan.md)

### Capture fields / 캡처 필드

`RSRV RAW 1` opens the decimal-u64 stream and `RSRV END` closes it. Each row is `tag tick slot id a b c d`; A/B/idle IDs are 0/1/2; an unavailable slot uses u64::MAX (reserved zero fields are literal zeros). Preserve stream order, including equal ticks. The pre-ERET sample precedes the final restore instructions. Only the separately captured exception stream witnesses actual ERET. 이 형식은 같은 틱에서도 원래 순서를 보존한다. 복귀 직전 게스트 표본과 실제 ERET의 외부 관측을 별도로 기록한다.

| Tag | Event / 사건 | a, b, c, d |
| --- | --- | --- |
| 1 | Configuration / 구성 | frequency, slot ticks, policy (aligned=0/CBS=1), case (wake=0/saturated=1/refused=2/late=3) |
| 2 | Epoch / 기준 | epoch, horizon, 0, 0 |
| 3 | Arm / 타이머 설정 | target, CTL readback, CVAL readback, kind (boundary=0/cutoff=1) |
| 4 | IRQ | target, kind, IAR, CTL |
| 5 | Allocation / 할당 | q before, d before, q after, d after |
| 6 | Demand / 수요 | released, remaining, pre-service state, selected action (none=0/long=1/block=2/exit=3) |
| 7 | Selection / 선택 | nominal start, nominal end, frame, ELR |
| 8 | Stop / 중단 | initial updates, final updates, initial accumulator, final accumulator |
| 9 | Frame / 프레임 | frame address, ELR, x19, TPIDR |
| 10 | Service bounds / 실행 범위 | lower start, first worker observation, upper stop, conservative elapsed upper (counter ticks) |
| 11 | Trap / 트랩 | action, syndrome, stack-local address, saved SP after frame |
| 12 | Return sample / 복귀 표본 | frame, ELR, raw return index, 0 |
| 13 | Result / 결과 | status (complete=0/refused=1/timing violation=2/invalid=3), reason, argument, argument |
| 14 | Lost service / 서비스 손실 | next nominal boundary, actual stop, lost neighbor, nominal excess |
| 15 | Refusal / 거부 | input (9/8=0/Q=0=1/Q>P=2/arithmetic overflow=3/P=0=4), reason, frames, operational arms |
| 16 | Fixture delay / 결함 주입 | sample after mask, sample before unmask, programmed cutoff, nominal slot end |
| 17 | Stack / 스택 | base, top, seed, 0 |
| 18 | Expected return / 예상 복귀 | frame, ELR, expected x19, expected TPIDR |
| 19 | Allocation service / 할당별 시간 상한 | allocation deadline (logical slot), cumulative elapsed upper (ticks), pre-ERET frame, pre-ERET PC |
| 20 | Diagnostic progress / 진단 진행 | first hook tick at/after nominal end, committed count then, pre-unmask tick, committed count then |
| 21 | Horizon snapshot / 종료 경계 상태 | committed count, accumulator, private progress address, 0 |

All eight columns are unsigned decimal 64-bit integers, including IDs, addresses and logical slots. The tick column and raw time samples are CNTPCT ticks. Row 1 provides Hz and slot length in ticks. Row 5 q/d and row 19 deadline are logical units; row 6 carries released/remaining work in grant units. Row 6 state codes 0–4 are unreleased, ready, saved, blocked, exited. Row 15 reasons 1–4 mean invalid, over capacity, overflow, unexpected. Row 17 seeds are 0xa19/0xb19/0xc19 for A/B/idle. Distinguish result status 2 reason 1 (late stop or overrun), 3 reason 10 (insufficient diagnostic mask/progress evidence), and 3 reason 8 (client state changed since its last trap). Decode other result reasons at the emitting branch of the [runtime](../kernel/src/ordinary_reservations.rs).

**Replay.** One entitlement unit is debited per grant. For its separate elapsed upper use row 10, U = upper_stop − lower_start, from the assembly sample before ERET through the sampled exception vector entry. Check lower_start ≤ first_worker_observation ≤ upper_stop; the first observation is a worker generation marker, not the first executed instruction. Sum U by client and allocation deadline with checked arithmetic and compare with row 19, S per grant and Q·S per allocation. U includes restore/entry overhead and any diagnostic hook inside the interval. It is a conservative bound on elapsed client service; it cannot isolate CPU cycles spent on arithmetic. Kernel validation after the vector sample is outside U.

Workers publish count and wrapping u64 accumulator (x=3x+1) together through alternating private stack cells after each 16-update chunk. Check row 8 differences are positive multiples of 16, independently advance each seed, and require exactly 256 committed updates for each short block/exit. Long grants interrupt ordinary work asynchronously. IRQ never fabricates a completed chunk. Join row 9 saved state, row 11 trap and row 18 planned return to independent IRQ/SVC/ERET, linked PCs and stack ranges. Block is SVC #1, exit SVC #2; subsequent idle/neighbor rows must leave an exited client’s count and accumulator unchanged. Row 21 reports each client encountered at the terminal horizon; early failure can truncate the pair. Verify both clients’ unchanged progress at boundaries from available evidence, distinguishing guest checks from externally observable events.

Only late S slot 0 instruments A’s ordinary loop. The hook samples once per chunk, masks near cutoff−1 ms, and unmasks no earlier than nominal end+2 ms. Row 20 witnesses committed updates during the overrun; require its second count to exceed its first, and correlate rows 14/16/19 with actual enforcement. Row 16 end precedes DAIF unmask. Neither timestamp is the instant the CPU stopped; use the vector sample for the upper stop bracket. Lost B opportunity is recorded separately from the outgoing A bound. The fixed RAM capacities are 1,024 eight-word rows and 64 three-word assembly return samples. Capacity overflow, missing markers, partial rows, unexplained returns or an invalid diagnostic outcome cannot qualify. Row 12 samples are emitted at terminal dump in return-index order with their original timestamps; join them to dispatch/row 18 by return index before comparing chronology. The same ELF is used in plain and external-trace arms. A complete stream ending in status 0 is still subject to independent replay.

**한국어 재현.** 한 줄은 십진 u64 여덟 열이며 A/B/유휴 ID는 0/1/2, 해당 슬롯 없음은 u64::MAX, 예약 필드 0은 실제 0이다. 카운터 틱은 사건 1의 주파수로 환산한다. 사건 5의 q/d와 19의 마감은 논리 단위다. 사건 6의 상태 0~4는 미릴리스·준비·저장·차단·종료, 사건 15의 사유 1~4는 잘못된 값·용량 초과·오버플로·예상 외 상태다. 19는 해당 할당 마감·누적 시간 상한·복귀 전 프레임·PC, 20은 명목 끝 이후 최초 후크 틱/완료 수와 마스크 해제 직전 틱/완료 수, 21은 종료 경계별 완료 수·누산기·전용 주소를 담는다.

할당 자격은 선택마다 한 단위를 차감한다. 별도로 사건 10에서 U=벡터 진입 표본−ERET 전 표본을 계산하고 작업·할당 마감별 누적합을 사건 19와 대조한다. 두 표본 사이에 실제 작업의 첫 세대 표식이 있어야 한다. U에는 복귀·진입 비용과 진단 후크가 포함된다. 순수 계산 CPU 시간으로 해석할 수 없다. 벡터 표본 이후 검사 비용은 U 밖이다. A/B는 0xa19/0xb19로 시작해 u64 순환식 x=3x+1을 16회씩 수행하고 두 스택 셀을 번갈아 진행 수와 누산기를 확정한다. 사건 8의 증가는 16의 양의 배수여야 하며 짧은 차단·종료는 정확히 256회다. 사건 9/11/18, 외부 예외·링크된 PC·스택을 대조한다. SVC #1은 차단, #2는 종료다. 종료 뒤 진척이 바뀌면 실패한다. 최종 경계의 두 작업을 사건 21로 대조하고 조기 실패 시 누락한 종료 행을 합격시키지 않는다.

late 전용 S의 0번 슬롯에서는 A의 실제 작업이 계속 진행한다. 매 묶음의 후크는 컷오프 1 ms 전부터 마스킹하고 명목 끝+2 ms 이후 해제한다. 사건 16의 두 번째 시각은 해제 직전이며 실제 중단 시각이 아니다. 사건 20의 두 완료 수 증가와 14/16/19 및 벡터 표본으로 실제 초과와 B의 기회 손실을 확인한다. 상태/사유 2/1은 시간 위반, 3/10은 마스킹·진척 증거 부족, 3/8은 경계에서 무단 상태 변화를 뜻한다. RAM은 8워드 행 1,024개와 복귀 표본 64개로 제한한다. 사건 12는 종료 시 원래 시각을 유지해 복귀 인덱스순으로 출력하므로 18의 순서와 연결한다. 오버플로·누락·설명되지 않는 복귀는 실패다. 일반 실행의 ERET 전 표본과 같은 ELF 추적 실행의 실제 ERET는 해당 실행에만 귀속한다.

CHG-056 · [First-opening measurements and conditional preparation / 최초 개방 계측 및 조건부 준비](reservation-opening-plan.md) · [#123](https://github.com/Romanof123/JNOX/issues/123)

### CHG-056 source revision of IMP-053 / IMP-053 소스 수정

Use the same build selectors, capture commands, eight-column decimal format and 1 ms opening limit. Record the exact ELF/source revision because the stream header is shared with v1. In a revised normal W/S run, join the following records to the existing boundary, selection and worker observations; reject absence of a required record on a claimed completed path. Preserve partial traces on failure.

| Tag | Header tick, slot, ID / 시각·슬롯·ID | a, b, c, d |
| --- | --- | --- |
| 24 | Post-admission sample, unavailable slot, idle / 수용 후·슬롯 없음·유휴 | first runtime sample, epoch E, 0, 0 |
| 22 | End of private preparation, slot 0, idle / 비공개 준비 끝·0·유휴 | setup start, E, max(E−end,0), 0 |
| 23 | After authoritative commit, slot 0, chosen owner / 실제 커밋 직후·0·선택 소유자 | actual boundary-vector entry, sample before commit, E, new generation 1 |

R terminates before all three records. The admitted late fixture emits 24 and uses synchronous opening without 22/23. In the normal candidate the prep bracket excludes outer mask/return instructions; report those unknown edges. Replay boot-sample→first worker using 24.a and 10.b, preserve the true E, and keep full outgoing charge and actual return/neighbor checks. CHG-056 affects REQ-002/005/023, which link to the existing IMP-053. EVD-053 v1 scored the earlier c2efde4 only: 8/8 ordinary runs stopped in slot 0, 2/2 rejected overload before start and 2/2 late runs first failed opening. [EVD-053 v2](evidence/evd-053-v2.en.md) records the matched study. [Original results](evidence/evd-053-v1.en.md) · [Repair design](reservation-opening-plan.md)

동일 빌드 선택자·캡처 명령·8열 십진 형식·1 ms 한계를 사용한다. 스트림 표지가 v1과 같으므로 ELF와 소스 신원을 보존한다. 정상 W/S는 24.a 최초 런타임 표본부터 10.b 실제 작업 표식까지 계산하고, 22의 내부 준비 구간 밖 마스킹·복귀 비용 및 23의 실제 커밋 구간을 구분한다. R 거부는 세 기록 전 종료한다. 기존 late 진단은 24만 추가하고 최초 동기식 판정을 유지한다. CHG-056은 REQ-002/005/023을 통해 기존 IMP-053에 연결된다. v1의 8/8 정상 첫 슬롯 실패, 2/2 시작 전 거부, 2/2 지연 사례의 선행 개방 실패는 c2efde4에서 나온 결과다. [EVD-053 v2](evidence/evd-053-v2.md)에 비교 실행 판정이 있다. [최초 결과](evidence/evd-053-v1.md) · [수정 설계](reservation-opening-plan.md)

CHG-059 · [Compact first-boundary handoff / 첫 경계 인계 비용 축소](reservation-compact-handoff-plan.md) · [#123](https://github.com/Romanof123/JNOX/issues/123)

Four of eight ordinary candidate boots completed, compared with five of eight paired predecessor boots and one of eight historical predecessor boots. Both overload refusals in each paired cohort succeeded. Full-profile qualification remains on hold. [EVD-053 v3](evidence/evd-053-v3.en.md)

후보 일반 실행 8건 중 4건, 같은 실험의 선행 버전 8건 중 5건, 과거 선행 버전 8건 중 1건이 완료됐다. 후보와 동시 비교군은 각각 과부하 2건을 모두 거절했다. 전체 프로필 적격 판정은 보류한다. [EVD-053 v3](evidence/evd-053-v3.md)
