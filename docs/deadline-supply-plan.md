# POL-005 finite interrupted CPU supply / 유한 CPU 공급과 중단 (#248)

Prospective criteria on 8609aab. One Cortex-A72 QEMU virt/GICv2/vCPU, original 16-slot horizon, 100 ms grid S+kQ. Reserve [1,2) for one independent service. All other whole slots are available. EDF checks every release/deadline interval against its available slots. Both policies simulate the finite table, account outgoing work up to the boundary, suspend the incumbent at the head of the queue, append releases in declaration order, and make no worker decision or charge in the withheld slot. At the next available boundary rank by deadline or sampled signed laxity, stable queue order on ties. No shifting the epoch or later deadlines.

| Profile | (id, release, deadline, cost, work duration ms) | Expected |
|---|---|---|
| ordinary-edf-supply / ordinary-llf-supply | (A,0,5,2,250), (B,1,7,2,30) | model A,service,A,B,B; both finish by 5. Actual A progress before interruption, independent stack runs sum of squares 1..64 = 89440 then waits to boundary; B releases at 1. Restore A saved frame/PC at 2, observe progress after resume, A and B return exactly once before absolute deadlines. |
| ordinary-edf-supply-refuse / ordinary-llf-supply-refuse | (A,0,2,2,250) | passes with full supply, one available slot by 2. EDF Demand (8), LLF Miss (9), park before worker initialization/dispatch. |

Freeze independent first cold launches in order EDF refused, LLF refused, EDF normal, LLF normal; retain failed attempts and raw UART separately. Then repeat original ordinary-edf, ordinary-llf, ordinary-llf-overload and ordinary-edf-overrun using EVD-048 criteria and original launch records. Enable deadline trace for detailed replay; retain ELF, command, hashes, clock frequency, brackets and frames. A service slot is a logical reservation; report actual restore-to-entry counts separately from scheduler gaps. Finite TCG observations and model costs do not establish hardware or product WCET.

기존 16슬롯 절대 그리드에서 [1,2)만 서비스에 예약한다. EDF는 릴리스·마감 구간별 가용 공급을 검사하고 두 정책 모두 실제 선언 순서와 추정 잔량으로 재생한다. 중단 시 기존 실행자를 큐 맨 앞에 보존하고 신규 도착을 뒤에 추가한다. 예약 슬롯에서는 작업을 선택하거나 비용을 부과하지 않는다. 다음 경계에서 EDF 마감 또는 LLF 여유가 같은 작업은 기존 순서를 따른다. 위 표의 두 정상 실행에서 A의 진행, 별도 스택 서비스 합계 89440, A의 동일 프레임·PC 복귀와 이후 진행, A/B의 정상 종료와 절대 마감을 검증한다. 두 거부 실행에서는 작업 시작 전에 공급 부족을 판정한다. 첫 실행 순서·실패·원시 기록을 보존하고 기존 네 프로필도 원래 기준으로 검증한다. 실제 카운터 서비스 시간은 논리 예약 및 스케줄러 공백과 구분하며 제품 WCET는 별도 검증한다.

[Ten pre-change deadline and cooperative build identities](evidence/deadline-cooperative-legacy-loads.en-ko.md).

[Independent first assessment / 독립 최초 결과](evidence/deadline-supply-first.en-ko.md) · [Corrective action / 시정 이슈 #254](https://github.com/Romanof123/JNOX/issues/254).

[EVD-095 English](evidence/evd-095.en.md) · [한국어](evidence/evd-095.md).
