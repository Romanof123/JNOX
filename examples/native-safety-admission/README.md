# Protected permit admission workload / 보호 허가 채널 수용 검증 작업

Three ordinary AArch64 executables exercise a CPU-only virtual permit output. The controller (slot 1, priority 7) outranks the sender (slot 0, 6) and supervisor (slot 2, 4). Each executable receives the same scenario argument. Source defines the requested releases and inputs. Admission and timing verdicts require the retained raw trace for each package and justified interference bounds. The original sixteen scenarios and their single-check boundary setup remain in [native-safety-channel](../native-safety-channel/README.md). The limits and evidence requirements are in the [admission design](../../docs/native-safety-admission.en-ko.md).

## Release calendar

The supervisor opens generation 1, chooses a common raw-counter epoch E 50 ms ahead of its first clock sample, sends E to both workers through synchronous calls and checks both acknowledgements before E. Trace the initial wait for all three roles through the first idle interval. After E the workers have no synchronous dependencies through the end of their measured jobs.

* The controller executes 16 jobs from E. A starts at E + 5k ms. After its work it issues exactly one native absolute arm-and-block for B at E + 5k + 2.5 ms. A and B both call the real expiry-first CHECK before input and COMPLETE before saving their measured snapshots. Each phase polls one loan, copies at most 44 bytes privately, releases and validates it. Each job calls the receiver's budget reset once and allows at most one generation-open or rearm action across both phases. Private stores and the segment finish marker follow COMPLETE. Include all trailing EL0/kernel work up to effective block, final park or exit when calculating full response.
* The sender executes 8 jobs at E + 10k ms. Publication includes encoding a frame, writing a live loan and performing SVC 34. Failed acquisition and publication have their own fields. A full queue preserves the held writer for at most one retry on the next job, using its original bytes and issue time. Each job attempts at most one retained publication plus three new frames.
* The supervisor has a finite job at each of E + 15, 17.5, 22.5, 35, 55, 57.5, 77.5 and 80 ms. In `lower-work` it adds one off-grid release at 19.9 ms. Each regular job reads state twice and requests zero or one command before logging the row. At 15 ms, the higher-ranked A runs before the supervisor posts the first rearm. At 17.5 ms B can consume it while both first permits are fresh. The lower-ranked supervisor at 17.5 ms captures the result. Require evidence of ENABLE by 18 ms and, where applicable, still enabled at 37 ms. Use both device transition brackets and observer timestamps. A pending request alone is insufficient. At 80 ms the supervisor completes the measurement and releases both parked workers using two rounds of IPC. Exit-driven changes are teardown.

| Argument | Controlled input and question |
| --- | --- |
| `healthy` | Synthetic first pair at E and E+10 ms, then live creation ticks. Seq 8 withdraws at its 70 ms release. Inspect rearm, sustained delivery and withdrawal. |
| `live` | All eight frames use the actual SVC 61 creation sample. Compare the first pair's exact raw spacing to 10 ms and the one rearm result. A spacing miss must leave ENABLE inhibited. Coincident nominal releases do not establish simultaneous execution. |
| `silence` | After the planned third acceptance the supervisor closes the gate at 22.5 ms. Confirm that acceptance precedes CLOSE, later acquisitions fail, original data ages out and the register inhibits. No post-release IPC forces that ordering. |
| `invalid` | After valid seq 3, MARK at 22.5 ms and repeatedly offer CRC-damaged seq 4. Inspect actual receive bytes, rejection, persistence of the last valid tuple and fault response. |
| `saturation` | After the same setup, offer at most three CRC-damaged frames per 10 ms release. Confirm occupancy, actual EAGAIN, the identity and bytes of any retained retry, and freshness under load. |
| `collision` | Seq 3 carries the E+20 ms issue, no send at 30 ms, then seq 4 carries an E+40 ms issue on the controller A release. Compare real publish/accept/detect clocks. A publication after the deadline is a late arrival. The separate private clone checks exact equality. |
| `delayed` | Empty sender jobs at 0/10/20 ms. At 30 ms submit seq 1 issued at E. Then submit a fresh synthetic pair at 40/50 ms and request rearm at 55 ms. B at 57.5 ms can apply it. |
| `boundary-witness` | Seq 3 carries E+20 ms **plus one raw tick**. Subsequent sender jobs stay empty. On a clone of the real accepted and enabled seq 3 state, check one tick before expiry then precisely 5 ms later. On another clone submit valid seq 4 exactly at expiry. The actual guest controller continues on the 2.5 ms grid throughout. |
| `lower-work` | Healthy stream plus 32 finite rounds of 128 EL0 arithmetic operations and a clock SVC per round, bracketed by two reads, at 19.9 ms. This changes the timer calendar. Determine any actual lower-priority residual kernel work at a higher-rank release from the trace. |
| `overrun` | Healthy stream plus finite extra work at job 4 A. Up to 4096 batches of 128 EL0 operations, stopping at the first observed 1.1 ms interval. Require refusal when the measured response exceeds the inclusive 1 ms phase limit. A run hitting its sample cap early needs independent scoring. |
| `gate-invalid` (11) | The supervisor requests REARM at 15 ms, observes ENABLE high with accepted sequence 3 immediately before CLOSE at 22.5 ms, then requests one fixed invalid downstream fixture at each of 25, 30, 35, 40, 45, 50 and 55 ms. |
| `gate-saturation` (12) | Same calendar, with three fixture requests per release on the two-entry queue. At least one full-queue EAGAIN must occur. Observe actual results and any intervening controller drain for every request. |

