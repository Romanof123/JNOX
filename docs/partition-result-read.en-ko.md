# P/Q result collection (#230, under #216)

## Contract before implementation

The archived 096e5a9 ordinary P ELF calls copy_from_user at 0x4020329c. Its success path calls the page iterator and memcpy. The caller then executes the 22 instructions at 0x402032a8–0x402032fc six times (48 byte loads). Diagnostic P calls the same copy at 0x40203370. Change only the six-word read: keep the existing live IRQ mask, installed/current root and PID, full extent and every page ownership check, plus alignment. Read six fresh little-endian words through the active user VA. Retain all trap/frame records and inputs. No claim about wall-clock savings follows from instruction counts.

일반 P ELF는 검사·복사 뒤 22개 명령을 여섯 번 실행하여 48바이트를 조립한다. 진단 P도 같은 복사 함수를 호출한다. 기존 IRQ 마스크, 설치 상태와 현재 루트·PID, 전체 범위와 각 페이지 소유권 검사를 그대로 수행하고 정렬을 확인한 뒤 활성 사용자 주소에서 여섯 단어를 매번 읽는다. 트랩·프레임 증거와 입력을 유지한다.

## Prospective controls / 사전 대조 계획

Compare 096e5a9 and candidate with identical user ELF, toolchain, QEMU, CPU and package arguments. For each P/Q × default/icount3 × ordinary/irq-v1 cell retain five cold baseline/candidate pairs in AB/BA/AB/BA/AB order with no retries. Save raw UART, RAM, exception logs, ELF hashes, arguments and all failed cohorts. Disassemble both builds and verify aligned loads and guards. Replay all rows, frame and timer invariants, compare same-PID and cross-PID transitions, and report first/later result-stage and ordinary opening-to-first-user distributions separately. Preserve the original clock origin, deadlines and 62,500-tick limit. Baseline default ordinary misses P 3/5, Q 1/5 in the cited archive. A shorter result stage cannot pass a missed ordinary opening.

동일한 사용자 ELF·도구·실행 인자로 P/Q, 기본/icount3, 일반/진단 각각에서 원본과 후보를 교대로 다섯 쌍씩 최초 실행한다. 재시도 없이 실패까지 보존한다. 양쪽 기계어와 검사, 행·프레임·타이머를 검증하고 동일/상이 PID, 최초/후속 전환 결과를 구분한다. 원래 시계 기점·기한·62,500틱 기준을 유지한다. 기존 일반 기본 클록 실패는 P 3/5, Q 1/5였다.

[Independent ordinary/diagnostic first pairs, code and compatibility results / 최초 독립 대조·기계어·호환 판정](evidence/native-partition-word-read-first.en-ko.md).
