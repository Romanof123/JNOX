# Native idle refill health — independent first evidence

Candidate a1a6a2f7440342125e257825775d4f2caacf0cd6, parent 2cdbe3bf2e8604be54c6851046d6d95d2e106f7c. Timing FAIL on the finite virtual profile. Instrumentation detected one genuine new idle refill overrun; original CLI failed two independent contradiction controls. All products matched. No bounded latency or hardware release claim. 원래 시간 상한을 넘은 실제 유휴 재충전이 한 번 발생했다. 모든 앱 계산은 일치했다. 리더의 반례 두 건은 별도 실패다.

Fresh WSL ext4 launches on same machine, original no-override clock/one Cortex-A72/TCG/GICv2/128 MiB; 62,500,000 Hz, P=15,625,000, limit=1,953,125, spacing bound=125,000,000. Public native-hierarchical-applications-first.tar.gz SHA-256 626341e4aefba74c506a1654ca92dd2a70dd5f435295360246e69e9e7ef96380, all members verified before launch. Comparator ELF bytes came from that archive; ordered user ELFs from its exact packages. Raw launches record actual kernel and QEMU hashes. Only artifact paths, different candidate kernels and any naturally varying execution/environment changed in the pairs. Original operator QMP pmemsave schedule retained, 90 ms sleeps after each sampling round; candidate capture waits for complete health footer. No host setting adjustment, preemption injection or guest delay. Historical host load was not reproducible or correlated. Run times are QMP intervals in outcome files.

|Cold position|Rows|IRQ max lag|Max IRQ/decision observation gap|Scored refills|Max refill lag|Late|Timing health|
|---|---:|---:|---:|---:|---:|---:|---|
|original-saturated-plain|101|115868|11729955|—|—|—|legacy/default|
|saturated-plain|101|24638|11732213|41|13247|0|within_limit|
|original-swap-plain|129|30584|11737271|—|—|—|legacy/default|
|swap-plain|131|31491|11736144|61|20578|0|within_limit|
|saturated-trace|101|34952|11725009|41|22359|0|within_limit|
|swap-trace|131|27071|11731567|61|27071|0|within_limit|
|child-leaf-plain|285|29963|14667379|122|25407|0|within_limit|
|child-leaf-trace|285|30185|14666086|122|30185|0|within_limit|
|wake-plain|91|22834|11734618|30|22834|0|within_limit|
|wake-trace|89|9989268|19177172|29|9989268|1|late|
|single-plain|24|15660|11726946|11|12655|0|within_limit|
|single-trace|24|20779|11726297|11|16728|0|within_limit|
|default-plain|—|—|—|—|—|—|legacy/default|
|saturated-passive|101|27158|11732679|41|27158|0|within_limit|
|swap-passive|131|24908|11733471|61|22042|0|within_limit|

**Wake traced decision 60→61.** The kernel programmed D=250487263 between counter samples 241301610 and 241303383. Pre CVAL/CTL=246578877/1, post=250487263/1. Landing=241303822. Final masked sample before DAIFClr=241303863, DAIF=0x3c0. The actual interrupted PC was native_sleep_idle+4 (0x402014a0). Earliest E=260476531; later entry sample=260477197, CVAL/CTL=250487263/5, IAR=30, saved SPSR=0x60000345. Idle charge=0. Lag=9989268 ticks (159.828288 ms), excess=8036143. Terminal first and maximum agree with independent raw RAM/UART and external exception PC. Exits reporter/histogram/energy=7/70/179. Exactly one violation in 29 true refills, 76 IRQs, 89 decisions. No gap or occupancy failure. The wake workload label identifies the workload; this specific IRQ was a refill-only kind 1.

No chronological inference from entry ISTATUS concerning its first transition. The instrumented last sample precedes DAIFClr/ISB/WFI, not a timestamp at WFI or actual ERET. The early arm and masked pre-unmask sample with saved unmasked state do not isolate the late interval. Passive saturated and swap separately captured timer0 programming/condition, PPI30 rise/cpumask 1, CPU0 bestirq/line/ack plus external exception/ERET. Per-event line ordering for every one of 97 and 127 IRQs is archived. The timer recalculation trace for irqstate=1 is printed downstream of synchronous GIC callbacks. No event wall/guest timestamps, backend drop counter, thread scheduler attribution or calibrated clock mapping. Complete expected IRQ counts and terminal records; QEMU terminated by operator after final memory dump. These passive launches did not reproduce a miss. The actual failing wake trace has only external exception/ERET, no passive GIC stream.

Earlier published firsts in the b276a2e hierarchy archive remain FAIL: saturated 60→61 4201556 (excess 2248431), swap 82→83 6487068 (excess 4533943). Fresh reboots of the archived original kernels kept separate. Prior raw capture evidence belongs only to its identified workload and binary; linked EL1 traces do not timestamp earlier hierarchy misses. The two original ELFs, manifests and their archived first UART/exceptions/rows are recoverable from the pinned public archive; origin-reference.json binds their sources.

Independent RAM page walks, slot hashes, original 47-field ledger and new 22-word timer RAM records, counter ordering/arm kind, private products and progress, waits, eligibility, cap/debt/refill accounting, frame roots/ELR and traced exception/ERET agree. Retained timer hardware observations and masked pre-unmask samples are in each idle-readbacks.json. The original min/max/count/sum/handling shares including probe time and all signed transactions are in metrics.json and each ledger. Single-frame-proof.json and sample-decode.json record live snapshots. Default CRC and all four linked EL1 ordinary/recurring first launches passed, 8/14 traced returns. Five direct health arithmetic tests, 7 policy tests and historical independent 10162 queries passed. Synthetic host accounting/CLI cases remain separate from guest observations. Full-product evidence is independently checked in metrics.json; the CLI compares only process exit codes.

The original CLI accepts an active startup arm mislabeled terminal as valid/within_limit/0, and reports valid/late/1 for a first dropped violation with an impossible previous arm identity. Expected both contradictions to be rejected with status 2. reader-cases.json preserves all initial controls. Synthetic short control streams exercise health arithmetic only; real product/terminal state is separately checked in all first boots. The instrumentation caught this natural miss; the original reader is not fully qualified for contradictory input. #333/#334/#21 original dispositions stand.
