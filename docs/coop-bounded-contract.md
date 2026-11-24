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

### Prospective correction after the first independent replay (#253)

A burst starts at its first accepted request and ends when the last callback returns with an empty queue. An empty turn and a refused first request establish no epoch. During a burst every post samples the same counter, including requests refused for an invalid ID, full queue, zero cost, deadline, overflow or interval. Keep the greatest observed sample including callback entry and return. A refusal changes no FIFO slot, depth, cost, peak or reservation. The observation watermark may advance. Equal timestamps are valid, including zero in a new epoch. A lower post sample (including wrap to zero) fails temporal validation and leaves the watermark in place. A later sample at or above the watermark may pass again if the burst remains valid.

Preserve decision precedence: invalid ID, full queue, zero cost, invalid interval, then the first FIFO-prefix failure (checked addition overflow before its deadline comparison). Structural refusals still observe the timestamp. A regressed callback boundary, unrepresentable or already infeasible running reservation, or callback return past its reservation invalidates admissions through drain. An active post past the reservation advances the watermark and fails interval validation; lower retries cannot erase the observation. The two ordinary selectors and bounded journal layout retain their original contracts.

구간은 빈 상태의 첫 수용으로 시작하고 마지막 콜백이 반환하여 큐가 비면 끝난다. 빈 큐에서의 거부나 빈 디스패치는 구간을 열지 않는다. 활성 구간에서는 모든 게시(잘못된 ID, 용량·비용·마감·산술·시간 오류 포함)와 콜백 시작·반환 시각의 최댓값을 보관한다. 거부 시 FIFO, 깊이, 비용, 최대 깊이, 실행 예약은 유지되지만 관측 최댓값은 증가할 수 있다. 같은 시각은 유효하다. 최댓값보다 낮은 게시 시각(0으로 순환 포함)은 시간 검사에서 거부한다. 유효한 구간에서는 최댓값 이상으로 재시도할 수 있으며, 새 구간은 0에서도 시작할 수 있다.

사유 우선순위는 잘못된 ID, 큐 포화, 0 비용, 시간 구간 오류, FIFO 순서의 첫 산술 초과 또는 마감 위반이다. 구조적 거부도 시각을 관측한다. 콜백 경계에서의 역행, 실행 예약의 산술 초과나 마감 위반, 예약을 넘긴 반환은 큐가 빌 때까지 수용을 막는다. 실행 예약이 지난 뒤 게시하면 최댓값을 갱신하고 시간 오류로 거부하므로 더 이른 시각으로 이를 되돌릴 수 없다. 기존 일반 구성 두 개와 관측 ABI는 그대로다.

[EVD-097 English](evidence/evd-097.en.md) · [한국어](evidence/evd-097.md).
