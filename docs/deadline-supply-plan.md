# POL-005 finite interrupted CPU supply / 유한 CPU 공급과 중단 (#248)

Prospective criteria on 8609aab. One Cortex-A72 QEMU virt/GICv2/vCPU, original 16-slot horizon, 100 ms grid S+kQ. Reserve [1,2) for one independent service. All other whole slots are available. EDF checks every release/deadline interval against its available slots. Both policies simulate the finite table, account outgoing work up to the boundary, suspend the incumbent at the head of the queue, append releases in declaration order, and make no worker decision or charge in the withheld slot. At the next available boundary rank by deadline or sampled signed laxity, stable queue order on ties. No shifting the epoch or later deadlines.

| Profile | (id, release, deadline, cost, work duration ms) | Expected |
|---|---|---|
| ordinary-edf-supply / ordinary-llf-supply | (A,0,5,2,250), (B,1,7,2,30) | model A,service,A,B,B; both finish by 5. Actual A progress before interruption, independent stack runs sum of squares 1..64 = 89440 then waits to boundary; B releases at 1. Restore A saved frame/PC at 2, observe progress after resume, A and B return exactly once before absolute deadlines. |
| ordinary-edf-supply-refuse / ordinary-llf-supply-refuse | (A,0,2,2,250) | passes with full supply, one available slot by 2. EDF Demand (8), LLF Miss (9), park before worker initialization/dispatch. |

Freeze independent first cold launches in order EDF refused, LLF refused, EDF normal, LLF normal; retain failed attempts and raw UART separately. Then repeat original ordinary-edf, ordinary-llf, ordinary-llf-overload and ordinary-edf-overrun using EVD-048 criteria and original launch records. Enable deadline trace for detailed replay; retain ELF, command, hashes, clock frequency, brackets and frames. A service slot is a logical reservation; report actual restore-to-entry counts separately from scheduler gaps. Finite TCG observations and model costs do not establish hardware or product WCET.

기존 16슬롯 절대 그리드에서 [1,2)만 서비스에 예약한다. EDF는 릴리스·마감 구간별 가용 공급을 검사하고 두 정책 모두 실제 선언 순서와 추정 잔량으로 재생한다. 중단 시 기존 실행자를 큐 맨 앞에 보존하고 신규 도착을 뒤에 추가한다. 예약 슬롯에서는 작업을 선택하거나 비용을 부과하지 않는다. 다음 경계에서 EDF 마감 또는 LLF 여유가 같은 작업은 기존 순서를 따른다. 위 표의 두 정상 실행에서 A의 진행, 별도 스택 서비스 합계 89440, A의 동일 프레임·PC 복귀와 이후 진행, A/B의 정상 종료와 절대 마감을 검증한다. 두 거부 실행에서는 작업 시작 전에 공급 부족을 판정한다. 첫 실행 순서·실패·원시 기록을 보존하고 기존 네 프로필도 원래 기준으로 검증한다. 실제 카운터 서비스 시간은 논리 예약 및 스케줄러 공백과 구분하며 제품 WCET는 별도 검증한다.

## Prospective correction after the first #252 run / #252 최초 실행 이후 수정 결정

The independent first on bacd8dc retained 5/6 host checks and a failed plain LLF normal boot. At 62,500,000 Hz (Q=6,250,000), grid 2 switched service to A at entry 19,407,096; the restore stamp was 19,409,081. A reached completion SVC at 27,348,788 after the pending grid-3 target 25,543,296 and before its deadline 38,043,296. No grid-3 IRQ preceded that SVC. B never ran. Keep this first and its verdict. The counters establish event order; the source also allows a delayed A start to displace its relative 250 ms stop.

For both admitted supply profiles only, interpret A’s 250 ms as an absolute stop S+250 ms (halfway between grids 2 and 3). Pass that checked counter to a linked A entry sharing the original progress/product loop. B keeps 30 ms relative to its own start. Print the actual absolute stop passed to A and require positive A progress at interruption and after restoration, including in plain output. Tight refused controls retain the original relative table and no worker launch. All model costs, job deadlines, calendar, PPI verification and k<grid SVC guard remain binding. A late resume with no positive post-restore work fails qualification. A delayed or misordered event still fails under the original rules. First corrected runs have their own positions.

The existing EDF admission visits each release/deadline interval in table order. With costs MAX and 2 and deadlines 5 and 7 the first interval [0,5) has demand MAX and supply 4, so reject Demand there. Set the second deadline to 5 in a second assertion: both jobs then contribute to the first interval and checked MAX+2 returns Overflow before comparison. Preserve the implementation and both refusal reasons.

독립 최초 실행(bacd8dc)은 호스트 검사 5/6 및 일반 LLF 실패로 보존한다. 그리드 2 서비스→A 진입 19,407,096, 복귀 19,409,081 이후 다음 타이머 25,543,296보다 늦고 A 마감 38,043,296보다 이른 27,348,788에 완료 SVC가 진입했다. 그 전에 그리드 3 IRQ는 없었고 B는 실행되지 않았다. 62.5 MHz에서 Q는 6,250,000이다. 두 정상 공급 프로필에서 A의 종료 기준을 S+250 ms로 고정하고 실제 인수와 중단 전·복귀 후 양의 진행량을 검증한다. B는 자기 시작부터 30 ms이다. 거부 표와 기존 네 프로필의 동작을 유지한다. 타이머 순서·마감·미관측 그리드 판정 기준은 그대로 적용하고 수정본 최초 결과는 따로 기록한다. EDF [0,5)에서 A 단독 수요 MAX는 공급 4를 초과하므로 Demand이다. B의 마감도 5로 설정한 별도 단언에서 MAX+2의 체크 덧셈이 Overflow를 반환해야 한다.

