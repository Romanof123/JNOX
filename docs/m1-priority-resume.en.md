# Fixed-priority resume contract

CHG-029 / IMP-027 / planned EVD-027 · POL-002 · #63, parent #21 · REQ-002/005/023, DES-003/012, VER-002/005/023.

One AArch64 virt CPU runs three epochs on separate stacks. A starts first. CNTP PPI 30 admits same-priority B after one quarter quantum of A service and HIGH after a further quarter. Both FIFO variants retain A on B arrival. Preemptive FIFO saves A ahead of B on HIGH arrival. HIGH exits, A resumes its exact instruction, yields to B, and B exits to A's post-yield instruction. Deferred FIFO returns to A on both arrivals. A yields, HIGH exits to B, then B exits to A. In preemptive RR, HIGH exits to interrupted A. A expires after its residual service, B exits and A completes. Each epoch ends with an empty queue.

The quantum is floor(CNTFRQ/10) physical counter ticks. Charge each dispatch from the scheduler's final counter sample before restoring a frame to the first sample in the next exception callback, including vector overhead. Exclude scheduler decision time. On each release subtract measured service, retain the residual across HIGH, and program exactly that residual on resume. At expiry accept at most one eighth quantum of interrupt lateness. The second release uses the same delay in both FIFO variants. Positive progress, task-owned stack words, register witnesses and saved/selected ELRs accompany dispatch decisions. Independent first-boot review must replay queue, service and exception returns and repeat the existing FIFO/RR profiles. Physical timing and product acceptance remain under #21.

```sh
chmod +x tools/priority_resume_capture.py
PRIORITY_RESUME_CAPTURE_DIR="$capture_dir" python3 tools/qemu_boot.py --profile priority-resume --qemu ./tools/priority_resume_capture.py --kernel "$kernel_elf"
```
