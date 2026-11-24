# J-NOX #54 / PR #200 independent first cohort

Candidate 6ed6afbaf282021a0fcc051796a8ddc5cd309a82. Prospective parent a7f89f2f82011af8feaac70663749f21d956632e. Windows rustc 1.95.0 built with --release --locked -j1. Each of five single-CPU Cortex-A72 virt/GICv2 TCG images ran once, in the order below, with 128 MiB and the default 62.5 MHz clock. Every guest produced its terminal record and its ext4 originals were copied and reopened by SHA-256 on D before the next launch.

|First guest|A/B/C grants|A/B/C whole units|IRQ/SVC/ERET|
|---|---|---|---|
|lottery1|10/8/14|10/8/14|33/0/33|
|lottery2|5/11/16|5/11/16|33/0/33|
|stride|6/6/4|6/12/8|11/6/17|
|lottery-miss|0/1/0|0/2/0|2/0/2|
|stride-delay|1/0/0|3/0/0|2/0/2|

The ordinary stride sequences were ABABABAB then ABCCABCC. C joined the incumbent floor after grant eight. The negative lottery control retained the first predicate at line 446, mode 3, slot 0, actual 2, expected 1. Each other guest reported five zeros. Replay uses the archived independent scorer to regenerate every lottery word, reconstruct normalized passes, private stack count/checksum/stamps, continuation and ordered external exception PCs. The host controls cover ticket boundaries, rejection, inactive/empty eligibility, eight 4,096-draw windows and five invalid transcript mutations.

With weight-one A in position five the sampled start was 2,595,086, the two-unit compare 7,595,086, the release threshold and last published stamp 10,095,086, and vector stop 10,099,228. Dividing 7,504,142 by 2,500,000 gives three whole units and 4,142 remaining counts. Pass increased by 3 × 720,720 to 2,162,160. A committed 32,291,840 iterations and checksum 13,818,159,716,955,899,905. B/C retained their initial tuples. The linked worker sets DAIF.I at 0x40202304, computes and publishes the tuple, compares the acquired threshold and clears DAIF.I at 0x40202398. The external idle return/IRQ ELR were 0x4020219c/0x402021a0, and A's were 0x402022f8/0x4020239c. Both IARs identify PPI30 with timer control 5. The guest reads back CNTP_CVAL_EL0 and checks equality after programming it. UART carries the target and the successful check is inferred from reaching the footer. These observations measure controlled guest IRQ deferral on QEMU. The historical original diagnostic retains its failure at 5,011,037 counts, two units plus 11,037, pass 1,441,440, source line 425 and Boolean actual/expected 0/1.

Only kernel/src/ordinary_proportional.rs changed. Linked worker A alone contains DAIF.I set/clear in the delayed build. All three worker bodies are byte-identical to the prospective parent in the other four images. Whole binaries, PT_LOAD contents and .text differ in all five ordinary images. Legacy proportional-bench is byte-identical. Five invalid Cargo selector combinations rejected as expected. The unchanged scheduling suite passed 134/134. No hardware or instruction-level service inference.

## 한국어

사전 순서대로 최초 게스트 다섯 개를 한 번씩 실행하고 다음 부팅 전에 원본을 D 드라이브에서 해시로 재검증했다. 표의 서비스는 실제 벡터 중단 시각과 시작 시각 차이에서 구한 정수 단위다. 시드별 모든 추첨과 복귀 PC, 전용 스택 진행량을 재생했다. 일반 스트라이드는 A/B 네 차례 뒤 C가 기존 최소 패스로 합류해 ABCC를 두 차례 실행했다. 복권 음성 대조는 446행에서 슬롯 0의 실측 2와 기대 1을 보존했다.

지연 구성 A는 두 단위 비교값을 유지한 채 세 단위 경계에서 실제 작업 표본을 발행하고 IRQ를 허용했다. 벡터 진입까지 7,504,142카운트가 지나 세 단위와 나머지 4,142를 관측했고 패스는 2,162,160 증가했다. B/C의 표본은 유지되었고 외부 IRQ의 ELR은 마스크 해제 바로 다음 명령을 가리켰다. 과거 최초 실패는 두 단위와 나머지 11,037, 패스 1,441,440으로 별도 보존한다. 다른 네 구성의 작업 함수 바이트는 부모와 일치하지만 전체 일반 ELF의 .text는 달라졌다. 레거시 ELF는 완전히 일치했다. 호스트 정책 시험 134개와 선택자 음성 대조 5개를 확인했다. 측정 범위는 이 QEMU 구성의 제어된 IRQ 지연이다.

## Offline replay

On Linux with Python 3 and pyelftools installed, run python3 replay.py and python3 workers.py from the extracted directory. The replay verifies the native capture digests and regenerates results.json. The build scripts record the isolated worktree/target layout used for this run. Retained libraries and source blobs can be reconstructed from the two public Git commits.

[Raw five-position corpus, command records and independent replay / 최초 5개 원본·명령·독립 재생](ordinary-stride-late-first.zip) · SHA-256 683d998ddcd981f01c2a4e01a31e500ccae683863933891e9a6283208f9c3b96.
