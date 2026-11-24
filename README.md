# J-NOX

English · [한국어](README.ko.md)

J-NOX is a lightweight real-time microkernel written in Rust for embedded systems with functional safety requirements.

The name stands for **Just-enough Nucleus, Open eXtensions**. The architecture calls for isolated services, native Rust drivers and a compatibility domain for Linux drivers.

On AArch64 QEMU `virt`, one static C executable built with the native v1 SDK entered user mode (EL0), computed 37 and exited with status 37. Its stdout write returned `-ENOSYS` at the syscall boundary and emitted zero bytes. [Native C startup and exit](docs/native-el0-startup.en.md)

On AArch64 QEMU, the kernel runs linked Rust tasks using fixed-priority FIFO or round robin. Both policies also boot on the Raspberry Pi 4 model. In six separate Pi 4 model runs, an interrupted CRC job finished on the other core under global scheduling and on its original core under partitioned or clustered scheduling. [CRC resume evidence](https://github.com/Romanof123/JNOX/blob/447e777d7a007fef405d5f9a6ac3c38ead475a15/docs/evidence/pi4-crc-mainline-first.en-ko.md) The scheduling release matrix links each tested configuration to its evidence and lists the remaining work. [Run linked Rust tasks](docs/fixed-runtime.en.md) · [Pi 4 model results](docs/evidence/evd-031.en.md) · [Scheduling release matrix](docs/scheduling-release-cases.en.md)

On single-core QEMU, three batch policies ran a telemetry workload that summarized samples, counted samples at or above 42 and rejected malformed input. A separate nested-group workload resumed across three quota windows and checked the accumulated total of 100224. [Batch results](docs/evidence/admitted-telemetry-batch-first.en-ko.md) · [Nested-group results](docs/evidence/recurring-hierarchical-first.en-ko.md)

On the single-core QEMU `virt` model, linked Rust jobs also run under earliest-deadline-first (EDF), sampled least-laxity-first (LLF) and weighted virtual runtime. The deadline tests follow a preempted worker back to its saved instruction. The weighted tests measure CPU share for two busy workers and how long a sleeping peer takes to run after wakeup. [Run EDF and LLF jobs](docs/ordinary-deadline-plan.md) · [Run weighted workloads](docs/ordinary-weighted.md)

**Planning draft 0.1** established the project baseline. Automotive development targets ISO 26262 ASIL D safety goals for a specified product configuration and cybersecurity engineering in accordance with ISO/SAE 21434. Product selection, physical-board validation, safe-state approval and certification remain open. The virtual bench has its own safety parameters. [Bench safety decision](docs/safety-parameters.en.md)

| Document | What it covers |
| --- | --- |
| [Product plan](docs/product-plan.en.md) | Product objectives, target platforms, scope and milestone deliverables |
| [Platform capability baseline](docs/platform-capabilities.en.md) | Core and optional profiles, proposed additions and acceptance criteria |
| [Safety and cybersecurity concept](docs/safety-security-concept.en.md) | Candidate hazards and threats, assumptions, safety boundaries and initial analysis plan |
| [First bench decision by the functional safety team](docs/safety-parameters.en.md) | Safe state and timing budget for a virtual output, and the evidence needed to set product values |
| [System requirements](docs/requirements.en.md) | Requirements, trace IDs and acceptance criteria |
| [System architecture](docs/architecture.en.md) | Kernel, Linux driver domain, scheduling, ports and user services |
| [Kernel architecture selection](arch/README.md) | AArch64 backend and build interface |
| [Application tracing contract](docs/application-tracing.en.md) | Proposed events, request timelines, capture limits and verification stages |
| [First boot of the M1 Rust kernel](docs/m1-boot.en.md) | Scope and UART acceptance criteria for the minimal AArch64 QEMU kernel |
| [Pi 4 physical bench](docs/boards/pi4-p1.en.md) | SD boot files, serial wiring and independent model and hardware acceptance |
| [M1 timer preemption](docs/m1-scheduler.en.md) | Criteria for observing the timer interrupt, higher-priority dispatch and interrupted thread resumption in one boot |
| [M1 equal-priority FIFO](docs/m1-fifo.en.md) | Acceptance criteria for ready order, voluntary yield and resumption of equal-priority threads |
| [Run linked Rust tasks](docs/fixed-runtime.en.md) | Build the AArch64 fixed-priority runtime, select an application table and boot it with FIFO or round robin |
| [Run ordinary Rust batch jobs](docs/ordinary-batch.en.md) | Submit and release jobs, compare SJF/SRTF/HRRN and recover linked frames from a QEMU capture |
| [Run multilevel queues](docs/ordinary-multilevel.en.md) | Run linked Rust workers under MLQ and MLFQ and inspect how blocking, demotion and priority boosts affect dispatch |
| [Scheduling policies and round robin](docs/scheduling-policies.en.md) | Selection, budget and isolation contracts, with execution evidence by policy |
| [IPC profiles](docs/ipc-mechanisms.en.md) | Ten IPC families, their resource limits and acceptance cases after scheduling qualification |
| [Verification plan](docs/verification-plan.en.md) | Methods, pass criteria, hardware verification and evidence format |
| [Development lifecycle and change control](docs/lifecycle.en.md) | Traceability from planning through implementation and verification, including redesign and reapproval after a change |
| [Trace links](trace/links.csv) | Machine-readable relationships shared by both language editions |

Project templates cover [change requests](.github/ISSUE_TEMPLATE/change-request.md), [pull request handoffs](.github/pull_request_template.md), and [verification evidence](docs/templates/verification-evidence.en.md).
