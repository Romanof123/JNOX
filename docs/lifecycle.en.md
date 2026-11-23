# Development lifecycle and change control

[한국어](lifecycle.md) · English

The team responsible for each stage maintains its records, from the product plan and safety and security analyses through system requirements, design, implementation records, verification specifications, test evidence, and release baselines. GitHub issues track progress. Teams record decisions and the reasons for approval in version-controlled documents linked to the relevant issue.

Teams maintain documents, issues, and work templates in both Korean and English. The responsible team updates both languages as part of the same change. At baseline approval, it checks that trace IDs, acceptance criteria, change scope, and approval status agree across both languages.

## Teams and handoffs

| Role | Responsibilities | Handoff |
| --- | --- | --- |
| Project manager | Manage scope, schedule, dependencies between teams, blockers, and approval status. Route change requests to the systems team and coordinate decisions with the product owner. | Baseline status, assignment history, approvals, and open decisions |
| Systems team | Own product planning, hazard analysis and risk assessment (HARA), threat analysis and risk assessment (TARA), requirements, interfaces, design, and change impact analysis. Work with the safety and security leads to define how requirements can be verified. | Assumptions and designs for approval, requirement IDs, interface contracts, and verification requirements |
| Component development teams | Implement the approved module designs and interfaces on separate branches. Submit unit tests and the rationale for each change. | Code, change issues, unit tests, implementation IDs, and known constraints |
| Integration team | Assemble the kernel, board support package (BSP), services, and Linux domain configuration. Check the configuration and look for contract violations, then freeze the exact candidate for verification. | Version-pinned images, configuration and tools, rebuild procedure, SHA-256 digests, and integration results |
| Verification team | Assign a fresh verifier each round to test against the requirements and reach a verdict in an independent environment. Disclose implementation history and the development and integration teams’ verdicts and logs only after the verification team has preserved its initial verdict and raw logs. | Independent verdict, environment, procedure, raw logs, newly observed issues, and coverage and structural review records |
| Release team | Check approved artifacts against the submitted digests, then package, sign, distribute, and verify recovery. | Signed package, release configuration, software bill of materials (SBOM), change list, and evidence of deployment and recovery |

Each round starts with a fresh verifier who had no role in designing, implementing, or integrating the change and who starts a new work session. The first handoff contains only the approved requirements, public interfaces, test procedures, inputs, expected results, and build identifiers. After preserving the black-box verdict and logs, the verifier performs the necessary source review, static analysis, coverage review, and safety analysis. Record repeat runs within a round separately from subsequent rounds. Agents can conduct verification using this workflow. The authorized organizations and personnel review independence and accountability for product safety assessment.

## Trace records

| Record | ID and location | Relationship |
| --- | --- | --- |
| Product objectives | `OBJ` · [Product plan](product-plan.en.md) | Establish the origin of hazards, threats, and requirements. |
| Safety and security concept | `HAZ`, `THR`, `ASM` · [Concept document](safety-security-concept.en.md) | Record what needs protection and the assumptions of use, then link the derived requirements. |
| Requirements | `REQ` · [System requirements](requirements.en.md) | Link upstream rationale, downstream designs, and acceptance decisions. |
| Design, decisions, and implementation | `DES`, `ADR`, `IMP` · [Architecture](architecture.en.md), followed by module designs, decision records, and change pull requests | Link design contracts and reasons for change to the implementation location and version. |
| Verification and results | `VER`, `EVD` · [Verification plan](verification-plan.en.md), followed by test specifications and version-pinned evidence | Link requirements to the actual test subject, environment, and verdict. |
| Change and release | `CHG`, `REL` · Change issues and baseline documents | Link impact, approval, release configuration, and supporting evidence. |

The trace data records the relationships between the IDs defined in these documents so they can be checked automatically. The current draft traces product planning through hazards and assumptions to requirements, designs, and test specifications. When development begins, teams will link `IMP`, `EVD`, and `REL` in the same way. The initial checks look for undefined IDs, orphan requirements, requirements without tests, changes without implementation provenance, release claims without evidence, and reuse of superseded evidence. Code line numbers help readers navigate. The identifiers, file paths, and commits establish the baseline reference. [Trace links](../trace/links.csv)

## From a change request to a new baseline

1. Open a `CHG` record with the reason for the change, previous baseline, originator, and affected objectives and assumptions. Even for a bug report, begin impact analysis with the intended use, operating conditions, and dependencies in the original plan.
2. Starting at the earliest affected point, the systems team reviews the plan, hazards, threats, and safe state. Record why each upstream assumption still holds. If the plan changes, document and obtain approval for the revised plan first.
3. Derive requirements, interfaces, designs, and the verification plan from the revised premises. Mark affected evidence as `Reverification required`. Submit the scope of reverification and the rationale for treating other items as unaffected.
4. The component development and integration teams work against the same approved baseline. In the pull request, link the `CHG`, `REQ`, `DES`, and `IMP` records and identify the impact on testing. Hand the exact candidate to the verification team.
5. After the verification team issues its new results, the release team releases the approved candidate using its digest and confirmation records. Preserve the previous baseline and its evidence as the historical record for that configuration.

For example, when replacing a virtual Linux device with physical hardware that uses DMA, first revisit the conditions of use in `OBJ-003`, ownership of board resources in `ASM-002`, and the possibility of a write crossing into the safety domain in `HAZ-002`. Then check that `REQ-001`, `REQ-004`, `DES-002`, `VER-001`, and `VER-004` reflect the changed conditions. Retain the virtual-device evidence with its original scope.

Use the project templates for change requests, pull request handoffs, and verification evidence. [Change request](../.github/ISSUE_TEMPLATE/change-request.md) · [Pull request handoff](../.github/pull_request_template.md) · [Verification evidence](templates/verification-evidence.en.md)

[Product plan](product-plan.en.md) · [Verification plan](verification-plan.en.md) · [ISO 26262 supporting processes](https://www.iso.org/standard/68390.html)
