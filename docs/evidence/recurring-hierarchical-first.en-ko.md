# Independent hierarchy service verification / 중첩 그룹 독립 검증

PASS for the declared normal workload in #212 under #21. Candidate 59de3c67fcca3c911d3fd6a9bdbad7b4d2cf64ee. Predeclared contract and fresh comparator 8eda62383cbcaf6b01285b6198b269c5ba19eea4. Native WSL ext4, Rust/Cargo 1.95.0, QEMU 6.2.0 A72 virt/GICv2, single vCPU. Four candidate first cold boots, in the frozen order, passed on first launch. Two supplemental first ordinary boots of the upstream comparator also passed. No injected delay or fault. Source initializes EL1h SPSR 0x345 for each worker. Normal apps exclude the late task mask. External IRQ/ERET identifies real preemption and resumption. PPI identity is inferred from guest source and configuration.

Boot outcome / 부팅 결과

|Image|Dispatches|Parent 0 debt, ticks|Idle durations, ticks|Maximum timer reaction|
|---|---:|---|---|---:|
|Original plain / traced|8 / 8|34511|11694437|34511|
|Recurring plain / traced|14 / 14|20082, 9875, 8563|3883823, 3919184, 11716227|21377|
|Upstream plain / traced|8 / 8|28711|11701725|28711|

Timing columns use diagnostic captures. All observations fit 8P and every timer reaction fit P/8 (1,953,125 ticks). P=15,625,000. Idle service debit was zero throughout. Each recurring stream contained peer receipts 33152, 33408 and 33664 with cumulative totals 33152, 66560 and 100224. The producer validated the final total before its resume marker. A block, sibling wake, three producer expirations in windows 0/1/2, two peer quota expirations, peer exit, three idle recoveries and final worker exits account for the handoffs. All six external logs link each restore to an initial entry or the exact previously interrupted PC, including idle. Three stopped diagnostic PCs land inside idle. Indexed frame addresses map to four distinct 16 KiB stack slots.

Independent per-dispatch tables in ledger.json give all eight signed balances, each node's outgoing debit, prior owner, actual elapsed, ready mask, grid window, parent/child/leaf debt, compare and frame addresses. Node order is parent 0, parent 1, children 0–2, leaves 0–2. The independent verifier advances each node to each crossed refill, ranks eligible ready leaves, and confirms actual chosen owners, compares, overrun and saved/restored frames. All three levels alone pay their owner's interval. Parent 0 exhausted while its sibling child still held credit. Competing non-owner paths paid zero except shared ancestor service. Raw fields report debt and charge. Positive balances are reconstructed.

Host controls include seven upstream-maintained candidate tests and the supplied corruption fixture/replay. Independently compiled model.rs uses integer tick steps across eight small periods, varying all eight masks and asymmetric caps, repeated settlements, stale and outstanding reservations, plus near-u64::MAX exact-boundary starts. 10,162 counted mask/horizon comparisons passed. Model path resolves alongside a candidate checkout at ../src. EVD-038 archive hash and each original member hash verified separately. Its historical ordinary/late measurements were archival only. The fresh old comparator used the ordinary image only.

한국어 판정. 사전 선언한 후보 네 이미지가 최초 부팅에서 모두 통과했다. 선행 커밋의 일반 이미지 두 개도 새로 부팅해 비교했다. 반복 작업의 세 차례 합과 총합 100224를 UART와 작업 종료 경로로 확인했다. 부모 0은 독립된 세 창에서 소진됐고 다른 부모의 동료 할당량은 두 번 소진됐다. 세 번 모두 유휴 복구가 실측됐으며 유휴 실행에는 비용이 부과되지 않았다. 복원 PC·스택·프레임, 형제별 잔액과 각 구간의 부모·자식·리프 차감을 독립 재구성했다. 별도 정수 모형의 준비 마스크 및 카운터 상한 비교 10162건이 통과했다. EVD-038 자료는 과거 기록이다. 현 판정 범위는 이 가상 단일 CPU와 구성된 3단계 정상 작업이다. 실기 지연, 제품 수용성·격리는 미검증이다.

[Original records and verifier checksum / 원본 기록과 검증자 해시](recurring-hierarchical-first.zip) · SHA-256 ccff2b51a34ff272f4ffedab1a71273374aa2bca125689c0350350532ff42304.
