# Draft Verification Plan

[한국어](verification-plan.md) · English

The verification team reviews the acceptance criteria against the product plan and approved requirements. The team performs the first run without access to the implementation team's test conclusions. After recording an independent verdict, it reviews structure, coverage, and tools. It records the test environment, commands, inputs, raw logs, observed failures, and instructions for reproducing them.

| Verification ID | Requirement under verification | Environment and acceptance criteria |
| --- | --- | --- |
| `VER-001` | `REQ-001` | Check CPU reads and writes across domain boundaries in emulation and on physical hardware. On the board, verify the DMA ranges assigned to Linux devices and the ranges blocked. Confirm that an invalid access preserves control data and ownership of resources at the boundary. |
| `VER-002` | `REQ-002` | Use QEMU to verify the behavior of preemption, schedule and budget exhaustion, lock contention, and timer supervision. On the physical board, exercise CPU, IRQ, DMA, and bus contention together. Compare worst-case response times and the interference analysis with the product limits. |
| `VER-003` | `REQ-003` | After a successful round trip, inject each condition in turn, including Linux or service stoppage, delayed, duplicate or stale data, and restart. Supply inputs with invalid length, version, or value range. Check deadlines, product-specific state transitions, rejection of prior-generation requests and invalid data, and the recovery procedure. |
| `VER-004` | `REQ-004` | On a running Linux system, compare the device's baseline behavior with its behavior after integration. Compare the driver source version and hash with the original. The driver module SHA-256 must match in both environments. Record the versions and hashes of the Linux kernel, build configuration, firmware, and device configuration separately. Explain any permitted integration configuration changes. |
| `VER-005` | `REQ-005` | On a single core, verify thread lifecycle, synchronization, and ordering. On SMP, check core affinity, contention, simultaneous termination and cancellation, interrupt handling, and extended operation. |
| `VER-006` | `REQ-006` | Exercise AF_UNIX and both TCP and UDP over each of IPv4 and IPv6 concurrently across multiple processes and threads. Check readiness and timeouts in nonblocking I/O, partial reads, I/O during closure, and backpressure and resource release under saturation. |
| `VER-007` | `REQ-007` | Confirm that identical script inputs produce the same static configuration and that invalid configurations are rejected. Test unbounded execution, allocation limits, and privilege requests in target-side scripts. |
| `VER-008` | `REQ-008` | For each CPU and BSP, record the scope of support for boot, exceptions, memory protection, timers, SMP, and applicable devices. Track results for QEMU, the physical board, and the safety configuration separately. |
| `VER-009` | `REQ-009` | Record the link map and boot and memory baselines for a fixed core configuration. On physical hardware, measure boot time, interrupt-to-task latency, context-switch latency, and IPC latency. Record test conditions and the maximum observed values. |
| `VER-010` | `REQ-010` | Check rejection of tampered images, unapproved versions, and images signed with revoked keys. Test power loss at each update step and confirm recovery to an approved image. |
| `VER-011` | `REQ-011`, `REQ-021` | Check permissions, queues, handles, and quotas for each domain. Under invalid requests and overload, confirm that safety queues, memory use, and processing time stay within their reserved limits. Inspect the Linux ABI process launch manifest. Exhaust its CPU, memory and file descriptor limits separately and record each enforcement outcome and preservation of safety-task reservations. |
| `VER-012` | `REQ-012` | Check upstream and downstream requirement links in both directions and look for orphan nodes. For each changed requirement or assumption, review the impact analysis and the tests that were rerun. |
| `VER-013` | `REQ-013` | Review the actual assumptions of use, single-fault and dependent-failure analyses, physical hardware results, tool confidence, coverage, residual risks, configuration hashes, and independent review records. |
| `VER-014` | `REQ-014` | Verify normal operation and error behavior for the approved API list. Measure whether safe operation and the ability to interpret evidence are preserved when logging saturates. |
| `VER-015` | `REQ-015` | Check requests, IRQs, and error handling for a native Rust driver such as a virtual UART driver. On a supported physical device, verify DMA buffer reclamation and access blocking, restart after termination, and the independent operation of other services. |
| `VER-016` | `REQ-016` | An independent reviewer compares the build configuration, dependencies, and final link to confirm that the kernel build excludes `std`. Check the inventory and invariants of `unsafe` code, compiler target, host, options and library scope, build reproducibility, and conditions that trigger a change review. |
| `VER-017` | `REQ-017` | Conduct a vulnerability-response exercise from intake through impact assessment, response, and notification. Review the security patch's safety impact analysis, reverification status, SBOMs before and after the patch, signatures and provenance, and approved rollback or recovery material. |
| `VER-018` | `REQ-018` | On a physical board or suitable hardware-in-the-loop (HIL) setup, inject kernel halts, timer and clock faults, monitoring-path failures, and reset failures according to the assumptions. Independently observe failure detection and the transition to safe output, measure their timing, and check the results against each product time budget and the conditions for resuming output. |
| `VER-019` | `REQ-019` | First confirm acceptance of an independently generated valid reference frame. Inject header and payload corruption, a valid frame for another channel, an unauthorized endpoint, duplicates, out-of-order or skipped sequences, an old generation, and future or expired timestamps separately. For each rejection, observe that no new value reaches the control task and that the last accepted issue time, sequence and rearm count are preserved. Exercise sender silence, sustained invalid traffic and queue saturation separately. Verify that old messages cannot enable the output after restart. |
| `VER-020` | `REQ-020` | Build external C programs `cat` and `wc` separately with the SDK. Run `cat sample.txt \| wc -l` in the J-NOX shell. For input bytes `61 0a 62 0a`, require a line count of 2, empty stderr, status 0 for both processes, and EOF on the pipe. With a missing file the producer must emit a diagnostic and exit nonzero, and the consumer must receive EOF and exit. |
| `VER-021` | `REQ-021` | Confirm the Debian executable SHA-256 below on both systems. Run `busybox wc -l sample.txt` with identical arguments, environment and input as an ordinary user on reference AArch64 Linux and on J-NOX with its Linux guest disabled. For input bytes `61 0a 62 0a`, require a line count of 2, empty stderr and status 0, and compare output byte for byte. For a file with read access denied, require denial and matching diagnostics and exit status. Separately check raw `-ENOSYS` for an unimplemented call and rejection before entry for a wrong ISA or unsupported interpreter. |
| `VER-022` | `REQ-022` | Build a Rust app from a clean, separate Cargo project with the public SDK. It copies the file named in its first argument to stdout using safe SDK APIs. Boot J-NOX and launch the app in user mode. For input `61 0a 62 0a`, require identical output, empty stderr and status 0. For a file outside its permissions require an access error, nonzero exit and no disclosure of file contents. Inspect the static ELF and link map or build records to confirm the first profile uses `core` without dependencies on `alloc` or `std`. Verify that the published SDK matrix states support for both libraries. Record the Rust target, SDK version, executable hash and process launch evidence. |
| `VER-023` | `REQ-023` | Check that the same workload distinguishes decisions driven by arrival, budgets, deadlines and weights. Exercise boundaries, ineligible inputs and overruns. Verify round robin through repeated timer preemptions and returns between non-yielding peers in a single guest exception trace. Record selection, live-dispatch and board evidence per policy. [Policy acceptance cases](scheduling-policies.en.md) [Periodic releases](m1-periodic.en.md) · [Response bounds and assumptions](fp-response.en.md) · [Sampled EDF and LLF](m1-deadline.en.md) |
| `VER-024` | `REQ-024` | With concurrent endpoints, saturate resources, race completion against timeout and death, reuse slots, and compare delivery, ownership and recovered resources against each selected contract. Replay the safety permit cases under VER-019. [IPC acceptance cases](ipc-mechanisms.en.md) |

