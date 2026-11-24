# CHG-057 · Ordinary lottery and stride clients / 일반 복권·스트라이드 클라이언트

[#54](https://github.com/Romanof123/JNOX/issues/54) · [#79](https://github.com/Romanof123/JNOX/issues/79) · [#21](https://github.com/Romanof123/JNOX/issues/21) · [CHG-023 contract](m4-proportional.en.md) · [한국어 기준](m4-proportional.md)

## English

**Existing evidence and remaining scope.** EVD-021 already ran the live policy and hardware timer against independently stacked assembly workers. Its original 80-row boot exposed 40 Hz stride timing and programmed-grant charging. The corrected image recorded 81 rows, 75 IRQs and six SVCs, including a deliberately late three-tick charge. EVD-035 preserved the controlled first failure; the historical four-IRQ failure remains unattributed. Reuse those mechanism records. CHG-057 plans an ordinary opt-in Rust application profile whose actual dispatch uses the same draw provider and service-charged stride under a frozen load. An independent verifier must distinguish executed source state from a prerecorded sequence. [EVD-021](evidence/evd-021.en.md) · [EVD-035](evidence/evd-035.en.md) · [Live policy](../kernel/src/proportional_policy.rs)

**Configuration.** Single Cortex-A72 QEMU virt, GICv2/TCG, 128 MiB, default virtual counter. Freeze compiler, source/ELF hash, QEMU version/argv and counter frequency F. Require exact F/100 and F/25 counter ticks. Use ordinary linked Rust A/B/C on private EL1 stacks, weights 1/2/4, IDs 0/1/2. Each maintains stack-local wrapping-u64 progress (x=3x+1; seed 1+ID), publishes completed chunks and resumes the interrupted computation. Require independently attributable positive progress for each grant and stable inactive clients. Freeze a single simulator ELF per workload configuration; separate explicit seeded lottery, stride and diagnostic launches. Keep the legacy benchmark, its selector precedence and its verdict separately identifiable.

**Lottery.** For this proposed release configuration the scheduler owns one persistent SplitMix64 state initialized exactly once per cold workload from the recorded configured seed. It calls the existing uniform-ticket path on each *actual* eligible dispatch. Neither a client, the replay script, a host-supplied winner list nor the timer chooses the draw. Declare this reproducible generator as the simulator draw source. No entropy or cryptographic property is claimed. Exercise the same code path with independent cold seeds 1 and 2, all clients continuously ready for 32 actual grants per seed at 100 grants/s (10 ms). Never reseed on IRQ or switch. Serialize eligibility changes before a draw. On empty eligible set dispatch idle without advancing the stream; reject zero total before modulo. For total T use cutoff = 2^64 mod T, discard raw words below cutoff, and use first accepted word mod T. At T=7 cutoff=2, so at most three words per dispatch because SplitMix64 permutes its state. Respect the existing bound T≤32×16 and cap attempts at cutoff+1. A mismatched state/iteration or impossible exhaustion invalidates the run; no fallback modulo, redraw to improve shares or replacement seed.

Record per decision preceding and resulting state, raw word count, accepted raw word, ready mask, tickets, reduced draw, chosen half-open interval and actual owner. Independently regenerate **every** word including discards, without importing the kernel implementation. Host-only boundary controls: for T=7 feed 0,1,2 to the reduction check (reject, reject, accept); first/last legal ticket 0/6 and adjacent interval edges select A:0, B:1–2, C:3–6. Reject 7 and zero total, exclude nonready weights. Label supplied-word controls separately from the autonomous guest source. Exact selected sequences must match replay; also enforce CHG-023 per-seed per-client absolute count error ≤8 grants relative to 32w/7 (expected counts 10/8/14 and 5/11/16). Separately replay independent host seeds 1–8, 4,096 draws each, ≤3 percentage points absolute deviation from w/7 for each client and seed. Count grants and measured service separately; no pooling or acceptance by a favorable seed.

**Stride.** At 25 residency ticks/s (40 ms), no draw or state advance. A yields by real SVC after one observed whole tick; B/C remain runnable for two ticks and timer preemption. With A/B ready expect A,B four times, service 4/8/0. Admit C once at the incumbent floor at the boundary between grants 8 and 9, with no fabricated prior service. Expect A,B,C,C twice, incremental service 2/4/8. Select min(pass,ID), record eligible floor and order before/after subtracting the common floor, clamp older sleeper debt as implemented; charge 720720/w × floor((actual sampled stop−start)/(F/25)) with checked arithmetic. Preserve raw partial-tick remainders, IRQ/return overhead and bounds: sampled residency is not an instruction-level CPU measurement. Require the original regular whole-tick counts, then separately hold a weight-one two-tick grant beyond the third tick under the opt-in simulator diagnostic. Charge all observed whole ticks (≥3); verify pass growth and preemption with PPI30. No ordinary client interface to suppress another client's timer.

**Independent verdict.** For each cold first launch use native Linux capture paths, retain prelaunch errors/timeouts and UART, raw rows, counter compare/start/stop, progress/stack, exact ELF and external IRQ/SVC/ERET PCs. Join actual return to the authorized task and saved continuation. Keep a bounded RAM trace with post-run output; any truncation, missing grant, overwritten failure or unaccounted residency is unqualified. Reuse #79 first-failure fields (source line, epoch, slot, actual, expected), with line zero meaning uncommitted. Qualify seed 1, seed 2, stride join and delayed-charge observations separately. An explicit first-lottery two-tick perturbation must fail the original one-tick test, retaining measured row and first predicate. Keep a naturally earlier failure primary even if replay later succeeds. Independently identify diagnostic vs ordinary images. Statistical coverage is finite and conditional on this source/load/window; physical board timing, interference and product scheduling qualification remain open. REQ-002/005/023 → DES-003/012 → VER-002/005/023. The first three ordinary runs are recorded in [EVD-055](evidence/evd-055.en.md).

## 한국어

기존 EVD-021은 실제 정책·타이머와 개별 스택의 어셈블리 작업을 실행했다. 최초 80행에서 스트라이드 40 Hz와 예정량 차감 문제를 발견했고, 수정 이미지의 81행·75 IRQ·6 SVC에서 지연된 3틱 전체 차감을 확인했다. EVD-035는 강제 실패의 첫 조건을 보존했으며 과거 네 번째 IRQ 뒤 실패의 원인은 아직 미확정이다. CHG-057은 기존 근거를 유지하고 일반 Rust 작업의 실제 경쟁과 선택된 난수원의 호출 경로를 검증할 계획이다. [EVD-021](evidence/evd-021.md) · [EVD-035](evidence/evd-035.md)

- **구성.** Cortex-A72 단일 코어 QEMU virt/GICv2/TCG, 128 MiB, 기본 시계와 정확한 F/100·F/25 틱을 사용한다. A/B/C의 ID는 0/1/2, 가중치는 1/2/4다. 전용 EL1 스택에서 seed=1+ID의 u64 순환식 x=3x+1과 완료 묶음을 기록한다. 실제 선택 작업의 양의 진행·중단 PC·재개와 비활성 작업 상태를 대조한다. 기존 선택자와 벤치 근거는 유지하고 일반·진단·시드별 이미지와 실행 명세를 분리한다.
- **복권.** 시뮬레이터 배포 구성에서 스케줄러가 실제 디스패치마다 기존 SplitMix64 상태를 사용한다. 콜드 시작 시 한 번만 시드를 설정하고 호스트·작업이 승자를 제공하지 않는다. 적격 작업이 없으면 유휴로 가고 RNG 상태를 유지한다. 총량 0은 나머지 연산 전에 거부한다. T 티켓에서 2^64 mod T 미만 원시값을 버린 뒤 첫 수용 값 mod T를 쓴다. T=7이면 0·1을 거부하고 최대 세 번 읽는다. 기존 T≤32×16, 시도≤cutoff+1을 지키고 예외 시 임의 대체나 재추첨 없이 실패한다. 재현 가능한 시뮬레이터 난수원으로 범위를 한정한다.
- **판정.** 시드 1·2를 각각 콜드 실행하고 세 작업이 계속 준비된 상태에서 100 grants/s(10 ms)로 32번 실제 부여한다. 상태 전후·소모 원시 단어 수·수용 값·준비 마스크·가중치·티켓 구간·실제 소유자를 남긴다. 독립 오라클이 거부값까지 재생한다. 호스트 공급값 대조에서 원시 0·1·2는 거부·거부·수용, 티켓 0=A, 1–2=B, 3–6=C, 7·총량 0은 거부다. 구현 공유 없이 순서 전체와 시드별 10/8/14 및 5/11/16회를 확인하고 각 작업의 32w/7 대비 오차 ≤8회를 적용한다. 호스트 시드 1~8은 각각 4,096회, 시드·작업별 w/7 대비 절대 오차 ≤3%p다. 부여 수와 실측 서비스를 분리한다.
- **스트라이드.** 초당 25서비스틱(40 ms)에서 A는 실제 1틱 후 SVC 양보, B/C는 2틱 IRQ 선점이다. A/B 네 쌍의 서비스 4/8/0 후 9번째 부여 전 C를 기존 패스 최솟값에 참여시키고 A,B,C,C 두 차례의 추가 2/4/8틱을 확인한다. 최소 (pass,ID)와 공통 최솟값 정규화 전후 순서 및 수면 작업의 낮은 패스 처리를 대조한다. 실제 표본 경과의 온전한 틱 수 ×720720/w를 차감하고 잔여 틱도 남긴다. 별도 진단에서 가중치 1의 2틱 부여를 3틱 이후까지 지연하고 모든 실제 온전한 틱(≥3)의 차감·PPI30·패스 증가를 검증한다. 이 계측은 명령별 CPU 시간이나 물리 지연 보장이 아니다.
- **독립 관측.** Linux 기본 저장소에 첫 부팅 실패·시간초과·UART·원시 행·비교값·진행·스택·ELF·IRQ/SVC/ERET를 보존하며 유실·덮어쓰기·잘린 기록은 부적격이다. #79의 최초 실패 필드(소스 행·epoch·슬롯·실제·기대값)를 유지한다. 소스 행 0은 미확정이다. 시드별 복권, 후발 참여, 지연 차감 결과를 각각 판정한다. 첫 복권 슬롯만 2틱으로 늘린 대조는 원래 1틱 기준에서 실패해야 한다. 자연 발생한 선행 실패도 그대로 남긴다. 실제 보드 간섭·제품 수용과 일반 작업의 독립 실행 근거는 미완료다.

## IMP-055 source candidate / 소스 후보

Use a native Linux checkout and target directory under /tmp. From kernel/, run each configuration in its own new directory. The five selectors are lottery1, lottery2, stride, lottery-miss and stride-delay. Each ELF embeds one configuration. The legacy assembly baseline uses unset profile/app and Cargo feature proportional-bench. Preserve the initial UART, QEMU exception log, launch arguments, QEMU version, toolchain, ELF digest and disassembly. Stop a parked guest only after the complete terminal record or the declared timeout. / Linux /tmp의 소스·빌드 경로에서 각 선택자를 새 디렉터리로 한 번씩 실행한다. 기존 어셈블리 비교군은 프로필·앱을 해제하고 proportional-bench 기능으로 빌드한다. 첫 UART·예외 로그·인수·버전·ELF·해시·역어셈블리를 남긴다. 종료 행 또는 정해 둔 시간 제한을 기록한다.

```sh
case_name=lottery1
out="/tmp/jnox-prop-$case_name"
mkdir "$out" || exit 1
CARGO_TARGET_DIR="$out/build" JNOX_PROFILE=proportional JNOX_APP="$case_name" cargo build --release || exit 1
cp "$out/build/aarch64-unknown-none/release/jnox-kernel" "$out/kernel.elf"
sha256sum "$out/kernel.elf" > "$out/sha256"
llvm-objdump -d "$out/kernel.elf" > "$out/disassembly"
qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -m 128M -accel tcg -nodefaults -display none -monitor none -serial "file:$out/uart" -d int -D "$out/exceptions" -kernel "$out/kernel.elf"
# Legacy comparison / 기존 비교군
env -u JNOX_PROFILE -u JNOX_APP cargo build --release --features proportional-bench
```

Leave `JNOX_CONTROL_CASE` unset for every proportional app. For the first integration gate, build `JNOX_PROFILE=proportional JNOX_APP=lottery1 JNOX_CONTROL_CASE=healthy cargo build --release` from `kernel/` on virt with no benchmark features. Require a nonzero exit and the diagnostic `JNOX_CONTROL_CASE must be unset for JNOX_PROFILE=proportional`.

모든 비례 배분 앱에서 `JNOX_CONTROL_CASE`를 해제한다. 첫 통합 검증에서는 virt에서 벤치마크 기능 없이 `kernel/`에서 위 명령을 실행한다. 종료 코드가 0이 아니고 위 진단 문구가 출력되어야 한다.

The boot dispatches a real idle frame while the eligible set is empty and checks RNG state continuity. The first PPI enables the frozen ready set. Starts are sampled immediately before each compare write. Stops are the counter saved by the shared vector after its scratch save, before the handler. The pre-ERET sample precedes the final GPR restore. Early yield, partial ticks, kernel tail and IRQ entry therefore need the original brackets. Worker observations come from private double-buffered stack tuples (count, checksum, physical counter); all three clients are inspected at every stop. END reports first failed source line, configuration, slot, actual and expected. Line zero denotes a completed run with no guest predicate failure. [First three cold boots](evidence/evd-055.en.md). / 빈 준비 집합에서 실제 유휴 프레임으로 복귀하고 난수 상태를 검사한다. 첫 PPI에서 준비 집합을 설정한다. 시작은 비교값 쓰기 직전, 중단은 벡터에서 레지스터 임시 저장 직후, 복귀 표본은 마지막 범용 레지스터 복원 전이다. 매 중단에서 세 작업의 스택 내 이중 버퍼 반복 수·체크섬·카운터를 확인한다. END는 최초 실패 소스 행·구성·슬롯·실측·기대값이며 0행은 게스트 조건 실패 없이 끝났음을 표시한다. [최초 콜드 실행 3회](evidence/evd-055.md).

| Raw tag / 행 | Six unsigned fields / 여섯 필드 |
| --- | --- |
| 1 | mode (0 seed1, 1 seed2, 2 stride, 3 lottery-miss, 4 stride-delay), seed, F, tick, packed weights, zero |
| 2 | client, stack low, stack high, entry PC, TPIDR, zero |
| 3 / 4 | slot, preceding state, resulting state, word count, raw word, ticket / slot, ready mask, total, low, exclusive high, owner |
| 5 / 6 | slot, ready mask, pre-normalization passes A/B/C, floor / slot, owner, normalized passes A/B/C, zero |
| 7 / 8 | slot, owner, start, compare, frame, entry/resume PC / slot, owner, returned frame, return PC, pre-ERET sample, vector sample |
| 9 / 10 | slot, owner, IRQ flag, interrupted PC, stop, whole ticks / slot, post-charge passes A/B/C, remainder, interrupted frame |
| 11 / 12 | slot, incumbent passes A/B/C, old/new masks / slot, observed client, count, checksum, sample time, private stack pointer |
| 13 / 14 | current slot, raw GIC acknowledge, timer control, three zeros / slot, task, selection=0 or stop=1, frame, PC, register fingerprint |

The lottery-miss image programs two ticks on its first grant and still applies the one-tick predicate. The stride-delay selector gives A a two-tick IRQ grant and charges its actual observed residency. Its three-tick predicate can succeed only on an observed late interrupt. This image contains no controlled IRQ delay; deliberately producing that late observation remains open. Preserve short/early failures and the measured charge. / lottery-miss는 첫 두 틱 부여에서 원래 한 틱 조건을 검사한다. stride-delay는 A에 두 틱 비교값을 적용하고 실제 경과 전체를 차감한다. 세 틱 이상이 실제 관측된 경우에만 해당 조건을 충족한다. 제어된 IRQ 지연 실험은 미결이며 최초 실패 및 차감 기록을 보존한다.

## Controlled first stride grant / 첫 스트라이드 부여의 지연 검증

On one virt CPU with the default clock, freeze cold-boot positions 1–5 as lottery1, lottery2, stride, lottery-miss, stride-delay. Retain each first UART and exception trace even on failure. Positions 1–3 must finish their existing windows, position 4 must report the existing slot-zero two-versus-one rejection, and position 5 must end successfully after A's first grant. The old diagnostic saw 5,011,037 counts at 2,500,000 per unit. The timer expired on schedule while A was interruptible. Extra iterations alone cannot assure late delivery.

기본 클록 단일 virt CPU에서 위 순서로 각각 최초 콜드 부팅을 보존한다. 앞 세 건은 기존 구간을 완료하고 네 번째는 첫 슬롯 2 대 1 거부, 다섯 번째는 A의 첫 부여에서 성공해야 한다. 기존 진단에서는 두 단위가 지난 직후 인터럽트가 전달되었다.

Only in position 5, A enters a bounded local IRQ critical section and continues publishing its ordinary Rust count/checksum/counter tuples. After publishing the first tuple at or beyond start + three units, A unmasks IRQs. The timer compare stays at start + two units. Independent replay must recover at least three whole units from vector stop minus start, match the remainder and weight-one pass increment, locate the final A sample at or beyond the release boundary and at or before vector stop, find unchanged B/C tuples, and corroborate the idle and A IRQ/ERET pairs and timer PPI. Preserve early failure if A never enters its section in time. This measures controlled guest IRQ deferral.

다섯 번째 구성에서만 A가 로컬 IRQ를 잠시 마스킹하고 실제 Rust 작업과 스택 표본 발행을 계속한다. 시작 후 세 단위에 도달한 표본을 발행하면 IRQ를 허용한다. 비교값은 두 단위다. 독립 검증은 벡터 중단값과 시작값으로 전체 단위·나머지·패스 증가를 복원하고 A 표본의 시각, B/C 불변, 유휴 및 A의 IRQ/ERET와 타이머 PPI를 대조한다. 조기 실패도 보존한다.
[Five first positions, linked IRQ return and full charge / 최초 5개 실행·실제 IRQ 복귀 및 전체 부과](evidence/ordinary-stride-late-first.en-ko.md).
