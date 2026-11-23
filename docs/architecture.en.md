# Draft J-NOX Architecture

[한국어](architecture.md) · English

```mermaid
flowchart TB
  CONTROL[Control tasks and safety monitoring] --> IPC[IPC with permission checks and time limits]
  NATIVE[Native J-NOX Rust drivers] --> IPC
  APPS[POSIX services · sockets · user scripts] --> IPC
  IPC --> KERNEL[no_std Rust microkernel]
  LINUX[Linux domain · existing drivers] <--> GATE[Shared queues and dedicated message gateway]
  GATE <--> IPC
  KERNEL --> BSP[Board port · timers · MMU/MPU · IRQ]
  LINUX --> PART[Platform isolation · DMA and reset controls]
  BSP --- PART
```

## DES-001 Microkernel

The privileged kernel manages address spaces, threads and scheduling, timers, exceptions, interrupts, permissions, and IPC. Explicit handles convey rights to communication endpoints, devices, and shared memory. The kernel validates handles, lengths, and permissions. It reserves safety-task resources at boot and bounds kernel memory allocation, wait times, and queue lengths during operation. Address protection uses the supported MMU or MPU. The design separates execute and write permissions and calls for verification of kernel and user stacks and exception paths.

The kernel uses `no_std` Rust. Safe Rust represents scheduler state, queues, and IPC state machines. Narrow modules contain `unsafe` operations for boot, context switching, MMIO, page tables, DMA, and FFI. Each block requires review of its preconditions and invariants for access rights, lifetimes, and synchronization. Control paths use static or bounded pools and return explicit errors. The configuration defines how to handle panics, stack exhaustion, and detected integrity faults. [Rust unsafe contracts](https://doc.rust-lang.org/book/ch20-01-unsafe-rust.html)

## DES-002 Linux driver domain

Linux runs in an independent execution domain with compatible drivers. One candidate arrangement for heterogeneous cores places J-NOX on R52 and Linux on A76. On AArch64 or x86-64 platforms where the domains share a CPU family, the project will compare verifiable static partitioning and hypervisor options. Each board requires a record of who controls memory, devices, interrupts, DMA, boot, and reset.

The driver runs against its original Linux module interface. Reuse depends on a compatible combination of Linux kernel, configuration, architecture, firmware, and device description. The first test will select a combination that can use the original module binary and verify its identity by hash. A separate Linux-side gateway and a J-NOX service connect to the existing device interface. This arrangement provides compatibility through Linux itself. [Linux internal driver API](https://docs.kernel.org/next/process/stable-api-nonsense.html)

Control messages carry a version, type, length, sequence number, expiry time, and restart generation. Communication uses fixed-size shared memory and limits the number of notifications. Initial ports will use a copy-based path to simplify verification. RPMsg/OpenAMP is a candidate transport for asymmetric multiprocessing (AMP). The safety gateway validates messages and discards expired data. After Linux restarts, the system reclaims queues, handles, and DMA ownership from the previous generation before reconnecting. An IOMMU/SMMU or board firewall limits device DMA. Assignment alongside safety functions requires evidence that the limit holds for the device. [Linux RPMsg](https://docs.kernel.org/staging/rpmsg.html) · [IOMMU and device isolation](https://docs.kernel.org/driver-api/vfio.html)

## DES-003 Threads and real-time scheduling

Each thread has a context, stack, priority, CPU affinity, and execution budget. Its process owns the address space and permissions. Verification proceeds in stages from multiple threads in one process to symmetric multiprocessing (SMP) across CPUs. The initial safety configuration uses fixed CPU assignments and a bounded thread count. The SMP synchronization contract defines lock ownership, lock ordering, maximum lock hold times, and behavior during cancellation, termination, and release.

| Policy | Plan |
| --- | --- |
| Fixed-priority preemption with FIFO ordering | The first policy selects the highest-priority ready thread and defines ordering among threads of equal priority. |
| Round robin at equal priority | Specify the time slice and compare its effects on interrupt latency and deadlines. |
| Static time partitions and CPU reservations | Limit CPU time per domain. Define budget-exhaustion notifications and overrun handling. |
| Deadline and sporadic scheduling | Candidates for an extended configuration. Use them in configurations with evidence for admission checks, overrun costs, and the effects of mixing policies. |

Select priority inheritance or a priority-ceiling protocol for each synchronization object. Analyze IPC priority propagation and bounds on nested waits. The fault response budget includes context switches, interrupt masking, and contention for shared memory and buses. Configurations that require load balancing or core migration need their own timing evidence.

## DES-004 Board and CPU ports

The common kernel has a separate port contract. Each port declares its capabilities for boot, context switching, MPU/MMU protection, address widths, pages and regions, timers, IRQs and interprocessor interrupts, core startup, caches, atomics and barriers, DMA isolation, and reset. The initial common test suite covers AArch64, R52 AArch32, and x86-64. MPU-based ports, including Cortex-M, specify the number of supported memory regions and actual isolation granularity. Pin board and firmware versions along with the CPU model.

## DES-005 User services and application APIs

The native API centers on synchronous request/reply, bounded queues, notifications, and shared memory. User libraries provide the selected subset of `pthread`, a monotonic clock, and APIs in the `socket` and `poll` families. Networking runs in a user process or the Linux domain. Planned stages add `AF_UNIX`, TCP and UDP over IPv4 and IPv6, multiple clients, nonblocking I/O, and readiness notification. The API matrix identifies features outside the selected POSIX scope. File services, ELF loader extensions, TLS, and packages will be evaluated for the extended configuration.

Host scripts support builds and tests. Target scripts run in ordinary user services with limits on permissions, execution time, and memory. The Linux domain can provide Python and a shell. Lightweight native runtime candidates will be evaluated for J-NOX. Schedule and resource tables for the safety configuration become fixed, explicit build inputs after their format and value ranges are validated.

## DES-006 Safety monitoring and observability

The design covers deadline monitoring for control tasks and services, candidates for an independent watchdog, clock and reset monitoring, and bounded event recording. The state model defines normal operation, degraded operation, a product-specific safe state, and approved recovery conditions. Safety outputs are accessed through paths whose independence has been verified on the board. Where safety communication is required, document the requirements that justify sequence, timing, and integrity checks and the fault response. Each configuration also bounds CPU and memory use for trace storage and dumps.

## DES-007 Secure boot, access control, and supply chain

Design a chain of verification from the board's root of trust to the kernel and safety configuration, gateway, and updates. Define signing keys, separation of test and production keys, permitted versions, recovery slots, update transitions, and audit events. Other defenses include the kernel permissions model, independent resources for the safety domain, restricted management interfaces, pinned dependencies, and a software bill of materials (SBOM). Cryptographic functions use reviewed implementations and defined key-management boundaries.

## DES-008 Development and safety configuration baselines

Maintain links across planning, hazards, requirements, designs, changes, verification results, and release configuration hashes. Record the compiler, linker, libraries, and code generators for each target combination, along with evidence supporting confidence in those tools. Ferrocene is a candidate qualified toolchain. Compare the scope of qualification for the specific host, target, version, and libraries with the intended J-NOX use. Any gaps require use restrictions or further assessment. [Ferrocene qualification scope](https://public-docs.ferrocene.dev/main/safety-manual/scope.html) · [Target and library scope](https://public-docs.ferrocene.dev/main/user-manual/targets/index.html)

## DES-009 Native Rust driver SDK

Native J-NOX drivers run as isolated Rust services by default. The SDK supports permission-checked MMIO mappings, IRQ receipt and acknowledgement, DMA buffer allocation, mapping and reclamation, timers, request/reply, status queries, and restart procedures. The device manifest records the version, device ID, MMIO range, IRQs, and DMA limits. The shared-memory contract defines byte order, size, alignment, ownership transfer, and barriers. A virtual UART is the first native driver candidate. Verification of the DMA contract uses a separate DMA-capable device. Native device paths that serve a safety function carry product-specific assumptions and technical safety requirements.

## DES-010 Safety IPC channel

The first channel carries the periodic virtual-output permit in `ADR-001`. The safety sender protects the message and the safety receiver checks it before using it for control. Native IPC and a Linux-domain gateway share this contract. A relay forwards the protected message unchanged. The channel configuration binds the sending and receiving capability handles, direction, data ID and message type. The receiver checks the ID in the message against this configuration. [Bench decision](safety-parameters.en.md)

1. **Publication and transport.** The sender serializes the version, type, length, data ID, generation, sequence number, issue time and payload in a defined byte representation and computes an end-to-end CRC over all those bytes. Before implementation, the systems team fixes field widths, byte order, maximum length, CRC parameters and coverage, and reference test vectors. The channel uses a bounded, preallocated queue. When the queue is full, a new send returns a saturation result immediately and preserves the queued entries.
2. **Acceptance and rejection.** The receiver first checks endpoint permissions and transport slot bounds. It copies the input once into a private buffer, bounded by both the assigned slot and the channel maximum. Using only that snapshot, it checks the declared length, CRC, version, type, ID, current generation, exact next sequence number, issue time and payload range. It commits receiver state only after all checks pass. A frame rejected for corruption, duplication, reordering, a sequence gap, the wrong generation, expiry or any other failed check leaves the last valid data, its issue time and the recovery count unchanged. When opening a new generation, the supervisor sets the initial sequence number at both endpoints. Renew the generation before sequence rollover and inhibit the channel if a generation identifier could be reused.
3. **Timing and recovery.** The receiver monitors input loss on its own monotonic clock. On the bench, sender issue times are controlled inputs on the same virtual clock. A port across clock domains must establish timestamp provenance, synchronization uncertainty and a delay bound before accepting data as fresh. Bound the work per activation for examining slots, handling notifications and recording diagnostics so the expiry check still runs when the queue is empty, invalid traffic saturates it or Linux stops. Expiry takes precedence on the same tick. On disconnect or restart, invalidate queued frames and handles from the old generation. Apply the inhibition deadline and the conditions for rearming in a new generation from `ADR-001`.

The CRC detects accidental corruption in transit. Where the threat analysis requires authentication, the design must justify keyed endpoint authentication and replay protection. Even an authorized source can send a plausible but incorrect value. Using that value in a safety decision requires evidence about the source or independent plausibility checks. The Functional Safety Team reviews the product-specific fault model, residual error risk and timing budget. [AUTOSAR E2E protocol reference](https://www.autosar.org/fileadmin/standards/R25-11/FO/AUTOSAR_FO_PRS_E2EProtocol.pdf)

[Requirements](requirements.en.md) · [Verification plan](verification-plan.en.md) · [Development lifecycle](lifecycle.en.md)
