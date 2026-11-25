# Sampled least-laxity-first dispatch for native jobs / 네이티브 작업의 격자형 최소 여유시간 우선 스케줄링

[#306](https://github.com/Romanof123/JNOX/issues/306) · CHG-121. IMP-121 and EVD-121 are reserved. Baseline `75657a3a87f3803fcf7687d1c70c776df5dc2560`. An operator can run the same independently linked finite jobs under EDF or LLF. Under LLF a waiting process can become more urgent than a running process even when no job arrives. This design specifies the timer observation that would demonstrate the change on one QEMU virt CPU. [Native EDF contract](native-edf-applications.en-ko.md) · [POL-005](scheduling-release-cases.en.md)

동일한 독립 ELF 묶음에 EDF 또는 LLF를 선택한다. LLF에서는 새 작업이 도착하지 않아도 대기 작업의 여유시간이 실행 작업보다 작아질 수 있다. 단일 QEMU virt CPU에서 실제 타이머 선점과 각 앱의 계산 완료를 각각 판정한다.

## Package and execution / 패키지와 실행

~~~sh
python3 tools/build_native_el0.py --out llf-pair --policy llf \
    --elf checksum.elf --job 0,6,1 --peer-elf histogram.elf --peer-job 0,7,3
~~~

Accept one to three slots using the existing EDF options, units, ELF verification, process isolation and startup path. Validate exactly one release, absolute deadline and estimated cost tuple per supplied image at both the CLI and raw Cargo interfaces. Keep the finite 16-grid horizon, unrestricted supply, checked counter conversions, constraints on incompatible options, and 100 ms grid Q = CNTFRQ/10. Record the selected policy and all tuples in the manifest and kernel evidence. Generalize numeric admission to run both the endpoint-demand check and the simulation using the selected deadline policy; preserve the existing EDF wrapper, refusal reasons and linked LLF callers. Admit completion exactly at the modeled deadline. Choose the common future epoch S after preparation, and refuse the entire package before EL0 entry on admission or time-conversion failure. [Numeric admission](../kernel/src/deadline_numeric.rs)

기존 EDF와 같은 옵션·단위·ELF 검증을 사용하고 주 앱 C 소스 입력도 유지한다. 슬롯당 튜플 하나를 CLI와 Cargo 양쪽에서 확인한다. 전체 구간의 수요 검사 후 선택한 정책으로 모의 실행한다. 격자 길이는 100 ms, 유한 지평은 16칸이며 모형상 마감 시각의 완료는 허용한다. 준비 후 미래 공통 시점 S를 정하고 거부 사유와 선택 정책을 기록한다.

Use the existing DeadlineScheduler with DeadlinePolicy::Llf. At each eligible physical PPI30 sample derive k = floor((sample − S)/Q), account the elapsed logical grids once to the actual incumbent, admit due releases and wakes, then rank live runnable jobs by signed i128 laxity D − k − remaining. Live estimates clamp at one until actual SDK exit. Keep the incumbent on equal laxity; remaining ties follow stable ready order. Permit at most one involuntary owner change per observed grid. Preserve the original yield, future and elapsed sleep, mid-grid wake, idle and exit semantics; an explicit scheduling syscall is counted separately. Handle delayed delivery with one observed selection, reporting skipped required samples and the actual interrupted frame. Raw counter service, scheduler gaps, first restore and exit time are independent of modeled cost. [Shared selector](../kernel/src/deadline_policy.rs) · [EL0 dispatcher](../kernel/src/native_edf.rs)

실제 PPI30 관측 격자에서 현재 실행자에게 경과량을 한 번 정산한 뒤 공개·깨움을 반영한다. 부호 있는 i128로 절대 마감에서 격자 번호와 잔여 비용을 빼며, 실행 중인 작업의 잔여 비용 하한은 1이다. 동률이면 실행자를 유지하고 나머지 동률은 준비 순서를 따른다. 격자 하나에서 비자발적 전환은 최대 한 번이다. 늦은 IRQ의 누락 격자를 기록하고 자발적 양보·차단·종료와 구별한다.

## Frozen crossover pair / 고정 입력과 우선순위 역전

A is a C checksum and B a Rust high-nibble histogram over a 4,096-byte private ramp x[i] = (17i+3) mod 256. Initialize before the reduction, read input bytes through volatile accesses, and update each private completed-block count only after publishing that block's results. A processes exactly 16 blocks, publishing a u64 byte sum of 8,355,840 and count 16, then exits 17. B processes exactly 131,072 blocks (512 MiB read in total), publishing sixteen u64 bins of 33,554,432 each and block count 131,072, then exits 29. Preserve the loads and histogram updates in the linked binary. Neither reduction calls the clock, yields, sleeps, reads policy state or stops on elapsed wall time. Freeze both binaries, symbols, disassemblies, inputs and iteration counts before the first scored launch. Check every bin, the full checksum, counts and statuses from each private mapping independently.

A는 C 체크섬, B는 Rust 상위 니블 히스토그램이다. 각자 4,096바이트 입력 배열을 만들고 같은 바이트 식을 사용한다. A는 16블록의 합계 8,355,840과 종료값 17, B는 131,072블록의 16개 빈 각각 33,554,432와 종료값 29를 남긴다. 블록 완료 수와 전체 결과를 사설 메모리에서 대조한다. 계산 루프에는 시계 읽기나 자발적 스케줄링 호출이 없다.

| Observed grid k / 격자 | A remaining, laxity | B remaining, laxity | Required running slot / 실행자 |
| --- | --- | --- | --- |
| 0 | 1, 5 | 3, 4 | B, both released in package order |
| 1 | 1, 4 | 2, 4 | B retains the tie |
| 2 | 1, 3 | 1, 4 | A preempts B with no new release |

The admission model completes A at k=3 and B at k=4. At runtime require both live at the first two timer samples, B's interrupted ELR at k=2 inside its productive loop, a single B→A change, A's valid result and exit, then B resuming at its saved PC on its original stack and continuing to the complete histogram. Check private progress on both sides of the interruption using retained samples or external observation. Record when each job actually exits against S+6Q or S+7Q. B's oversized fixed input makes a live crossover plausible and may also produce a genuine miss. An early B exit, initialization PC, skipped grid, or missing frame leaves the corresponding crossover claim unqualified; retain that position. A later miss cannot annul an observed crossover or establish a successful deadline.

모형은 A를 격자 3, B를 격자 4에 완료한다. 실제 실행에서는 격자 1에서 B의 동률 유지, 격자 2에서 신규 공개 없이 B의 계산 루프를 선점하는 A를 확인한다. A 종료 후 B가 원래 PC와 스택에서 복귀해 전체 결과를 완성해야 한다. 물리 종료 시각은 슬롯별 마감과 별도로 대조한다. B가 일찍 끝나거나 표본을 놓치면 해당 최초 관측을 보존하고 그 항목은 미검증으로 둔다.

## First positions and controls / 최초 실행 및 대조

Freeze the package, commands, compiler, QEMU version, CPU (one Cortex-A72), GICv2, RAM (128 MiB), default counter clock, journal capacity and host timeout (120 seconds per cold boot) before launch. Run the LLF pair plain then with external exception tracing; run EDF on the identical ELFs and tuples plain then traced. EDF should enter A first. Reconstruct chronological decisions independently from sampled counters, armed compares, original releases/deadlines, runnable order, remaining estimates, signed ranks, actual owner, saved/restored ELR and SP, raw service and terminal products. Identify launch/restore brackets separately from the EL0 execution they bound. Record journal loss, timeouts, and all initial failures. Do not substitute a retuned binary or later position for a first capture.

추적 없이 LLF를 먼저 실행하고 다음은 외부 예외 추적을 켠다. 같은 ELF·튜플을 EDF로도 같은 순서로 실행한다. 모든 실행 조건과 120초 제한을 먼저 고정한다. 실제 카운터·비교기·준비 순서·부호 있는 순위·문맥·제품으로 판정을 재구성하며 원본 실패와 기록 손실을 남긴다.

- Run short A alone with (0,4,1) for a finite completion. Run long B alone with (0,1,1) for a negative-laxity control: if still live at k=1 its remaining estimate is 1 and laxity is −1. Record raw service, actual completion, full product and exit after any observed miss. Equality and one-count-late deadline classification also receive deterministic host controls.
- Reject (0,1,2) before user entry. Retain host controls for equal-ranked waiters, one decision per sampled grid, negative ranks, and release/suspend/resume with the same absolute deadline. Continue executing an admitted live process after estimate exhaustion or a measured miss.
- Keep the original native EDF first positions, linked EDF/LLF evidence and external return witness under their original CPU, clock and observation scopes. Report native LLF functional selection, actual deadlines, and external timing evidence separately. Physical-board and integrated-load qualification remain open.

단독 단기 작업, 마감이 한 칸인 장기 작업의 음수 여유시간, 과수요 거부를 별도 실행한다. 비용 추정량이 소진되거나 마감이 지나도 허용된 작업은 실제 종료까지 수행한다. 기존 연결형·네이티브 EDF 및 외부 복귀 관측의 범위를 유지하며 실물 보드와 통합 부하의 보장은 별도 판정한다.
