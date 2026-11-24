# Ordinary Rust weighted workloads / 가중 일반 Rust 작업

CHG-052 · IMP-049 · issue #116 · [frozen criteria](ordinary-vruntime-plan.md). Implementation and replay instructions. [EVD-049](evidence/evd-049.en.md) · [한국어](evidence/evd-049.md). [English architecture](architecture.en.md) · [한국어 아키텍처](architecture.md).

Use a fresh output directory per attempt. From the repository root on Linux with GNU timeout, QEMU AArch64, Rust aarch64-unknown-none and AArch64 binutils, run the following. Each guest parks after the terminal UART. The host limit is 20 seconds; timeout alone is never a successful observation. Retain partial files on every failure. Capture the first cold plain and diagnostic run separately in each clock cohort. Default TCG uses its normal clock. The separately declared instruction-counted cohort uses shift=3. Both cohorts use the same Q/H/G bounds. Only observed runs satisfying all frozen conditions may pass. Preserve each cohort's failures.

매 시도마다 새 출력 디렉터리를 만든다. 아래 명령은 리눅스 저장소 루트에서 수행한다. 호스트 제한은 20초이며 종료 UART 뒤 게스트는 대기한다. 시간 초과만으로 통과 판정할 수 없다. 실패 시 부분 기록도 보존한다. 기본 TCG 클록과 별도 선언한 shift=3 명령 계수 클록 각각에서 일반·진단 최초 결과를 보존한다. 두 클록 모두 동일한 Q/H/G 기준을 적용하며 모든 조건을 실제 충족한 실행만 통과한다.

```bash
set -eu
out=$(mktemp -d)
git rev-parse HEAD > "$out/revision"
git diff --binary HEAD > "$out/source.patch"
qemu-system-aarch64 --version > "$out/qemu-version"
for clock in default icount3; do
for app in weighted-share weighted-wake; do
  for mode in plain diagnostic; do
    dest="$out/$clock-$app-$mode"
    mkdir "$dest"
    (
      cd kernel
      unset JNOX_WEIGHTED_TRACE
      if [ "$mode" = diagnostic ]; then export JNOX_WEIGHTED_TRACE=1; fi
      JNOX_PROFILE=weighted JNOX_APP="$app" JNOX_BOARD=virt cargo build --locked --release --target-dir "$dest/build"
    )
    elf="$dest/build/aarch64-unknown-none/release/jnox-kernel"
    sha256sum "$elf" > "$dest/sha256"
    aarch64-linux-gnu-nm -an "$elf" > "$dest/symbols"
    aarch64-linux-gnu-objdump -d "$elf" > "$dest/disassembly"
    clock_args=()
    if [ "$clock" = icount3 ]; then clock_args=(-icount shift=3,align=off,sleep=off); fi
    cmd=(qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -accel tcg
         "${clock_args[@]}" -m 128M -nodefaults -display none -monitor none
         -serial stdio -kernel "$elf" -d int -D "$dest/exceptions.log")
    printf '%q ' "${cmd[@]}" > "$dest/argv"
    status=0
    timeout 20s "${cmd[@]}" > "$dest/uart" 2> "$dest/stderr" || status=$?
    echo "$status" > "$dest/status"
  done
done
done
for clock in default icount3; do
for app in weighted-share weighted-wake; do
  for mode in plain diagnostic; do
    opts=()
    if [ "$mode" = diagnostic ]; then opts+=(--diagnostic); fi
    if [ "$app" = weighted-wake ]; then opts+=(--wake); fi
    python3 tools/weighted_replay.py "$out/$clock-$app-$mode/uart" "${opts[@]}"
  done
done
done
rustc +1.95.0 --edition=2024 --test tools/scheduling_tests.rs -o "$out/policy-controls"
"$out/policy-controls" vruntime_tests::
```

The shared command compiles the enclosing scheduling test crate and selects its weighted policy tests. 위 공통 명령은 스케줄링 테스트 크레이트를 컴파일한 뒤 가중 정책 테스트 모듈만 실행한다.

**Reading the capture.** WEIGHTED1 records wake-mode, frequency, Q, journal capacity, A/B stack bases, stack bytes and frame bytes. W is one immutable closure in lease order. Its columns are sequence, owner, cause (0 timer, 1 ordinary return, 2 caller blocks, 3 peer wake), ready mask at dispatch, start, programmed compare, stop, scheduler gap, A/B pre-charge runtime, A/B charged runtime, A/B post-event runtime, A/B cumulative raw service, floor before charge, charged floor, post-event floor, acceptance sample (zero outside wake), A/B progress, block generation, full solo closures, latest B resume witness, completed resume generation, selection begin and end. All virtual times/floors are exact half ticks. Counter and service values are raw ticks. B's signed lag equals doubled floor minus doubled B runtime. Row 36 freezes the continuous result. Later rows belong to the exit tail. Both entries exit on their next horizon observation, with an additional finite u32::MAX loop guard. At most 64 closures fit. A capacity breach halts with failure. There are two fixed 16 KiB stacks, two policy records, one outstanding lease and no heap.

F records sequence, restored frame/ELR, subsequently saved frame/ELR and the counter just before integer register restore. Compare the full frame extent against the owner's stack and correlate every frame/ELR with disassembly and the exception/ERET stream. The diagnostic marker excludes the final GPR reloads and ERET. Selection samples bracket policy dispatch. Start-to-stop includes restore, ordinary work and exception entry through the first handler sample. UART is deferred until completion/failure. The wake sample follows placement before selection. B reads the counter at the saved post-SVC ELR and publishes it alongside resumed progress. The separate dispatch-to-witness limit is G. Report both acceptance-to-start and acceptance-to-witness. External exception logs and ELF disassembly require separate inspection. A partial journal or ambiguous exception ordering cannot establish a pass.

**기록 해석.** WEIGHTED1은 사례·주파수·Q·행 용량·두 스택 시작 주소·스택 및 프레임 크기다. W는 실행권 순번, 소유자, 원인(0 타이머, 1 정상 반환, 2 호출자 차단, 3 동료 깨움), 준비 마스크, 시작·비교·정지·스케줄러 간격, A/B 정산 전·정산 후·이벤트 후 가상시간, A/B 누적 실서비스, 정산 전·후·이벤트 후 바닥, 깨움 수락 시각, 두 진행 횟수, 차단 세대, 단독 타이머 종료 횟수, 최근 B 복귀 명령 시각·복귀 세대, 선택 시작·끝 순서다. 가상시간과 바닥은 반 틱 단위의 정수다. 연속 사례에서 36행 이후는 종료 구간이다. B의 부호 있는 지연은 두 배 바닥에서 두 배 B 가상시간을 뺀 값이다. 최대 64행 초과 시 실패하며 스택은 각 16 KiB다.

F는 순번, 복원 프레임·ELR, 이후 저장 프레임·ELR, 정수 레지스터 복원 직전 시각이다. 전체 프레임이 소유 스택 안에 있는지 확인하고 예외/ERET 로그·ELF 역어셈블과 대조한다. 마지막 정수 레지스터 복원과 ERET는 진단 표식 뒤에 수행된다. 실서비스에는 디스패치 후 복원과 핸들러 첫 표본까지 예외 진입이 포함된다. 수락 시각은 배치 후 선택 전에 읽는다. B는 차단 SVC 다음 첫 명령에서 시각을 읽는다. 디스패치부터 해당 명령까지 별도 상한은 G이다. 수락부터 디스패치와 실제 복귀 명령까지 각각 보고한다. 부분 기록이나 순서가 모호한 결과는 통과 근거로 사용할 수 없다.
