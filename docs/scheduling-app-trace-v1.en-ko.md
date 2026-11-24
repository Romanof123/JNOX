# CPU0 scheduling and linked application trace v1 / CPU0 스케줄링·응용 추적 v1

Prospective CHG-102 · IMP-102 · EVD-102 · #268 / #137 / #21. Baseline ec00258bad43e510e8fd9ee609a98845a4107231. This slice covers one ordinary linked Rust workload on AArch64 virt, CPU0, four immutable tasks. The broader [application tracing proposal](application-tracing.en.md) and native C SVC ABI have separate gates. No IPC, driver instrumentation, arbitrary task emit, syscall, generalized IRQ tracing or prebuilt application support is introduced.

## Producer and authority / 생성 위치와 권한

Add kernel/src/sched_app_trace.rs and compile its module and entire call statements with cfg(jnox_sched_app_trace). JNOX_SCHED_APP_TRACE unset means compiled OFF. Accept disabled, active, saturate only for virt/multilevel/mlfq-wake. Reject unknown values and unsupported selector combinations before launch. Keep JNOX_MULTILEVEL_TRACE independent. Proposed crate-local Rust entry points below use slot usize (0..3), timestamps u64. start(freq:u64), block(owner:usize,stop:u64), ready(owner:usize,target:usize,w:u64), irq_enter(owner:usize,irq:u32,instance:u64), irq_exit(owner:usize,irq:u32,instance:u64), dispatch(out:Option<usize>,next:usize,start:u64), receipt(value:u64,end:u64), freeze(). Restrict receipt to the trusted linked target path, with fixed slot 0/type. Kernel controls all other events and startup/freeze. Export only via authorized halted VM observer. There is no untrusted producer isolation claim for this shared EL1 image.

| ID | Site, semantics | actor, subject, arg0, arg1 |
|---|---|---|
| 1 BLOCK | multilevel_service::Runtime::finish after successful close_interval for kind 2; original SVC stop | owner, owner, 0, 0 |
| 2 READY | immediately after successful policy.wake, fresh post-enqueue Q; original wake SVC stop W in arg0 | waker, target, W, 0 |
| 3 DISPATCH | Runtime::select after actual selection and self.start=counter(), initial dispatch included; boundary before restoration/ERET | previous or none, selected, 0, 0 |
| 4 TIMER_ENTER | fresh counter after timer/control/due validation immediately before state.finish. Instance is original handler counter before IAR, strictly increasing per accepted IRQ | interrupted owner, interrupted owner, IRQ line, instance |
| 5 APP_RECEIPT | mlfq_wake::target after packet calculation, put(28,receipt), original counter read and put(9,E); pass the SAME E and receipt | slot 0, slot 0, receipt digest, 0 |
| 6 TIMER_EXIT | fresh counter immediately after finish returns selected frame and before Rust return, paired by same line, instance and original owner | interrupted owner, interrupted owner, IRQ line, instance |

Initialize before first dispatch. Freeze after last scheduler event at terminal; the last task exits by SVC kind 1 after all four receipts. Require this terminal path and every prior IRQ pair closed. No selection emits no dispatch. An incomplete/nonreturning IRQ is diagnostic. Counter is the existing ISB + CNTPCT_EL0 and frequency CNTFRQ_EL0. A record timestamp belongs to its documented observation; commit order can lag that sample if the application is interrupted. Do not reposition E or take a second endpoint sample. Within-session counter reset/wrap, invalid joins or unsupported stop invalidate affected conclusions. No host time conversion required. Count deltas / CNTFRQ give seconds for this virtual clock only.

## Binary ABI and identity / 이진 형식·식별

Externally named, 8-aligned symbol jnox_sched_app_trace_v1. All words little-endian u64 at offsets 8*i; static object size 8320 bytes = 128 + 128*64. Header words 0..15: eight literal ASCII bytes JNXTRC01, schema 1, header bytes 128, record bytes 64, physical slots 128, logical capacity (128 normally, 4 saturation), state (0 disabled, 1 active, 2 frozen), CNTFRQ, boot ordinal 1, session ordinal 1, committed count, dropped kernel, dropped application, flags (bit 0 any counter saturated, all other bits zero), CPU 0, reserved zero. One cold boot and at most one capture per independent capture directory. Full names use independent launcher-generated run UUID + boot + session. Never merge two boots with the same external run UUID. Disabled initializes descriptor and frequency but never arms/finalizes recording. OFF has no recorder symbol or storage.

Record k (zero based) starts at 128+64*k. Its words 0..7: commit sequence k+1, event ID, counter ticks, actor, subject, arg0, arg1, reserved zero. Sequence zero means unpublished. Thread refs use (generation<<32)|slot, generation=1 for the single incarnation of each statically linked task, zero ref means absent. CPU and session inherited from header. No PID reuse, restart, migration, clock conversion across captures or process spans.

