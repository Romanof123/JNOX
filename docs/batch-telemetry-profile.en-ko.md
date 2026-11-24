# POL-003 · Bounded telemetry batch profile / 유한 텔레메트리 배치 구성

Systems owns this reference job mix under [#211](https://github.com/Romanof123/JNOX/issues/211) and [#21](https://github.com/Romanof123/JNOX/issues/21). The product owner selects the deployed workload and capacity in [#1](https://github.com/Romanof123/JNOX/issues/1). Trace REQ-002/005/023 → DES-003/012 → VER-002/005/023. The baseline is ca0b3dd3f87f0f459449271d957af1eb8e77d2cd. Keep the [ordinary batch contract](ordinary-batch.en.md) and the [existing EVD-037 cases](evidence/evd-037.en.md).

## Configuration and admission / 구성과 수용

Use Rust 1.95.0, locked AArch64 release, QEMU 6.2 virt/GICv2, one Cortex-A72 TCG CPU, 128 MiB and the default architectural counter. Select `JNOX_BOARD=virt JNOX_PROFILE=batch JNOX_BATCH_WORKLOAD=telemetry` with existing `JNOX_APP=batch-sjf`, `batch-srtf` or `batch-hrrn`. Unset the workload selector to retain the ordinary example. Invalid workload names and use outside batch must fail at build time.

Three typed application jobs consume bounded, immutable, in-memory ASCII frames. They use the [telemetry pilot's record grammar](telemetry-migration-pilot.en.md) with u16 samples, leading zeroes and mandatory LF. A malformed frame discards all provisional aggregates. The table declares the first submission order, maximum accepted frame length in bytes, ranking estimate in counter ticks, and semantic result before any implementation or launch.

| ID · job | Limit | Estimate | Exact frame (LF as `\n`) | Result |
|---|---:|---:|---|---|
| 0 · summarize | 32 | 30000 | `0\n0007\n65535\n` | count 3, min 0, max 65535, sum 65542 |
| 1 · count values ≥42 | 24 | 20000 | `18\n42\n43\n65535\n` | 3 qualifying values |
| 2 · validate | 16 | 10000 | `11\n65536\n` | invalid sample (4), no partial summary |

The coordinator has an estimate of one tick. It submits all three immediate jobs through the existing SVC. Its estimate keeps the SRTF incumbent at rank one and both other policies retain the current job. The coordinator plus three workers fill the existing four live slots. In this state a repeated valid class-0 request must return the scheduler's capacity error. Before the three admissions, attempt an unknown request and a registered class-2 frame of 17 bytes; reject both in the application before SVC. This bounded reference mix uses no heap, filenames, standard streams or Linux calls. Estimates rank work and do not assert maximum execution times. Each worker reports its actual counter samples around processing and only the fields of its result, or the input error. The optional existing journal allows independent reconstruction of each complete dispatch-to-decision charge, including overhead.

## Frozen acceptance / 사전 판정

1. Launch one fresh image per position, in order: SJF, SRTF, HRRN without the journal, then SJF, SRTF, HRRN with `JNOX_BATCH_TRACE=1`. Freeze revision, per-image hash, tool versions, exact QEMU arguments and host timeout (10 s); retain all first UARTs and exception traces. For a direct Linux launch use `timeout 10s qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -accel tcg -smp 1 -m 128M -nographic -nic none -monitor none -serial file:uart.log -d int -D irq.log -kernel "$elf"` in a fresh directory. A halted idle system can reach the host timeout.
2. Require exactly the three real results above, each once, positive worker counter intervals, the two application rejections and one full rejection. Check SJF and SRTF start validate, threshold, summarize. For HRRN replay the ready response ratios using the captured admission/selection ticks and the original tie rule. Detailed HRRN ranking is scored on the traced position. The ordinary position establishes the actual output and completion of the three callbacks. Retain unexpected orders or missing captures.
3. In traced positions reconstruct the coordinator and three accepted generations, the capacity refusal, four actual completions, final idle and empty state. Match selected frame/PC and SVC returns to the independent exception log. Verify charging across the complete trace and distinguish it from the worker-only elapsed ticks. Compare the three baseline ordinary loaded images and, if a legacy payload changes, exercise that affected existing workload against EVD-037. No clock manipulation or extra spinning is part of the workload.

시스템팀은 지정된 QEMU 환경의 참조 작업 종류·입력·용량을 정하고 제품팀은 실제 제품 구성과 최대 부하를 #1에서 승인한다. 참조 구성은 메모리에 준비된 LF 종단 u16 텔레메트리 레코드를 처리한다. 작업 0은 개수·최소·최대·합계, 작업 1은 42 이상 표본 수, 작업 2는 전체 프레임 형식을 판정한다. 오류가 나면 부분 결과는 폐기한다. 표의 길이는 바이트, 추정치는 작업 순위를 위한 카운터 틱이다. 실물 WCET 및 제품 실행 한도는 별도 근거가 필요하다.

기존 배치 프로필에서 위의 세 정책과 텔레메트리 선택자를 사용한다. 알 수 없는 요청과 16바이트 한도를 넘은 17바이트 검증 프레임은 제출 전에 거부한다. 제출자는 1틱 순위를 갖고 세 작업을 기존 SVC로 접수한다. 네 슬롯이 모두 사용 중일 때 재요청한 정상 작업은 실제 스케줄러 용량 오류를 받아야 한다. 각 작업은 처리 전후의 실제 카운터와 계산 또는 오류 결과를 출력한다. 세 정책 일반 실행과 같은 순서의 저널 실행 등 총 여섯 최초 부팅에서 표의 출력, 횟수와 시간 표본을 확인한다. 추적 시에는 수용·거절·실제 완료와 최종 유휴 복귀, 전체 과금 및 HRRN 선택을 검산한다. 기존 일반 이미지가 변경되면 해당 EVD-037 경로도 검증한다. 재시도·부분 자료·실보드 및 승인 상태는 각 범위로 기록한다.
[Independent first cohort / 독립 최초 실행](evidence/admitted-telemetry-batch-first.en-ko.md). All six normal/trace positions passed, the three legacy loaded images matched, and three additional legacy replays passed. 이 참조 작업 구성의 수용과 결과를 검증했으며 제품 용량과 WCET 승인은 #1·#21에서 추적한다.

2026-11-25 · Retrospective trace / 사후 추적 · [#211](https://github.com/Romanof123/JNOX/issues/211) · [#214](https://github.com/Romanof123/JNOX/pull/214) · [EVD-091 (EN)](evidence/evd-091.en.md) · [EVD-091 (한국어)](evidence/evd-091.md) · [#239](finite-scheduling-index.en-ko.md)
