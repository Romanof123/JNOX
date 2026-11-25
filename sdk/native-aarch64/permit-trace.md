# Native permit qualification journal

SVC 62 takes job, phase, stage and absolute physical-counter target in x0 through x3. Phase 1/2 identifies controller A/B (slot 1), phase 3 sender (slot 0), and phase 4 supervisor (slot 2). Stage 1 sleeps until a future target using the scheduler event's counter sample. An already due target polls immediately. Stages 2 and 3 mark user start and end of application work. Zero means success. The application owns job numbering and target selection.

COUNTS reports counter frequency, attempts, rows retained and rows lost. ROW contains the append index followed by twelve unsigned words. Signed status values use two's complement. Slots absent from scheduler selection are UINT64_MAX.

| Kind | Words after kind, in order |
|---|---|
| 1 | PID, job, phase, stage, target, vector sample, scheduler time, saved user PC, status, 0, 0 |
| 2 | reason, scheduler time, before event, after selection, old runnable mask, new runnable mask, old PID, selected PID, exit flag, pre-user-return counter, return PC |
| 3 | vector sample, register sample time, raw GIC IAR, physical CVAL, physical CTL, interrupted PC, saved PSTATE, idle-frame flag, 0, pre-IRQ-return counter, return PC |
| 4 | PID, SVC, input command word, vector sample, scheduler time, after service, status, 0, 0, 0, 0 |
| 5 | before programming, after readback, enabled requested, requested target or zero, physical CVAL, physical CTL, 0, 0, 0, 0, 0 |

Use mask transitions to bound readiness and selection inside the event bracket. Row 2 includes changes, exits and SVC 62 boundaries. Row 5 records changes to requested compare values. Row 3 includes idle and user IRQs; decode the interrupt ID before attributing any interval to the physical timer.

Vector samples follow exception entry. Return samples precede architectural ERET. A subsequent exception from the selected user context gives an upper bound on resumed execution. A zero return cell has no recorded return. The start/end markers delimit application work. For a whole segment include housekeeping through its next actual block or exit, and examine interrupt and dispatch brackets separately. Missing observations, terminal work after a marker and journal overflow limit conclusions. Finite counter observations supply neither a hardware interrupt assertion timestamp nor a product WCET.

Safety EVENT 12 records every Session::check in qualification and named boundary builds. Its eight data words are the original decision tick, caller, generation, accepted sequence, last issue, prior fault, resulting fault and gate state. Caller is the control command for its entry check, 0x102 for the first commit check and 0x202 for the second. Sequence zero means no accepted permit. Inspect both fault flags, monotonicity and prior issue before classifying an observation as fresh. The separate journal timestamp follows the check. Require zero safety journal losses.

## Watchdog package / 감시 타이머 패키지

Package the three application ELFs for the chosen watchdog scenario. SLOT 0 sends permits, slot 1 runs the controller and slot 2 runs the supervisor. The following Bash command uses FIFO. For RR, use a separate output directory, replace `--policy fifo` with `--policy rr --quantum-ticks 625000`, and retain the priority order.

~~~sh
python3 tools/build_native_el0.py --out "$OUT" --board virt --policy fifo \
  --elf "$SENDER_ELF" --peer-elf "$CONTROLLER_ELF" --peer-elf "$SUPERVISOR_ELF" \
  --arg "$SCENARIO" --peer-arg "$SCENARIO" --peer2-arg "$SCENARIO" \
  --priority 6 --peer-priority 7 --peer-priority 4 \
  --ipc-route 2:0 --ipc-route 2:1 --loan-route 0:1 \
  --safety-channel --safety-qualification --safety-watchdog
~~~

Supply the scenario argument expected by these ELFs. The selector may appear once. Raw Cargo builds use `JNOX_NATIVE_SAFETY_WATCHDOG=1` in the corresponding three-process protected qualification build. Both entrypoints require virt, FIFO or RR, and controller > sender > supervisor. A selected package records its three roles, kernel physical counter clock and status flag masks in the manifest's `safety_watchdog` object. A manifest identifies a configured build; timing and output observations come from its guest.

Read SVC 61 through C `jnox_safety_info()` or Rust `safety_channel::info()`. In the returned flags word, bit 4 means watchdog primed and bit 5 means watchdog fault. The same bits are available in qualification SVC 63's returned state. Status reads take no deadline or heartbeat timestamp. The existing permit-frame issue field remains part of the protected frame protocol.

감시 타이머를 사용할 때는 일반 virt FIFO/RR 패키지에 보호 채널, 검증 프로필, 감시 타이머 선택자를 각각 한 번 지정한다. 슬롯 순서는 송신자(0), 제어기(1), 감독자(2)이며 우선순위는 제어기 > 송신자 > 감독자다. 매니페스트에 역할, 커널 카운터와 상태 비트 마스크를 기록한다. SVC 61 상태 조회의 비트 4는 감시 시작, 비트 5는 감시 오류를 나타낸다. 조회 API로 시각이나 기한을 입력하지 않는다. 실제 실행·가상 출력 및 시간 결과는 해당 게스트의 기록으로 확인한다.
