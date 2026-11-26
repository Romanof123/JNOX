# Draft Safety and Security Concept

[한국어](safety-security-concept.md) · English

This document records candidate hazards and threats for the bench scenario. Once the first product's functions and operating states are defined, the project will perform hazard analysis and risk assessment (HARA) to evaluate hazards, establish safety goals, and assign ASILs. In parallel, threat analysis and risk assessment (TARA) will examine assets, attack paths, and their impact. The project will assess developing J-NOX as a Safety Element out of Context (SEooC) by documenting assumptions of use and integration responsibilities. [ISO 26262 software development](https://www.iso.org/standard/68388.html) · [ISO 26262 supporting processes](https://www.iso.org/standard/68390.html) · [ISO/SAE 21434](https://www.iso.org/standard/70918.html)

## Candidate hazards and threats

| ID | Source objectives | Failure or threat and its impact | Planned response |
| --- | --- | --- | --- |
| `HAZ-001` | `OBJ-001`, `OBJ-004` | Driver-service load, an interrupt storm, priority inversion, or memory bus contention delays a safety output. | Assign cores, interrupt resources, and CPU budgets. Analyze response time, monitor deadlines, and define the transition to a safe state. |
| `HAZ-002` | `OBJ-001`, `OBJ-003` | A driver-service fault or a DMA operation modifies safety-related code or data. | Isolate address spaces, devices, and DMA. Verify the scope of resets. |
| `HAZ-003` | `OBJ-001`, `OBJ-003` | Control logic acts on a value that is late, stale, or out of sequence. | Check validity, freshness, and sequence. Discard expired outputs and select the product-specific safe state. |
| `HAZ-004` | `OBJ-001`, `OBJ-004` | A kernel, watchdog timer, clock, or synchronization failure allows an error to persist while operation appears normal. | Provide an independent monitoring path, analyze diagnostic coverage, and define testable fault responses. |
| `THR-001` | `OBJ-002`, `OBJ-003` | A driver or application service gains privileges and uses shared queues, DMA, or interrupts to cross a boundary or exhaust safety-service resources. | Apply least privilege, isolation, fixed limits, and message validation. Test behavior under resource overload. |
| `THR-002` | `OBJ-002` | A forged image or configuration, an obsolete version, or a component altered in the supply chain is put into operation. | Verify signatures, enforce version policy, recover to an approved image, and maintain build provenance and the software bill of materials (SBOM). |
| `THR-003` | `OBJ-002`, `OBJ-006` | Socket, scripting, or management interfaces grant excessive privileges or resources. | Enforce authorization, runtime isolation, resource limits, and input validation. Record operational security events. |

## Scope of the safety argument for IPC

To address `HAZ-003`, `REQ-019` and `DES-010` propose receiver-side end-to-end checks for selected safety-related channels. The first case for `VER-019` is the virtual output permit in `ADR-001`. Inject corrupt, misrouted, duplicate, expired and prior-generation messages. Verify that a rejected message cannot change the last valid permit’s issue time or the accepted sequence number, or advance the rearm count. After valid permits stop, the verification team independently observes fault detection and the transition to inhibited output at the device while invalid messages continue to arrive.

For each product channel, the Functional Safety Team reviews the fault model, the coverage and residual risk of the selected integrity check, and the independence of the receiver’s timebase and output path. For deliberate modification under `THR-001`, assess sender authentication and access control against the threat analysis. An authorized sender can supply a false value with a valid CRC. Substantiate values used in safety decisions with independent input or physical plausibility checks as required by the product hazard.

## Assumptions of use and integration obligations

| ID | Source objectives | Assumption to confirm | Proposed owner |
| --- | --- | --- | --- |
| `ASM-001` | `OBJ-001`, `OBJ-004` | The product defines the safe state for each hazard, the fault-tolerant time interval (FTTI), the minimum control period, and the behavior during system failure. Where required, it provides a path for continued operation with reduced functionality. | The Functional Safety Team leads derivation. The item integration authority decides on product use after independent review. |
| `ASM-002` | `OBJ-001`, `OBJ-003`, `OBJ-005` | At board level, the project can identify ownership of boot, memory, clocks, resets, interrupts, and DMA isolation, as well as the extent of shared power and interconnect resources. | BSP and hardware leads |
| `ASM-003` | `OBJ-001`, `OBJ-003` | J-NOX limits who may restart each isolated driver service and which devices a reset affects. I/O needed to reach a safe state remains accessible during a driver-service failure. | System architecture and verification leads |
| `ASM-004` | `OBJ-002` | Image signing, key rotation, and recovery policies are defined, along with responsibility for production updates. Emergency updates also follow the safe-state and reverification procedures. | Security and release leads |

