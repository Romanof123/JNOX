# Capturing the ordinary Rust FIFO workload

Build from the joint source in `kernel/` with the repository Rust toolchain. Save the build log, toolchain versions and source revision alongside the capture. Use separate target directories for the FIFO image and each ordinary control.

```sh
JNOX_BOARD=virt JNOX_PROFILE=mutex JNOX_APP=ordinary-fifo cargo build --locked --release -j1
```

From the repository root, freeze the built ELF before launching it. Supply the actual ELF path and full joint source commit. The helper creates a fresh directory, copies the ELF, records its digest, symbols, disassembly, QEMU version and both launch commands, and prints the manifest digest. Preserve that digest and the [prospective decision rules](ordinary-mutex-fifo-plan.en-ko.md) before launch.

```sh
python3 tools/ordinary_mutex_fifo_capture.py freeze "$new_capture_dir" "$elf" "$joint_sha"
python3 tools/ordinary_mutex_fifo_capture.py run "$new_capture_dir" "$freeze_digest" plain
python3 tools/ordinary_mutex_fifo_capture.py run "$new_capture_dir" "$freeze_digest" traced
```

Each position has its own first-attempt directory with UART, stderr and process outcome. The second also records QEMU exception entries and returns. Collection stops after a complete `FIFO END` line and 100 ms of UART silence, or at the 15 s observation limit. Preserve all attempted positions. The independent verifier must replay each epoch in both transcripts and match the traced returns to the frozen ELF.

The journal uses unsigned 16-digit hexadecimal words with all ones for an absent field. Between `FIFO RAW 1` and `FIFO END`, H reports terminal reason, decision count and restore count. S identifies epoch, task, stack bounds and entry. R reports restore ordinal, frame, PC and counter. J reports epoch, sequence, operation, argument, actor, selection, saved frame and PC, selected frame, PC and x0, runnable mask, X wait mask, effective priorities packed by task ID, owner, completion mask, grantee, protected count and value, saved x0, observed return result, previous count and previous value. Operations 1–7 represent acquire, release, activate, exit, seed, worker observation and epoch start. Count and protected value are observed after the operation. The acquisition result and prior protected values in a worker observation come from worker execution.

Record the plain and traced first runs for each ordinary-pi and ordinary-ceiling control separately with their original capture and score rules. The existing [control qualifier](../tools/qualify_ordinary_mutex.py) accepts the historical cohort ZIP layout.
