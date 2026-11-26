# Platform delivery checklist

[한국어](platform-checklist.md) · CHG-077 · AArch64 product planning

Use this checklist to select work from CAP-001 through CAP-021 for a release profile. Each open box names the team and the observation needed to close it. Record the revision, image, profile, device or model, raw result and independent verdict with that box. The minimal control image and the extended services image have separate resource and acceptance records. [Capability catalogue](platform-capabilities.en.md) · [Product milestones](product-plan.en.md)

**Starting evidence.** Linked Rust threads run in the AArch64 kernel. On the emulated Pi 4, two cold boots each recorded two directed wakes and return to interrupted work. The scheduling matrix records each policy's coverage and open cases. In one separate virtual permit integration cohort, 0 of 20 positions met operational criteria. These results support specific kernel and model claims. General IPC services, persistent media, visible windows, physical timing and product safety each need their own evidence. [Linked-task runtime](fixed-runtime.en.md) · [Model wake evidence](evidence/evd-056.en.md) · [Scheduling matrix](scheduling-release-cases.en.md) · [Permit cohort](evidence/evd-054-integration.en.md)

A separate first cold boot on AArch64 QEMU virt ran one static native v1 C process at EL0 through computation and exit status 37. The kernel returned −38 (ENOSYS) for its write to fd 1. Application stdout was empty. Successful target file I/O, the POSIX source profile and the general process lifecycle remain open. [Native C startup and exit](native-el0-startup.en.md)

## Product owner worksheet

