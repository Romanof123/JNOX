# Independent strict MLQ first-service verification / 엄격 MLQ 최초 서비스 독립 검증

**PASS for the conditional AArch64 virt contract / AArch64 virt 조건부 계약 통과.** Revision 0a10dde84166f285f99c7a0b35317636624a0a45. One Cortex-A72 TCG vCPU, GICv2, Rust 1.95.0. All four original boots ran serially once. Raw files and executables were hashed before reading implementation or earlier outcomes. The local clone omitted the unadvertised revision. The subsequent git archive of the exact object supplied the source for every build. No rejected acquisition or guest retry occurred.

고정 커밋의 소스 아카이브에서 최초 게스트 네 개를 순차 실행했다. 최초 저장소 복제에서 해당 커밋을 찾지 못해 정확한 커밋의 소스 아카이브를 사용했다. 구현을 열기 전에 원본 해시를 보존했고 게스트 재시도는 없었다.

| Accepted / 승인 | Plain | Trace |
|---|---:|---:|
| High owner 0 service | 38,896 | 23,458 |
| High owner 1 service | 5,937 | 5,584 |
| Equal owner 2 service at target dispatch | 4,033,151 | 4,023,841 |
| Gaps / 공백 | 21,848 | 32,108 |
| Equal excess / 동급 초과 | 33,151 | 23,841 |
| Target entry latency / 대상 진입 지연 | 9,044 | 6,989 |
| Aggregate allowance consumed / 총 여유 사용 | 64,043 | 62,938 |
| Elapsed / 경과 | 4,108,876 | 4,091,980 |
| Completed blocks / 완료 블록 | 8,822 | 8,827 |
| Odd-count product / 홀수 총계 | 18,067,456 | 18,077,696 |

At 62,500,000 counts/s the bound in counts is 8M + 8M + min(8M,4M) + 4M = 24M. Both equality cases completed, and both B−1 cases parked before worker dispatch with state 2 and words 6–29 zero, empty exception logs and the expected UART. The trace refusal journal was entirely zero. The 4096 independently enumerated samples yield a sum of 34,816, squares of 382,976, 2,048 odds per block and a maximum of 16. Every 240-byte witness was decoded with little-endian u64s. The raw 30-word arrays accompany the verifier.

승인 두 건 모두 작업 0, 1 종료 후 동급 작업 2에서 실제 IRQ가 발생했다. 작업 3이 진입 카운터를 읽고 최댓값 16을 전달했다. 작업 2는 저장된 PC로 재개했고 전달값을 읽은 다음 4096개 표본의 마지막 블록 전체를 처리했다. 거부 두 건은 작업 진입 전 상태 2이며 실행 필드가 모두 0이다.

Both exception streams show SVC, SVC, IRQ, SVC, SVC. The equal-worker interrupt ELR and its eventual ERET destination match in each build. The traced frame journal shows owner order 0,1,2,3,2, then idle, aligned saved frames in their own 16 KiB stacks, physical timer INTID 30, timer control 5, and SVC #1. All four functions have separate entry addresses. Traced interval differences reproduce owner charges through the terminal, including 4,026,825 for the equal peer. The target reads CNTPCT; disassembly places the peer receipt load before its 4096-byte loop and the receipt decision after the loop. The plain build exposes the aggregate witness and exception stream. Per-event timing and full frame images are available in the trace build. Independent raw checks, original checker accepts, and deliberately corrupted receipt, timing, worker, dispatch and refusal fields all passed their expected positive/negative outcomes. The host model suite covered FIFO positions, higher/lower classes, absent demand, min(Q,demand), overflow, equality and B−1.

레거시 MLQ/MLFQ 12개 부팅이 모두 통과했다. 기준 ZIP의 SHA-256을 확인하고 28개 구성의 ELF를 직접 재빌드했다. 16개 deadline 구성은 PT_LOAD가 동일하며 심볼·문자열 표만 달랐다. 12개 multilevel 구성의 .text 및 로드 배치는 동일했다. .rodata에서 소스 위치 레코드의 행 번호가 구성당 3–6바이트 달랐다. 전체 ELF 28개 모두 바이트 단위로 달랐다.

All 12 legacy multilevel boot selectors passed. For 28 freshly built legacy configurations the baseline ZIP hash was e6b226c7e8f8d430bbe0167b437acbb7626d771eb090f28e2f7f71971861f20e. All 16 deadline configurations match every PT_LOAD byte and layout, with non-loaded symbol/string table changes. Each of the 12 multilevel configurations keeps its executable text and segment layout. Every modified loaded byte was decoded as a Rust source line for src/multilevel_service.rs, 3–6 bytes per configuration. Some also differ in non-loaded tables. No whole ELF was identical. See legacy/comparison.json and review/location-diffs.json. Physical timing, application WCET and production qualification remain unmeasured / 실기 타이밍, 제품 WCET 및 제품 적격성은 이번 측정 범위 밖이다.

[First four corrected QEMU witnesses with real IRQ and post-resume reductions](strict-class-first-service-corrected.zip) · SHA-256 8a27b284676a2876cc174ad2c977d3275bdd91fb8b0add7b5161111d29e0a722.
