# Independent verification of JNOX #214 / JNOX #214 독립 검증

PASS for the bounded reference workload on QEMU virt. Candidate e015d25b2ead31b43367922e94f0db5b4e650c82, prospective contract 23372b3dc13248a0b39da802a290bf575860f581, baseline ca0b3dd3f87f0f459449271d957af1eb8e77d2cd. Issue #211 under #21. Independent cases were recorded before opening the candidate source or reading launch outcomes. plan.txt SHA-256 d998fbad425e2734137dcd0dc75f98c723b26a8abd869193681765a9ac58b672.

## First launches / 최초 부팅

Rust 1.95.0 locked AArch64 release, one build job, WSL ext4, QEMU 6.2.0, virt/GICv2, one Cortex-A72 TCG CPU, 128 MiB, default architectural counter (62,500,000 Hz). run.sh records exact build and launch commands. All six runs reached their 10 s host timeout. Their first ELF, UART, exception log and hashes were preserved before the next launch. No retries.

| Position | Mode | Policy | Observed job IDs |
|---|---|---|---|
|1|ordinary|SJF|2,1,0|
|2|ordinary|SRTF|2,1,0|
|3|ordinary|HRRN|2,1,0|
|4|journal|SJF|2,1,0|
|5|journal|SRTF|2,1,0|
|6|journal|HRRN|0,2,1|

Each callback ran once per image with positive counter duration. Job 0 produced count 3, min 0, max 65535, sum 65542. Job 1 counted 3 samples at least 42. Job 2 returned error 4 with no provisional fields. Each exception stream contains four submission and four completion SVCs. The dispatcher never received unknown request 4 or overlength request 3. After three accepted submissions the normal class-0 repeat reached the dispatcher with four live slots and returned capacity status 2.

Three journals each captured nine decisions, generations 1–4, four completions and a dispatch into the linked idle routine on its idle stack. check.py recomputes every selected owner, selected frame, saved and return PC, remaining estimate and complete dispatch-to-decision interval. It checks frame addresses against the linked stack allocations and every return PC against QEMU; QEMU's exception log exposes PCs, while stack/frame identity is derived from the journal and linked symbols. replay.json contains exact rational HRRN metrics, rank outcomes, worker boundaries and cumulative charges. Different HRRN orders across independent boots are compatible with arrival and selection time. None of the three reference journals contained an equal best rank. The supplemental unmodified ordinary workload exercised equal-estimate waiting peers, incumbent retention, generation reuse and stale rejection through real SVCs. All three supplemental first journals passed the upstream EVD-037 UART replay.

Three pairs of legacy ordinary ELF PT_LOAD segment payloads and runtime layout fields matched base/candidate. The native scheduling suite passed 134/134, including cross-product HRRN boundary and exhaustive small calendars. Two invalid selector combinations failed at build time. The unchanged parser extracted from the candidate passed all 65,536 u16 values with leading zeroes, 12 malformed inputs, empty input, and a 32-byte maximum-count frame. Source uses checked u16 multiply/add and rejects missing LF. The 32-byte maximum bounds aggregate count by 16 and sum by 1,048,560, so u64 aggregate overflow is unreachable. Parser edge cases are host evidence. The three specified worker frames are guest evidence. No hardware, WCET, deployed capacity or product acceptance claim is made.

## 한국어 판정

지정된 QEMU 참조 구성에서 통과했다. 일반 세 정책과 저널 세 정책을 표 순서대로 최초 부팅했다. 매 부팅에서 실제 작업 세 개가 한 번씩 완료됐고, 작업 전후의 카운터가 증가했다. 요약은 3개·최소 0·최대 65535·합계 65542, 임계값 이상은 3개, 잘못된 입력은 오류 4였다. 미등록 요청과 17바이트 검증 요청은 SVC 전에 거절됐다. 네 슬롯을 채운 뒤 재요청한 정상 작업은 커널에서 용량 상태 2를 받았다.

저널 세 건에서 매 선택과 세대 1–4, 완료 네 건, 전체 실행 구간 과금과 유휴 전환을 재구성했다. 예외 로그의 복귀 PC와 ELF 심볼의 프레임 위치를 대조했다. HRRN 일반 부팅과 저널 부팅은 서로 다른 순서를 냈으며 저널의 실제 시각에서 계산한 유리수 순위가 선택 결과와 일치했다. 참조 작업에는 1위 동률이 없었다. 별도로 기존 배치 앱을 세 정책으로 최초 부팅해 동등 추정치 작업, 실행자 유지, 슬롯 재사용 및 오래된 세대 거절을 확인했다.

기존 일반 이미지 세 쌍의 적재 내용과 배치가 기준 커밋과 일치했다. 호스트 스케줄링 시험 134개 및 입력 파서 경계 시험을 통과했다. 지원하지 않는 선택자 두 구성이 빌드 단계에서 거절됐다. 실보드 시간 상한 및 배포 용량 승인은 이 결과의 적용 범위 밖이다.

[Independent first boots and replay / 독립 최초 부팅과 검증 자료](admitted-telemetry-batch-first.zip) · SHA-256 d2dafdf3a6629005857bf24ba352d4731c2532134e087e6c6c37102226ac8274.
