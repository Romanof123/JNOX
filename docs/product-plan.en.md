# J-NOX Product Plan 0.1

[한국어](product-plan.md) · English

**Status**　Draft for review · 2026-11-23

The first J-NOX product is an embedded operating system that gives control software predictable execution times and can use existing device drivers in a separate Linux domain. The first bench demonstration uses a periodic control task with virtual I/O. During normal operation the task receives and processes data from a Linux driver service. If the service stalls or the data arrives late or corrupted, J-NOX uses its own timer and authority to select the defined fallback output. System-level analysis will establish the actual use case and safe state for the first product.

## Product objectives

The product scope includes time sharing, deadline-driven scheduling, cyclic tables, budget reservations, fair service and multicore placement alongside fixed-priority FIFO. Each policy has an applicable profile and release evidence. [Scheduling policies](scheduling-policies.en.md)

Allocate product capabilities by profile and track each acceptance criterion alongside its current evidence. The catalog records the platform baseline and additional proposals awaiting an adoption decision. [Platform capability baseline](platform-capabilities.en.md)

Safety tasks will be able to select a safety IPC channel for control data received from another task or through a Linux gateway. The first use case is a virtual output permit. When the last accepted permit expires, the receiver inhibits the output independently. [Safety IPC requirement](requirements.en.md)

In `ADR-001`, the functional safety team fixed the criteria for disabling a virtual output when a permit signal is lost on the first software bench. These criteria are design inputs for the first control task and its tests. For a product application, the functional safety and systems teams will derive the safe state and time limits from operating conditions and hazard analysis. [First bench decision](safety-parameters.en.md)

| ID | Objective | Initial success criteria |
| --- | --- | --- |
| `OBJ-001` | Support ASIL D safety goals in a specified automotive system | An independent reviewer can examine the safety argument and verification evidence for the identified system, assumptions, board support package (BSP), toolchain, and build configuration. |
| `OBJ-002` | Achieve high cyber resilience and maintain it throughout the product lifecycle | Each threat can be traced to its mitigation and verification. Tests verify the isolation boundaries and safety functions under an assumed compromise of the Linux domain. Signed boot, update, and recovery can be reproduced. |
| `OBJ-003` | Reuse existing Linux drivers unchanged | J-NOX can use the service for one specified device with the original Linux driver source and module. Tests cover control behavior during driver failure and Linux restart. |
| `OBJ-004` | Provide a small kernel, hard real-time operation, and multithreading | Report resource use for the minimal image and control-task time bounds for the worst-case configuration on a physical board. Reproduce the supported scheduling and thread behavior. |
| `OBJ-005` | Extend support across all Arm profiles and the x86 family | Maintain common kernel and service contracts. For each CPU and board combination, report the status of boot, functional, real-time, and safety verification. |
| `OBJ-006` | Provide a service environment for Rust and C applications and existing Linux CLIs | Run native Rust and C applications built with the J-NOX SDK. In the extended configuration, directly execute listed, previously built Linux user binaries on the same CPU architecture without rebuilding them. Support concurrent sockets, diagnostic and automation scripts, and selected POSIX APIs. |
| `OBJ-007` | Manage changes against the plan and reproduce the evidence | Each requirement can be traced from a product objective or hazard through design, implementation, test, and evidence. A reviewer can follow a result back to its original intent. |
| `OBJ-008` | Provide a Rust kernel and a native J-NOX driver ecosystem | Review the toolchain scope for each target and the boundaries of `unsafe` code. Independently verify native Rust drivers and their SDK. |

## Configurations and scope of support

- **Safety configuration.** Pin the versions of safety tasks, kernel, required BSP, minimum I/O path, and any applicable isolation and monitoring components. Set task, memory, interrupt (IRQ), and CPU budgets in the system configuration. Scope the first candidate ASIL D application to one board and one use case.
- **Extended configuration.** Select user services, a Linux driver domain, sockets, scripting, file services, and development tools under the same kernel contracts. Choose components for each board and use case.
- **Device support.** Record when a port builds, boots in an emulator, passes functional tests on a physical board, and acquires product safety evidence. A change of board triggers a review of the evidence for timers, interrupts, DMA, resets, and memory interference.

