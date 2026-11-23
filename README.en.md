# J-NOX

[한국어](README.md) · English

**Just-enough Nucleus, Open eXtensions.** J-NOX is a lightweight, real-time microkernel OS designed for functional safety. Services and drivers run in independent execution domains.

**Planning draft 0.1** is the first baseline. For automotive applications, the project aims to define a configuration that can support ISO 26262 ASIL D safety goals and to develop cybersecurity in accordance with ISO/SAE 21434. The first target system, board, safe state and quantitative timing limits are still open. Product conformity assessment and certification are at the planning stage.

| Document | What it covers |
| --- | --- |
| [Product plan](docs/product-plan.en.md) | Product objectives, target platforms, scope and milestone deliverables |
| [Safety and cybersecurity concept](docs/safety-security-concept.en.md) | Candidate hazards and threats, assumptions, safety boundaries and initial analysis plan |
| [First bench decision by the functional safety team](docs/safety-parameters.en.md) | Safe state and timing budget for a virtual output, and the evidence needed to set product values |
| [System requirements](docs/requirements.en.md) | Requirements, trace IDs and acceptance criteria |
| [System architecture](docs/architecture.en.md) | Kernel, Linux driver domain, scheduling, ports and user services |
| [Verification plan](docs/verification-plan.en.md) | Methods, pass criteria, hardware verification and evidence format |
| [Development lifecycle and change control](docs/lifecycle.en.md) | Traceability from planning through implementation and verification, including redesign and reapproval after a change |
| [Trace links](trace/links.csv) | Machine-readable relationships shared by both language editions |

Project templates cover [change requests](.github/ISSUE_TEMPLATE/change-request.md), [pull request handoffs](.github/pull_request_template.md), and [verification evidence](docs/templates/verification-evidence.en.md).
