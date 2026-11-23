# Verification evidence EVD-…

[한국어](verification-evidence.md) · English

| Field | What to record |
| --- | --- |
| Source | Planning baseline, `REQ`, `VER`, `CHG`, and the commit SHA of the verification candidate |
| Verification candidate | SHA-256 digests of the image and configuration, and the exact board, CPU, firmware, and tool versions |
| Verification round | New verifier, execution date, whether this was the first black-box run, and confirmation of independence from implementation |
| Environment | Functional and performance scope, load, physical board or virtual environment, measurement instruments, and clock |
| Procedure | Commands for reproduction, inputs, expected results, and log capture method |
| Evidence | Paths or digests for raw logs, inputs, and outputs, plus material needed to reproduce failures |
| Verdict | Passed / Failed / Blocked / Reverification required, rationale, and residual risks |
| Follow-up review | Required structural and coverage reviews, issues, and scope for reverification |

When linking this record to a product configuration, state the scope the test actually verified and the assumptions of use that still need confirmation.
