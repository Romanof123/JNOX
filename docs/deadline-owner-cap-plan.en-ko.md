# CHG-100 / IMP-100 prospective contract (#260)

EDF and sampled LLF use the unchanged SUPPLIED table, RESERVED bit 1, absolute origin S, period Q and finite H=16. Opt in with JNOX_DEADLINE_OWNER_CAP=4 (accept) or 3 (refuse) on the existing ordinary-{edf,llf}-supply selectors. Both supply and full CPU admission must succeed first. Refuse before worker frames, timer arming or worker launch. A decimal unsigned ceiling is compiled into the guest.

Observe integer architectural counter timestamps in [S,S+(H+1)Q). For every half-open window [a,a+Q) wholly inside this domain, count dispatches that actually return a different owner frame (workers, service and idle). Timestamp by exception entry, not by logical slot or return instruction. Initial boot-to-idle and terminal park have no returning dispatch and are excluded. A final selected idle frame that is never restored does not count. Idle-to-first-worker counts. Equal owners and timer/SVC entries remain distinguishable. Stop observing after park, with no further events. No transitions/s or physical timing guarantee is claimed.

For general positive integer width W up to the domain length, a successful run has at most one timer entry in each of H+1 disjoint Q-tick cells at its programmed absolute grid, verified for each IRQ. An integer W-tick window touches at most 1+ceil((W-1)/Q) cells, capped by H+1. Each one-shot job can complete once by SVC, regardless of estimate or proximity to IRQ. Each entry has at most one returning handoff. Sum the timer and job bounds with checked arithmetic. For Q>1, W=Q and two jobs this gives 4 reevaluations and at most 4 transfers. Late/wrong-cell timer delivery, unexpected SVC crossing the pending grid, missed deadline or failed accounting invalidates evidence. No dispatch may be postponed or suppressed.

Independent verification shall enumerate integer windows on small artificial horizons against arbitrary legal IRQ offsets and early completions, including adjacent-boundary IRQs, repeated owner, service, idle, terminal park and exact right endpoint. Rebuild actual owner chains from raw frames, prior restore counters, exception entries and final return status. Score every possible sliding window via event-anchored windows (including end clipping); compare against the guest peak and cap. Check unchanged absolute deadlines, service result 89440, both worker products/progress and refusal zero state. Compare original profiles and combined tree separately. Scope remains finite uniprocessor evidence.

## 한국어 계약

EDF·표본 LLF는 기존 SUPPLIED 작업, RESERVED 비트 1, 절대 원점 S와 주기 Q, H=16을 유지한다. 기존 공급 프로필에서 JNOX_DEADLINE_OWNER_CAP=4는 허용, 3은 거부한다. 전체 CPU와 예약 공급 판정을 통과한 뒤 프레임 생성·타이머 시동·작업 실행 전에 한도를 판정한다.

정수 카운터 영역 [S,S+(H+1)Q)에 완전히 속한 모든 [a,a+Q)에서 예외 진입 시각으로 실제 다른 프레임을 복원하는 디스패치를 센다. 작업·서비스·유휴 간 전환과 최초 유휴→작업을 포함한다. 최초 유휴 진입, 동일 소유자 복귀 및 마지막 park에서 복원되지 않은 유휴 선택은 제외한다. park 이후 이벤트는 없다. 물리적 초당 전환 보장은 범위 밖이다.

폭 W의 정수 구간은 최대 min(H+1,1+ceil((W-1)/Q))개 타이머 셀과 겹친다. 각 IRQ가 지정된 셀 안에 한 번만 진입했는지 검사한다. 작업당 조기 완료 SVC는 최대 한 번이며 경계 인접 완료도 포함한다. 이벤트당 실제 전환은 최대 한 번이다. 검사된 덧셈으로 두 상한을 합치며 이 구성은 4이다. 지연·마감 위반 시 실패하고 재디스패치를 늦추거나 생략하지 않는다. 독립 검증은 작은 영역 전수 조사, 경계·같은 소유자·종료 사례 및 실제 프레임·카운터 기반 모든 이동 구간 재계산, 공급 결과·거부 전 무실행·기존 프로필 호환을 확인한다.

Host regression (from the checkout root, with Rust 2024 support):

```powershell
rustc --edition=2024 --cfg jnox_ordinary_deadline --test tools/deadline_supply_tests.rs -o "$env:TEMP/jnox-deadline-supply-tests.exe"
& "$env:TEMP/jnox-deadline-supply-tests.exe" owner_window_
```

호스트 회귀 시험은 커널의 실제 함수를 불러와 작은 영역의 모든 IRQ 오프셋·조기 완료 시각·구간 폭·시작점을 열거한 최대 이벤트 수와 비교한다.

