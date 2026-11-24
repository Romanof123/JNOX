# PR 274 independent first captures / 독립 최초 실행

Public candidate `4ecce4b0f3afdd1a676bcffb81fae33acf4b1d6d` differs from public main `6c32c913e4991820333e168b39ee4cb85dc2e336` in seven files. WSL ext4, Rust 1.95.0, --locked --release -j1, virt/GICv2, AArch64 Cortex-A72 TCG, one CPU. Twelve designated first builds and captures succeeded in order. The preflight shell script initially failed to parse its final CRLF, before any case directory or build; it was corrected before position 01. Raw first hashes were sealed before opening legacy evidence or implementation.

| Outcome / 결과 | EDF plain/trace | LLF plain/trace |
|---|---|---|
| Normal / 정상 | Finished / Finished | Finished / Finished |
| Ceiling 3 / 한도 3 | Refused / Refused | Refused / Refused |
| Tight supply / 공급 부족 | Demand / Demand | Miss / Miss |

Each normal recorded two service calls, arguments covering 1–64 and 65–128, returning 89,440 and 617,824 respectively. The release disassembly shows BL, RET and storage of the returned register before the caller's counter sample. Frozen rows, UART, exception return PCs, worker stack addresses, selected frames, absolute counter dates and recomputed wrapping products agree. A's block count held across each reserved cell and increased between interruptions and again before completion. Both calls returned before their absolute boundaries. Four ceiling and four supply refusals had zero launch, frame/compare/restore and return records and zero exception entries. Independent model jobs A/B occupy 0,2–8,10 / 11. Real B completes in cell 10 after A's early SVC.

All four accepted guests recorded 11 grid IRQs and two completion SVCs. Of thirteen entries, six returned to a different owner. The final B-to-idle selection parked and never restored idle. Aligned and arbitrary Q-width peaks were three entries and two returning transitions, with ceilings three and four entries respectively. The proved arbitrary-width bound is min(17,1+ceil((W−1)/Q))+2. Measured IRQ delays against S+kQ ranged from 10,736 to 27,665 counts at 62.5 MHz, all inside their assigned cells. assessment.json includes measured owner entry/restore counters and each integer-width event-span breakpoint. Peaks stay constant between breakpoints, and the upper bound is nondecreasing. These timings cover this emulated fixture.

The host suite passed 10 tests including invalid dimensions, overflow, exhaustive small-window bounds and interval supply. A separate clipped-calendar enumerator checked 41,976 intervals in 1,266 small calendars. An independent selector compared 9,792 complete or first-miss slot sequences and admission decisions against the production scheduler across both tables and policies. With both reserved slots [0,10) offers 8 against demand 9. With either single slot reserved it offers 9.

All 24 original deadline, supply and capped selectors were rebuilt through tools/build_kernel.py and booted afresh. Their terminal and ordered UART field labels match the original strict-class-owner-window-combined.zip. All 24 .text and .data payloads match. All 24 have 28–40 changed bytes in .rodata and different complete ELF and load hashes. Exact original/candidate hashes and load segments appear in legacy-summary.json. The generic boot passed two cold boots and the separate deadline benchmark passed one. Invalid owner cap and recurring application outside the deadline profile both failed build as intended. Other profiles were checked through the scoped build.rs diff.

한국어 — EDF·LLF의 정상·한도 거부·공급 거부를 각각 일반/추적으로 최초 실행하여 12건 모두 ELF, UART, 예외, QMP, 정지 PC와 RAM을 보존했다. 정상 4건 모두 셀 1·9에서 서로 다른 입력 64개를 계산해 89,440과 617,824를 반환했다. A의 프레임과 ELR은 두 번 모두 정확히 복원됐고, 정지 중 블록 수는 그대로였으며 두 실행 구간과 최종 완료까지 진행량이 늘었다. 공급 부족은 EDF Demand·LLF Miss, 한도 3은 두 정책 모두 시작 전에 거부했다. 여덟 거부 모두 프레임·타이머·초기 복원 카운터가 0이었다.

각 정상 실행의 IRQ 11건·완료 SVC 2건 중 실제 다른 소유자로 복귀한 횟수는 6회였다. Q폭 이동 구간의 관측 최댓값은 진입 3·전환 2이며 계약상 상한은 4다. IRQ 44건 모두 지정 셀 안에서 전달됐다. 기존 24개 설정의 새 부팅은 원본과 종료 결과·행 종류 순서·기계어·데이터가 일치했다. 읽기 전용 데이터 28–40바이트의 차이로 전체 ELF와 로드 해시는 달랐다. 기본 부팅 2회와 별도 deadline 벤치마크 1회도 통과했다. 물리 하드웨어 WCET 측정은 이 검증 범위 밖이다.

[Independent recurring service records / 반복 예약 검증 원본](deadline-recurring-first.zip) · SHA-256 863aec969b8581108e1e54b449baa64119cad42eea569a695f07df3600a70bba.