Record these four answers in [the decision issue](https://github.com/Romanof123/JNOX/issues/162). Systems and functional safety use the selected application to derive and approve the numeric budgets. Each example is a starting point for discussion.

| Decision | What to enter |
| --- | --- |
| First application | Name its input, useful output and what the user or equipment should observe when the input disappears. |
| Time | Identify which events need a calendar timestamp, whether timestamps must agree across devices and what the application may do before it knows the date. |
| Data | List the settings, records or logs that must survive sudden power loss. Give the acceptable loss and the maximum useful recovery time, or mark them to be derived. |
| Display and compatibility | Name the first screen or select headless operation. Identify the input device, any safety-related indication and the first existing X11 executable and architecture if required. |

## Before buying hardware

- [ ] **Product owner and systems team.** Choose the first use case, safe output on failure, required deadline, headless or displayed operation, network needs and storage data that must survive power loss. Identify the exact board revision, available board and safety documentation, probe/serial/power-cut access, storage medium, display/input if selected, and the first native or reused driver. Record who owns the board, purchase budget and approval before ordering. The Pi 4 model supplies a reproducible starting point for porting. [Physical bench plan](boards/pi4-p1.en.md)
- [ ] **PM and verification.** Link the approved per-profile scheduling cases and applicable board evidence in #21 before opening IPC implementation in #24. Service/interface planning can proceed now. Record processor, footprint, IRQ, queue and timeout budgets for each selected profile. [Scheduling tracker](https://github.com/Romanof123/JNOX/issues/21) · [IPC workstream](https://github.com/Romanof123/JNOX/issues/24)

## Kernel, boot and service boundaries

- [ ] **BSP and release · CAP-001/002/009/014.** Pin firmware, boot configuration, image layout, load addresses and debug symbols. Rebuild from the manifest, compare hashes, then collect a power-on serial trace through startup, RAM discovery, timer/IRQ setup and a runnable task. Reproduce thread wait/wake, termination and affinity under contention. Keep direct-load model, firmware boot and physical results identifiable.
- [ ] **Kernel and user runtime · CAP-004/005/018.** Extend the observed single-process startup and exit to parent wait/reap on exit or fault and reclamation of mappings, descriptors and handles. Reject out-of-bounds segments and incompatible images. Run another process through a permission fault and verify isolation. Publish supported syscalls and errors for each ABI. Freeze interpreter and shared-object versions when enabling dynamic linking. [Loader design](architecture.en.md)
- [ ] **Kernel and IPC · CAP-003.** Run a client and server in different address spaces. Exercise request/reply, bounded notification, shared-buffer ownership and rights. Race reply with timeout or peer death, saturate the queue and restart the server. Verify one terminal outcome, cleanup, restored priorities and rejection of stale handles. Score safety-channel freshness, loss and rearm independently. [IPC profiles](ipc-mechanisms.en.md)
- [ ] **Driver SDK, naming and isolation · CAP-004/006/017/019.** Register a device path, open it concurrently, enforce per-open rights and invalidate handles on restart. Run a user-space Rust driver for one device. Exercise MMIO, IRQ, DMA-buffer and reset ownership, contain a forbidden transfer and verify the control workload retains its CPU and memory allocation under competing demand.
- [ ] **Driver SDK and BSP · CAP-020.** Record the upstream PL031 source revision and hash, adapter revision, supported API subset, configuration and native ELF hash. Build the unchanged source and retain the diagnostic for an unsupported feature. Boot J-NOX alone, read the RTC from the EL0 driver, and capture a denied access from its peer. Record an alarm IRQ separately. For the follow-up `virtio_net` profile, exercise native packet I/O and the DMA bounds on its queues. [Driver-source profile](native-linux-driver-source.en-ko.md)

## User-facing services

- [ ] **Time services and BSP · CAP-001/012.** Publish monotonic, wall-clock and timer semantics. Change wall time while an absolute monotonic timeout is pending. Verify cancel/expiry races, loss of synchronization and wake from each enabled power state. Record resolution and uncertainty with raw observations. [System time plan](system-time-plan.en.md)
- [ ] **Filesystem, block and BSP · CAP-006/007/008.** Start with a read-only image and a RAM filesystem. Add writable media with declared file and directory commit points. Test path resolution, permissions, full media, flush, removal and remount. On the selected medium cut power around each commit phase and check acknowledged content, metadata and recovery time. [Filesystem plan](filesystem-plan.en.md)
- [ ] **Runtime, networking and SDK · CAP-005/010/011/018.** Run a separately built Rust application, a shell pipe between two native C programs, and the pinned same-ISA Linux CLI under ordinary rights. Capture arguments, I/O and exit status. Exercise concurrent local and IPv4/IPv6 TCP/UDP sockets, nonblocking readiness and backpressure. If selected, interrupt a cross-node request and restart one isolated network stack under load. [Application acceptance cases](verification-plan.en.md)
- [ ] **Display and peripherals · CAP-021.** Keep the headless configuration bootable. On a selected display/input device, use a known input to change a visible frame, record both identities, then test window focus, client restart and buffer reclamation under load. Track audio underrun and USB detach/re-enumeration separately for profiles that select them. [Window system plan](window-system-plan.en.md)

## Operations and product release

- [ ] **Diagnostics and tools · CAP-013/014.** Decode a bounded IRQ-to-thread-to-service trace against the exact image. Force overflow, account for lost events, compare overhead with capture disabled, and symbolize a permitted crash dump while excluding other processes' data. Package the chosen SDK, symbols, manifests and repro commands. [Application tracing](application-tracing.en.md)
- [ ] **Security, supervision and release · CAP-015/016.** Deny an unauthorized service operation and revoke a live handle. Hang and restart a service under load, observe bounded recovery and stale-generation rejection, then reject a tampered image and a prohibited downgrade. Interrupt a signed update, boot the approved recovery image and read back its identity. Retain signing policy, rollback rules, component inventory and fault records.
- [ ] **BSP, safety and independent verification · selected CAP profile.** On the identified physical configuration measure CPU/IRQ/DMA interference, memory and stack peaks, worst observed control response and output transitions. Compare with justified analytical bounds, item hazards and the approved safe-state timing. Sign off assumptions, residual risks, recovery instructions and ownership for field updates. Functional-safety approval applies only to that product configuration. [Safety concept](safety-security-concept.en.md) · [Bench timing decision](safety-parameters.en.md)