[Ten pre-change deadline and cooperative build identities](evidence/deadline-cooperative-legacy-loads.en-ko.md).

[Independent first assessment / 독립 최초 결과](evidence/deadline-supply-first.en-ko.md) · [Corrective action / 시정 이슈 #254](https://github.com/Romanof123/JNOX/issues/254).

[EVD-095 English](evidence/evd-095.en.md) · [한국어](evidence/evd-095.md).

## Build, capture and replay / 빌드·원시 UART 수집·재생

The normal supply interface uses the four JNOX_APP selectors below, raw UART and the supply replay. The separate independent deadline benchmark has its own fixed transcript in `tools/qemu_boot.py --profile deadline` beginning with `DEADLINE EDF A FIRST`.

Run this Bash block from the repository root on Linux with GNU timeout, sha256sum, Python 3, the Rust aarch64-unknown-none target and qemu-system-aarch64 available. Start with the other JNOX selectors unset. For evidence runs, set `export TMPDIR="$HOME/jnox-evidence"` and run `mkdir -p "$TMPDIR"` on persistent storage before starting. Archive each attempt and verify its SHA-256 before ending the session or restarting WSL. If first raw files are lost, retain the observation as such and label any new launch separately. 검증 시 TMPDIR은 영속 저장소로 지정하고 세션 종료 전 원본과 해시를 확인한다. Each invocation creates a fresh attempt directory, and each selector builds its plain and diagnostic ELF separately. The plain run uses an unset JNOX_DEADLINE_TRACE; the trace run sets it to 1.

```bash
(
set -eu
root=$(pwd -P)
run=$(mktemp -d "${TMPDIR:-/tmp}/jnox-deadline-supply.XXXXXXXX")
printf 'Capture directory: %s\n' "$run"
git rev-parse HEAD > "$run/source-commit"
result=0
for mode in trace plain; do
  for app in ordinary-edf-supply-refuse ordinary-llf-supply-refuse ordinary-edf-supply ordinary-llf-supply; do
    out="$run/$app-$mode"
    mkdir "$out"
    export JNOX_BOARD=virt JNOX_PROFILE=deadline JNOX_APP="$app"
    unset JNOX_DEADLINE_TRACE
    flags=()
    if [[ $mode == trace ]]; then
      export JNOX_DEADLINE_TRACE=1
    else
      flags=(--plain)
    fi
    build_rc=0
    (cd "$root/kernel" && CARGO_TARGET_DIR="$run/target" cargo build --locked --release -j1) > "$out/build.log" 2>&1 || build_rc=$?
    echo "$build_rc" > "$out/build.rc"
    if (( build_rc != 0 )); then result=1; continue; fi
    cp "$run/target/aarch64-unknown-none/release/jnox-kernel" "$out/kernel.elf"
    qemu_rc=0
    timeout 10s qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 \
      -accel tcg -smp 1 -m 128M -nographic -nic none -monitor none -serial stdio \
      -d int -D "$out/exceptions.log" -kernel "$out/kernel.elf" \
      < /dev/null > "$out/uart.raw" 2> "$out/qemu.stderr" || qemu_rc=$?
    echo "$qemu_rc" > "$out/qemu.rc"
    replay_rc=0
    python3 "$root/tools/deadline_supply_replay.py" "$app" "$out/uart.raw" "${flags[@]}" \
      > "$out/replay.json" 2> "$out/replay.stderr" || replay_rc=$?
    echo "$replay_rc" > "$out/replay.rc"
    (cd "$out" && sha256sum kernel.elf uart.raw > SHA256SUMS)
    if (( qemu_rc != 124 || replay_rc != 0 )); then result=1; fi
  done
done
exit "$result"
)
```

A refused guest prints `ORDINARY DEADLINE REFUSED BEFORE LAUNCH`. Replay expects reason 8 for EDF or 9 for LLF. A corrected normal guest emits `A_STOP`, `A_PROGRESS`, `RESULT` and `ORDINARY DEADLINE FINISHED`. Both terminal paths park the CPU. GNU timeout normally returns 124 at the host limit even after successful completion. Read the guest verdict from the retained UART and replay status. Trace replay checks the individual D/J rows. For example, with RUN set to the printed capture directory:

```bash
python3 tools/deadline_supply_replay.py ordinary-edf-supply-refuse "$RUN/ordinary-edf-supply-refuse-plain/uart.raw" --plain
python3 tools/deadline_supply_replay.py ordinary-llf-supply "$RUN/ordinary-llf-supply-plain/uart.raw" --plain
python3 tools/deadline_supply_replay.py ordinary-llf-supply "$RUN/ordinary-llf-supply-trace/uart.raw"
```

위 명령은 각 시도의 ELF·원시 UART·예외 로그·빌드 및 호스트 종료 코드·재생 결과를 별도 디렉터리에 보존한다. 거부 코드와 정상 작업의 완료 여부는 원시 출력과 재생으로 판정한다. 완료 후에도 게스트가 대기하므로 호스트의 124는 관찰 시간 종료를 뜻한다. 독립 벤치마크의 고정 출력 검사는 `qemu_boot.py --profile deadline`으로 수행한다.

[Corrected independent assessment / 수정 후 독립 판정](evidence/deadline-supply-corrected.en-ko.md).

[Published-command verification and raw archive / 공개 실행 명령 검증과 원본](evidence/deadline-supply-command-replay.en-ko.md).
