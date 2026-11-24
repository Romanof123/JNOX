# M2 hard CBS blocking and wakeup

[한국어](m2-cbs-wake.md) · CHG-031 · IMP-029 · planned EVD-029 · #65

POL-008 uses whole-slot grants, one CPU, EDF and ascending server ID for ties. An active server has pending work. A sleeping client has completed its burst and blocked by SVC. An active, depleted server waits until its scheduling deadline. A positive budget makes an active server eligible. At replenishment a depleted active server receives Q with deadline d+P. At wake time t renew to (Q,t+P) if t≥d or qP>Q(d−t). Otherwise retain (q,d), including equality and zero budget. The entire dispatched slot is charged even when SVC relinquishes its tail to idle.

A=(2,8) releases one-slot bursts at 0,4,5,13,22, then 100 slots at 23. Neighbor B=(1,4) releases 100 slots at 1. Observe [0,34). At 4 equality retains (1,8). At 5 zero remains until 8. At 13 excess density renews to (2,21) and B wins contention. At 22 the expired deadline renews to 30. At 23 strictly lower density retains the allocation. During sustained excess demand B must receive one grant per complete four-slot reservation interval from 1. Real exception returns must resume A after each of five SVCs. Check source-independent EDF replay, including all grants and idle intervals, against guest RAM and traced IRQ/SVC/return PCs.

REQ-002/005/023 → DES-003/012 → IMP-029 → VER-002/005/023. Record first cold boot and ELF identity, timer compares, actual counters and per-task saved-frame evidence. Reboot the original EVD-006 hard CBS profile independently. Board timing and product interference require separate measurements.

## Capture and replay

Build the cbs-wake-bench feature. From the repository root in a POSIX shell run the following with a fresh output path and the retained ELF.

```sh
chmod +x tools/cbs_wake_capture.py
CBS_WAKE_CAPTURE_DIR="$capture" python3 tools/qemu_boot.py --profile cbs-wake --qemu ./tools/cbs_wake_capture.py --kernel "$elf"
```

The guest appends CBS WAKE TIMER STOP and CBS WAKE PASS to the standard boot and ready lines. Read cbs_wake_witness as 34 rows of 24 little-endian u64s. Columns 0–1 give the tick and release bitmask. Columns 2–5 hold A(q,d), B(q,d) immediately after releases. Column 6 gives the granted ID (A=0, B=1, idle=2). Columns 7–10 hold the same allocations after charging. Columns 11–15 give epoch, quantum, compare, dispatched PC and frame address. Columns 16–19 give IRQ counter, interrupted ID, PC and x20. Columns 20–22 give the SVC counter, return PC and x20, or zero. Column 23 holds the IRQ frame address. The first entry frame is 16 bytes above subsequent saved frames on the same task stack. A positive residual budget at equality must survive wake unchanged. Account for lazy replenishment when reading unselected servers.
