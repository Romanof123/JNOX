# Ordinary reservation first boots / 일반 예약 최초 부팅

Candidate 62ea1ebcb653dc51595eb8361785dd8c12e5e9fe against immediate parent d1e1f3d0c878d3bdca5c771de4c1488796f4d65b. Three of eight first boots passed the ordinary horizon, work, return and deadline-charge replay. Five stopped outside the 62,500-tick opening limit. The gate requires eight passes. / 최초 실행 8회 중 3회만 실제 작업·복귀·기한별 과금 및 전체 관측 구간 기준을 충족했다. 나머지 5회는 개시 시각 한도를 초과해 중단됐다. 전체 통과 조건은 8/8이다.

|#|App|Trace 추적|Historical slot/status/reason|Fresh slot/status/reason|Guard operands 피연산자|Mask|Grants 실측|
|--:|---|---|---|---|---|--:|--:|
|1|aligned-wake|off|34/0/0|0/2/5|3244671,3179409|3|0|
|2|cbs-wake|on|0/2/2|0/2/2|3226848,3160513|31|1|
|3|aligned-saturated|on|0/2/5|0/2/5|3218225,3154626|7|0|
|4|cbs-saturated|off|0/2/2|10/0/0|0,0|31|8|
|5|aligned-wake|on|1/2/2|34/0/0|0,0|31|18|
|6|cbs-wake|off|0/2/5|0/2/2|3232096,3166664|31|1|
|7|aligned-saturated|off|0/2/5|10/0/0|0,0|31|8|
|8|cbs-saturated|on|10/0/0|0/2/5|3228197,3159557|15|0|

2/5 stopped before selection. Receipt masks 3, 7 and 15 reached precommit, commit and cutoff programming respectively. 2/2 captured a real worker marker outside the inclusive limit. The separate historical integrated cohort completed 2/8. Positions 1 and 8 lost completion, and 4, 5 and 7 gained completion. These single observations do not establish a speedup. / 2/5는 선택 전 중단이며 마스크 3·7·15는 각각 커밋 직전·직후·컷오프 설정까지 기록했다. 2/2는 실제 작업자 표지가 한도를 초과했다. 별도 과거 집단은 2/8이었으며 단일 관측 비교로 성능 개선을 정량화할 수 없다.

Windows Rust 1.95.0 one-job release builds used the production size profile and no stage selector. One QEMU 6.2.0 TCG Cortex-A72 virt/GICv2 CPU ran with 128 MiB and the default 62.5 MHz counter. All eight processes reached their UART footer and parked until timeout (124). Each native Linux recording was copied to D: and reopened for a complete digest check before the next guest. Only traced positions have external exception histories. The compact decoder and independent ledger replay joined the captured return frames, worker progress and conservative per-deadline charge. A stopped prefix retains only its observed grants. / Windows Rust 1.95.0 단일 작업 크기 최적화 빌드, 기본 시계와 1 ms 가드, QEMU 15초 제한을 유지했다. 매 실행의 원시 UART·종료 상태·선택된 예외 기록을 보존했다. 독립 재생은 실제 복귀·작업량·기한별 부과를 대조했다.

On all four apps, linked .text decreased by 32 bytes, from 23,796 to 23,764 for aligned-wake and 23,800 to 23,768 for the other three. In aligned-wake the parent precommit block at 0x402019c8–0x402019ec subtracts epoch with a borrow check and compares elapsed ticks. Candidate 0x402019c8–0x402019dc retains the entry lower bound and branches above the loaded absolute endpoint. The release store and acquire readback follow. For checked D=epoch+H, epoch≤t≤D is equivalent to checked t−epoch≤H. IRQ entry≥target=epoch and before≥entry, post≥before, planned≥armed≥post preserve the lower bound. Selection explicitly checks both bounds. Checked endpoint overflow fails construction, and a representable full horizon already requires room beyond D. No unsafe widening identified. / 네 앱의 연결된 텍스트가 각각 32바이트 줄었다. 확인된 덧셈으로 상한을 구성하고 IRQ 진입부터 표본의 단조 순서를 검사한다. 선택 시 하한과 상한을 독립 검사하므로 유효 시각 범위는 동일하다.

All four legacy bench ELFs (periodic, aperiodic, CBS and CBS-wake) retain the same entry point, PT_LOAD header fields and segment bytes against this immediate parent. The conditional operational comparison therefore required zero guests. / 기존 벤치 4종 모두 직전 부모와 진입점·적재 세그먼트 속성·바이트가 일치해 조건부 추가 실행은 0회였다.

[Eight first boots, ELF comparisons and replay / 최초 부팅 8건·ELF 대조·재생](reservation-absolute-opening-first.zip) · SHA-256 3c1d463d188738a798569e266aaea6d569eab189d80e37d056d4d5be708ff1ac.