### Linux binary test fixture

The `VER-021` fixture is Debian trixie arm64 `busybox-static` version `1:1.37.0-6+b9`. The package SHA-256 is `c833be48abfa16bc19c4966ec93e289ff1ce5d2f1476cad3a57bd105378cd15c` and the extracted `usr/bin/busybox` SHA-256 is `7d93682be37cf6ed46699f6fff546b80bf133cf8ec342c2d04bc23387f78bc34`. The executable is 1,975,064 bytes, has a `PT_TLS` segment and has no `PT_INTERP`. Attach the guest-disabled boot configuration, the J-NOX user process launch and exit records, and the raw outputs from both systems to the verification evidence. [Debian reference package](https://deb.debian.org/debian/pool/main/b/busybox/busybox-static_1.37.0-6+b9_arm64.deb)

## Verdicts and Evidence

The safety IPC permit-channel test (`VER-019`) uses the virtual bench budgets in `ADR-001`. At fault injection, close the normal sender path and invalidate undelivered valid frames. Run sender silence, sustained invalid traffic and queue saturation separately. In the latter two runs a separate test injector supplies invalid frames to the normal receive queue downstream of the closed gate. In the saturation run, record when the queue fills and the next send is rejected. For sequence, timestamp and channel-ID test cases, recompute a valid CRC after changing the field so that the intended check actually determines rejection.

In each of the three runs, the maximum interval between completed freshness checks is 5 ms. The other limits are 25 ms from fault injection to detection, 5 ms from detection to observed output inhibition, and 30 ms end to end. Record the receive-error counts, rejection reasons, valid data deliveries and actual virtual output transitions in the independent test evidence.

The first bench case for `VER-002` and `VER-003` uses the timing budgets in `ADR-001`. The verification team records the fault injection (`t_fault`), the fault latch being set (`t_detect`), and the independently observed transition at the virtual device (`t_safe`) on the same monotonic clock. It evaluates both intervals and the end-to-end time. `VER-018` covers clock and reset fault testing on the target hardware or suitable HIL setup. [Bench acceptance criteria](safety-parameters.en.md)

Record the verification level as document or design review, host unit testing, QEMU integration testing, physical board testing, product HIL, or independent review. Use QEMU for functional behavior and reproducibility. Verify actual timing bounds and board-level DMA, reset, and electrical independence in an appropriate environment. Report the measured latency distribution alongside the analytically established timing bound. Record coverage against the selected safety requirements and analysis methods.

Each verification result records its `EVD` ID, baseline, `VER` and `REQ` IDs, who performed the test and when, source commit and image SHA-256, board, firmware, instrument and tool versions, procedure, inputs, expected results, raw log paths, verdict, and exceptions. Track the status as `Planned`, `Passed`, `Failed`, `Blocked`, or `Reverification required`. This document specifies planned tests. Product evidence receives an `EVD` record through the approved verification procedure.

[Requirements](requirements.en.md) · [Safety and security concept](safety-security-concept.en.md) · [Trace links](../trace/links.csv)

The two-CPU QEMU configuration exercises fixed homes, global migration and singleton cluster enforcement. [Two-CPU scheduling](m3-smp.en.md)

The mutex configuration checks effective priority on real block, timer timeout and ownership handoff. [Mutex release cases](m1-mutex.en.md)

[Multilevel queues](m4-multilevel.en.md) · [Mixed-criticality budgets](m3-mixed-criticality.en.md)

[Lottery and stride](m4-proportional.en.md) · [Weighted virtual runtime](m4-vruntime.en.md)

[EEVDF](m4-eevdf.en.md) · [Hierarchical groups](m4-hierarchy.en.md) · [Energy-aware placement](m3-power-placement.en.md) · [Bounded policy extension](m4-bounded-extension.en.md)

## Deferred dispatch, job overlap and server wakeup

For each frozen image, independently capture one guest and compare its task progress, exception returns, counters and state transitions with the contract.

- POL-002 · CHG-029 · IMP-027 · [Fixed priority, queue order and remaining service](m1-priority-resume.en.md)
- POL-004 · CHG-030 · IMP-028 · [Periodic deadline misses and overlapping releases](m1-periodic-overrun.en.md)
- POL-004 · CHG-035 · IMP-033 · [Exact periodic pair](pol-004-exact-periodic.md)
- POL-008 · CHG-031 · IMP-029 · [Hard-CBS wakeup and deadline renewal](m2-cbs-wake.en.md)

## Linked Rust task configuration

CHG-032 / IMP-030 selects a single-core runtime and a static Rust task table at build time. The same kernel interface accepts FIFO and RR applications with different priorities and initial releases. EVD-030 calls for the first guest captures, task exception and stack checks, and terminal idle for each application configuration. [Fixed-priority runtime](fixed-runtime.en.md)

CHG-033 and IMP-031 select the ordinary FIFO/RR task tables on the P1 BCM2711 profile. Verification tracks the raw payload, model boot and physical observation separately. [Pi 4 linked-task runtime](boards/pi4-fixed-runtime.en.md)

[EVD-031](evidence/evd-031.en.md)

CHG-034 and IMP-032 define two linked workloads for simultaneous release/expiry and overdue wakes after a bounded masked IRQ. Use ordinary execution and the optional diagnostic trace to reconstruct actual charged service. [Simultaneous and late timer events](fixed-runtime-edges.en.md)

[EVD-032](evidence/evd-032.en.md)

POL-004 / IMP-033. The exact periodic pair dispatched AB under RM and BA under DM, then repeated A at tick 10 and B at tick 12 in both epochs. [EVD-033](evidence/evd-033.en.md)

[EVD-034](evidence/evd-034.en.md) reconstructs service error across 128 grants and 16 wake delays at fixed weights and slices. [Repeated-wake design](m4-eevdf-wake-design.md)

CHG-037 / IMP-035 links POL-010 to [EVD-035](evidence/evd-035.en.md) under VER-002/005/023. [Proportional failure observation](m4-proportional.en.md#first-failure-observation-79)

[Linked cooperative event contract and acceptance criteria for #80](coop-runtime.en.md). CHG-038 / IMP-036 / POL-001 / [EVD-036](evidence/evd-036.en.md).

[POL-004 M3 virtual control workload and FS-M3-01](pol-004-m3-control-plan.md#english). CHG-044 records the simulation design. Implementation and independent evidence are pending.

[Host segmented admission method](fs-m3-admission.md) · CHG-045 / IMP-043, POL-004 #94. EVD-043 is planned for independent host verification.
