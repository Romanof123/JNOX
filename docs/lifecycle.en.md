# Development lifecycle and change control

[한국어](lifecycle.md) · English

The team responsible for each stage maintains its records, from the product plan and safety and security analyses through system requirements, design, implementation records, verification specifications, test evidence, and release baselines. GitHub issues track progress. Teams record decisions and the reasons for approval in version-controlled documents linked to the relevant issue.

Teams maintain documents, issues, and work templates in both Korean and English. The responsible team updates both languages as part of the same change. At baseline approval, it checks that trace IDs, acceptance criteria, change scope, and approval status agree across both languages.

## Teams and handoffs

| Role | Responsibilities | Handoff |
| --- | --- | --- |
| Project manager | Manage scope, schedule, dependencies between teams, blockers, and approval status. Route change requests to the systems team and coordinate decisions with the product owner. | Baseline status, assignment history, approvals, and open decisions |
| Systems team | Own product planning, requirements, interfaces, architecture, and change impact analysis. Coordinate TARA with the security lead and allocate safety requirements to components with the Functional Safety Team. | Item definition, requirement IDs, interface contracts, system design, and verification requirements |
| Functional Safety Team | Lead safety planning and HARA. Set test inputs and acceptance limits for the bench. With the systems and integration teams, derive the safe state, FTTI, and fault-response budget for each product hazard. Recommend product release on safety grounds or withhold the recommendation when evidence is insufficient. | Safety goals, assumptions of use, parameter decisions, rationale for timing budgets and margins, open findings, and release recommendation status |
| Component development teams | Implement the approved module designs and interfaces on separate branches. Submit unit tests and the rationale for each change. | Code, change issues, unit tests, implementation IDs, and known constraints |
| Integration team | Assemble the kernel, board support package (BSP), services, and Linux domain configuration. Check the configuration and look for contract violations, then freeze the exact candidate for verification. | Version-pinned images, configuration and tools, rebuild procedure, SHA-256 digests, and integration results |
| Verification team | Assign a fresh verifier each round to test against the requirements and reach a verdict in an independent environment. Disclose implementation history and the development and integration teams’ verdicts and logs only after the verification team has preserved its initial verdict and raw logs. | Independent verdict, environment, procedure, raw logs, newly observed issues, and coverage and structural review records |
| Release team | Check approved artifacts against the submitted digests, then package, sign, distribute, and verify recovery. | Signed package, release configuration, software bill of materials (SBOM), change list, and evidence of deployment and recovery |