For each product hazard and operating condition, the FTTI is the shortest time from fault occurrence until a hazardous event becomes possible if the safety mechanisms do not intervene. After defining the item and hazardous event, the Functional Safety Team derives that interval with the systems and integration teams. They reserve a safety margin within the FTTI and allocate the remaining fault-response budget across detection, scheduling, communication, state transition, and actuator response. Record the maximum delay observed in testing and the upper bound supported by analysis separately. [ISO 26262 vocabulary](https://www.iso.org/standard/68383.html) · [ISO 26262 concept phase](https://www.iso.org/standard/68385.html)

The Functional Safety Team sets synthetic inputs, fallback outputs, and test deadlines for the bench. In a product, the operating conditions and hazard determine whether maintaining availability or stopping output is the safe action. The product safe state and timing limits require evidence of physical behavior, independent review, and an integration approval record. [Bench decision ADR-001](safety-parameters.en.md)

The safety case covers kernel code that affects safety requirements, the selected BSP and boot chain, device, memory and scheduling settings, and the hypervisor and safety services where applicable. When a safety function depends on data from a driver service, including one built from Linux source, trace that dependency and assess the risk remaining after communication checks and an independent fault response. Dependent failure analysis also covers shared power, memory, buses, resets, and clocks.

Security operations include vulnerability intake and impact assessment, review of the safety impact of security patches, the SBOM and evidence for signed releases, audit logs, and incident response. Boot verification failure and loss of power during an update are joint safety and security verification scenarios.

[Product plan](product-plan.en.md) · [Derived requirements](requirements.en.md) · [Verification plan](verification-plan.en.md)

## Evaluating a functional-safety OS (informative)

Start with the item hazard analysis and risk assessment (HARA). Which hazard can the OS influence, what safe state must the item reach, and how much time is available? For each candidate, match the exact release, BSP, toolchain and enabled services to the safety manual, assumptions of use and release evidence.

**RT-Thread.** The community kernel is one candidate. For a commercial safety edition, request the certificate and report for the proposed configuration. The vendor links a separate RT-Thread Safety AUTO certificate and describes AUTOSAR Classic Platform service protection, timing monitoring and memory protection for its vehicle-control offering. A historical SGS-TÜV Saar certificate dated 21 February 2022 covers RT-Thread Smart for Safety v1.0 as a component of safety-related software up to ASIL D. Confirm current validity and configuration coverage with the supplier. [Vendor automotive product page](https://www.rt-thread.com/col26/list.html) · [Certificate reproduced on slide 24](https://www.esbf.org/wp-content/uploads/2024/08/202408_XPX.pdf#page=24)

**J-NOX.** ADR-001 makes the bench acceptance criterion inspectable. Permits arrive every 10 ms and expire at age 20 ms. From loss, detection has a 25 ms budget. Output inhibition has a further 5 ms, with 30 ms overall. The product’s physical fault-tolerant time interval (FTTI) must be derived from its hazard and checked through the actuator. Current J-NOX runtime observations cover QEMU only. Product safety approval remains open. [ADR-001 bench parameters](safety-parameters.en.md)

For each driver service sharing the target, examine its CPU allowance, MMIO and DMA access, IRQ routing and reset authority. Check that stale or repeated IPC permits cannot renew permission. Verify that the output is inhibited by its deadline when fresh permits stop. Trace any failed bound or isolation assumption upstream to the hazard, requirement and design decision, then repeat the affected verification on the selected release and board.

For an automotive release decision, the RT-Thread safety editions provide published product-level certification evidence. The proposed J-NOX safety profile still needs a frozen hardware configuration, an approved safety manual and independent evidence for the complete fault-to-physical-output path.
