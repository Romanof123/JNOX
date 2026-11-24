# Sampled EDF and least-laxity-first dispatch

[한국어](m1-deadline.md)

CHG-017 · POL-005 · REQ-023/002/005 → DES-012/003 → IMP-015 → VER-023/002/005 → EVD-015. [#43](https://github.com/Romanof123/JNOX/issues/43)

EDF chooses the smallest absolute deadline. LLF chooses the smallest signed value of `deadline − now − remaining`. Queue all simultaneous arrivals before deciding. Remaining service is charged to the incumbent and stays at least one until explicit completion. A tie retains the incumbent and preserves queue order among waiting jobs. A negative laxity is eligible and shows that the estimate cannot fit the remaining time.

The positive quantum q anchors a grid at start s. Only the first selection on an exact grid boundary can displace a running job. An off-grid call charges elapsed service and retains the incumbent. Dispatch from idle, including after completion, is immediate. For s ≤ a ≤ b the number of involuntary switches in (a,b] is bounded by `floor((b−s)/q) − floor((a−s)/q)`. An application must budget completion or voluntary transitions separately. Missed grid points do not cause retroactive switches.

Release A (deadline 8, estimate 1) and B (10, 8) together at tick 0. EDF runs A. Its tick-1 timer releases C (3, 3). C preempts A and completes at 1, A resumes its saved PC and completes at 2, B completes at 3. LLF runs B first. With one-tick sampling A and B tie at tick 5 (both laxity 2). At 6 A has laxity 1, B has 2. A preempts B and completes at 6, returning to B. At 7 a new C (deadline 8, estimate 3, laxity −2) preempts B. C completes at 7 and B finishes at 8. The workload injects these logical times. The dispatcher programs the physical counter compare register from a fixed epoch and interval, including the next LLF boundary in each nonterminal IRQ. Workers unmask IRQs after establishing their witnesses. A sample outside its one-period acceptance window fails this run. Completion time and service estimates remain logical inputs.

From the kernel directory build the pinned release with `cargo build --release --locked --features deadline-bench -j1`. From the repository root run `python3 tools/qemu_boot.py --profile deadline --kernel "$ELF"`. A fresh verifier retains the revision, ELF digest, first UART verdict and exit status before reading the implementation or trace. Use `--qemu` with a wrapper adding `-d int -D "$TRACE"` to capture the same guest. Require the lines below, 0.1 seconds of silence, eight timer IRQs and six SVC #1 completions. Map the IRQ and return PCs at equality, crossover, negative slack and final completion to the ELF. Observer replay must reject missing, reordered and trailing output.

```text
JNOX M1 BOOT
JNOX M1 READY
DEADLINE EDF A FIRST
DEADLINE EDF IRQ C READY
DEADLINE EDF C FIRST
DEADLINE EDF A RESUMED
DEADLINE EDF B FIRST
DEADLINE LLF B FIRST
DEADLINE LLF B TIE RETAINED
DEADLINE LLF T6 CROSSOVER
DEADLINE LLF A FIRST
DEADLINE LLF B RESUMED T6
DEADLINE LLF IRQ C READY
DEADLINE LLF C FIRST
DEADLINE LLF B RESUMED T7
DEADLINE PASS
```

Host coverage compares bounded traces for both policies, ties, signed arithmetic, quantum boundaries, elapsed service and rejected inputs. The PC trace establishes actual dispatch. A physical deadline, WCET or production overload action requires its own workload, board measurements and safety review.

For independent counter readback, the linked symbol `deadline_clock_evidence` contains two consecutive records of ten little-endian u64 values. Each stores counter frequency, origin, interval and seven observed IRQ counts. The EDF epoch uses the first IRQ field. Capture its 160 bytes from the parked guest before shutdown; then check each observation against its programmed absolute interval. These are guest physical-counter samples.

[EVD-015](evidence/evd-015.en.md)
