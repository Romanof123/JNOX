# QEMU clock diagnostic / QEMU 시계 비교

Frozen c5ab397e93d345a4fef4420fe9cf7fa3e4be97ac. Replay order was FIFO healthy, FIFO overrun, RR healthy, RR overrun. Each ran a fresh default clock, then prespecified shifts 0 and 1 (1 and 2 virtual ns per guest instruction). All four original firsts are included. The same kernel bytes, package manifest, role ELFs, phase calendar, scheduler and 62500-tick limit were used within each position. Existing default captures lacked a contemporaneous monotonic host-duration record. New host ms spans QEMU process launch through termination, including UART/QMP terminal capture. Host times do not measure individual guest phases.

|Position / clock|Host ms|First A full response lo–hi|Job 4 A lo–hi|Max phase upper|COMPLETE gap upper|Overrun ticks / batches|Grid IRQ upper|
|---|---:|---:|---:|---:|---:|---:|---:|
|fifo-healthy-archived|—|122984–123068|14298–14371|123068|216396|0 / 0|23159|
|fifo-healthy-default|275.5|116380–116457|14529–14591|116457|225080|0 / 0|22207|
|fifo-healthy-0|191.7|692–710|694–712|1363|156927|0 / 0|75|
|fifo-healthy-1|186.1|1385–1421|1388–1425|2726|157603|0 / 0|150|
|fifo-overrun-archived|—|135989–136087|89796–89853|136087|216077|70004 / 61|25181|
|fifo-overrun-default|273.9|133897–133968|84520–84572|133968|217852|69381 / 75|37995|
|fifo-overrun-0|194.4|692–710|69542–69560|69560|225018|68816 / 611|75|
|fifo-overrun-1|192.0|1385–1421|70349–70386|70386|225051|68897 / 305|150|
|rr-healthy-archived|—|116629–116699|15587–15661|116699|172140|0 / 0|21660|
|rr-healthy-default|271.1|85050–85113|15658–15732|85113|171846|0 / 0|22240|
|rr-healthy-0|193.4|728–746|729–748|1406|156936|0 / 0|77|
|rr-healthy-1|191.7|1456–1493|1459–1496|2813|157622|0 / 0|155|
|rr-overrun-archived|—|120890–120963|83496–83548|120963|223995|69446 / 50|24880|
|rr-overrun-default|271.7|91313–91388|84467–84524|91388|209716|69683 / 54|24221|
|rr-overrun-0|191.3|728–746|69543–69562|69562|224979|68777 / 584|77|
|rr-overrun-1|191.8|1456–1493|70504–70541|70541|225124|68969 / 292|155|

## First empty receive / 첫 빈 수신

The first A phase had complete fixed-prefix coverage. Each row lists the stage-2 mark to syscall event offsets in actual issue order (63 CHECK, 35 receive, 61 state, 63 COMPLETE), followed by stage-3 mark and the two SVC63 entry-to-decision durations. All values are 16 ns architectural counter ticks. Consecutive offsets include mixed EL0/EL1 work. The fixed issue timestamp alone cannot isolate loan service cost.

|Healthy clock|Issue offsets|Stage 2→3|CHECK / COMPLETE|IRQ intersecting body|
|---|---|---:|---|---|
|fifo-healthy-archived|[2904, 66816, 80815, 83853]|93410|[55789, 2605]|[]|
|fifo-healthy-default|[2955, 58952, 72877, 75911]|84951|[47258, 2609]|[]|
|fifo-healthy-0|[93, 194, 288, 369]|468|[31, 38]|[]|
|fifo-healthy-1|[186, 389, 577, 738]|936|[63, 75]|[]|
|rr-healthy-archived|[3651, 62449, 75956, 78771]|87951|[51332, 2492]|[]|
|rr-healthy-default|[3159, 31369, 44887, 47580]|56328|[18902, 2425]|[]|
|rr-healthy-0|[98, 205, 304, 390]|494|[32, 38]|[]|
|rr-healthy-1|[196, 410, 607, 779]|987|[64, 75]|[]|

Both instruction-clock healthy runs per policy remained below the phase limit, while all original and fresh default healthy runs exceeded it. The programmed 1.1-ms loop still drove job 4 A beyond the same limit in every overrun run, with fewer loop batches as virtual time per instruction increased. The measured empty body scaled near 2× between shifts while host wall duration did not follow that scaling. This establishes clock-model sensitivity and a positive guest-work control. It does not partition the default-clock excess among translation, host scheduling, IRQ delivery or other emulator costs. No per-PC executed-instruction plugin was found installed. Counter deltas under icount describe virtual time, with timer/idle advancement possible. No instruction count is inferred from full release latency. Each boot exited 71/72/73 with 32 COMPLETEs. Results JSON includes every phase, actual COMPLETE brackets and IRQ intersections. Fixed journal coverage ends at its retained prefix. The diagnostic profiles do not change the original admission verdict.

동일 커널에서 기본 시계의 정상 사례는 모두 1 ms 위상 제한을 넘었다. 명령당 1·2 ns인 고정 시계에서는 정상 사례 모두 한도 안에 들었고, 의도적으로 1.1 ms 작업을 넣은 job 4 A는 두 시계 모두 한도를 넘었다. 빈 수신 본문의 단계별 시각은 명령 시계 배율을 따랐다. 기본 시계 초과분에서 호스트 스케줄링과 번역 비용이 각각 차지하는 몫은 분리할 수 없다. 실제 장치나 기존 승인 판정에는 적용하지 않는다.

[Raw captures and offline replay](native-permit-clock-comparison.tar.gz) (SHA-256 `4979284142447e02d3db460258eaba2f434bb400923993d2a6a67b87635e649f`).