CPU0 regular task, SVC and validated IRQ writers read prior DAIF.I, set only I across the bounded slot operation, then clear only I if originally clear on every exit. Preserve D/A/F, with compiler memory barriers around both mask transitions. Thus an IRQ arriving while recording is pending until restoration; an IRQ before reservation can publish first. Already-masked callers remain masked. FIQ, NMI, debug/SError, recursive logging, SMP and concurrent readers unsupported. No allocation, formatting, spinning, guest exporter, user-supplied pointer or RAM overwrite. When disabled return on the enable check. On full increment source drop count with saturation and sticky flag on overflow; do this before any slot/body copy. On accepted event write body, release publish sequence, then advance committed. Freeze with same exclusion, drain only with CPU halted and frozen; incomplete freezes retained as diagnostic evidence. Record storage remains 8320 bytes including header at logical capacity 4. Report any other static RAM, stack and code size separately via ELF/disassembly, including IRQ mask duration or unmeasured limits.

## Decode, integrity and independent acceptance / 복호화·검증

Implement tools/sched_app_trace_decode.py separately from the producer. Exchange raw trace.bin, kernel.elf, manifest.json, manifest.sha256, decoded.json, original mlfq_wake_witness.bin, UART, register/IRQ transcript and launch/build logs. Manifest records schema, dictionary, profile, effective flags/mode, frequency, run UUID, IDs, source commit/dirty status, command/tool versions, exact ELF symbol address/size from symbol table and raw extraction address/length. Hash exact raw file bytes using SHA-256, including ELF, capture and all artifacts used as evidence; sidecar hashes exact manifest bytes (decoded output may instead have a separate derived digest). Verify live captured bytes are for symbol at stated address in launched ELF, flags and static extent, strict schema/lengths, unused reserved fields, namespace/type/arguments, commit sequence, unused zero slots, counts and clock before deriving intervals. Disabled implies zero commits/losses; frozen count ≤ logical capacity; loss implies full; a saturation flag requires a counter at u64::MAX (MAX alone does not prove overflow). Require same line, instance and interrupted owner per IRQ pair, strictly increasing instances, no overlap or orphan. Report TIMER_EXIT minus TIMER_ENTER as Rust service duration; it can overlap E−S. A hash list requires independent launch provenance for attribution. Mark truncated, unknown, malformed, discontinuous or lossy coverage incomplete; never substitute a timestamp from the witness for an absent event.

Join successful READY(1,0) Q carrying W, subsequent first DISPATCH(*,0) S and APP_RECEIPT(0) E in same run/boot/session/generation; require a matching prior BLOCK and no intervening target stop before E. Require W≤Q≤S≤E. Publish W,Q,S,E, Q−W, S−Q (ready to dispatch), S−W, E−S and E−W separately. Use original witness for stop, frame/PC restoration and peer FIFO/reset checks; compare all matching recorder samples against it. PM supplies the historical baseline after first acquisition. Actual counts/digests vary with timing.

Retain first OFF, runtime-disabled, active and saturating guest executions including failures. Independent reviewer reconstructs four useful packet results and supplier sequence from witness per [original design](mlfq-wake-design.en-ko.md), validates ordinary UART completion, IRQ and timer terminal state, restored frame, and independently checks U=floor(CNTFRQ/100), P=10U, D=U, A=2U, R=U, H=2U, B=P+D+3(A+D)+R+H=23U. Preserve per-boundary lateness, first boost/precharge, ahead-peer order, stop-to-start gaps, uninterrupted partial and original exact wall decomposition. Verify (Q−W)+(S−Q)+(E−S)=E−W ≤ B. In overflow mode require first four committed immutable and exact source losses by independent witness replay, useful work and original bounds still pass; withheld trace latency when missing. Verify runtime-disabled zero commits/losses and compiled-OFF ELF/map/disassembly absence. Compare RAM, text, exact time deltas and directly observed overhead for named runs; one QEMU observation cannot establish WCET. Host controls cover invalid header/length/ID/commit/loss, hash/ELF substitution, generation mismatch, saturation-counter limit, disabled, IRQ-mask preservation and attempted nested IRQ while task writes. Flag any nesting behavior supported only by inspection or a model.

Re-run latest affected selectors under both existing plain/full-frame trace switches: mlfq-wake and mlfq-wake-refuse (B−1, zero dispatch), mlq, mlfq, mlq-admit/refuse, mlq-saturated/blocked/exited and mlfq-delayed when common multilevel build/selection paths change. Preserve numerical and functional assertions and first captures. Test unrelated selector option rejection without widening runtime scope. Independent acceptance attaches provenance, exact missing evidence and failed attempts.

## 한국어 규약

CPU0 단일 부팅에서 커널만 수면(1), 준비(2), 배정(3), 타이머 처리 진입(4)과 복귀(6)를 기록한다. 링크된 0번 태스크만 결과 게시(5)를 남기며 허용 값은 실제 게시한 64비트 계산값 하나이다. IRQ 진입·복귀는 같은 인터럽트 번호·원래 소유자·인스턴스로 대응한다. 마지막 태스크의 종료 SVC에서 배정 대상이 없어지면 동결하며 열린 IRQ가 있으면 완전한 기록으로 인정하지 않는다.

