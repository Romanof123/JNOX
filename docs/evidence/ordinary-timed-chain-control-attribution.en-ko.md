# #236 ordinary timer attribution / 일반 타이머 귀속 분석

At public commit 317d7365ad966441453eabb38aeab1c78e751f7b, docs/evidence/ordinary-timed-chain-{first,integration}.zip preserve two independent six-boot cohorts. Run python analyze.py PATH_TO_JNOX to regenerate results.json from Git. No boots or builds. Raw excerpt files are byte-for-byte copies from the combined ZIP. The legacy stimulus is 6,250 ticks (100 µs at 62.5 MHz). Chain uses a distinct 62,500-tick waiter deadline and 4 MiB workload.

|Combined traced LOW|Start|Target/readback/IRQ CVAL|Final restore sample|IRQ entry|Entry minus target|Restore to entry|
|---|---:|---:|---:|---:|---:|---:|
|PI|804897|811147|808477|935290|124143 (1986.288 µs)|126813|
|Ceiling|966988|973238|971691|1096196|122958 (1967.328 µs)|124505|

In both, comparator enabled CTL=1, interrupt PPI30, sampled IRQ CTL=5. PI's final successful LOW LOCK(X) SVC at 0x4020247c returned via ERET to 0x40202480. Ceiling's final successful LOW LOCK(Y) SVC at 0x40202550 returned via ERET to 0x40202554 after a prior LOCK(X) at the same wrapper. The IRQ saved that exact return PC and same LOW frame in each case. Both instructions are ordinary_mutex::checked+0xc, word b5000040, CBNZ X0 to error path, immediately before RET. The branch had not advanced before the IRQ. IRQ selected HIGH at 0x40201c54 for PI and LOW at 0x40202554 for ceiling, but stopped before any corresponding ERET. External logs corroborate all legacy exception PCs/returns. Restore timestamps sample the restore path, not ERET retirement. They precede CVAL by 2670 and 1547 ticks respectively.

LOW owned the required lock, its saved IRQ mask was clear, entry exceeded CVAL and ceiling was below midpoint. The sole failed stop(16) condition was IN_WORK=0. Both have zero CHUNKS, no C rows, all K markers zero (including LOW post-final-acquire stamp), and zero B/HIGH/A/M. Zero values are absent progress evidence, not measured zero durations. Exact PC locates the interruption before stamp(0) and mutex_batch_compute. Times include the architectural IRQ vector's work before its counter sample. They do not split time among frame restore, ERET, exception delivery, QEMU translation, tracing I/O or host scheduling. No host descheduling duration, hardware latency bound or causal trace overhead is measurable here.

|IRQ completed chunks, feature plain/trace → combined plain/trace|Feature entry minus CVAL ticks|Combined entry minus CVAL ticks|
|---|---|---|
|PI 42/76 → 57/0|6939/9342|6869/124143|
|Ceiling 80/29 → 18/0|6491/7796|7340/122958|
|Chain 2102/2118 → 2101/1365|7571/6854|9916/95758|

Passing legacy IRQ ELRs point inside mutex_batch_compute at the input-index AND (PI 0x40202bb0, ceiling 0x40202cd8). Each has IN_WORK=1. Every selected ELF matches byte-for-byte between cohorts. SHA-256 PI fc5be57112e78bb9a4d2de6bb4fc17078331e69c7b26eac10660c9eeea6075ce, ceiling 172a88efd01ebda34e6cd05bc32ab4318f814cdec53f40c95d98dc9fb852727c. Comparing corresponding PT_LOAD file bytes gives zero differences in 17,688 PI bytes and 18,032 ceiling bytes, with identical addresses, flags and memory sizes. A different linked legacy instruction stream cannot explain the cohort differences.

Proposed separate diagnostic: freeze one traced first cold PI and one traced first cold ceiling using the exact original ELFs and commands plus host-only monotonic timestamps of vCPU scheduling switches and QEMU exception-log write start/end. Correlate those with the final successful SVC return and following IRQ log records, accounting for buffering and distinct clocks. Report measured off-CPU and blocking intervals or explicitly uncorrelatable data. Preserve the 100 µs comparator and existing work predicate. Keep historical verdicts. A return-anchored variant answers a separate question and already has public prior data in docs/evidence/ordinary-mutex-anchor-integrated.

한국어. 통합 추적 두 건에서 마지막 잠금 성공 뒤 실제 ERET 복귀 PC와 IRQ가 저장한 LOW PC가 일치했다. 모두 성공 결과를 검사하는 첫 CBNZ에서 멈췄으며 작업 진입 전이었다. 타이머 설정·소유권·마스크·천장 중간점은 정상이고 사유 16의 실패 조건은 IN_WORK=0 하나다. 복원 경로 표본은 비교 시각보다 각각 2670·1547틱 빨랐다. IRQ 진입 표본은 비교 시각보다 각각 124143·122958틱 늦었다. LOW 계산·게시·읽기 표식은 모두 없었다. 최초 추적 두 건에서는 같은 ELF로 76·29청크를 기록했다. 두 코호트 사이 적재 바이트 차이는 0이다. 개별 호스트 지연, QEMU 기록 비용, 게스트 복귀 비용을 이 기록만으로 분해할 수 없다. 다음 진단은 기존 ELF·100 µs·작업 판정을 유지한 각 1회 추적에서 호스트 vCPU 스케줄 전환과 로그 쓰기 시간을 수집해 마지막 SVC 복귀와 IRQ 구간을 대조하는 것이다. 기존 판정은 각 최초 기록에 귀속한다.

[Instruction, timer and control-flow analysis of both original stalled controls](ordinary-timed-chain-control-attribution.zip) · SHA-256 83bb204472d64b808ecfc705e7c2dca6dacb64f90c67932329eb08f81175f2a2.
