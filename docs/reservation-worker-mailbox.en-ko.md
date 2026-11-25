# Per-worker reservation mailbox / 작업자별 예약 메일박스

Technical base a4d2f340020f7e8666d04a113040207805bbfc9a (#201). Original cohort position 1, aligned wake/plain, is guest slot zero. The actual epoch was 3,168,946 and the inclusive deadline was 3,231,446. Adjacent sample deltas below are physical counter ticks; 5 is the traced first boot of the identical aligned-wake ELF.

| Position | Epoch→entry | Entry→source end | Source end→before | Before→post | Post→armed | Armed→selection | Selection→pre-ERET | Pre-ERET→worker |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
|1|25410|6970|16956|3142|6856|3018|2260|3781|
|5|24540|6518|12044|2580|4695|1669|1973|2665|

Selection at 3,231,298 left 148 ticks. The recorded pre-ERET sample at 3,233,558 was already 2,112 late; the real worker marker at 3,237,339 was 5,893 late. Selection→pre-ERET includes receipt publication, IRQ epilogue, system/FP restore and the assembly recorder up to its ordered sample at 0x40201254. It does not distinguish individual costs. The actual ERET is later at 0x402012a4. Even removing the entire 2,260-tick interval would leave the recorded marker 3,633 ticks late if other intervals were fixed.

In the existing linked live validator 0x40202c30–0x40202c98 five separate static array addresses are formed and indexed to read one worker's generation, action, goal, first marker and progress pointer. Each of the two live frame validations executes this code. The same fields are addressed at publication and in the real worker. Group the five atomic fields for each worker in one static array of mailboxes; obtain each worker's reference by safe indexing. The five independent atomic values, initialization, access order and memory orderings remain identical. Preserve null-pointer validation before progress dereference, initial frame/address checks, publication and generation order, actual work, physical timer, ordered time samples, return plan, frame restore and accounting. This changes address calculation and data locality. It makes no latency prediction. In particular, a single cold boot cannot isolate mailbox-address cost, host scheduling, or timer delivery. Mixed initial values may grow data-backed load bytes.

Freeze the original order: aligned-wake/plain, CBS-wake/traced, aligned-saturated/traced, CBS-saturated/plain, aligned-wake/traced, CBS-wake/plain, aligned-saturated/plain, CBS-saturated/traced. Preserve the first attempt, raw receipts, terminal and per-grant evidence at each position under original clock, profile, selectors, trace and cutoff controls; only 8/8 passes. Inspect linked static addresses, atomics and actual dispatch/worker code in all four new ELFs. Compare the same adjacent boundaries and retain all outcomes. Independently compare entry and PT_LOAD bytes/headers for legacy periodic, aperiodic, CBS and CBS-wake benches and exercise the established controls for changed loads.

원래 순서 1번은 게스트 슬롯 0이다. 선택 시점의 여유는 148틱이며 실제 복귀 직전 표본은 이미 2,112틱 늦었다. 이후 실제 Rust 표지까지 3,781틱이 추가됐다. 선택→복귀 직전 구간에는 영수증 기록과 IRQ 복귀·컨텍스트 복원·어셈블리 기록기가 함께 들어 있어 개별 원인은 분리되지 않는다. 같은 ELF인 추적 위치 5와 비교하면 소스 확인 종료→게시 직전 차이는 4,912틱이다. 이 차이가 특정 명령의 비용이라는 근거는 없다.

기존 검증 함수는 한 작업자의 원자 필드 5개를 읽기 위해 서로 다른 정적 배열의 주소와 인덱스를 다섯 번 계산한다. 작업자별 구조체 하나로 배치하고 안전한 인덱싱으로 접근한다. 기존 초기값, 각 원자 접근과 순서, 프레임·포인터·뱅크 실측, 타이머와 실제 복귀·작업자 표지·기한별 부과를 유지한다. 실제 명령·데이터 배치와 시간 변화는 후속 검증 대상이다. 위 최초 8개 위치와 기존 벤치 4종을 고정하고 원시 증거 및 부하 변경 시 실행 대조를 유지한다.
