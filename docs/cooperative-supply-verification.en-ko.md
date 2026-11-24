# Combined AArch64 workload verification handoff

English · [한국어](#한국어)

**Scope.** One Cortex-A72 on QEMU virt/GICv2 with the default clock. Integration supplies the exact combined Git commit and configuration to a verifier who did not design, develop or integrate either feature. This sheet gives expected outcomes for VER-002, VER-005 and VER-023. Seal the initial verdict and raw captures before consulting issue comments, implementation notes, earlier evidence or source.

1. Use one Rust build or guest at a time. Save the compiler, target, flags, ELF SHA-256, guest command, clock rate, raw UART, launch result and any RAM capture on persistent storage. Keep the first attempt for every position, including a launcher failure or incomplete capture. Verify archive digests on a second readable copy before releasing the lane.
2. Build `JNOX_BOARD=virt JNOX_PROFILE=deadline` with each of four `JNOX_APP` values in order: `ordinary-edf-supply-refuse`, `ordinary-llf-supply-refuse`, `ordinary-edf-supply`, `ordinary-llf-supply`. Run trace mode (`JNOX_DEADLINE_TRACE=1`) and plain mode (unset). Use bounded raw UART acquisition. Check pre-launch refusal reasons 8/9 with zero worker dispatch. In each normal trace expect the service at [1,2), independent sum 89,440, A's identical frame and PC on resumption, positive A progress before and after, absolute A stop at epoch + 250 ms, and A/B results and deadlines. The plain interface exposes a smaller set of fields. Preserve and score only what each interface exposes.
3. Build `JNOX_BOARD=virt JNOX_PROFILE=fixed JNOX_APP=coop-bounded`. Run one ordinary boot and an independent RAM capture of 79 little-endian u64s at `coop_bounded_observations`. Verify peer progression 0 to 20 after voluntary yield, three accepted blocks in FIFO order, two deadline refusals with a free slot, positive reservation remaining on the nested request, and combined reductions 52 and 132. Independently reconstruct queue depth, accounting, deadline decisions, timestamps and reductions.
4. Exercise a decreasing sample after any previously observed sample in a nonempty burst, including a refused request and a callback boundary. A refusal preserves FIFO and reservation state. Equal samples are valid. A bad callback interval prevents further admissions during the burst. Once drained, a new interval can start at zero. Check both EDF arithmetic cases (an earlier infeasible interval and a checked addition overflow). Use an independent oracle or direct assertions and the existing host suites.
5. Compare the eight legacy deadline and two legacy cooperative ELF load images with the registered baseline. Check ordinary EDF, LLF, LLF overload, EDF overrun (plain and trace), coop-events and coop-single on the combined source. Keep file size, load bytes, code identity and actual runtime results separately. Check the requirement graph and bilingual scheduling release matrix. This scope supplies finite QEMU observations. Board timing and product release remain open.

## 한국어

검증팀은 두 기능의 설계·구현·통합에 참여하지 않은 새 담당자를 지정한다. 통합팀이 고정한 결합 커밋과 위 입력·예상값만 전달한다. 최초 판정과 원본을 남긴 뒤 이전 결과와 소스를 열어 본다.

- Cortex-A72 한 코어, 기본 클록, virt/GICv2에서 빌드와 게스트를 순차 실행한다. 첫 시도에서 기동 실패나 수집 중단이 생겨도 독립 위치로 보존한다. 영속 저장소의 ELF, 원시 UART·RAM, 실행 명령 및 환경을 다른 사본에서 해시로 확인한다.
- EDF·LLF 각각 공급 부족과 정상 입력을 진단 및 일반 모드로 실행한다. 작업 기동 전 이유 8·9로 거부하는지, 정상 진단에서 [1,2) 서비스와 합 89,440, A의 같은 프레임·PC 복귀, 중단 전후 양의 진척, 기준 시각 + 250 ms 종료 기준, 두 작업의 결과·마감을 확인한다. 일반 모드에서 관측 가능한 값은 별도로 채점한다.
- 협력형 일반 부팅 및 79워드 RAM 캡처를 수행한다. 자발적 양보 후 동료 값 0→20, 세 블록의 FIFO 완료, 빈 슬롯이 있어도 마감 거부 두 건, 중첩 게시의 양의 잔여 예약, 합 52 및 가중합 132를 대조한다. 거부 때 큐·예약을 보존하면서 관측 최대 시각을 유지하고, 동일 시각·시각 역행·콜백 오류·큐 배출 뒤 새 구간의 0을 시험한다.
- EDF 최초 구간 수요 초과와 실제 덧셈 오버플로를 따로 확인한다. 기존 마감 구성 8개와 협력형 2개의 이미지 및 실제 실행도 대조한다. 양 언어 추적 그래프와 출시 표를 확인한다. 실기 시간 및 제품 승인은 별도 판정으로 남는다.