The two gate cases require the downstream qualification package. Their supervisor also observes at 17.5, 57.5, 77.5 and 80 ms. The regular sender submits valid, enabled permits on its original eight releases. It checks that three publications succeeded and that all five later acquisitions returned EPIPE. The controller follows its ordinary 32 phases. For each successful fixture, join its handle and bytes to the controller receive, release and rejection. Check actual occupancy around EAGAIN, preserve the last accepted sequence and issue, and match protected expiry to the low output transition. At least one fixture must complete before expiry and another must begin at or after it.

## Reproduce on the integrated revision

Run with Bash, the AArch64 SDK/toolchain and QEMU virt. The integration must contain the opt-in package flag, absolute-wait and trace ABI (SVC 62), protected-frame dependency and the three executables. Store the assembled source revision, emulator version and hash, toolchain versions, effective quantum, invocation, manifest and all ELF hashes with the raw RAM/UART. In particular, use a fresh capture directory for each run.

```bash
set -eu
out=$(mktemp -d)
app=examples/native-safety-admission
rev=$(git rev-parse HEAD)
for role in sender controller supervisor; do
  sh sdk/native-aarch64/build-rust.sh "$app/$role/Cargo.toml" "$out/$role.elf"
done
for policy in fifo rr; do
  quantum=()
  if [ "$policy" = rr ]; then quantum=(--quantum-ticks 625000); fi
  for scenario in healthy live silence invalid saturation collision delayed boundary-witness lower-work overrun gate-invalid gate-saturation; do
    pkg="$out/$policy-$scenario"
    fixture=()
    case "$scenario" in gate-invalid|gate-saturation) fixture=(--safety-downstream-injector);; esac
    python3 tools/build_native_el0.py --out "$pkg" --board virt --policy "$policy" "${quantum[@]}" \
      --elf "$out/sender.elf" --peer-elf "$out/controller.elf" --peer-elf "$out/supervisor.elf" \
      --arg "$scenario" --peer-arg "$scenario" --peer2-arg "$scenario" \
      --priority 6 --peer-priority 7 --peer-priority 4 \
      --ipc-route 2:0 --ipc-route 2:1 --loan-route 0:1 --safety-channel --safety-qualification "${fixture[@]}"
    sha256sum "$out"/{sender,controller,supervisor}.elf "$pkg/kernel.elf" "$pkg/manifest.json" > "$pkg/qualification.sha256"
    python3 tools/native_el0_capture.py "$out/capture-$policy-$scenario" "$pkg/kernel.elf" \
      "$out/sender.elf" "$app/sender/src/main.rs" "$rev" --manifest "$pkg/manifest.json" \
      --plain --expect-exit 71 --timeout 40 --uart-limit 8388608
  done
done
```

Expected clean process exits are 71, 72 and 73. They indicate completion of the workload. Score each scenario, rank, phase response, check gap and timing boundary separately, including the initial idle and final B. A missed continuation yields no qualifying job even if the kernel lets the process continue after a late stage-1 call.

## Raw evidence

All columns below are u64 in the private address space of the corresponding ELF. Interpret signed statuses as two's complement. Bit 63 alone denotes a skipped operation. `admission_header` in each ELF contains magic, case ID (table order), E, frequency in ticks/s, number of completed segments and internal error (zero on clean execution). For the controller, row `2*k` is A and row `2*k+1` is B. Use each process's ELF symbols and actual translation-table root for private RAM.

| Symbol | Column order |
| --- | --- |
| `admission_sender_rows`, `admission_sender_count`, `admission_sender_bytes` | Job, target, creation tick (0 on retry), post-attempt tick, seq, encoded issue, handle, acquire status, publish status, retry bit. Each byte row has the actual 44-byte input. Match the previous failed publication by handle and bytes on retry. |
| `admission_controller_rows` | Target, post-CHECK tick, post-COMPLETE tick, poll status, private accept error (positive enum), kernel commit status, private decode decision tick, command (1 open / 2 rearm), local command error, kernel rearm status, kernel flags/seq/issue, private seq/issue/recovery/enabled/faulted. |
| `admission_supervisor_rows` | Target, before/after, command or 0, signed command result, before/after flags, before/after seq, before/after issue, generation. |
| `admission_boundary_inputs` | Present, last valid issue, one-tick-before input and latch bit, 5-ms-later input and latch bit, exact-expiry input, private frame error, equal-input final seq/enabled, gap-input final enabled. A zero present field leaves the witness open. |
| `admission_overrun`, `admission_lower_work` | Respectively start/end/batch count, and start/end/round count/checksum. |
| `admission_gate_attempts`, `admission_gate_attempt_count` | Supervisor-only. Each of up to 21 rows holds supervisor job index, index within the burst, ticks sampled before and after the request, and signed status. The count identifies written rows. |

