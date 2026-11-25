# Virtual permit completion watchdog / 가상 허가 출력 완료 감시기

This opt-in example runs three ordinary Rust processes on single-core QEMU virt. One ELF is packaged into three independent address spaces. Role arguments select application code, and the kernel authenticates slot 0 for loan publication, slot 1 for protected control, and slot 2 for supervision. The controller runs priority 7, the sender 6 and the supervisor 4. The existing sixteen [permit scenarios](../native-safety-channel/README.md) and separate [qualification calendar](../native-safety-admission/README.md) retain their original workloads and thresholds.

The supervisor opens generation 1 before exchanging a common raw-counter epoch E. It checks both role/case acknowledgements before E. The first successful controller COMPLETE after OPEN primes the watchdog. If L is the actual heartbeat tick and F the counter rate, the first disallowed tick is `D = L + floor(F × 5 / 1000) + 1`. OPEN alone has no heartbeat deadline. The existing 20 ms permit freshness check remains independent.

The controller normally checks every 2.5 ms from E through E+70 ms, in A/B phases on a 5 ms job calendar. Each phase makes one nonblocking loan attempt, copies and releases a received 44-byte frame, validates its protected fields and submits an eligible private snapshot. Each job shares a single command budget. Successful fused CHECK/COMPLETE calls return device state from the same protected syscall. The lower-ranked sender has seven releases at 10 ms intervals. The first pair uses the explicit E/E+10 ms calendar for reproducible ENABLE setup. Both are real publications through the ordinary loan route. Capture the actual construction and publication ticks separately.

| Argument | Stimulus and evidence required |
| --- | --- |
| `healthy` | Continuous completions, valid samples and one explicit generation-1 rearm at 15 ms. Require actual ENABLE and priming, advancing L/D, continuing ENABLE until the final 70 ms observation, and no watchdog fault before teardown. |
| `sleep-late` | From an open phase at 20 ms sleep once for 10 ms on the monotonic clock. The supervisor can observe inhibition near 27.5 ms, before the controller attempts its first late COMPLETE. A same-generation REARM must fail. |
| `compute-late` | Enter the exported finite AArch64 `wd_compute_loop` symbol at 20 ms. Its loop contains arithmetic, a decrement and a branch. It makes no syscall or clock read. Before the epoch exchange, a warmup and three 65,536-round measurements choose a bounded count targeting 12 ms (maximum 32,000,000 rounds). Require an actual timer IRQ during computation and the device transition before the late COMPLETE. If the measured loop runs short, the active-fault witness fails. |
| `controller-exit` | Exit from a protected active phase at 20 ms. Confirm the real exit event, kind-6 endpoint inhibition and rejection of later REARM and OPEN. The sender makes no further mapping accesses after the first two samples. |
| `recover` | After the sleeping fault and refused generation-1 rearm, OPEN generation 2 at 35 ms. Confirm the new unprimed, inhibited state. Subsequent real controller completions re-prime it. Publish a fresh pair with explicitly scheduled 40/50 ms issue ticks through separate production loans. Observe ENABLE off after the second commit; request generation-2 REARM at 55 ms and require the later independent transition to enabled. |
| `recover-live` | Same recovery using the actual live creation sample for **both** generation-2 issue ticks. Require both committed sequences. If the two observed issues are spaced exactly 10 ms and still fresh, require rearm success and ENABLE. Otherwise require rearm refusal and continued inhibition. Both outcomes retain the two real issue times. |

The supervisor requests the first rearm at 15 ms, then independently reads the actual register flags at the 17.5 ms release. Immediately before injection, the controller's actual CHECK snapshot must show primed and enabled in generation 1, at least two committed samples, and a prior successful COMPLETE. It records that guard before exiting, sleeping or computing. Code 95 records unsuccessful setup. A startup gap or later teardown fault cannot satisfy the active-fault case. The delayed completion is attempted on the same open phase with SVC 60/3; expected status is -110. The fault profiles skip 22.5–32.5 ms controller releases and resume observation at 35 ms. They deliberately violate the normal schedule. Completion of the fixture does not admit a timing policy.

## Capture from the integrated revision

Use Bash with the native Rust SDK, the protected virtual watchdog kernel and package flag, and QEMU virt. Build once and retain separate output directories for each policy and case.

```bash
set -eu
out=$(mktemp -d)
app=examples/native-safety-watchdog
sh sdk/native-aarch64/build-rust.sh "$app/Cargo.toml" "$out/worker.elf"
revision=$(git rev-parse HEAD)
for policy in fifo rr; do
  quantum=()
  if [ "$policy" = rr ]; then quantum=(--quantum-ticks 625000); fi
  for scenario in healthy sleep-late compute-late controller-exit recover recover-live; do
    pkg="$out/$policy-$scenario"
    python3 tools/build_native_el0.py --board virt --policy "$policy" "${quantum[@]}" --out "$pkg" \
      --elf "$out/worker.elf" --peer-elf "$out/worker.elf" --peer-elf "$out/worker.elf" \
      --arg sender --arg "$scenario" --peer-arg controller --peer-arg "$scenario" \
      --peer2-arg supervisor --peer2-arg "$scenario" --priority 6 --peer-priority 7 --peer-priority 4 \
      --ipc-route 2:0 --ipc-route 2:1 --loan-route 0:1 --safety-channel --safety-qualification --safety-watchdog
    sha256sum "$out/worker.elf" "$pkg/kernel.elf" "$pkg/manifest.json" > "$pkg/watchdog.sha256"
    python3 tools/native_el0_capture.py "$out/capture-$policy-$scenario" "$pkg/kernel.elf" \
      "$out/worker.elf" "$app/src/main.rs" "$revision" --manifest "$pkg/manifest.json" \
      --plain --expect-exit 71 --timeout 40 --uart-limit 8388608
  done
done
```

