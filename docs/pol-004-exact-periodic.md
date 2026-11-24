# POL-004 exact periodic pair

[한국어](#한국어)

## English

CHG-035 / IMP-033 extends the POL-004 periodic workload. The first guest boot passed the independent replay. [EVD-033](evidence/evd-033.en.md)

One logical tick represents one common time unit. A has (period, relative deadline) (10,9), B has (12,4). Each policy runs ticks 0–13 on successive physical timer interrupts. Enqueue B before A at tick 0. RM must dispatch A then B, DM must dispatch B then A. In each epoch A must run again at tick 10 and B at tick 12. Tick 13 checks both jobs have completed and the controller has resumed. This finite window exercises the first repeated release of each task.

Accept only the exact UART transcript for profile periodic-exact plus an independent exception trace with 28 physical timer IRQs and eight worker SVC #1 completions. Each epoch needs two SVCs after IRQ 1, one after IRQ 11 and one after IRQ 13. Inspect the ELF vectors, worker markers and completion handler against that trace. Record the revision, ELF hash and first launch result.

REQ-002 / DES-003 / VER-002 covers IRQ delivery. REQ-005 / DES-003 / VER-005 covers worker completion and controller restoration. REQ-023 / DES-012 / VER-023 covers policy selection and later releases. [Periodic design](m1-periodic.en.md) · [Architecture](architecture.en.md) · [Verification plan](verification-plan.en.md)

## 한국어

CHG-035 / IMP-033은 POL-004 주기 작업 시험을 확장한다. 첫 게스트 부팅이 독립 재현을 통과했다. [EVD-033](evidence/evd-033.md)

한 틱은 두 작업에 공통인 시간 단위다. A의 (주기, 상대 마감)은 (10,9), B는 (12,4)다. 물리 타이머 IRQ마다 한 틱씩 진행하고 각 정책을 0–13틱 동안 실행한다. 0틱에는 B를 먼저 큐에 넣고 RM은 A→B, DM은 B→A 순서로 실행해야 한다. 각 정책에서 10틱에 A, 12틱에 B를 다시 실행하고 13틱에 완료 및 제어기 복귀를 확인한다.

합격 조건은 periodic-exact UART 출력 전체 일치, 물리 타이머 IRQ 28회, 작업 완료 SVC #1 8회다. 각 구간에서 첫 IRQ 뒤 SVC 2회, 11번째와 13번째 IRQ 뒤 각각 1회가 필요하다. ELF의 벡터·작업 진입 표시·완료 핸들러와 예외 기록을 대조하고 리비전, ELF 해시 및 첫 실행 결과를 남긴다.

IRQ 전달은 REQ-002 / DES-003 / VER-002, 작업 완료와 제어기 복귀는 REQ-005 / DES-003 / VER-005, 정책 선택과 후속 릴리스는 REQ-023 / DES-012 / VER-023에 할당한다. [주기 작업 설계](m1-periodic.md) · [아키텍처](architecture.md) · [검증 계획](verification-plan.md)