헤더는 리틀엔디언 64비트 16워드이며 위 순서대로 크기·용량·상태·주파수·식별자·게시/손실 수를 담는다. k번 레코드는 128+64k 바이트에서 시작한다. 여덟 워드는 게시 순번(k+1), 이벤트, 카운터, 행위자, 대상, 인자 두 개, 예약 0이다. 실행 UUID와 부팅·세션 번호가 일치하고 슬롯·세대가 같은 기록만 연결한다. 깨움 SVC 정지 W, 준비 큐 삽입 직후 Q, 첫 재배정 S, 결과 게시 직후 E로 S−Q와 E−S를 따로 계산한다. 네 태스크 계산값과 원래 23U 한도를 독립 검사한다.

기록 중 일반 IRQ만 차단하고 원래 마스크를 복원한다. 포화 시 복사 전에 출처별 손실을 집계하며 기존 슬롯을 보존한다. 누락 구간 지연은 판정할 수 없다. 비활성·활성·포화·컴파일 제외 조건의 최초 결과와 실패를 보존하고 승인된 정지 VM 관찰자가 원본을 수집한다.

## Implementation capture command / 구현 원본 수집 명령

On Linux with aarch64-linux-gnu-nm available, run one cold guest per fresh directory from the repository root. Export identical board/profile/app and trace settings for build and launch. For each case select disabled, active, saturate or unset JNOX_SCHED_APP_TRACE for OFF. Record the exact build command and tool versions in the build log. Keep both logs and seal the first attempt after the observer returns, including a failing observer result.

```sh
export JNOX_BOARD=virt JNOX_PROFILE=multilevel JNOX_APP=mlfq-wake
export JNOX_SCHED_APP_TRACE=active
out=$(mktemp -d)
export SCHED_APP_TRACE_CAPTURE_DIR="$out/guest"
export CARGO_TARGET_DIR="$out/target"
(cd kernel && set -x && cargo +1.95.0 -V && rustc +1.95.0 -Vv && aarch64-linux-gnu-nm --version && cargo +1.95.0 build --locked --release -j1) >"$out/build.log" 2>&1
python3 tools/qemu_boot.py --profile "$JNOX_APP" --qemu tools/sched_app_trace_capture.py --kernel "$CARGO_TARGET_DIR/aarch64-unknown-none/release/jnox-kernel" >"$out/boot.log" 2>&1
python3 tools/sched_app_trace_decode.py seal "$SCHED_APP_TRACE_CAPTURE_DIR" --build-log "$out/build.log" --boot-log "$out/boot.log"
python3 tools/sched_app_trace_decode.py decode "$SCHED_APP_TRACE_CAPTURE_DIR"
```

각 모드는 새 디렉터리에 수집하고 실패한 첫 관찰도 원본 로그와 함께 봉인한다.


## Prospective halted-extraction correction (#272) / 정지 상태 수집 수정 계획

Source 33d8fbfedededb5147d5de93bef044822810c846 passed the public boot observer and seal for reported positions, then failed public decode because the trace wrapper requested halt evidence but the QMP snapshot path never recorded the post-stop reply. Preserve each original directory, manifest and first verdict. Register dumps and successful RAM saves cannot supply that missing observation.

For a new trace-wrapper acquisition, persist the pre-stop query-status reply and CPU query separately as they arrive in qmp.json. Require a running VM and exactly CPU0. After QMP stop succeeds, issue a second query-status on that connection. Write the returned status object verbatim in meaning to halted.json before reading registers or RAM. Require running=false and status=paused. Preserve the stage and any QMP error or ambiguous reply on failure. Only a newly observed paused result authorizes extraction in this path. Keep the existing contract for other capture clients.

On the frozen correction, a mocked QMP peer must exercise the real trace wrapper handoff and snapshot protocol, verify command order and both files, and assert zero reads of registers or RAM after invalid running/paused transitions or a failed post-stop query. Exercise an existing client without the new handoff. Independent acquisition must repeat OFF, disabled, active and saturate with both full-frame settings in eight separate fresh directories, retaining first failures and original admission bounds. Check raw pre/post replies, dump order, seal and decode, independent chronology/loss, terminal state and build/boot provenance. Archive any new mismatch under its observed source and run UUID.

원본 33d8fbf에서 공개 부팅 관찰과 봉인은 성공했으나, 정지 후 QMP 응답 누락으로 보고된 최초 복호화가 거부됐다. 원본 파일·봉인·판정은 보존한다. 새 수집에서는 정지 전 상태와 CPU 응답을 받는 순서대로 저장하고 실행 중인 단일 CPU0를 확인한다. stop 성공 뒤 같은 연결에서 query-status를 다시 요청해 실제 응답을 halted.json에 기록한 다음 running=false 및 status=paused를 확인해야 레지스터와 메모리를 읽을 수 있다. 오류 단계와 응답도 보존한다. 모의 QMP는 실제 래퍼 경로의 순서·파일·실패 시 읽기 차단과 기존 클라이언트 호환성을 검사한다. 독립 팀은 고정된 수정 소스에서 두 프레임 설정과 네 모드를 새 실행 UUID로 각각 수집하고 최초 실패까지 보관한다.
