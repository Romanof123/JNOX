# Native AMC clock pairs · final independent review / 통합 독립 검증

**Hold the evidence reader at 93488785d3b71ef1d1b88d58ee228409573810d3.** Six authentic sessions meet the declared finite virtual observations on independent raw review. Six rebound false records also received a passing public verdict. The results below separate the actual guest behavior from defects in corroborating its records.

최종 리더 승인은 보류한다. 보존된 실제 여섯 실행은 각각의 가상 배포 기준에 맞지만, 불가능한 인터럽트 응답 네 건과 외부 벡터가 모순되는 두 건도 수정 리더를 통과했다.

The original booted source was 896518d8edcfc850dc16361dcde2d1edd87c0d4c. Git tree/blob identities match the final head for arch, boards, kernel, SDK, examples and the packaging entry point. No new builds or boots were needed for guest applicability. Reverified the public raw archive member manifest, unchanged original preboot seal, all six session hashes, actual kernel and ordered application bytes and disassemblies through the unmodified public entry point and independent RAM decoder. Reran the native production C sampler host controls successfully. Those controls substitute a host counter; the original C/Rust EL0 observations supply guest evidence.

|First|Original→final public|Page/fallback pairs|Actual IAR at HI1/g0|b→i→c→x (ticks)|
|---|---|---:|---|---|
|Normal plain|met→met|842/0|30|10541623→10584911→10592527→11075108|
|Normal traced|met→met|1255/0|30|10224740→10264622→10269668→10694666|
|Staggered plain|met→met|1549/0|30|19539169→19559096→19563900→19984617|
|Staggered traced|met→met|1540/0|30|19457872→19485226→19490128→19904411|
|Disabled normal traced|met→met|0/995|30, diagnostic|9990668→10664574→10668630→11158400|
|C/Rust compatibility|failed→met|663/0|return gate handoff|row 909 preempted, row 1515 restored|

Authored sampler call paths, private page backing, EL0 read-only permissions, roots, per-generation charges and retired products, next return timestamps and paired CPU/monotonic operands were checked on authentic bytes. Zero interval/lifecycle/archive drops. At 62.5 MHz the four mandatory b→x latencies were 8.535760, 7.518816, 7.127168 and 7.144624 ms. Disabled retained the same four ELF bytes and used fallback. The compatibility first charged LO0/g0 2,020,261 ticks before the handoff, HI1/g0 1,267,255, and LO0/g0 4,381,486 total. Same low PC/SP/root were restored and subsequent samples bracketed peer execution. Fresh low generation began with settled charge 0 and sequence 2. The first public verdict remains 5/6. The original #348 traced normal/staggered misses remain SVC-first.

활성 네 실행은 동일 소유자의 실제 PPI30·기한·커밋·앱 관측 순서를 충족했다. 비활성은 폴백을 사용했다. 호환성 대조는 IRQ 대신 복귀 직전 결정에서 선점해 같은 낮은 작업의 PC/SP/루트를 복원했고, 상위 작업 전후에 낮은 작업의 실제 클록 쌍을 남겼다. 과거 두 SVC 선행 사례와 최초 리더의 5/6 판정은 보존한다.

**Counterexamples.** For each mandatory run, changing only IAR from 30 to 1054 (0x41e) in the exact crossing interval, updating its RAM digest and rebinding the session, yielded observations-met. GICv2 IAR bits 12:10 read as zero for PPIs. This change is an impossible raw acknowledgement. It does not establish that a different CPU ran the workload; the deployment has one CPU. The public classifier masks to the low ten bits. For both traced timed runs, keeping IRQ class and ELR but changing the reported EL1 vector from lower-EL IRQ to the adjacent synchronous slot was also accepted. Normal traced 0x40202c80→0x40202c00; staggered traced 0x40202480→0x40202400. The genuine trace matches the IRQ vector in each kernel. Source for the raw IAR interpretation: [Arm GIC architecture specification, GICC_IAR, p. 4-135](https://documentation-service.arm.com/static/5f8ff196f86e16515cdbf969).

By contrast, the public reader rejected all four changed INTIDs, missing-PPI cases, misattributed owners, and corrupted CPU sample operands. It rejected false fallback accounting and fabricated control gate opcode, IRQ/SVC cause, handoff order, charge, restore PC and root, and raw bracket CPU. Both contradictory textual IRQ/SVC labels in external traces were rejected. The 29 RAM controls and four external-trace controls retain their command outcomes. The independent decoder binds to the original RAM and actual words; original captures are unmodified. Positive hardware attribution, physical latency/WCET, FTTI, product acceptance and bounds outside these finite virtual cases remain open.

각 필수 사례에서 원시 IAR를 0x41e로 바꾸거나 추적의 IRQ 진입 벡터를 동기 진입으로 바꾸고 해시를 갱신하면 리더가 모순을 놓친다. 반면 잘못된 인터럽트 번호·소유자·CPU 표본, 거짓 선점 원인, 차감량·복원 정보 등은 거부했다. 여섯 원본을 다시 부팅할 필요 없이 리더를 수정한 뒤 같은 자료와 변조 대조로 검증할 수 있다.