Once the Functional Safety Team freezes the bench criteria, development and integration can proceed within that scope. For a product value, the team submits the rationale for independent review. The value becomes binding when the designated item integration authority (the OEM or its delegate under the development agreement) records the applicable scope, version, and approval decision. The PM tracks decisions and unresolved items. [First bench decision ADR-001](safety-parameters.en.md) · [ISO 26262 management of functional safety](https://www.iso.org/standard/68384.html)

Each round starts with a fresh verifier who had no role in designing, implementing, or integrating the change and who starts a new work session. The first handoff contains only the approved requirements, public interfaces, test procedures, inputs, expected results, and build identifiers. After preserving the black-box verdict and logs, the verifier performs the necessary source review, static analysis, coverage review, and safety analysis. Record repeat runs within a round separately from subsequent rounds. Agents can conduct verification using this workflow. The authorized organizations and personnel review independence and accountability for product safety assessment.

## Trace records

Teams may write their assigned modules concurrently. The PM pins the integrated source commit and schedules builds, QEMU runs and reviews in sequence on the shared host. The verifier receives the acceptance criteria and candidate digest and observes the outcome independently of development verdicts. Cases that can share initial state run in one guest, with inputs, outputs and cleanup recorded per case. Assign a fresh boot when independent initial state or recovery is part of the acceptance criteria.

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
2. Starting at the earliest affected point, the systems team reviews the plan and threats. The Functional Safety Team reassesses affected hazards, safe states, and timing allocations. Record why each upstream assumption still holds. If the plan changes, document and obtain approval for the revised plan first.
3. Derive requirements, interfaces, designs, and the verification plan from the revised premises. Mark affected evidence as `Reverification required`. Submit the scope of reverification and the rationale for treating other items as unaffected.
4. The component development and integration teams work against the same approved baseline. In the pull request, link the `CHG`, `REQ`, `DES`, and `IMP` records and identify the impact on testing. Hand the exact candidate to the verification team.
5. After the verification team issues its new results, the release team releases the approved candidate using its digest and confirmation records. Preserve the previous baseline and its evidence as the historical record for that configuration.

For example, when replacing a virtual Linux device with physical hardware that uses DMA, first revisit the conditions of use in `OBJ-003`, ownership of board resources in `ASM-002`, and the possibility of a write crossing into the safety domain in `HAZ-002`. Then check that `REQ-001`, `REQ-004`, `DES-002`, `VER-001`, and `VER-004` reflect the changed conditions. Retain the virtual-device evidence with its original scope.

Use the project templates when opening work or handing it off. [Engineering task or defect](../.github/ISSUE_TEMPLATE/engineering-task-or-defect.md) · [Change request](../.github/ISSUE_TEMPLATE/change-request.md) · [Pull request handoff](../.github/pull_request_template.md) · [Verification evidence](templates/verification-evidence.en.md)

[Product plan](product-plan.en.md) · [Verification plan](verification-plan.en.md) · [ISO 26262 supporting processes](https://www.iso.org/standard/68390.html)

## Following a safety-related change through issues and pull requests

The PM assigns a named team or person to each open action on the existing issue, records the next decision and its owner, and returns the scoped outcome to the parent issue. The systems owner links the parent plan, change, affected requirements, design and verification criteria before implementation. The Functional Safety Team records affected hazards, assumptions, timing allocations and outstanding authority decisions. The implementation PR points back to the issue and design. Integration adds the exact source commit, configuration and artifact digests. The independent verifier links the first-run raw archive, planned denominator, observed outcome and limitations back to the PR and issue. The release owner links any actual release decision and baseline to that evidence. Maintain links in both directions at handoff so a reviewer can start from the release, issue or requirement and locate the same candidate.

The PM uses GitHub `team:*` labels to identify the role owning the next action and updates them at handoff. Account assignments and formal signatures are recorded separately where applicable. During active work the PM reconciles both language editions and durable issue, PR and evidence links periodically and after every related merge. Keep the parent open while backlog decisions, board dependencies or product conditions remain, with each assigned to its documented role.

Classify each verdict by subject and environment (host model, virtual workload, physical board or named product configuration). Close a scoped issue once its declared criteria and artifacts have been independently assessed and all unresolved obligations have an owner and linked disposition. A merged plan records agreed criteria; a merged implementation identifies source. Physical timing, production safety acceptance and release each require their own applicable evidence and authority. Preserve the first failure, partial capture, unlaunched positions and subsequent controls against their original digests. Open the corrective action at the earliest affected assumption, record the proposed change and re-verification scope, then retain the new candidate and verdict alongside the original.

### Worked audit · 2026-11-24

* **Reservations.** The release parent led to the ordinary reservation case and integration PR. Two of eight first QEMU runs reached their horizons; six stopped under the original 1 ms guard. The archived compatibility run on candidate `259aaebb` stopped after IRQ four before B resumed. The original control completed five IRQs, and the separate wake workload matched its reference. The follow-up assigns diagnosis and independent re-verification; the shared failure marker leaves the cause open. Keep the ordinary and compatibility denominators separate. [Release #21](https://github.com/Romanof123/JNOX/issues/21) → [case #123](https://github.com/Romanof123/JNOX/issues/123) → [integration #172](https://github.com/Romanof123/JNOX/pull/172) → [finding #176](https://github.com/Romanof123/JNOX/issues/176) · [archive commit 430df8bf](https://github.com/Romanof123/JNOX/commit/430df8bf)
* **Weighted scheduling and IPC.** The earlier weighted task case closed within its scope. The two-process EL0 case established a new owner and planning PR for measured service, isolated address spaces and wake bounds. Its independent results belong to the new issue. IPC planning can proceed; IPC family implementation starts after the scheduling parent records completion and links the evidence. [Task case #55](https://github.com/Romanof123/JNOX/issues/55) → [native case #177](https://github.com/Romanof123/JNOX/issues/177) → [plan #178](https://github.com/Romanof123/JNOX/pull/178) · [IPC gate #24](https://github.com/Romanof123/JNOX/issues/24)

### Open issue and PR inventory (#6, REQ-012, REQ-013)

The PM will save the unfiltered REST issues list (which includes PRs) and run the offline command below. In a POSIX shell with GitHub CLI, capture all pages in one successful request sequence:

```sh
gh api --paginate --slurp 'repos/Romanof123/JNOX/issues?state=open&per_page=100&sort=created&direction=asc' > open-pages.json &&
python tools/open_tracking.py open-pages.json
```

The checker accepts the saved page arrays or a single REST list. It checks each supplied item's current team label and prints every open issue whose body and PR body reference each other. A PR with no such issue fails. Use plain #number, owner/repo#number or a canonical github.com issue/PR URL in body prose. Code examples and HTML comments do not establish a handoff. Exit 0 means the supplied records satisfy these checks, 1 means findings, and 2 means invalid input. Only the collector can confirm it followed every next-page link and used the intended repository and access permissions. Concurrent changes during pagination may require a new capture. The PM confirms primary ownership and tracking intent and records approvals separately.
