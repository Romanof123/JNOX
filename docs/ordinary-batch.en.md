# Run ordinary Rust batch jobs

English · [한국어](ordinary-batch.md)

Set `JNOX_BATCH_WORKLOAD=telemetry` to run the bounded in-memory summarize, threshold and validation jobs on these same policies. See the [telemetry admission profile and capture criteria](batch-telemetry-profile.en-ko.md).

The single-core AArch64 virt profile accepts trusted Rust functions through `submit(work, argument, estimate, delay)`. Every task receives its own generation handle. Returning from the function completes the job. Explicit completion succeeds only for the running generation. Four live jobs share four 16 KiB task stacks. A fifth stack belongs to idle. Estimates range from 1 through 4,294,967,295 architectural counter ticks. A delayed submission reserves its slot immediately. Accepted work runs once.

| Submission decision | Observable result |
| --- | --- |
| Estimate zero or 2³² | Estimate error (1), generation unchanged |
| Delay overflows current counter plus delay | Deadline error (4) |
| Four slots occupied | Capacity error (2), live jobs retained |
| Generation counter exhausted | Exhausted (3), no wrap or slot reuse under an old identity |
| Completion using another job's or a retired generation | Stale, execution continues on the caller |
| More CPU time than estimated | Remaining rank saturates at one until explicit completion |
| Equal waiting ranks | Earliest admission among simultaneous arrivals |
| Equal SRTF rank with incumbent | Incumbent continues |

The kernel samples CNTPCT at boot, accepted or rejected SVCs and timer service. The interval since the previous decision is charged to its incumbent. This includes kernel overhead inside that interval. CNTP_CVAL targets the earliest pending release. HRRN ages a delayed job from its declared counter deadline, including IRQ overshoot. Ready work starts only after a real dispatch. These configuration bounds and observations do not establish physical latency, WCET or a product capacity limit.

## Run and capture

Run from the repository root on Linux with Rust 1.95.0, the aarch64-unknown-none target, Python 3, QEMU AArch64 and aarch64-linux-gnu-nm installed. Choose a fresh output directory. Run sequentially.

```sh
set -eu
out=$(mktemp -d)
unset JNOX_FIXED_TRACE JNOX_BATCH_TRACE
export JNOX_BOARD=virt JNOX_PROFILE=batch
for app in batch-sjf batch-srtf batch-hrrn; do
  export JNOX_APP="$app" CARGO_TARGET_DIR="$out/$app"
  (cd kernel && cargo +1.95.0 build --release --locked -j1)
  elf="$CARGO_TARGET_DIR/aarch64-unknown-none/release/jnox-kernel"
  python3 tools/qemu_boot.py --kernel "$elf" --profile "$app" --runs 2 \
    --timeout 10 --capture-dir "$out/$app-ordinary"
  export JNOX_BATCH_TRACE=1 CARGO_TARGET_DIR="$out/$app-diag"
  (cd kernel && cargo +1.95.0 build --release --locked -j1)
  elf="$CARGO_TARGET_DIR/aarch64-unknown-none/release/jnox-kernel"
  export JNOX_BATCH_CAPTURE_DIR="$out/$app-qmp"
  python3 tools/qemu_boot.py --kernel "$elf" --profile "$app-trace" --runs 1 \
    --timeout 10 --qemu tools/batch_runtime_capture.py
  python3 tools/batch_runtime_observer.py "$app" "$elf" "$JNOX_BATCH_CAPTURE_DIR"
  unset JNOX_BATCH_TRACE
done
```

## Work and evidence

Let F be CNTFRQ in ticks per second. Producer 1 has estimate F. It admits older jobs 2 and 3 with estimate 2F, and job 4 with estimate F/2 and delay F/10. It computes a checksum until just over the arrival deadline (F/500 extra work-window ticks after F/10), then returns. Integer durations round down. A tail job 5, delayed F/20, computes for F/1000 against a one-tick estimate before submitting another one-tick job 6. Scheduling uses actual counter samples. Large IRQ or host delays can change observed ordering and should be retained as failed acceptance runs.

| Policy | Expected producer and peer observations |
| --- | --- |
| SJF | Work ends, short, older one, older two |
| SRTF | Short, restored work ends, older one, older two |
| HRRN | Work ends, older one, older two, short |

The ordinary observer compares the complete work transcript, waits for 100 ms of unchanged UART, then rechecks it after stopping QEMU. The diagnostic observer accepts at most 64 records and 24 KiB, checks every counter, decision, timer compare and restored frame, and freezes the accepted bytes through shutdown. Linux QMP captures CPU state after the quiet window, verifies the idle PC against the linked symbol and reads the exported record array. Offline replay cross-checks UART with guest memory and reports cumulative CPU ticks per owner, including idle as zero owner.

Each record holds 15 unsigned 64-bit fields in order: kind (0 boot, 1 IRQ, 3 submit, 4 completion), CNTPCT, outgoing and incoming generation, outgoing frame address and ELR, incoming frame address and ELR, argument or completion token, estimate, relative delay, status, admitted token, selected remaining estimate, and hardware compare (0 disabled). Completion status 1 denotes stale. Initial frames restore x0–x2 at a diverging C-ABI trampoline and aligned EL1h stack top. Compare outgoing IRQ frame/ELR with incoming completion frame/ELR for SRTF. The separate idle line records an actual instruction address and stack pointer. The capture directory preserves raw UART, exceptions, guest memory and stopped registers.

POL-003 / IMP-037. Six ordinary boots and three diagnostic boots produced 60 independently replayed scheduling decisions across SJF, SRTF and HRRN. [EVD-037](evidence/evd-037.en.md)
