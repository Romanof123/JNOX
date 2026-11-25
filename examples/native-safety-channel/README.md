# Protected virtual permit bench

Three ordinary Rust processes use the native CPU-only loan channel. Slot 0 creates frames. Slot 1 runs the protected-frame library and controls the virtual ENABLE register. Slot 2 opens generations, requests rearm and injects faults. All outputs are in guest RAM. The fixtures target single-core Cortex-A72 QEMU virt FIFO/RR and use the CHG-311 wire format and ADR-001 timing parameters.

The supervisor distributes a common future raw-counter epoch over synchronous routes 2→0 and 2→1. These requests provide timing and barriers; OPEN, REARM and gate closure use the dedicated kernel-authenticated interface. The scheduled producer outranks the controller, and both outrank the supervisor. The producer holds its loan while filling 44 bytes. The controller checks the configured route, aperture slot, VA and declared length, copies exactly 44 bytes into private storage, releases the mapping, and performs CRC/semantic validation. Only a successful private decision goes to the kernel COMMIT or rearm operation. CHECK runs first and COMPLETE runs last in each 5 ms activation. Every activation attempts at most two loan receives and one command.

**Issue time.** Modes other than `observed` and `live-startup` deliberately put a scheduled synthetic tick on the wire. `observed` uses that fixed 0/10 ms pair only to establish recovery. Starting with sequence 3 it samples the live supervisor-independent SVC 61 clock when creating every new permit, including withdrawal. `live-startup` uses measured issuance from the very first frame. It attempts one rearm against the actual first-pair spacing, and checks the output stays inhibited when that spacing misses the exact 10 ms rule. The sender records the nominal release, actual start, wire issue, and syscall completion separately. The same receiver and kernel freshness rules apply in all modes.

| Case | Planned stimulus and expected observation |
| --- | --- |
| healthy | Healthy synthetic set frames every 10 ms through 50 ms. REARM at 15 ms, enabled observations through 58 ms, explicit zero permit at 60 ms. |
| observed | Synthetic recovery as above; seq 3–7 use independently sampled creation ticks. The fifth receiver-header field counts valid-state disagreements. |
| live-startup | All seven issues use actual creation ticks. Inspect the two original issue ticks and the one rearm request. Inhibition is expected when the first pair misses exact spacing. |
| silence | After ENABLE, slot 2 waits for the controller's acknowledgement following the final good seq 3 activation, then closes the gate. A later acquisition is rejected. |
| phase | Same ordered loss injection. After the last good frame, target checks one tick before nominal 25, 30, 35 and 40 ms and one 5 ms period later. Compare actual completion stamps to the requested boundaries. |
| collision | Gate stays open. Last valid issue at 20 ms, no 30 ms issue, next in-sequence frame issued for 40 ms alongside a 40 ms controller release. Expiry wins when processing reaches the threshold. |
| invalid | MARK leaves gate open; independent CRC corruption, wrong data ID with recomputed CRC, future issue, sequence gap/duplicate and wrong channel with recomputed CRC are delivered during periodic checking. Valid state should remain at seq 3. |
| saturation | MARK leaves gate open. A burst fills both entries with invalid frames, and the third publish should return EAGAIN while retaining its writer. At 28 ms the original handle retries identical bytes and identical issue time. Invalid traffic continues. |
| queued | After seq 3, enqueue two later frames and retain a third writer on full. CLOSE occurs before the next planned controller drain. Those queued permits must never commit; retry of the retained writer while closed should return EPIPE. |
| delayed | Deliver seq 1 with issue 0 at 25 ms. Check seq remains zero, then accept a new seq 1 at 30 ms and seq 2 at 40 ms before a separate rearm. |
| restart | Withdraw at 30 ms. Stage a writer at 33 ms, CLOSE at 35 ms, OPEN generation 2 at 40 ms. Attempt to publish the old handle; deliver an old-generation frame through a fresh loan. Reject the old-generation and premature rearm commands. Accept new generation set frames at 45/55 ms, rearm at 59 ms, and withdraw at 75 ms. |
| early | An early rearm at 6 ms must be refused. Sample output after the second permit and before issuing a separate rearm at 15 ms. |
| boundary | The first rearm is scheduled for 20 ms after issue 0, with the second issue at 10 ms. The first recovery sample is no longer fresh, so output stays off. |
| one, no-rearm, bad-spacing | Omit respectively the second sample, the rearm command or the required exact spacing (issues 0 and 11 ms). Check output stays off. |

## Active receiver cases / 활성 수신기 시험

