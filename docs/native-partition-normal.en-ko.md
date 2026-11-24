# Native EL0 cyclic partitioning on AArch64 virt

[Tracking ticket #216](https://github.com/Romanof123/JNOX/issues/216) · [POL-006 / #99](https://github.com/Romanof123/JNOX/issues/99) · [Native ABI / #12](https://github.com/Romanof123/JNOX/issues/12) · [Target gates / #21](https://github.com/Romanof123/JNOX/issues/21)

## Engineering requirements and design

Trace REQ-002/005/023 → DES-003/012 → VER-002/005/023 for scheduling and REQ-020 → the native ELF launch path. The [upstream partition plan](pol-006-rust-partition-plan.en.md) defines the timing contract. This configuration replaces its EL1 fixture with two ordinary, separately mapped native EL0 processes and a third EL0 idle context. The three copies of one static SDK ELF have distinct TTBR0 roots, ASIDs, writable result pages, stacks and persistent register frames. EL1 selects contexts and programs the physical counter timer on a single Cortex-A72. EL0 reads the counter and a read-only dispatch generation. Only an ordinary exit SVC occurs after the scoring interval.

| Selector | Repeating half-open windows in units of q |
| --- | --- |
| `partition-p` | A [0,2), B [2,3), idle [3,4) |
| `partition-q` | B [0,1), idle [1,2), A [2,4), B [4,5) |

1. Admit each complete, positive, static table with the existing cyclic executive. Use frequency F divisible by 1000, q = F/20, one epoch E and absolute boundaries. A is ready from the beginning. B becomes ready on entry to the first explicit idle slot. The first B window therefore dispatches idle. Every consecutive same-context slot, including the B-to-B wrap in Q, receives a distinct timer closure, dispatch generation and post-return observation. The idle process records its arrival and does no worker arithmetic.
2. Score three complete frames and the first return in frame four. Execute that last window to its ordinary closing interrupt to collect the first return observation. Mark its service as an observation extension. Then publish completion and dispatch A followed by B solely to collect their normal exits. Each drain has a q-tick limit. Preserve the original clock grid across every dispatch. A missed boundary makes this healthy run incomplete.
3. Keep intended owner, ready mask, actual root and selected/interrupted PC, SP and TLS. Snapshot the active process's private result on every closure. A and B maintain independent chunk counts n; each completed chunk adds 1 to n and 3 (A) or 5 (B) to its checksum. The final results have positive n, exact checksums and exits 41/42. Every eligible measured window must show new private work. A snapshot during a final partial update is resolved by the coherent exit result. The idle loop never increments either worker result.
4. For each nominal release d retain the previous IRQ entry i (E for initial dispatch), selection, last pre-ERET sample r, first observed user tick x and closing IRQ entry j; preserve actual CVAL/CTL/IAR. Score d ≤ i ≤ r ≤ x < j and x−d ≤ F/1000. Replay from the original E across wraps; count each [d,next d) once, including outgoing spill. Keep the measured entry-to-next-pre-ERET interval, pre-ERET-to-user observation bracket and user-observation-to-next-entry interval distinct. The vector samples j just after exception entry; this last interval includes that entry uncertainty. Use the independent QEMU exception trace to check ERET and interrupted PCs. Never convert the pre-ERET sample into an EL0 time. Close timing only when the raw counter evidence supports the original limit.
5. Retain the first cold capture of each table and clock (`default` then `icount3`), one guest at a time. Package through `tools/build_native_el0.py --app partition-p|partition-q --out NEW_DIR`. Capture through `tools/native_weighted_capture.py TABLE CLOCK NEW_CAPTURE KERNEL USER PROBE SOURCE_REV`. A terminal marker only identifies capture completion. Independent validation needs the raw RAM, linked kernel and user ELFs, launch/configuration, UART and ordered exception trace. Include first-frame readiness, explicit idle, all wraps, bounds and both final result pages. Recheck the unchanged weighted selectors after shared-path changes.

The #178 source provides the two-process launch, vector and timer primitives. Its weighted measurements do not establish these results. Native P/Q results, physical timing, target-specific safety constraints, fault cases and product acceptance require separate evidence.

## 한국어 요구사항과 설계

[상위 POL-006 계획](pol-006-rust-partition-plan.md)의 표와 1 ms 기준을 그대로 사용한다. Cortex-A72 단일 코어에서 A, B 및 유휴 문맥을 각각 별도 TTBR0, ASID, 쓰기 가능 결과 페이지, 스택과 보존 프레임에 둔다. 동일한 네이티브 SDK ELF를 세 번 적재하며 EL1이 스케줄링과 물리 타이머를 담당한다. EL0는 읽기 전용 디스패치 세대 값과 물리 카운터를 읽는다.

1. q=F/20, F는 1000의 배수다. 표 P는 A 2q, B 1q, 유휴 1q 순서이고 Q는 B 1q, 유휴 1q, A 2q, B 1q 순서다. 기존 순환 스케줄러가 양의 창 길이와 프레임 전체 길이를 검사한다. 처음에는 A만 준비 상태다. 첫 명시적 유휴 창에서 B를 준비시킨다. 준비 전 B 창에는 유휴 문맥이 실행된다. Q에서 B가 프레임 경계를 넘어 연속 실행할 때에도 각 창에 대한 타이머 인터럽트와 복귀 후 EL0 관측을 따로 기록한다.
2. 세 프레임 전체와 네 번째 프레임의 첫 복귀를 측정한다. 마지막 관측 창은 닫는 인터럽트까지 실행하고 서비스 통계를 별도 표시한다. 이후 완료를 공개하고 A와 B의 종료 SVC를 차례로 회수한다. 각 회수 시간은 q 이내다. 모든 타이머 경계는 최초 E를 기준으로 계산한다. 정상 실행 중 놓친 경계는 미충족으로 기록한다.
3. 명목 소유자, 준비 마스크, 실제 TTBR0, PC, SP, TLS와 각 IRQ의 CVAL/CTL/IAR를 보존한다. 작업별 완료 횟수 n과 체크섬 증가량 3(A), 5(B), 종료값 41/42를 대조한다. 각 준비된 작업 창에서 실제 진척을 확인한다. 유휴 문맥은 복귀 시각만 기록한다. 경계에서 완료 중이던 부분 갱신은 최종 결과의 일관성으로 확인한다.
4. 명목 경계 d, 직전 IRQ 진입 i(최초에는 E), 복귀 직전 표본 r, 해당 문맥이 직접 읽은 카운터 x, 닫는 IRQ 진입 j에서 d ≤ i ≤ r ≤ x < j 및 x−d ≤ F/1000을 확인한다. 경계를 넘은 선행 소유자의 시간, 커널 처리, 복귀 전후 관측 구간을 중복 없이 계산한다. 예외 진입 직후의 표본에 포함된 진입 오차를 명시한다. QEMU 예외 로그와 실제 PC 및 복귀 순서를 독립 대조한다.
5. 각 표의 최초 콜드 부팅 원본을 기본 시계와 icount3에서 순차 보관한다. RAM, ELF, 설정, UART, 예외 로그를 보존하고 두 기존 weighted 선택자도 회귀 확인한다. #178의 가중 스케줄링 결과, 기존 EL1 결과, 실보드 승인과 제품 기능안전 평가는 각각 별도 증거를 요구한다.
