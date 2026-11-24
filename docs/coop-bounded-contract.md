# FIFO deadline admission / FIFO 마감 수용 (#250)

Optional fixed profile `coop-bounded`, one owner, one CPU, three FIFO slots, two linked handlers. Each handler has a positive configured counter cost. Every request carries an absolute inclusive completion deadline. From a monotonic nonwrapping counter sample, account first for the running handler's reserved finish minus now, then every queued cost in FIFO order, then the candidate. Check every earlier deadline again. Reject invalid ID, full queue, zero cost, arithmetic overflow, regressed time, expired running reservation or infeasible prefix without changing queue, peak or reservations. Equality succeeds. A returned overrun invalidates further admission for the burst. Existing accepted work drains. Empty service starts a fresh interval. Conditional feasibility assumes consecutive service and that configured costs cover all elapsed ticks including dispatch gaps. Time lost after a decision can invalidate it. No physical WCET or product limit is established.

선택 구성은 단일 CPU와 소유자, 슬롯 세 개, 두 핸들러를 사용한다. 감소·순환하지 않는 카운터의 현재값에 실행 중인 예약의 잔여 시간, 대기열의 순서별 비용, 후보 비용을 더한다. 기존 마감도 다시 확인하고 마감과 같으면 허용한다. 잘못된 ID, 용량 초과, 0 비용, 산술 초과, 시간 역행, 실행 예약 초과 또는 마감 실패 시 상태를 보존한다. 실행 초과 뒤에는 대기 작업을 비우고 다음 구간을 시작한다. 구성 비용에는 디스패치 간격도 포함되어야 하며 이후 지연은 판정을 무효화할 수 있다. 실물 WCET와 제품 승인은 별도다.

First run: the owner observes a ready equal-priority peer held until voluntary yield, both peers return before the bounded burst. Enqueue sample blocks 1 and 2. With one free slot, reject block 9 with an immediate deadline. Block 1 submits nested block 3 after a deliberately infeasible nested request. FIFO order 1,2,3. Each block reduces four input readings into a sum and weighted sum, combined across all blocks. Capture request times, deadlines, decisions, queue depth/demand, raw start/finish and useful results separately from host recomputation. Preserve coop-events and coop-single controls. Model controls cover exact equality, active residual, stale time, zero/overflow, full/invalid and recovery after overrun.

첫 실행은 자발적 양보 후 동순위 작업 두 개가 실행되고, 블록 1·2가 수용된 뒤 빈 슬롯에서 블록 9가 마감으로 거부되어야 한다. 블록 1 내부에서도 촉박한 후보를 거부한 뒤 블록 3을 수용한다. 1·2·3 순서로 네 개씩의 측정값 합과 가중합을 구한다. 원시 시각, 거부 전후 깊이·비용과 결과를 별도 판독하여 비교한다. 기존 두 구성과 경계·오류 사례를 함께 확인한다.

| Constant / 상수 | First-run value / 첫 실행 값 |
|---|---|
| Slots, registrations / 슬롯·등록 | 3, 2 |
| Cost in counter ticks / 구성 비용 | 12,500,000 (producer), 6,250,000 (reduce) |
| Accepted deadlines / 수용 마감 | submission sample + 50,000,000 |
| Refusals / 거부 | outer 9 at now, nested 8 at now + 12,500,000 |
| Blocks / 데이터 | [3,1,4,1], [5,9,2,6], [5,3,5,8] |
| Reductions / 계산 | sum and sum of (1-based position × reading), per block, then totals |
| Guest journal / 관측 | 79 unsigned 64-bit words, little-endian, ELF symbol coop_bounded_observations |

Use one monotonic counter source for admission and dispatch. After the owner yields, the ready peer stores 2, the delayed peer multiplies that by 10, and both return. Capture header must show 0 before yield and 20 after. Runtime row deadlines and samples use that same guest counter. 서명 없는 카운터의 동일 구간에서 판정하며 양보 전 0, 두 작업 복귀 후 20을 확인한다.