## Run the eight ordinary guest configurations / 일반 게스트 8개 구성 실행

From the repository root on Linux or WSL, use a POSIX shell with GNU timeout, sha256sum, Python 3, the project's Rust target and QEMU AArch64. Start with unrelated JNOX selectors unset. The generic deadline benchmark uses separate build inputs and its own transcript. This recipe retains every first attempt under a new persistent directory. EVD-095 supply replay can score accepted captures in both modes because the cap enables D/J output even in plain mode. Refusal occurs before the supply replay's worker inputs exist. The terminal checks below establish completion or refusal; independent owner-window scoring uses the frozen contract above.

```sh
(
set -eu
root=$(pwd -P)
archive=${JNOX_EVIDENCE_DIR:-"$HOME/jnox-evidence"}
mkdir -p "$archive"
archive=$(cd "$archive" && pwd -P)
run=$(mktemp -d "$archive/owner-cap.XXXXXXXX")
printf 'Capture directory: %s\n' "$run"
git rev-parse HEAD > "$run/source-commit"
result=0
for mode in plain trace; do
  for app in ordinary-edf-supply ordinary-llf-supply; do
    for cap in 3 4; do
      out="$run/$app-$cap-$mode"
      mkdir "$out"
      export JNOX_BOARD=virt JNOX_PROFILE=deadline JNOX_APP="$app" JNOX_DEADLINE_OWNER_CAP="$cap"
      unset JNOX_DEADLINE_TRACE
      if [ "$mode" = trace ]; then export JNOX_DEADLINE_TRACE=1; fi
      rc=0
      (cd "$root/kernel" && CARGO_TARGET_DIR="$run/target" cargo build --locked --release -j1) > "$out/build.log" 2>&1 || rc=$?
      echo "$rc" > "$out/build.rc"
      if [ "$rc" != 0 ]; then result=1; continue; fi
      cp "$run/target/aarch64-unknown-none/release/jnox-kernel" "$out/kernel.elf"
      rc=0
      timeout 10s qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 \
        -accel tcg -smp 1 -m 128M -nographic -nic none -monitor none -serial stdio \
        -d int -D "$out/exceptions.log" -kernel "$out/kernel.elf" \
        < /dev/null > "$out/uart.raw" 2> "$out/qemu.stderr" || rc=$?
      echo "$rc" > "$out/qemu.rc"
      (cd "$out" && sha256sum kernel.elf uart.raw > SHA256SUMS)
      if [ "$rc" != 124 ]; then result=1; fi
      check=0
      python3 - "$out/uart.raw" "$cap" > "$out/terminal.log" 2>&1 <<'PY' || check=$?
import sys
from pathlib import Path

raw = Path(sys.argv[1]).read_bytes()
lines = raw.replace(b"\r\n", b"\n").split(b"\n")
if lines[-1] != b"":
    raise SystemExit("incomplete final line")
lines.pop()
accepted = sys.argv[2] == "4"
expected = b"ORDINARY DEADLINE FINISHED" if accepted else b"ORDINARY OWNER CAP REFUSED BEFORE LAUNCH"
terminals = [line for line in lines if line.startswith((b"ORDINARY DEADLINE ", b"ORDINARY OWNER CAP "))]
if terminals != [expected] or lines[-1:] != [expected]:
    raise SystemExit("unexpected or incomplete terminal transcript")
if not accepted and any(line.split()[:1] in ([tag] for tag in
        (b"JOB", b"D", b"J", b"A_STOP", b"A_PROGRESS", b"RESULT", b"OWNER_EVENT", b"OWNER_PEAK")) for line in lines):
    raise SystemExit("execution record in refusal")
print(expected.decode())
PY
      echo "$check" > "$out/terminal.rc"
      if [ "$check" != 0 ]; then result=1; fi
      if [ "$cap" = 4 ]; then
        rc=0
        python3 "$root/tools/deadline_supply_replay.py" "$app" "$out/uart.raw" \
          > "$out/supply-replay.json" 2> "$out/supply-replay.stderr" || rc=$?
        echo "$rc" > "$out/supply-replay.rc"
        if [ "$rc" != 0 ]; then result=1; fi
      fi
    done
  done
done
exit "$result"
)
```

저장 디렉터리는 기본적으로 홈 아래의 영속 경로다. 매 실행의 ELF·UART·예외 로그와 종료 코드를 보존한다. 한도 4의 두 모드는 D/J 원본으로 기존 공급 검증을 실행하며, 한도 3은 거부 문구와 작업 실행 기록 부재를 확인한다. 모든 이동 구간의 소유자 전환 검증에는 위 계약을 적용한다.

[First independent result / 최초 독립 판정](evidence/deadline-owner-window-first.en-ko.md).
