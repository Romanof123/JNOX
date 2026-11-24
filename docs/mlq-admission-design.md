# Conditional strict MLQ first service (#259)

On one virt CPU, a closed ready set joins in declaration order. Classes 2, 1 and 0 run linked bounded sample reductions. Class 0 also has a FIFO peer ahead of the target. No wakes, yields, sleeps or further arrivals. Plan to compare identical applications at deadlines B and B-1 raw CNTPCT counts.

For each higher owner add its finite cumulative charged service budget through return (including exception entry and timer lateness). For each equal peer ahead add min(quantum, service budget), charging any excess beyond its first quantum separately against a single whole-window allowance. Include one finite aggregate allowance for epoch-to-first-start, all inter-owner gaps, equal-peer quantum excess, and target dispatch-to-entry. The target must reach its entry counter on its first dispatch. The checked sum B admits equality. Missing required demand, invalid input or arithmetic failure refuses before release. Service bounds are supplied platform assumptions, not inferred WCETs. Capture raw epoch before selection, actual worker entry, per-owner charges, final target dispatch start and products. Violated premises must signal failure.

# 엄격 MLQ 최초 서비스 조건부 승인 (#259)

virt CPU 한 개에서 선언 순서로 준비 큐에 넣는다. 클래스 2, 1, 0 작업은 정해진 데이터 축약을 수행하며 대상 앞에 동급 작업도 둔다. 추가 도착이나 깨우기는 없다. 동일한 작업을 카운터 단위 기한 B 및 B-1로 비교한다.

상위 작업의 종료까지 누적 서비스와 앞선 동급 작업의 최소값(퀀텀, 서비스 상한)을 더한다. 최초 선택 전부터 작업 진입까지의 디스패치 공백, 동급 퀀텀 초과와 대상 진입 지연에는 구간 전체에 대한 별도 상한을 적용한다. 산술 오류나 필요한 상한 누락은 실행 전에 거부한다. 실제 진입 및 서비스 측정으로 전제를 검증하며 측정은 최악 실행 시간 증명이 아니다.

## Frozen oracle / 고정 판정 기준

IDs 0..3 join in order, with classes [2,1,0,0]. Target ID 3. ID 0 and ID 1 each have 8,000,000 raw counts of total charged demand through exit. ID 2 has 8,000,000 counts. Q=4,000,000, aggregate allowance=4,000,000, B=24,000,000. mlq-admit deadline B, mlq-refuse deadline 23,999,999. These are exact CNTPCT ticks independent of CNTFRQ. Data: 4096 unsigned samples, x[i]=1+i mod 16. IDs reduce sum (34816), sum of squares (382976), odd count (2048), and maximum (16), respectively. All tasks return. Refusal precedes dispatch. Compare existing mlq, mlq-saturated and mlfq selectors.

mlq_admission_witness is an array of 24 little-endian u64 words. In order: status (0 reset, 1 admitted, 2 refused, 3 measured, 4 completed, 5 violation), frequency, Q, allowance, B, deadline, epoch, first target dispatch start, target worker counter, four cumulative service values at target dispatch, summed gaps before target dispatch, equal excess, target entry latency, combined allowance used, elapsed, four products, worker-entry bitmask, target dispatch count through first entry. Joined diagnostic event rows are the existing ordinary_multilevel_witness. UART acceptance is MLQ ADMIT / MLQ VERIFIED / MULTILEVEL DONE, refusal MLQ REFUSE. 전제 위반은 상태 5와 MLQ VIOLATION으로 기록한다.

## Prospective refinement / 실제 동급 퀀텀 만료

ID 2 processes successive complete blocks of the same 4096 samples, accumulating the total odd count. After each block it checks an absolute counter threshold set to its own first-entry counter plus Q+1. Because that entry is later than its dispatch start, the peer spans the first programmed deadline. A finite limit of 2,000,000 blocks guards termination. Reaching the cap without the threshold is a premise failure. Its exact product is 2048 times word 24 (completed block count). The table now has 27 words: word 25 counts genuine timer expiries of ID 2 before target dispatch, word 26 counts later ID 2 dispatches. Require exactly one such expiry, target dispatched immediately from it, peer product still unpublished at target first service, at least one subsequent peer dispatch and verified final peer product. Demand ceilings include final return, checked also at completion. Other numeric inputs and outputs retain their values. 동급 작업은 실제 샘플 블록을 처리하며 최초 퀀텀 만료 후 재개하여 홀수 개수 합계를 남긴다.
