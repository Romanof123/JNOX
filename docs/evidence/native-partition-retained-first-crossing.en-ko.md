# #238 archived first-crossing analysis / 최초 초과 지점

The original 1 ms acceptance failed in 25/920 windows from 80 captures. All 25 were generation 2 under default clock. With five launches per cell, the first common timestamp strictly beyond 62,500 ticks was as follows. IRQ is the closing IRQ of generation 1. The origin is its scheduled close, also the nominal opening of generation 2.

|Table/profile|IRQ|Selected|Pre-ERET|First EL0 observation|
|---|---:|---:|---:|---:|
|P/A ordinary base|0|2|0|2|
|P/B ordinary retained|0|4|0|1|
|P/C diagnostic base|0|3|1|0|
|P/D diagnostic retained|0|5|0|0|
|Q/A ordinary base|0|1|0|0|
|Q/B ordinary retained|0|2|0|0|
|Q/C diagnostic base|0|1|0|1|
|Q/D diagnostic retained|0|1|0|1|

gen2.csv gives the actual run, nominal opening, and all available elapsed samples for each of the 80 transitions. Empty interior cells mean unavailable. For the 13 diagnostic misses the first recorded over-limit markers by run number are result_begin (04,46), audit_begin (45), audit_end (03,06,12), schedule_begin (11,20), mmu_begin (14), publish_begin (05), pre_eret (13) and el0 (51,59). A marker at the beginning of a function or stage bounds work completed up to that point. It alone cannot assign cost to the named stage.

Source d01fd1587fb0563e1e8dd3d4eaeac39b9346e072. Receipt 1ee5db1038f93eb0d630ccf4a6738460e8ab5cb4 embeds blob 057eb6f96ec5ba4f83218a0de5514376eb5f4c1c. The reducer verifies the full input SHA-256. Ordinary and diagnostic common samples come from evidence/*.json, produced by tools/native_partition_replay.py from dispatch, trap and user-result records. For C/D, phase.json carries the same session's generation 1→2 native_partition_irq_phase_v1 RAM row, decoded by tools/native_partition_phase.py. kernel/src/native_partition.rs places the diagnostic markers. Each CSV phase value subtracts the nominal opening from an unsigned counter reading. Differences of adjacent timestamps measure elapsed intervals, including instrumentation and default-clock variability. Worker chunks/checksums are progress evidence and have separate units. The external exception trace corroborates PCs and exception/return flow. The retained 104-word comparisons, pointers and assembly restore path validate saved frames and support inference about restored contents. No direct full CPU-register sample at ERET exists in these captures.

For the next original-limit investigation, inspect gen1 trap observation/result/audit and the transition into gen2 dispatch with their signed remaining budgets, then separately inspect the return-to-first-user-observation bracket. P goes from worker PID 0 to the first idle PID 2 dispatch. Q goes from idle PID 2 to the same idle PID. Across all twenty default runs per table the latter bracket is 5,372–9,710 ticks in P and 1,317–4,273 in Q. This difference merits inspecting first idle entry versus idle resume. Nineteen runs already exceeded the limit by selection, so a change confined to the return bracket cannot recover them. The data cannot isolate cold translation, guest path cost, host scheduling, or other contributions, establish each ordinary run's interior phase, or demonstrate that a proposed change meets the original bound. No new execution or public change.

기본 클록 2세대에서만 25건이 초과했다. 공통 시점 기준으로 직전 IRQ 진입 때는 0건, 선택 때 19건, ERET 직전 1건, EL0 최초 관측 때 5건이 처음 한도를 넘었다. 위 표는 P/Q와 일반 A/B 및 진단 C/D별 결과다. CSV에는 각 실행의 실제 관측값을 담았다. 일반 실행의 세부 단계는 기록되지 않았다. 진단 표식은 해당 실행의 RAM에서 얻었고 각 단계 이름은 타임스탬프를 읽은 경계를 뜻한다. P는 작업자에서 유휴 태스크로 처음 진입하고 Q는 유휴 태스크를 재개한다. 원래 기준을 유지하려면 먼저 선택 전 예산 소모를 조사하고, P/Q 차이가 분명한 복귀 후 첫 사용자 관측 구간을 별도로 살펴볼 수 있다. 저장 프레임과 외부 예외 경로는 확인됐으나 ERET 당시 전체 레지스터 실측이나 호스트 지연의 원인 판별은 이 자료로 할 수 없다.

Reproduce using Python 3 standard library from this directory.

    python crossing.py path/to/ind238d01-first.zip > regenerated.csv

[Per-position independent first crossing analysis of all 80 observations](native-partition-retained-first-crossing.zip) · SHA-256 1433b402f3789f7c3c46a327f3823b3c3eb934f2b48d307dbe4600b48a4e1f4c.
