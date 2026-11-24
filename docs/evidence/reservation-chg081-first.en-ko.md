# Reservation dispatch refactor · independent first boots / 예약 디스패치 정리 독립 최초 부팅

Across B/E/F, the matched parent completed 3/6 W and 2/6 S. The candidate completed 4/6 W and 5/6 S. Each stop retained its 62,500-tick opening limit at 62.5 MHz. Two observations per selector/site cannot establish a product bound or causal speedup.

B/E/F 전체에서 부모의 완료 수는 W 3/6, S 2/6이고 후보는 W 4/6, S 5/6이다. 각 최초 실행의 62,500틱(1 ms) 개시 한도와 실패 판정을 보존했다. 선택자·관측점당 두 관측치의 적용 범위는 이 가상 시스템이다.

Parent 87bee9296a8c6f3deeb18debc57d184272429f4a (tree c77b0073c0f1c0c1c0d3a8bcace51f8f1590767c). Candidate efabd263c24a8ad8bd71c614ccfd2f437020ff81 (tree d9658ac21fed05594f930adc6e53ff4a1ac7a812). One-job locked release builds used Windows Rust 1.95.0. WSL Ubuntu 22.04 QEMU 6.2, default TCG clock, one AArch64 Cortex-A72, GICv2/CNTP, 128 MiB and ext4. Each position retained a 15-second first cold attempt. W uses internal return and progress records. S additionally joins the same process’s IRQ/SVC/ERET trace.

Both parent baseline PT_LOAD images matched the historical reference, and all six matched their archived v3 counterparts. The E-site images retained the EOI store, adjacent DSB/ISB/MRS and ADR witness for 0x402013f0. Candidate F followed both live frame checks (832-byte frames, 16,384-byte stacks). The successful second check reaches ISB at 0x40201938, MRS at 0x4020193c and the linked ADR. Frame failure reaches the nonreturning call at 0x40201934. B/E/F had 26/27/27 counter sites.

The exact prepared address and canonical stack top minus 832, together with the fixed-size/alignment assertions, entail initial containment. Validation still reads live ELR, saved status, x19, thread identity, client state, progress and allocations. Masked opening checks both coherent worker snapshots against the retirement ledger. Only the selected grant's redundant reread was replaced. The retirement handler retains its separate snapshot, and both clients are sampled again at the horizon. In the W baseline the three linked snapshot call sites became two, retaining calls at 0x402014c8 and 0x40202d94.

부모의 두 기준 적재 이미지는 과거 기준과 일치했고 6개 모두 v3 원본과 일치했다. 후보는 준비 주소와 스택 끝−832를 확인하며, 16 KiB 스택의 크기·정렬 보장으로 초기 프레임이 범위 안에 든다. 실제 저장 레지스터·상태와 할당 검사는 유지됐다. IRQ 차단 구간에서 두 작업의 진행값이 반납 원장과 같은지 확인한 뒤 선택된 작업에 검증값을 사용한다. 반납 시 독립 판독 및 지평선에서 두 작업의 최종 판독을 유지했다.

| Image | .text bytes parent → candidate | IRQ load instructions | IRQ branch instructions | BSS bytes parent → candidate |
|---|---:|---:|---:|---:|
| WB | 23864 → 23796 | 128 → 129 | 131 → 127 | 116504 → 116504 |
| WE | 24020 → 23952 | 132 → 133 | 133 → 129 | 116528 → 116528 |
| WF | 24008 → 23940 | 131 → 132 | 131 → 127 | 116528 → 116528 |
| SB | 23868 → 23800 | 128 → 129 | 131 → 127 | 116504 → 116504 |
| SE | 24028 → 23960 | 132 → 133 | 133 → 129 | 116528 → 116528 |
| SF | 24016 → 23948 | 131 → 132 | 131 → 127 | 116528 → 116528 |

W is aligned-wake/plain (34 slots). S is cbs-saturated/traced (10 slots, four grants each on completion). Each split uses only endpoints from its own guest. Guard 2/5 stopped dispatch before selection. Guard 2/2 recorded a late worker marker. A dash means the endpoint was unobserved.

W는 정렬 깨움 34슬롯, S는 포화 CBS 10슬롯이다. 완료한 S 실행은 두 작업에 각 4회 부여했다. 각 구간은 같은 게스트에서 측정했다. 2/5는 선택 전 개시 중단, 2/2는 작업자 표식 지연이다. 대시는 미관측이다.

