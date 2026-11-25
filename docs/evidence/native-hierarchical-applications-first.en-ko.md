# Native hierarchy first records / 네이티브 계층형 최초 검증

Candidate b276a2ed8e9715fc6ced057b84681d77ccead755 against main 3906f6fcfe53f106b3f63c7080399a5fa4495dbf. Frozen ae7a0bddf8754e4fcb855ae04846015f6fe0c3e0. WSL ext4, single A72 TCG virt/GICv2 CPU, 62,500,000 Hz. All ten natural cold boots completed with original products and exits. Two plain firsts missed the frozen idle IRQ bound. Release timing verdict FAIL. No reruns or delayed IRQ injection.

열 번의 네이티브 최초 부팅에서 최종 산출물과 종료값은 모두 일치했다. 포화 및 교환 일반 부팅에서 실제 유휴 IRQ가 1,953,125틱 상한을 초과하여 전체 시간 판정은 실패다. 재실행 및 인위적인 지연 주입은 없었다.

|Position|Rows|Max IRQ lag|Max observation gap|Quota grants N|Positive overhead sum|True idle recoveries|Parent throttle windows|
|---|---:|---:|---:|---:|---:|---:|---:|
|saturated-plain|107|4201556|15912671|60|478508|43|12|
|saturated-trace|101|32238|11735283|56|595635|41|11|
|swap-plain|133|6487068|18199516|67|546850|62|31|
|swap-trace|133|24484|11725686|67|671776|62|31|
|child-leaf-plain|285|31709|14673231|159|925512|122|0|
|child-leaf-trace|285|27694|14668199|159|1267314|122|0|
|wake-plain|91|20000|11732380|45|355825|30|1|
|wake-trace|92|21826|11724047|46|486696|30|1|
|single-plain|24|22535|11735483|11|98874|11|0|
|single-trace|24|21596|11727210|11|117520|11|0|

Full per-row signed ledger, IRQ lags, counter brackets, cost categories, throttles, wake sequences and all first RAMs are supplied. N includes uninterrupted grant-end IRQs where the path minimum limited the reservation. An interval touching all three levels is counted once. All measured occupancy excesses fit N × 1,953,125; every decision gap ≤125,000,000.

Idle misses / 유휴 지연 초과

|Case/row|Prior selection end|Prior pre-transfer|Idle landing|Compare|PPI30 entry|Settlement|Restored EL0 pre-ERET|
|---|---:|---:|---:|---:|---:|---:|---:|
|Saturated 60→61|316957749|316958876|316958880|328667725|332869281|332871606|332873364|
|Swap 82→83|582595401|582596499|582596503|594305368|600792436|600793949|600799106|

Both misses were idle→histogram, true interrupted ELR 0x40201488 (native_sleep_idle+4), zero idle charge, ready 2 and exited 5. On entering idle, parent0 debts were 5349 and 6013. The assembly landing sample preceded compare by 11708845 and 11708865 ticks. Actual IRQ-entry counter exceeded compare by 4201556 and 6487068. Pre-ERET occurred 4205639 and 6493738 ticks after compare. Later histogram IRQs confirm execution. One compare-programming call per decision from implementation, bracketed by selection end and pre-transfer. The plain boots have no external IRQ/return stream, independent register write count, or timestamps at CVAL write, WFI, physical assertion, or actual ERET. QMP live reads and host scheduling are part of the environment. Attribution to a specific host or guest mechanism is unproven.

두 초과 모두 히스토그램만 준비된 상태에서 유휴 루프를 실제 중단했다. 두 착지 표본은 비교값보다 약 1171만 틱 빨랐다. IRQ 입력·실제 WFI·복귀 명령 그 자체의 타임스탬프와 호스트 스케줄링 기록은 없으며, 각 원인의 기여도는 분리할 수 없다.

Payload and causality / 계산 결과와 인과관계

Each ELF hash matched its manifest slot and mapped image. Independent TTBR-root page walks: CRC 134217728 bytes, CRC 2124873618, exit 237; Rust histogram 536870912 bytes, 2097152 × [1,8,28,56,70,56,28,8,1], exit 70; energy 1073741824 samples, 5864240971776, exit 179. Reporter recorded three deadlines, restored its post-SVC continuation each time and produced 3/mask 7/exit 7. Its deadlines agree with armed ticks and private observations are all due. Each block interval includes measured peer computation. Final RAM preserves complete private products. Periodic reads saw histogram private bytes and saved C-loop progress registers. The 128-byte sampled window omits CRC/energy progress words at offsets 264/272; their loop counters and full final RAM independently show progress.

The tier-by-tier signed replay matches all reported debts and balances, early closures, service/idle intervals, owners, comparisons, identity and frames in all runs. External traces correlate every exception origin/ELR and every EL0 ERET in traced runs. Lower-limit cases show child0 depletion with funded ancestors/leaf, then independent leaf1 depletion with funded parent and child. Single CRC shows exclusive leaf debt, true zero-charge idle interrupted in WFI loop and timer-driven restoration of exact CRC PC, SP and page root. Initial user mode derives from the exact binary initialization/ERET path (0x340); live saved exception frames confirm EL0t/I-clear and continued ramp progress at resumed PCs. External QEMU int output records return level and PC, not an independent live SPSR sample precisely at ERET.

모든 최초 실행에 대해 사설 주소 변환 후 최종 결과를 계산했고, 추적 실행에서는 각 예외의 진입 EL·중단 PC와 전체 EL0 복귀 PC를 대조했다. 하위 예산 구성의 자식과 리프가 각기 독립적으로 소진됐다. 단일 CRC는 조상 잔액이 남은 채 리프가 소진되고 실제 유휴에서 타이머로 복귀했다. 복원된 프레임은 EL0t 및 I=0이며, 복귀 뒤 반복 횟수가 증가했다.

