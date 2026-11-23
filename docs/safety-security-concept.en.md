# Draft Safety and Security Concept

[한국어](safety-security-concept.md) · English

This document records candidate hazards and threats for the bench scenario. Once the first product's functions and operating states are defined, the project will perform hazard analysis and risk assessment (HARA) to evaluate hazards, establish safety goals, and assign ASILs. In parallel, threat analysis and risk assessment (TARA) will examine assets, attack paths, and their impact. The project will assess developing J-NOX as a Safety Element out of Context (SEooC) by documenting assumptions of use and integration responsibilities. [ISO 26262 software development](https://www.iso.org/standard/68388.html) · [ISO 26262 supporting processes](https://www.iso.org/standard/68390.html) · [ISO/SAE 21434](https://www.iso.org/standard/70918.html)

## Candidate hazards and threats

| ID | Source objectives | Failure or threat and its impact | Planned response |
| --- | --- | --- | --- |
| `HAZ-001` | `OBJ-001`, `OBJ-004` | Linux load, an interrupt storm, priority inversion, or memory bus contention delays a safety output. | Assign cores, interrupt resources, and CPU budgets. Analyze response time, monitor deadlines, and define the transition to a safe state. |
| `HAZ-002` | `OBJ-001`, `OBJ-003` | A Linux or driver fault, or a DMA operation, modifies safety-related code or data. | Isolate address spaces, devices, and DMA. Verify the scope of resets. |
| `HAZ-003` | `OBJ-001`, `OBJ-003` | Control logic acts on a value that is late, stale, or out of sequence. | Check validity, freshness, and sequence. Discard expired outputs and select the product-specific safe state. |
| `HAZ-004` | `OBJ-001`, `OBJ-004` | A kernel, watchdog timer, clock, or synchronization failure allows an error to persist while operation appears normal. | Provide an independent monitoring path, analyze diagnostic coverage, and define testable fault responses. |
| `THR-001` | `OBJ-002`, `OBJ-003` | Linux or a user service gains privileges and uses shared queues, DMA, or interrupts to cross a boundary or exhaust safety-service resources. | Apply least privilege, isolation, fixed limits, and message validation. Test behavior under resource overload. |
| `THR-002` | `OBJ-002` | A forged image or configuration, an obsolete version, or a component altered in the supply chain is put into operation. | Verify signatures, enforce version policy, recover to an approved image, and maintain build provenance and the software bill of materials (SBOM). |
| `THR-003` | `OBJ-002`, `OBJ-006` | Socket, scripting, or management interfaces grant excessive privileges or resources. | Enforce authorization, runtime isolation, resource limits, and input validation. Record operational security events. |

## Assumptions of use and integration obligations

| ID | Source objectives | Assumption to confirm | Proposed owner |
| --- | --- | --- | --- |
| `ASM-001` | `OBJ-001`, `OBJ-004` | The product defines the safe state for each hazard, the fault-tolerant time interval (FTTI), the minimum control period, and the behavior during system failure. Where required, it provides a path for continued operation with reduced functionality. | The Functional Safety Team leads derivation. The item integration authority decides on product use after independent review. |
| `ASM-002` | `OBJ-001`, `OBJ-003`, `OBJ-005` | At board level, the project can identify ownership of boot, memory, clocks, resets, interrupts, and DMA isolation, as well as the extent of shared power and interconnect resources. | BSP and hardware leads |
| `ASM-003` | `OBJ-001`, `OBJ-003` | Authority to restart Linux and the effects of device resets are bounded. I/O needed to reach a safe state remains accessible during a Linux failure. | System architecture and verification leads |
| `ASM-004` | `OBJ-002` | Image signing, key rotation, and recovery policies are defined, along with responsibility for production updates. Emergency updates also follow the safe-state and reverification procedures. | Security and release leads |

For each product hazard and operating condition, the FTTI is the shortest time from fault occurrence until a hazardous event becomes possible if the safety mechanisms do not intervene. After defining the item and hazardous event, the Functional Safety Team derives that interval with the systems and integration teams. They reserve a safety margin within the FTTI and allocate the remaining fault-response budget across detection, scheduling, communication, state transition, and actuator response. Record the maximum delay observed in testing and the upper bound supported by analysis separately. [ISO 26262 vocabulary](https://www.iso.org/standard/68383.html) · [ISO 26262 concept phase](https://www.iso.org/standard/68385.html)

The Functional Safety Team sets synthetic inputs, fallback outputs, and test deadlines for the bench. In a product, the operating conditions and hazard determine whether maintaining availability or stopping output is the safe action. The product safe state and timing limits require evidence of physical behavior, independent review, and an integration approval record. [Bench decision ADR-001](safety-parameters.en.md)

The safety case covers kernel code that affects safety requirements, the selected BSP and boot chain, device, memory and scheduling settings, and the hypervisor and safety services where applicable. When a safety function depends on data from Linux, trace that dependency and assess the risk remaining after communication checks and an independent fault response. Dependent failure analysis also covers shared power, memory, buses, resets, and clocks.

Security operations include vulnerability intake and impact assessment, review of the safety impact of security patches, the SBOM and evidence for signed releases, audit logs, and incident response. Boot verification failure and loss of power during an update are joint safety and security verification scenarios.

[Product plan](product-plan.en.md) · [Derived requirements](requirements.en.md) · [Verification plan](verification-plan.en.md)