| Revision | Position | Mask | Terminal slot/owner/status/reason | Raw operands | IRQ−epoch | Source→probe | Probe→pre | Whole | Marker−epoch | Grants |
|---|---|---:|---|---|---:|---:|---:|---:|---:|---:|
| parent | 01-WB | 31 | 0/0/2/2 | 3216434/3152467 | 14213 | — | — | 21327 | 63967 | 1 |
| parent | 02-WE | 31 | 34/2/0/0 | 0/0 | 21408 | 2832 | 16245 | 19077 | 61086 | 18 |
| parent | 03-WF | 3 | 0/2/2/5 | 3220179/3151651 | 30071 | 24173 | 4266 | 28439 | — | 0 |
| parent | 04-SF | 31 | 10/2/0/0 | 0/0 | 18776 | 13859 | 2353 | 16212 | 56023 | 8 |
| parent | 05-SE | 15 | 0/2/2/5 | 3226152/3162987 | 26835 | 2863 | 13660 | 16523 | — | 0 |
| parent | 06-SB | 3 | 0/2/2/5 | 3227053/3160722 | 38033 | — | — | 19548 | — | 0 |
| parent | 07-SB | 31 | 0/0/2/2 | 3217865/3155304 | 18207 | — | — | 19285 | 62561 | 1 |
| parent | 08-SE | 31 | 0/0/2/2 | 3218472/3153502 | 22749 | 3279 | 13850 | 17129 | 64970 | 1 |
| parent | 09-SF | 31 | 10/2/0/0 | 0/0 | 17339 | 17162 | 2924 | 20086 | 62136 | 8 |
| parent | 10-WF | 31 | 34/2/0/0 | 0/0 | 17369 | 18011 | 3029 | 21040 | 61986 | 18 |
| parent | 11-WE | 15 | 0/2/2/5 | 3221692/3158458 | 23101 | 3467 | 20502 | 23969 | — | 0 |
| parent | 12-WB | 31 | 34/2/0/0 | 0/0 | 16582 | — | — | 18055 | 60601 | 18 |
| source | 01-WB | 31 | 34/2/0/0 | 0/0 | 17458 | — | — | 18245 | 59039 | 18 |
| source | 02-WE | 31 | 34/2/0/0 | 0/0 | 16051 | 2622 | 12774 | 15396 | 50159 | 18 |
| source | 03-WF | 31 | 34/2/0/0 | 0/0 | 18409 | 16900 | 2935 | 19835 | 61693 | 18 |
| source | 04-SF | 31 | 10/2/0/0 | 0/0 | 20662 | 14493 | 2310 | 16803 | 59585 | 8 |
| source | 05-SE | 15 | 0/2/2/5 | 3218134/3154822 | 24299 | 3602 | 15993 | 19595 | — | 0 |
| source | 06-SB | 31 | 10/2/0/0 | 0/0 | 25125 | — | — | 14757 | 60909 | 8 |
| source | 07-SB | 31 | 10/2/0/0 | 0/0 | 21488 | — | — | 14886 | 56993 | 8 |
| source | 08-SE | 31 | 10/2/0/0 | 0/0 | 18467 | 3354 | 15842 | 19196 | 61995 | 8 |
| source | 09-SF | 31 | 10/2/0/0 | 0/0 | 21093 | 16104 | 2888 | 18992 | 62134 | 8 |
| source | 10-WF | 15 | 0/2/2/5 | 3221052/3158533 | 25603 | 17203 | 2521 | 19724 | — | 0 |
| source | 11-WE | 15 | 0/2/2/5 | 3218150/3154968 | 22448 | 3437 | 18738 | 22175 | — | 0 |
| source | 12-WB | 31 | 34/2/0/0 | 0/0 | 17629 | — | — | 18396 | 58855 | 18 |

### Same-boot interval comparison / 같은 실행의 구간 비교

| Revision | Site | Local majority in both boots | Split whole range, ticks | Baseline whole range, ticks | Same reached stage | Range overlap |
|---|---|---|---|---|---|---|
| parent | WE | suffix | 19077–23969 | 18055–21327 | False | True |
| parent | WF | prefix | 21040–28439 | 18055–21327 | False | True |
| parent | SE | suffix | 16523–17129 | 19285–19548 | False | False |
| parent | SF | prefix | 16212–20086 | 19285–19548 | False | True |
| source | WE | suffix | 15396–22175 | 18245–18396 | False | True |
| source | WF | prefix | 19724–19835 | 18245–18396 | False | False |
| source | SE | suffix | 19196–19595 | 14757–14886 | False | False |
| source | SF | prefix | 16803–18992 | 14757–14886 | True | False |

A locally larger phase describes that instrumented run. Unequal reached stages or disjoint whole ranges preclude transferring a split to the baseline. Diagnostic overhead remains included.

큰 구간은 해당 계측 실행 내부에서 관측된 결과다. 도달 단계가 다르거나 전체 구간 범위가 겹치지 않으면 기준 이미지에 비율을 적용할 수 없다. 구간에는 계측 비용이 포함된다.

### Additional first controls / 추가 최초 대조

| Selector | Verdict | Terminal status/reason | Raw operands |
|---|---|---|---|
| aligned-late | first stop before qualifying late-enforcement verdict | 2/2 | 3226021/3152382 |
| aligned-refused | expected admission refusal | 1/2 | 9/8 |
| aligned-saturated | horizon completed | 0/0 | 0/0 |
| cbs-late | first stop before qualifying late-enforcement verdict | 2/2 | 3227899/3156829 |
| cbs-refused | expected admission refusal | 1/2 | 9/8 |
| cbs-wake | horizon completed | 0/0 | 0/0 |

The archive includes per-position raw inputs, both frozen manifests, linked ELFs, maps and disassembly, guard dispositions, ownership, cumulative service, actual returns and recurrence checks. The original v3 cohort remains a separate historical observation. Static reserves do not establish stack high-water or board latency.

자료에는 각 최초 실행의 원본·ELF·링크 맵·디스어셈블리·가드 판정, 원장·작업 갱신·실제 복귀 대조가 들어 있다. 기존 v3 최초 결과는 별도로 유지했다. 정적 예약 크기로 최대 스택 사용량이나 실제 보드 지연을 추정하지 않았다.

[Metrics / 측정값](reservation-chg081-first.json) · [Replay archive / 재검증 자료](reservation-chg081-first.zip) · [Frozen plan / 고정 설계](../reservation-stage-attribution-plan.md) · [Historical v3 / 기존 v3](reservation-stage-v3-first.en.md)

Signed by role: Independent execution and verification reviewer / 독립 실행·검증 담당.
