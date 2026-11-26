# Independent fused firsts / 결합 연산 독립 최초 실측

Frozen c5ab397e93d345a4fef4420fe9cf7fa3e4be97ac. Refuse admission for FIFO and RR. Every fused position has a measured phase lower bound above the inclusive 62,500-tick limit. QEMU virt, single Cortex-A72, wall-clock TCG, 62.5 MHz. No physical evidence or justified analytical bounds. Each tar contains terminal RAM, UART, manifest and kernel. All 28 exited 71/72/73. The four healthy controls used current legacy selection or the archived corrected controller (rollback) without fused-phase. All 24 fused boots joined 64 real PID1 SVC63 CHECK/COMPLETE pairs of qualifier and safety records each, including 32 COMPLETE timestamp brackets. Qualification and safety journals are authoritative for the recorded scope, fixed journals retain prefixes only.

FIFO와 RR의 24개 결합 실행은 모두 위상 응답 하한이 1 ms를 넘었다. 기존 방식 및 이전 제어기 복귀 표본 4개도 종료값 71/72/73을 기록했다. 고정 저널은 유한한 앞부분만 남긴다. 물리 장치나 분석적 최악 시간 상한은 검증하지 않았다.

|Position|Original / corrected phase upper|Fused phase lower–upper|Job / sender / COMPLETE gap upper|Grid IRQ upper|ENABLE 18 / 37 ms|All controller pre-wakes & finishes|
|---|---:|---:|---:|---:|---:|---:|
|fifo-boundary-witness|123340 / 119713|101636–101730|244738 / 256257 / 179537|35463|1 / 1|1|
|fifo-collision|134297 / 142525|129928–130000|201976 / 217857 / 201199|23177|1 / 1|1|
|fifo-delayed|125792 / 117276|114043–114116|222799 / 122505 / 205845|20287|0 / 0|1|
|fifo-healthy-irq|67833 / 67349|67487–67523|223773 / 149018 / 174140|13296|1 / 1|1|
|fifo-healthy|116640 / 130275|122984–123068|188303 / 217416 / 216396|23159|1 / 1|1|
|fifo-invalid|119437 / 126071|116701–116776|198630 / 212882 / 209449|30243|0 / 1|1|
|fifo-live|120730 / 119472|115649–115722|252561 / 260979 / 174622|26551|0 / 0|1|
|fifo-lower-work-irq|66438 / 67240|65679–65712|221962 / 130604 / 181002|23788|1 / 1|1|
|fifo-lower-work|131172 / 102891|92690–92783|241503 / 252768 / 180770|25821|0 / 1|1|
|fifo-overrun|116746 / 125817|135989–136087|187531 / 234192 / 216077|25181|1 / 1|1|
|fifo-saturation|132348 / 135696|135701–135777|189472 / 218995 / 225705|23898|1 / 1|1|
|fifo-silence|89471 / 125870|120523–120594|183173 / 211613 / 221877|21971|1 / 1|1|
|rr-boundary-witness|120123 / 124275|146522–146610|194825 / 277012 / 206279|84945|1 / 1|1|
|rr-collision|120685 / 121175|134868–134940|195516 / 223872 / 207475|25720|1 / 1|1|
|rr-delayed|127708 / 99456|142906–142980|215174 / 148898 / 201810|29749|0 / 0|1|
|rr-healthy-irq|68141 / 69687|70165–70206|223854 / 141752 / 175936|11683|1 / 1|1|
|rr-healthy|116964 / 93341|116629–116699|247729 / 257445 / 172140|21660|1 / 1|1|
|rr-invalid|96455 / 105812|131188–131259|190315 / 219572 / 199141|27007|1 / 1|1|
|rr-live|99733 / 135712|74501–74568|229859 / 142367 / 169133|17802|0 / 0|1|
|rr-lower-work-irq|78627 / 78684|85555–85605|219877 / 154668 / 177310|11773|1 / 1|1|
|rr-lower-work|119627 / 114296|106989–107054|244601 / 252635 / 173382|22171|1 / 1|1|
|rr-overrun|114470 / 136558|120890–120963|189967 / 208053 / 223995|24880|1 / 1|1|
|rr-saturation|127922 / 139368|107376–107439|246104 / 254203 / 171880|19526|1 / 1|1|
|rr-silence|121952 / 142165|105697–105760|247247 / 256580 / 169718|20796|1 / 1|1|

All entries are raw ticks. Compare phase ≤62500, job ≤312500, sender ≤625000, COMPLETE gap ≤312500, grid IRQ ≤31250, and require finish strictly before the next phase. A bracket spanning an exact boundary is inconclusive. Job = final B response plus 156250. See observations.json for each blocked-to-ready transition, actual continuation, phase finish endpoints, missing records, fault-to-detect/safe, private rows, boundary clones, rank and publication joins. A post-wake selection by itself does not prove pre-wake service. A qualifying hardware bound remains unavailable.