Retain tool/QEMU versions, command, effective manifest, hashes, clock frequency, each role's exit code (normally 71/72/73), UART and RAM. `--expect-exit` scores only the primary exit. Check all three, all private statuses and the device events. Read each symbol through the **corresponding process page-table root** even though ELF symbol addresses are shared. Missing observations or dropped events leave the relevant timing or outcome open.

## Raw fields

Private rows contain little-endian u64 words. Negative syscall statuses use two's complement. Bit 63 alone denotes a skipped operation.

| Symbol and owner | Words in order |
| --- | --- |
| `wd_header` per process | Magic, role (0/1/2), case ID (table order), E, F, internal error. |
| `wd_controller_rows[29][12]`, slot 1 | Target, observed entry, generation, entry flags, receive status, private validation error, commit status, private rearm error, kernel rearm status, successful post-COMPLETE sample, end flags, end sequence. Unexecuted rows remain zero. The injection row is stored separately. |
| `wd_injection[12]`, slot 1 | Reached, guard passed, actual post-CHECK tick, flags, generation, sequence, wire issue, last successful post-COMPLETE **user sample**, requested compute rounds, late COMPLETE status, post-attempt tick and flags. Use event 13 for kernel L. Exit intentionally leaves the final three fields unwritten. |
| `wd_calibration[4]`, slot 1 | Probe rounds, minimum observed ticks, target duration in ticks, computed rounds. |
| `wd_sender_rows[7][10]`, `wd_sender_bytes[7][44]`, slot 0 | Target, actual creation sample, generation, sequence, wire issue, synthetic-calendar bit, handle, acquire/publish/release status. Byte rows hold the encoded frames; skipped exit-case rows have no frame. |
| `wd_supervisor_rows[12][13]`, slot 2 | Target, before/after, before generation/flags/sequence/issue, command and generation argument, actual status, after generation/flags, cumulative mismatch bit. |
| `wd_recovery_pair[5]`, slot 2 | Two committed wire issue ticks observed after 40/50 ms publication, pre-rearm sample, exact-spacing bit, first-sample-fresh bit. |

Correlate `NATIVE SAFETY EVENT` kind 13 with kind 2 (register write/readback), 3 (freshness fault), 6/7 (loan and receiver bytes), 8 (exit), 10 (COMPLETE) and 11 (publish). Kind 13 data words are operation, generation, prior L/D, decision counter, source and resulting L/D. Operations 1–6 are OPEN, prime, refresh, timeout, clock fault and death. Source 30 identifies the physical timer IRQ path in this **virtual** machine; 0x100+reason is scheduler polling, source 59/60/61/63 is a protected syscall and 1 is endpoint exit. Check rendered counter stamps, not textual event order: the device records its write/readback bracket before event 13 is appended. Require timeout decision ≥ D, independent output readback and continued inhibition. Capture the native qualifier IRQ's interrupted PC against the disassembled loop range to attribute the compute fault. A timeout first evaluated by a later syscall does not demonstrate autonomous IRQ detection. Observe all raw bounds and report overrun/missed conditions independently.

## 한국어 재현·해석

위 명령으로 동일한 ELF를 세 독립 슬롯에 배치한다. 제어(1) > 송신(0) > 감독(2)의 실제 디스패치, 초기 유휴, 각 세대 및 시나리오를 별도로 확인한다. 정상 완료가 처음 일어난 뒤에만 5 ms 감시가 시작된다. 계산한 D와 실제 타이머 IRQ 시각, ENABLE 쓰기·읽기 구간을 대조한다. 5 ms 정확한 경계는 허용되며 다음 표현 가능한 틱부터 만료다. 20 ms 데이터 신선도는 따로 판정한다.

오류 주입 직전 감시 준비와 실제 ENABLE, 두 샘플 승인, 직전 완료를 확인해야 한다. `compute-late`는 유한 무시스템호출 계산을, `sleep-late`는 실제 절전을, `controller-exit`는 활성 제어자 종료를 관찰한다. 지연된 완료와 같은 세대 재무장이 출력을 되살리면 실패다. `recover`는 새 세대와 명시한 시각의 두 실제 전송으로 복구를 검사하고, `recover-live`는 두 발행 시각 자체를 실측한다. 실제 간격이 정확히 10 ms가 아니면 재무장 거부와 출력 억제를 확인한다. 루프가 빨리 끝났거나 주입 전부터 출력이 꺼져 있던 경우에는 계산 중 IRQ 검증을 충족하지 못한다. 각 슬롯 RAM 및 원본 UART를 보존한다. 이 예제의 측정 범위는 가상 CPU와 가상 출력이다.
