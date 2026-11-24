# Independent first cohort / 독립 최초 코호트

PR #207, f959b0febd0cb7a17c87d208a96f6c8edd53bbfd. Prospective design / 사전 설계 26cf76e29e407522a1aad80bec2b452564644b81. Primary #49, parent #21, POL-014.

| Original criterion / 원래 기준 | PI plain, traced IRQ chunks | Ceiling plain, traced IRQ chunks | Qualified / 적격 |
|---|---|---|---|
| Normal / 기본 | 2, 36 | 33, 78 | 4/4 |
| Rust prepared | 0, 40 | 77, 64 | 4/4 |
| Rust stages | 10, 0 | 19, 22 | 4/4 |
| Selected return | 42, 0 (reason 16) | 0, 0 | 3/4 |

Normal PI transferred X to HIGH after HIGH blocked and donated 9; LOW resumed ahead of MEDIUM, committed, then returned to base 2. Normal ceiling retained LOW at IRQ on the priority-9 tie, yielded at 128 chunks, transferred Y to HIGH after contention, kept priority 5 with X and restored 2 after X release. All qualified positions published version 1, count 65,536, sum 557,056, all 256 cumulative chunks, MEDIUM 32,896 and three exits. Ceiling audit read (1,256). Independent FIFO tie replay checked every selected task. Linked images each have one full vector call inside choose and a distinct pretransfer call inside release. The change in mutex_policy.rs only exposes priorities within the crate. The rank-before-transfer semantics and waiter ordering are covered by the native suite, including chains and withdrawal.

기본 PI에서는 HIGH의 차단으로 LOW가 9를 상속해 MEDIUM보다 먼저 재개한 뒤 X를 넘기며 2로 복구했다. 기본 천장은 IRQ 동률에서 LOW를 유지하고 128묶음에서 양보했다. Y를 넘긴 후 X를 보유하는 동안 5를 유지했다. 적격 15회는 실제 보호 데이터, 감사값, 모든 작업 종료 및 복귀를 재생했다. 각 바이너리에서 결정용 벡터 호출과 이전 전 대기자 순위용 호출을 분리해 확인했다.

All 16 timers programmed exactly 6,250 ticks (100 µs at 62.5 MHz), one PPI30, arm CTL 1, interrupt CTL 5 and unchanged CVAL. Default compare-to-IRQ-entry ticks in frozen order were 9038, 6482, 6534 and 7087. All four stage records captured ordered write/readback/restore-entry/R markers. Selected-return ceiling plain reported compare already elapsed at the final pre-ERET sample and subsequently interrupted actual work. Position 14 selected-return PI traced still had 588 ticks left at its final pre-ERET sample; its IRQ entered 6,324 ticks after compare, interrupted LOW's first stack adjustment before the work flag, then entered 0x40201280. HIGH was selected but had no ERET, blocking operation, committed data or exit. Zero chunks elsewhere occurred during real indexed computation, result storage or first chunk bookkeeping. Frame bounds, saved caller PCs, selected restores and all eight external vector/return streams were replayed. Plain executions have guest-only return evidence. Exact K intervals and residency brackets appear in replay.log.

16회 모두 62.5 MHz에서 정확히 6,250틱을 설정했고 PPI30 및 CVAL을 확인했다. 14번의 마지막 ERET 전 표본에서는 비교까지 588틱이 남았지만 IRQ는 비교 후 6,324틱에 작업 함수 진입 명령을 중단했다. HIGH 선택 이후 실제 복귀와 보호 작업은 없었다. 반면 selected-return 천장 일반 실행은 ERET 전 이미 비교를 지났으나 실제 연산 중 IRQ를 받았다. 외부 추적 8회에서 벡터·ELR·ERET를 대조했다. 일반 실행의 복귀 증거는 게스트 기록이다.

16 cold launches on Linux ext4, original 15-second allowance, all stopped after footer. Serial builds, no warmups or retries. Per-position host PID, status, argv, elapsed and sealed hashes accompany original UART/stderr/trace. Rust policy/queue suite 134/134, decoder controls 5/5. Historical cohorts remain original 3/4 and optional 3/4, 2/4, 4/4. The new finite samples support default ordinary semantics and retain an optional failure. They establish no latency improvement, physical blocking maximum, production lock-set suitability or target interference bound.

기존 기본 3/4와 선택형 3/4·2/4·4/4는 별도 분모다. 이번 기본 실행은 4/4였고 선택형 복귀 지점 한 번이 부적격이었다. 최초 실행만 보존했고 실제 보드의 최대 블로킹·간섭·시간 개선은 미확인이다.

[Independent matrix and first raw captures / 독립 판정표와 최초 원자료](mutex-priority-snapshot-first.zip) · SHA-256 e30741160f2750c1cfabbf29f7048e665a27b763f32e5e13ec79cce79d1b7918.