Join `NATIVE QUAL ROW` markers by PID/job/phase/raw target. Stage 1 requests an absolute wait, stage 2 is the first EL0 marker after it returns, stage 3 follows segment work. Establish actual block, kernel-ready transition, dispatch/return interval, decision brackets, next block or final park from the scheduler/IRQ rows. The EL0 markers alone cannot timestamp actual ready or all trailing cost. Cross-check SVC 60 decisions with `NATIVE SAFETY EVENT` types 2 (register change), 3 (expiry), 4 (injection), 6/7 (loan and bytes), 10 (completion) and 11 (publication). Require all expected rows and zero dropped records. Score target→finish, consecutive completed-check gaps including B→next A, sender responses, 25/5/30 ms fault bounds and one-tick outcomes with raw integer ticks and documented observer uncertainty. A decision based on the live counter is a separate observation from a scripted or private-clone tick.

## 한국어 재현 및 판정

게이트 검증은 15 ms에 재무장하고 22.5 ms에 마지막 수락 순번 3과 ENABLE을 확인한 뒤 닫는다. 이후 송신자의 정상 프레임 다섯 개는 대여 요청에서 EPIPE를 받아야 한다. 감독자는 25~55 ms 동안 5 ms마다 고정 시험 프레임을 한 건 또는 세 건 요청한다. 실제 대여 핸들, 수신 바이트, 거부 상태와 큐 점유율을 원자료에서 연결하고 마지막 수락 시각, 보호 만료 판정 및 출력 하강을 확인한다.

통합 커밋에서 위 Bash 명령으로 송신자·제어자·감독자 ELF를 한 번 빌드한 후 FIFO와 RR의 각 시나리오를 별도 디렉터리에 패키징한다. 우선순위는 제어자 7, 송신자 6, 감독자 4다. ELF·커널·매니페스트의 해시, QEMU 및 도구 버전, 카운터 주파수, 실제 양자와 UART/RAM 원본을 함께 보관한다. 세 프로세스가 E 전에 초기 교신을 마치고 모두 대기했는지 확인한다.

제어 작업은 E부터 5 ms마다 16번 해제된다. A를 마치면 같은 작업이 E+5k+2.5 ms에 한 번만 절대 대기·재개된다. A와 B 각각 실제 만료 우선 검사, 최대 1회 대여 수신, 완료를 수행한다. 수신 예산과 열기·재무장 한도는 작업 전체에서 공유한다. 송신자는 10 ms마다 8번 실행된다. `live`에서는 매 프레임을 실제 생성 시각으로 발행하며 최초 두 허가의 간격이 정확히 10 ms인지 확인한다. `healthy` 등 명시된 합성 첫 쌍과 구분한다. 일반 복구는 감독자가 15 ms에 재무장을 요청하고 B가 17.5 ms에 처리할 기회를 갖는다. 18 ms의 ENABLE과 37 ms 시점 상태는 실제 레지스터 쓰기·읽기와 시간 구간으로 판정한다.

무송신·반복 손상·포화·충돌·지연 사례에서 각각 최종 정상 프레임, 큐 상태, 발행과 수신 순서, 검출과 가상 출력 차단을 확인한다. `boundary-witness`의 한 틱 전/5 ms 뒤 및 정확한 동일 시각 비교는 승인된 실제 상태에서 복제한 모델 입력이다. 독립 실시간 가상시계 경계 실험과 기존 단일 검사 `phase` 사례는 별도 결과로 보관한다. `lower-work`는 19.9 ms에 감독 부하를 추가한 비격자 실험이며 `overrun`은 의도적인 위상 예산 초과 대조군이다.

스케줄러 준비·디스패치·판정·실제 대기까지의 전체 종료와 타이머 비교·IRQ 진입/종료를 연결한다. B에서 다음 A까지를 포함한 검사 간격, 송신 응답, 25/5/30 ms를 각각 검토한다. 누락·손실·시계 오차가 있는 구간은 미정이다. 유한 관측으로 최악 블로킹이나 IRQ 버스트 상한, FIFO/RR 정책 동등성, 물리 구동기 응답을 확정할 수 없다. 이 앱은 별도의 IRQ 발생기를 제공하지 않는다.