| Order | Port | Scope of experiments and support |
| --- | --- | --- |
| P1 | Arm AArch64 Cortex-A72/A76 on QEMU `virt` | Reference kernel behavior, MMU, EL0/EL1, preemption, and IPC. Identify the execution environment whenever emulator results are cited alongside physical-board timing data. |
| P2 | Arm Cortex-R52 on QEMU MPS3-AN536 and a candidate physical board | AArch32 port with MPU protection and periodic control. R-Car V4M is a candidate for running Linux on A76 and J-NOX on R52. First assess board availability and whether resource ownership can be enforced. |
| P3 | x86-64 on QEMU, then physical hardware | Verify UEFI/ACPI, APIC, timers, SMP, and IOMMU behavior for supported devices. Evaluate options for isolating the Linux domain. |
| P4 | Arm AArch32, additional Cortex-R and Cortex-M cores, and IA-32 | Select representative cores and boards to extend the capability matrix. For boards with limited MPU resources or other protection constraints, specify feasible configurations and required external isolation. |

The BSP contract accounts for differences between Arm A/R/M profiles and across x86 operating modes and devices. Physical boot, reset ownership, and the feasibility of DMA isolation on R-Car V4M require board-level verification. [Arm architecture](https://developer.arm.com/-/media/Arm%20Developer%20Community/PDF/Learn%20the%20Architecture/Introducing%20the%20Arm%20architecture.pdf) · [Intel system programming manuals](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html) · [Renesas R-Car V4M](https://www.renesas.com/en/about/newsroom/renesas-leads-adas-innovation-power-efficient-4th-generation-r-car-automotive-socs)

## Milestone deliverables and approval criteria

CHG-007 sets the implementation order across these milestones. Develop scheduling first. The PM links the completed per-profile scheduling cases and applicable board evidence in #21 before opening IPC development in #24. The IPC-dependent parts of M1, M2 and M4 follow this dependency. [Scheduling tracker](https://github.com/Romanof123/JNOX/issues/21) · [IPC workstream](https://github.com/Romanof123/JNOX/issues/24)

| Milestone | Deliverables | Completion criteria |
| --- | --- | --- |
| M0 Planning | Product plan, safety and security concept, requirements, draft design, verification plan, and trace links | Review the relationships between documents. Record the first bench scenario and open decisions. |
| M1 Kernel | First AArch64 build, task and thread lifecycles, fixed-priority preemption, timers, memory isolation, and IPC | Repeatable QEMU tests and negative tests pass against the same commit. |
| M2 Drivers | A running Linux system with one reused driver, one native Rust driver, SDK, cross-domain protocol, and fault tests | Verify the contract for each driver path. The bench control task performs its specified action when Linux stalls. |
| M3 Porting and physical hardware | R52 board tests, x86-64 reference port, load and fault injection, and thread and SMP evidence | Measure timing, memory, DMA, and restart boundaries on the selected board. Record evidence separately for each CPU. |
| M4 Services and operations | Native Rust and C SDK, Linux user-space ABI, POSIX APIs, concurrent sockets, scripting, secure update and recovery | Demonstrate native Rust application execution, a pipeline between native C programs, and direct execution of an existing Linux CLI separately. Verify API contracts and resource limits. Start with static AArch64 executables. Qualify x86-64 and dynamically linked Linux ELF through separate compatibility tests. Only selected dependencies enter the safety configuration approval scope. |
| M5 Product safety evidence | Safety manual, residual risks, configuration baseline, and independent review package for the first product configuration | Confirm that the plan matches the actual conditions of use. Have the evidence supporting the ASIL D safety goals and cybersecurity assessed within that scope. |
| M6 Additional ports | Porting plan and verification evidence for each representative P4 board | The systems team approves resource and protection assumptions for each Arm AArch32, additional R/M, and IA-32 board. Update support status to reflect the common boot, isolation, and scheduling tests and board-specific tests that pass. |

M0 decisions cover the first product use case and operating environment, the first physical board and Linux device, the safe state and allowable fault response time, access to the hardware safety manual, and responsibility for safety and security reviews. Trace each decision from the relevant objective and assumption IDs into the downstream documents.

[Safety and security concept](safety-security-concept.en.md) · [Requirements](requirements.en.md) · [Lifecycle](lifecycle.en.md)