Costs / 계측 비용

Ticks from ordered entry to last transfer sample (probe overhead included); maximum share uses this handling duration divided by the physical interval between consecutive settlement samples. This sample-to-sample denominator can be short on adjacent SVCs; it does not isolate net workload overhead. Terminal has no ERET, launch has no exception. Unlisted category combinations have zero observations. Separate stage brackets are in the original 47-word rows.

|Case|Source/type|N|min|max|total|max share|
|---|---|---:|---:|---:|---:|---:|
|child-leaf-plain|IRQ/idle-enter|121|1528|22832|307793|0.005974|
|child-leaf-plain|IRQ/idle-recover|122|2870|8944|523993|0.001713|
|child-leaf-plain|IRQ/switch|38|1801|25968|141493|0.013183|
|child-leaf-plain|SVC/idle-enter|1|12626|12626|12626|0.003692|
|child-leaf-plain|SVC/switch|1|2269|2269|2269|0.002114|
|child-leaf-trace|IRQ/idle-enter|121|1556|23445|378170|0.007776|
|child-leaf-trace|IRQ/idle-recover|122|2880|9648|519337|0.001721|
|child-leaf-trace|IRQ/switch|38|1957|25176|151391|0.012783|
|child-leaf-trace|SVC/idle-enter|1|11835|11835|11835|0.005744|
|child-leaf-trace|SVC/switch|1|2093|2093|2093|0.005094|
|saturated-plain|IRQ/idle-enter|42|1949|29340|167816|0.003753|
|saturated-plain|IRQ/idle-recover|43|2675|10022|178625|0.002583|
|saturated-plain|IRQ/switch|18|2054|29093|98547|0.007410|
|saturated-plain|SVC/idle-enter|1|2005|2005|2005|0.003059|
|saturated-plain|SVC/switch|1|13803|13803|13803|0.004196|
|saturated-trace|IRQ/idle-enter|40|1583|22451|150390|0.002871|
|saturated-trace|IRQ/idle-recover|41|3188|8494|183113|0.002192|
|saturated-trace|IRQ/switch|16|2400|24730|69900|0.006290|
|saturated-trace|SVC/idle-enter|1|2642|2642|2642|0.000826|
|saturated-trace|SVC/switch|1|16304|16304|16304|0.036486|
|single-plain|IRQ/idle-enter|11|1803|23928|50942|0.006085|
|single-plain|IRQ/idle-recover|11|2174|17285|43692|0.001478|
|single-trace|IRQ/idle-enter|11|2071|22850|51108|0.005813|
|single-trace|IRQ/idle-recover|11|2070|17052|46649|0.001457|
|swap-plain|IRQ/idle-enter|61|2078|22878|212601|0.002926|
|swap-plain|IRQ/idle-recover|62|2719|8067|247433|0.002078|
|swap-plain|IRQ/switch|6|2791|29197|46674|0.007435|
|swap-plain|SVC/idle-enter|1|12708|12708|12708|0.002466|
|swap-plain|SVC/switch|1|3392|3392|3392|0.002094|
|swap-trace|IRQ/idle-enter|61|1791|22181|234621|0.002837|
|swap-trace|IRQ/idle-recover|62|3161|11153|261608|0.002879|
|swap-trace|IRQ/switch|6|2887|27621|46394|0.007020|
|swap-trace|SVC/idle-enter|1|22171|22171|22171|0.004220|
|swap-trace|SVC/switch|1|4264|4264|4264|0.002605|
|wake-plain|IRQ/idle-enter|29|2151|24979|116739|0.003759|
|wake-plain|IRQ/idle-recover|30|2974|9316|131158|0.002391|
|wake-plain|IRQ/same|1|2346|2346|2346|0.002001|
|wake-plain|IRQ/switch|18|2271|9708|70584|0.007295|
|wake-plain|SVC/idle-enter|1|3893|3893|3893|0.001203|
|wake-plain|SVC/same|6|984|29814|39269|0.442837|
|wake-plain|SVC/switch|4|1391|13119|21496|1.145414|
|wake-trace|IRQ/idle-enter|29|2008|44485|138064|0.006695|
|wake-trace|IRQ/idle-recover|30|3210|12531|144569|0.003224|
|wake-trace|IRQ/same|1|2180|2180|2180|0.001847|
|wake-trace|IRQ/switch|19|2553|23235|88372|0.017053|
|wake-trace|SVC/idle-enter|1|2619|2619|2619|0.000674|
|wake-trace|SVC/same|6|619|31441|40740|0.429534|
|wake-trace|SVC/switch|4|1431|13593|22605|0.800293|

Historical controls / 기존 대조

All four linked ordinary/recurring firsts passed. Original diagnostic has 8 rows, 1 true idle recovery, max IRQ lag 19309 ticks. Recurring has 14 rows, 3 true idle recoveries, max 20043, two peer quota expirations and parent-0 debts 18558/9505/9274 in successive windows. Receipts 33152,33408,33664; cumulative 100224. Both independent linked frame/PC/ledger replay and supplied readers accepted. Default native CRC package returned 237 with exact product. Seven maintained arithmetic tests, upstream reader fixtures and the historical independent 10162 query comparator passed. These linked controls concern EL1 jobs, scoped to each original workload.

가상 단일 CPU에서 연결된 기존 일반 작업은 진단 8행, 반복 작업은 14행으로 원래 기준을 만족했다. 독립 정수 모형 10,162개 대조도 통과했다. 실제 하드웨어 시간이나 제품 부하의 보장은 이 검증 범위 밖이다.