trace-audit.json includes each actual SVC63 entry-to-decision duration (excludes full architectural return), syscall bodies within the 128-row prefix, prefix losses and per-archive SHA-256. Legacy and rollback have zero SVC63 rows. Candidate source restricts SVC63 to the qualification build and PID1 commands 1/3. Negative role/command calls were source-reviewed only. Safety ABI 60/61 remains visible in healthy legacy controls. No guest position was retried. Earlier shell line-ending and uninitialized Git-archive package setup errors preceded all boots. The live sources were rebuilt from a detached Git checkout.

수치는 가상 카운터 틱이다. 상한과 하한 사이에 판정 기준이 걸리면 결과는 미확정이다. 세부 상태·발행·재개·출력 전이는 observations.json, 시스템콜 표본과 해시는 trace-audit.json에 보존했다.

## Comparable work and availability / 유효 작업과 가용성

Each cell gives original / corrected / fused accepted COMMIT count and virtual ENABLE samples at 18 and 37 ms. Per-run counts of actual receives, private rejections, rearm outcomes, fault intervals and joins are in useful-work.json. Changes in the work or device outcome preclude attributing raw duration changes solely to fusion.

|Position|Accepted COMMIT|ENABLE 18,37|
|---|---|---|
|fifo-boundary-witness|3 / 3 / 3|1,1 / 1,1 / 1,1|
|fifo-collision|3 / 3 / 3|1,1 / 1,1 / 1,1|
|fifo-delayed|4 / 4 / 4|0,0 / 0,0 / 0,0|
|fifo-healthy|8 / 8 / 8|1,1 / 1,1 / 1,1|
|fifo-healthy-irq|8 / 8 / 8|1,1 / 1,1 / 1,1|
|fifo-invalid|3 / 3 / 3|1,1 / 1,1 / 0,1|
|fifo-live|8 / 8 / 8|0,0 / 0,0 / 0,0|
|fifo-lower-work|8 / 8 / 8|1,1 / 1,1 / 0,1|
|fifo-lower-work-irq|8 / 8 / 8|1,1 / 1,1 / 1,1|
|fifo-overrun|8 / 8 / 8|1,1 / 1,1 / 1,1|
|fifo-saturation|3 / 3 / 3|1,1 / 1,1 / 1,1|
|fifo-silence|3 / 3 / 3|1,1 / 1,1 / 1,1|
|rr-boundary-witness|3 / 3 / 3|1,1 / 1,1 / 1,1|
|rr-collision|3 / 3 / 3|1,1 / 1,1 / 1,1|
|rr-delayed|4 / 4 / 4|0,0 / 0,0 / 0,0|
|rr-healthy|8 / 8 / 8|1,1 / 1,1 / 1,1|
|rr-healthy-irq|8 / 8 / 8|1,1 / 1,1 / 1,1|
|rr-invalid|3 / 3 / 3|1,1 / 1,1 / 1,1|
|rr-live|8 / 8 / 8|0,0 / 0,0 / 0,0|
|rr-lower-work|8 / 8 / 8|1,1 / 1,1 / 1,1|
|rr-lower-work-irq|8 / 8 / 8|1,1 / 1,1 / 1,1|
|rr-overrun|8 / 8 / 8|1,1 / 1,1 / 1,1|
|rr-saturation|3 / 3 / 3|1,1 / 1,1 / 1,1|
|rr-silence|3 / 3 / 3|1,1 / 1,1 / 1,1|

All 24 observed controller phases sets had complete pre-wake waits, ready and dispatch witnesses and bounded finishes. The largest job, sender and COMPLETE-gap upper endpoints were 252561, 277012 and 225705 ticks. Grid IRQ arrival-to-return exceeded 31250 ticks in FIFO and RR boundary-witness (35463, 84945). Each optional interference position recorded 26 completed IRQ27s. The boundary private clones recorded the one-tick-before and exact-expiry cases separately from the real 2.5-ms dispatch. Live first-pair spacings of 523142/563428 ticks missed the exact 625000-tick target. Both remained inhibited at the two sampled instants. FIFO invalid and FIFO lower-work remained inhibited at 18 ms and were enabled at 37 ms.

Across all 24 fused boots, maximum syscall entry-to-decision samples were 65822 ticks for CHECK and 4725 for COMPLETE. Real healthy fixed prefixes show empty 6→4 calls and accepted 9→7. All measured full phase upper endpoints remained below the 156250-tick next release, yet all positions exceeded the independent 62500-tick limit. Qualifier and safety rows had zero drops. Fixed drops are reported independently.

실제 수신·반영 횟수와 두 시점의 출력 상태를 실행별로 대조했다. FIFO invalid 및 lower-work는 18 ms에 억제, 37 ms에 활성 상태였다. FIFO·RR 경계 사례에서 가상 타이머 인터럽트의 compare→복귀 시간이 500 µs를 넘었다.

[Raw captures and offline replay](native-safety-fused-first.tar.gz) (SHA-256 `a86b87964850374a00b2b07818d6f5f1fe4d43f286d4b482e50a88245f181233`).