For VER-019, start each selector in its own ordinary three-process package under FIFO and RR. Use the existing build command below with the same selector in all three argument vectors. The supervisor waits for the controller to acknowledge accepted sequence 3 with the gate open, output enabled and fault clear. After this acknowledgement the sender offers a single test input at E+24 ms, a valid sequence 4 at E+30 ms, and a sequence 5 withdrawal at E+36 ms. The prior-generation case operates generation 1 before OPEN at E+35 ms and shifts these three times by 40 ms in generation 2. All wire issue times in these selectors are scheduled inputs. The receiver uses its actual post-decode clock. [Acceptance and traceability #397](https://github.com/Romanof123/JNOX/issues/397).

| Selector | Fixed stimulus | Expected private result |
| --- | --- | --- |
| `bad-crc`, `bad-header-crc` | Flip the payload permit or the version after computing the checksum | Crc (2) |
| `bad-length` | Declare 43 bytes inside a 44-byte loan | Length (1) |
| `bad-version`, `bad-type`, `bad-channel`, `bad-data-id` | Change the selected header field and recompute CRC | Format (3) |
| `bad-validity`, `bad-permit`, `bad-reserved` | Change the selected payload field and recompute CRC | Format (3) |
| `duplicate`, `reordered`, `gap` | Deliver sequence 3, 2 or 5 while 4 is expected | Sequence (8) |
| `old-generation` | Deliver a generation-1 frame while generation 2 is active | Generation (7) |
| `future-issue`, `expired-issue`, `issue-order` | Set issue to E+45 ms, E, or the prior accepted issue | Future (9), Expired (10), IssueOrder (11), respectively |
| `wrong-owner` | With an ordinary writer staged in PID 0, PID 2 requests a producer loan through SVC 33. It receives no handle. PID 0 publishes its original grant. | Kernel EACCES (−13) before allocation |

Check the input handle and complete private payload, selected rejection, unchanged accepted sequence/issue/permit/recovery, absence of a kernel COMMIT for the rejected frame, subsequent sequence-4 acceptance while sequence 3 is still fresh and the physical-counter bracket around output withdrawal. For the role case, match the authenticated PID, operation, errno and generation in safety journal kind 9 to the supervisor's result. Match the sender's staged handle to its later publication and actual delivery. The gate remains under its normal application policy in all cases.

각 입력은 독립 부팅에서 활성 상태의 수신기에 한 번 전달한다. 순번 3이 승인되고 출력이 켜졌는지 먼저 확인한다. 거부 전후의 마지막 승인 순번·발행 시각·허가·복구 횟수, 실제 오류 코드, 커널 커밋 여부를 확인한다. 다음 정상 순번 4는 직전 허가가 만료되기 전에 승인돼야 한다. 권한 시험에서 감독 슬롯은 인자가 없는 대여 획득 호출을 사용한다. 원래 소유자의 핸들과 데이터는 소유자에게 남는다. 실제 커널의 호출자 기록 및 같은 핸들의 정상 게시·수신을 대조한다.

## Build and capture

Use `phase-decision` or `phase-complete` in all three argument vectors with `--safety-boundary`. To advance the controller's requested wakes for its ninth and tenth activations, append two extra `--peer-arg` values after the controller selector. Each is 0–12500 ticks written with one to five ASCII digits. Omit both for zero leads. The register and eligibility remain under their normal owners. Both fixtures keep the original real final-sample acknowledgement, CLOSE, sender refusal and complete controller service sequence. [Worst-phase acceptance](../../docs/native-safety-boundary-bench.en-ko.md) · [#400](https://github.com/Romanof123/JNOX/issues/400).

In the two measured phase images, event 16 records the journal tick, counter samples immediately before and after the final active-to-inactive store and its readback, generation, accepted sequence, accepted issue, fault, and active flag before/after. The receiver array `safety_phase_samples[2][12]` records activation index, nominal target, lead, requested wake, observed wake, entry INFO tick, private entry latch result, private pre-COMPLETE INFO tick and latch result, post-COMPLETE INFO tick and latch result, and post-COMPLETE protected flags. Each row corresponds to activation 8 or 9 (zero based).

세 역할에 같은 위상 선택자를 넣는다. 두 리드 값은 제어기 기상 요청만 앞당긴다. 이벤트 12의 만료 판정 피연산자, 이벤트 16의 활성화 종료 저장 구간, 수신기 자체의 검사 시각은 각각 원래 경로에서 기록한다.

The ordinary `recover-latch` and `recover-hold` selectors exercise channel generation recovery with three live processes. Both first enable on generation 1, stop issuing permits after sequence 3 at E+20 ms, and wait for a real age-expiry latch and output fall. OPEN starts generation 2 at E+50 ms. The new set permit issues are E+55 and E+65 ms. An early REARM between these two commits must be discarded. The positive case issues a separate REARM at E+70 ms and withdraws with sequence 3 at E+76 ms. The hold case omits both operations. Verify the actual clock and device transitions throughout, including continuous inhibition after the pair in the hold case. [Recovery criteria and provenance](https://github.com/Romanof123/JNOX/issues/399).

두 시험 모두 첫 세대에서 실제 허가 만료로 출력이 꺼진 뒤 새 세대를 연다. 이른 재무장 거부 후 두 번째 허가가 도착해도 출력은 차단 상태여야 한다. 양성 시험은 별도 명령으로 재무장한 뒤 허가를 철회한다. 대조 시험은 새 명령 없이 출력을 끝까지 관측한다. 프로세스는 세 개 모두 살아 있는 상태에서 실행한다.

For the measured REARM cases, give all three apps the selector `rearm-15` or `rearm-20`. Package with `--safety-boundary` in addition to the safety channel and loan route. The optional second supervisor argument (`--peer2-arg 00000` through `--peer2-arg 12500`) advances only its wake request, in counter ticks. Omission selects zero. The sender issues exactly two set permits at E and E+10 ms. The supervisor verifies inhibited sequence 2, then makes its first REARM call. The four words in `safety_rearm_launch` give the nominal decision target, lead, requested wake, and observed wake. The ordinary command row records the actual return. Its expected-status sentinel 2⁶³ allows either normal outcome during measurement.

In a boundary image, safety event 15 records the journal timestamp followed by the original eligibility operand, caller PID, generation, first issue, second issue, streak, pending before consumption, and eligibility. A journal stamp or requested wake can differ from the decision tick. For standalone reproduction use the ordinary build command below with your selector and add `--safety-boundary` to the package command. [Acceptance and measurement rules](../../docs/native-safety-boundary-bench.en-ko.md) · [#398](https://github.com/Romanof123/JNOX/issues/398).

세 앱에 같은 재무장 선택자를 지정한다. 두 허가의 발행 시각과 수신기의 검사는 원래 경로를 따른다. 감독자의 기상 요청만 틱 단위로 앞당길 수 있다. 이벤트 15에서 실제 권한 주체·재무장 판정 시각·허가 두 건과 결과를 읽고 출력 전이와 대조한다. 목표 틱과 실제 틱의 편차를 부팅마다 남긴다.

Run these commands at the integrated repository revision containing the protected-frame library and safety SDK/runtime. The processor, frequency, user ELF hashes, launch manifest and QEMU binary identity belong to each independent capture.

```bash
set -eu
out=$(mktemp -d)
app=examples/native-safety-channel
for role in sender receiver supervisor; do
  sh sdk/native-aarch64/build-rust.sh "$app/$role/Cargo.toml" "$out/$role.elf"
done
revision=$(git rev-parse HEAD)
cases=(healthy observed live-startup silence phase collision invalid saturation queued delayed restart early boundary one no-rearm bad-spacing
       bad-crc bad-header-crc bad-length bad-version bad-type bad-channel bad-data-id bad-validity bad-permit bad-reserved
       duplicate reordered gap old-generation future-issue expired-issue issue-order wrong-owner rearm-15 rearm-20
       recover-latch recover-hold phase-decision phase-complete)
for policy in fifo rr; do
  quantum=()
  if [ "$policy" = rr ]; then quantum=(--quantum-ticks 625000); fi
  for scenario in "${cases[@]}"; do
    boundary=()
    if [[ "$scenario" = rearm-* || "$scenario" = phase-* ]]; then boundary=(--safety-boundary); fi
    pkg="$out/$policy-$scenario"
    python3 tools/build_native_el0.py --out "$pkg" --board virt --policy "$policy" "${quantum[@]}" \
      --elf "$out/sender.elf" --peer-elf "$out/receiver.elf" --peer-elf "$out/supervisor.elf" \
      --arg "$scenario" --peer-arg "$scenario" --peer2-arg "$scenario" \
      --priority 7 --peer-priority 6 --peer-priority 4 --ipc-route 2:0 --ipc-route 2:1 \
      --safety-channel --loan-route 0:1 "${boundary[@]}"
    python3 tools/native_el0_capture.py "$out/cap-$policy-$scenario" "$pkg/kernel.elf" \
      "$out/sender.elf" "$app/sender/src/main.rs" "$revision" --manifest "$pkg/manifest.json" \
      --plain --expect-exit 71 --timeout 25 --uart-limit 8388608
  done
done
```

Expected role exits are 71, 72 and 73. A start offset of 50 ms allows the three roles to exchange the epoch. The controller runs 17 releases. Each worker acknowledges READY after finishing its measurements and private page accesses, then awaits STOP. The supervisor initiates this two-phase termination after nominal 85 ms. A device change due to process exit belongs to teardown.

## Raw observations

Read each symbol using its own ELF and process page-table root. Arrays have fixed capacity; use the associated count for the valid prefix. Signed errors occupy their two's-complement u64 bits. `0x8000000000000000` means an operation was skipped.

| Producer, slot 0 | Layout |
| --- | --- |
| safety_sender_header | Magic, case ID, epoch, Hz, unexpected syscall-result count |
| safety_sender_rows, safety_sender_count | Planned release tick, actual creation/start, actual end, mode, input kind, generation, sequence, wire issue tick, loan handle, acquire result, publish result |
| safety_sender_bytes | Entire 44-byte frame per sender row. Modes 0,1,2 are acquire/publish, stage only, and retry prior handle. Input kinds 0–9 are nominal, CRC, data ID, future, gap, duplicate, channel, late, prior generation and observed-time issuance. Kind 10 selects the fixed active-case input in the table above. |

| Controller, slot 1 | Layout |
| --- | --- |
| safety_receiver_header | Magic, case, epoch, Hz, sequence/issue disagreement count, internal run status |
| safety_activations, safety_activation_count | Planned release, actual wake, post-CHECK stamp, post-COMPLETE stamp, entry generation/flags, private expiry latch, two receive statuses, private/kernel rearm status, end flags/sequence/issue, private sequence/issue/recovery/enabled/faulted |
| safety_received_rows, safety_received_bytes, safety_received_count | Activation index, attempt index, handle, VA, length, library status (Error enum code), kernel COMMIT status, private decision stamp, observed end stamp, kernel generation/flags/sequence, four private valid-state words before, four after. The four words are sequence, issue, permit and recovery count. Byte rows are the actual private copies. |
| safety_active_received | One row per active-case receive. Each half records private generation/enabled/fault and kernel tick/generation/flags/sequence/issue, before and after validation. |
| safety_active_ack | LAST_SAMPLE request and reply, info status, private generation/sequence/issue/permit/enabled/fault, kernel tick/generation/flags/sequence/issue. |
| safety_exact_clock_control | A deterministic private clone of the live seq 3 receiver in collision/phase. Fields are present, last issue, expiry tick, latch one tick before, latch at equality, same-tick next-frame rejection, final sequence/enabled/faulted, worst-phase next-check tick, its latch, its enabled bit. This clock-input control never drives ENABLE. |
| safety_exact_rearm_control | In early/boundary, independent receiver copies receive exact 15 ms and 20 ms clock inputs after the observed qualifying pair. Fields are present, first tick/status/enabled, second tick/status/enabled. An absent row indicates that the observed pair was unavailable in time. |

| Supervisor, slot 2 | Layout |
| --- | --- |
| safety_supervisor_header | Magic, case, epoch, Hz, unexpected observation count, internal run status |
| safety_supervisor_rows, safety_supervisor_count | Planned tick, actual before/after, command (0 observation), generation argument, actual/expected return, before/after flags, before/after generation, before/after sequence, before/after last issue, comparison result |
| safety_active_ipc | Four rendezvous rows containing peer PID, request, reply and status. The final two rows serve the staged-loan role case. |
| safety_wrong_owner | Presence, two counter bounds, signed acquire result, generations before/after, flags before/after, sequences before/after, issues before/after, expected-state flag and cleanup status. |
| safety_live_startup | Observed first and second wire issues, sampled pre-rearm tick, exact-spacing predicate, expected eligibility on this run |

Use kernel `NATIVE SAFETY EVENT` rows and `jnox_virtual_permit_enable` RAM for independent state-transition evidence. The producer's before/after calls, the controller's model and the supervisor's polling each answer different questions. Type 11 brackets actual publication; type 7 records authenticated receive data; type 4 brackets injection; type 3 records detection; type 2 captures actual atomic output transition; type 10 brackets COMPLETE. Check dropped records. Compare requested release/issue ticks against actual brackets before calling two operations simultaneous. The deterministic equality rows can accompany a scheduled guest observation when exact guest-tick coincidence was missed.

Score release-to-completion and maximum completed-check spacing separately. Score ordered fault/detect/safe observations and the 25 ms, 5 ms and 30 ms budgets using the conservative ends of their raw intervals. A successful app exit cannot establish these timing outcomes. These fixtures provide no physical actuator, hardware reaction time, WCET admission or product safety qualification.
