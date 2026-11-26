# Native permit expiry after confirmed input loss / 입력 폐쇄 후 실제 허가 만료

The fixed instruction-clock bench reproduced the exact protected decision pair in three cold boots per policy. A separate cohort reproduced the exact final activation-store pair in three cold boots per policy. All four selected cohorts exceeded the 5 ms maximum gap somewhere else in the same run.

고정 명령 클록에서 FIFO와 RR 각각 세 번의 콜드 부팅으로 실제 보호 판정 두 건의 정확한 시각을 재현했다. 별도 실행군에서 실제 활성 상태 해제 저장 두 건도 각 정책별 세 번 재현했다. 네 실행군 모두 전체 실행 중 다른 구간에서 5 ms 최대 완료 간격을 초과했다.

The third genuine permit carried scheduled issue I. Protected delivery followed I by 62,844–62,860 ticks across the four selected cohorts. Successful private CRC verification and protected COMMIT retained sequence 3. The controller replied with its accepted sequence to LAST_SAMPLE and the supervisor could reach CLOSE only after receiving 3. The subsequent producer acquisition returned EPIPE. No frame was delivered after closure. The virtual register changed 0→1 before closure and 1→0 only after the protected expiry latch. The terminal register readback was 0. The 25/5/30 ms intervals below use the real closure bracket, separately captured detection interval and atomic output-swap/readback bracket.

송신자가 정한 I와 실제 보호 전달 사이에는 62,844–62,860틱이 있었다. 수신자가 실제 바이트의 CRC를 확인하고 순번 3을 반영했다. 감독자는 LAST_SAMPLE 응답 3을 받은 뒤에만 CLOSE를 수행했다. 그 뒤 생산자 획득은 EPIPE였고 추가 전달은 없었다. 게이트 폐쇄 전 출력이 올라갔고 보호 래치 이후 내려갔다. 최종 출력 판독은 0이었다. 아래 응답 상한은 게이트 폐쇄, 래치, 실제 레지스터 변경과 판독 구간으로 계산했다.

|Engineering cohort|Inputs (i8/i9 leads, NOPs)|Targets at I+1,249,999 and +1,562,499|Selected actual retire gap|Run maximum retire gap upper|Maximum release to retire upper|25/5/30 ms upper ticks|
|---|---|---|---|---:|---:|---|
|FIFO decision v1|412/134|6,043,609 and 6,356,109|312,789–312,790|313,293|1,341|1,498,229 / 10 / 1,498,234|
|RR decision v1|441/141|6,043,625 and 6,356,125|312,811–312,812|313,316|1,393|1,498,141 / 10 / 1,498,146|
|FIFO completion v2|424/434, 0/0|6,043,646 and 6,356,146|312,500|313,292|1,341|1,497,928 / 10 / 1,497,933|
|RR completion v2|452/463, 2/0|6,043,661 and 6,356,161|312,500|313,317|1,393|1,497,820 / 10 / 1,497,825|

At 62.5 MHz, 5 ms is 312,500 ticks. On the decision runs, the final protected freshness operand in activation 8 was fresh exactly one tick before D. The next protected operand was activation 9's opening CHECK exactly H later, which detected expiry. All EVENT 12 sites, including commit checks 2, 258 and 514 and consume 4, are enumerated in each receipt. On the completion runs, EVENT 16 recorded equal endpoints around each genuine store. The intervening opening CHECK latched 300 ticks before the second FIFO retire or 322 ticks before the second RR retire. The private receiver first latched on its post-COMPLETE INFO at activation 8, after the target. This private state update did not drive the virtual register.

판정 실행에서는 i8의 마지막 보호 피연산자가 D 직전 한 틱에 유효했고, H 뒤 i9의 첫 CHECK에서 만료를 검출했다. 완료 실행에서는 각 실제 저장의 양끝 시각이 같았다. 두 저장 사이 i9 CHECK는 두 번째 저장보다 FIFO 300틱, RR 322틱 먼저 래치했다. 사설 모델은 i8 COMPLETE 후 INFO에서 이미 만료를 판정했으며 출력은 보호 경로에서 변경했다.

The disassembled function clears the real active byte. Two CNTPCT reads enclose ISB, STRB zero, DSB SY, LDRB of the same byte, DSB LD and ISB. The first sample precedes the store and the second follows its readback. The earlier v1 complete captures retained complementary one-tick uncertainty even across immediate integer-lead neighbors. The successful FIFO v2 result used the changed receiver image with both NOP counts zero. The RR result used two NOP iterations at activation 8.

실제 active 바이트의 0 저장과 동일 바이트 판독이 두 CNTPCT 읽기 사이에 있다. v1에서는 인접 정수 입력에서도 한 틱의 불확실성이 남았다. v2 FIFO의 NOP 인수는 모두 0이지만 수신 ELF가 바뀌었다. RR에서는 i8에 NOP 반복 두 번을 적용했다.

The original fixed-clock FIFO/RR phase controls latched 133/140 ticks late. Collision controls published a CRC-valid next frame at offsets [517,650]/[531,664] from D, with first latch operands +1,045/+1,082. Both collision supervisors exited 95 and neither run established ENABLE high. Operating-clock observations and all calibration misses remain in the row reports. Every measured run had zero safety-event losses. The independent scheduler journal dropped records, so complete scheduler timelines cannot be reconstructed. Simulated response observations do not establish physical-device latency or an operating-clock scheduling bound.

원래의 고정 클록 phase 대조군은 FIFO/RR 각각 133/140틱 늦었다. collision 대조군의 실제 게시는 모두 D 뒤였고 출력 활성화도 성립하지 않았다. 두 감독자의 종료 코드는 95였다. 운용 클록과 보정 중 빗나간 실행을 모두 보존했다. 안전 이벤트 유실은 0이었으나 별도의 스케줄러 기록에는 유실이 있었다. 물리 장치나 실운용 스케줄링의 보장은 이 가상 실험 범위를 벗어난다.

Both packets include the original capture hashes, complete synthetic guest memory represented by unique pages, native ELFs, all source selectors, full QEMU launch arguments, protected UART and per-boot private values. Detached extraction and replay reproduced 44 v1 and 15 v2 boots. The two tables below retain every signed deviation.

[Original captures and replay v1](native-permit-phase-first.tar.xz) (SHA-256 `b87f47d99067701deb1f99abc10099f96070aa0c1a24879e93909146567b4017`).

<details>
<summary>Per-boot results v1</summary>

# Closed-gate expiry and completion / 게이트 폐쇄 후 만료와 완료

One Cortex-A72, one CPU, QEMU virt and a 62.5 MHz guest counter. The operating-clock and single-thread instruction-clock launches are scored separately. Each row is a distinct cold boot. A tick is 16 ns. The instruction-clock arm uses shift 0. The virtual ENABLE was enabled by real recovery before loss in the measured phase cases.

단일 Cortex-A72 QEMU 가상 장치의 62.5 MHz 카운터로 각 콜드 부팅을 독립 판정했다. 실제 재무장으로 출력을 올린 뒤 게이트를 닫았다. 실행 클록과 고정 명령 클록을 구분한다. 한 틱은 16 ns이다.

I is the third accepted wire issue scheduled by the sender. EVENT 7 and the accepted COMMIT occur later. T8=I+1,250,000−1 and T9=T8+312,500. Decision columns give site 3 COMPLETE and the next site 1 CHECK operands minus T8/T9. Retire intervals give both counter endpoints minus those same targets. A [0,1] or [−1,0] observation leaves the equality unresolved. Release upper uses actual requested release to observed final active-store readback. Gap upper covers every consecutive successful retire.

I는 송신자가 정한 세 번째 허가의 발행 시각이다. 실제 전달·반영은 그 뒤였다. 판정 열은 i8 COMPLETE와 다음 i9 CHECK의 실제 피연산자 편차이고, 완료 열은 active 필드 저장 전후의 시각 편차이다. 구간이 0과 이웃 틱을 함께 포함하면 정확한 일치를 입증할 수 없다. 최대 간격은 전체 완료 쌍에 걸쳐 계산한다.

|Boot / 부팅|Inputs / 인수|Decision Δ8,Δ9|Retire Δ8 / Δ9|25/5/30 ms upper (ticks)|Release upper|Max gap upper|Safety / scheduler drops|
|---|---|---|---|---|---:|---:|---|
|fifo-phase-complete-count-first|00000 00000|[426,134]|[[435,436],[421,422]]|[1185864,10,1185869]|1341|313292|0 / 236|
|fifo-phase-complete-count-neighbor+1|00424 00435|[-11,-301]|[[0,0],[-1,0]]|[1497928,10,1497933]|1341|313292|0 / 236|
|fifo-phase-complete-count-neighbor-1|00424 00433|[-11,-299]|[[0,0],[1,2]]|[1497930,10,1497935]|1341|313292|0 / 236|
|fifo-phase-complete-count-repeat2|00424 00434|[-11,-300]|[[0,0],[0,1]]|[1497929,10,1497934]|1341|313292|0 / 236|
|fifo-phase-complete-count-repeat3|00424 00434|[-11,-300]|[[0,0],[0,1]]|[1497929,10,1497934]|1341|313292|0 / 236|
|fifo-phase-complete-count-s01|00435 00421|[-22,-287]|[[-11,-11],[13,14]]|[1497942,10,1497947]|1341|313292|0 / 236|
|fifo-phase-complete-count-s02|00424 00434|[-11,-300]|[[0,0],[0,1]]|[1497929,10,1497934]|1341|313292|0 / 236|
|fifo-phase-complete-default-first|00000 00000|[12213,7373]|[[12388,12392],[15165,15169]]|[1160189,1170,1160376]|153805|327344|0 / 234|
|fifo-phase-complete-default-selected1|12388 12500|[352,-4180]|[[1898,1903],[3347,3352]]|[1159663,1487,1159896]|161649|324434|0 / 233|
|fifo-phase-complete-default-selected2|12388 12500|[479,-3765]|[[2013,2018],[2931,2936]]|[1158548,1481,1158793]|152796|330817|0 / 233|
|fifo-phase-complete-default-selected3|12388 12500|[-4705,-2654]|[[-4667,-4663],[7312,7317]]|[1456681,1936,1456980]|156984|332136|0 / 233|
|fifo-phase-decision-count-first|00000 00000|[425,134]|[[434,435],[421,422]]|[1185863,10,1185868]|1341|313293|0 / 236|
|fifo-phase-decision-count-repeat2|00412 00134|[0,0]|[[11,11],[300,301]]|[1498229,10,1498234]|1341|313293|0 / 236|
|fifo-phase-decision-count-repeat3|00412 00134|[0,0]|[[11,11],[300,301]]|[1498229,10,1498234]|1341|313293|0 / 236|
|fifo-phase-decision-count-s01|00425 00134|[-13,0]|[[-2,-2],[300,301]]|[1498229,10,1498234]|1341|313293|0 / 236|
|fifo-phase-decision-count-s02|00412 00134|[0,0]|[[11,11],[300,301]]|[1498229,10,1498234]|1341|313293|0 / 236|
|fifo-phase-decision-default-first|00000 00000|[14901,8738]|[[15134,15139],[13451,13456]]|[1157901,1414,1158134]|158818|323072|0 / 234|
|fifo-phase-decision-default-selected1|12500 08738|[3994,-424]|[[5359,5364],[7244,7250]]|[1157789,1300,1158038]|165001|323126|0 / 234|
|fifo-phase-decision-default-selected2|12500 08738|[5792,-2776]|[[7075,7080],[1805,1808]]|[1162557,1227,1162796]|183155|327280|0 / 234|
|fifo-phase-decision-default-selected3|12500 08738|[7986,-126]|[[8182,8186],[4666,4672]]|[1152978,1544,1153237]|173192|324702|0 / 234|
|rr-phase-complete-count-first|00000 00000|[454,141]|[[463,463],[450,450]]|[1185783,10,1185788]|1393|313316|0 / 235|
|rr-phase-complete-count-neighbor+1|00452 00463|[-11,-322]|[[-1,0],[0,0]]|[1497820,10,1497825]|1393|313316|0 / 235|
|rr-phase-complete-count-neighbor-1|00450 00463|[-9,-322]|[[1,2],[0,0]]|[1497820,10,1497825]|1393|313316|0 / 235|
|rr-phase-complete-count-repeat2|00451 00463|[-10,-322]|[[0,1],[0,0]]|[1497820,10,1497825]|1393|313316|0 / 235|
|rr-phase-complete-count-repeat3|00451 00463|[-10,-322]|[[0,1],[0,0]]|[1497820,10,1497825]|1393|313316|0 / 235|
|rr-phase-complete-count-s01|00463 00450|[-22,-309]|[[-12,-11],[13,13]]|[1497833,10,1497838]|1393|313316|0 / 235|
|rr-phase-complete-count-s02|00451 00463|[-10,-322]|[[0,1],[0,0]]|[1497820,10,1497825]|1393|313316|0 / 235|
|rr-phase-complete-default-first|00000 00000|[12980,7292]|[[13177,13181],[13289,13295]]|[1164155,1222,1164368]|143920|329235|0 / 234|
|rr-phase-complete-default-selected1|12500 12500|[-847,-5042]|[[-805,-801],[4035,4040]]|[1459615,1546,1459865]|173759|325881|0 / 234|
|rr-phase-complete-default-selected2|12500 12500|[2513,362]|[[3836,3841],[6451,6456]]|[1157798,1275,1158065]|170171|327111|0 / 234|
|rr-phase-complete-default-selected3|12500 12500|[-1251,-5227]|[[-1199,-1195],[1994,1998]]|[1464310,1414,1464562]|167243|322959|0 / 234|
|rr-phase-decision-count-first|00000 00000|[454,141]|[[463,463],[450,450]]|[1185782,10,1185787]|1393|313316|0 / 235|
|rr-phase-decision-count-repeat2|00441 00141|[0,0]|[[10,11],[322,322]]|[1498141,10,1498146]|1393|313316|0 / 235|
|rr-phase-decision-count-repeat3|00441 00141|[0,0]|[[10,11],[322,322]]|[1498141,10,1498146]|1393|313316|0 / 235|
|rr-phase-decision-count-s01|00454 00141|[-13,0]|[[-3,-2],[322,322]]|[1498141,10,1498146]|1393|313316|0 / 235|
|rr-phase-decision-count-s02|00441 00141|[0,0]|[[10,11],[322,322]]|[1498141,10,1498146]|1393|313316|0 / 235|
|rr-phase-decision-default-first|00000 00000|[14280,7473]|[[14462,14466],[10806,10811]]|[1161556,1201,1161761]|162646|335559|0 / 233|
|rr-phase-decision-default-selected1|12500 07473|[-103,3210]|[[-66,-61],[11110,11115]]|[1472924,1596,1473185]|203723|323681|0 / 234|
|rr-phase-decision-default-selected2|12500 07473|[-1756,977]|[[-1723,-1719],[9853,9858]]|[1443813,1706,1444104]|164141|326553|0 / 234|
|rr-phase-decision-default-selected3|12500 07473|[6191,416]|[[7222,7227],[6157,6162]]|[1147833,995,1148033]|173276|334546|0 / 234|

## Original selectors / 원래 시험

- fifo-collision-count-first — next publication offsets [[517,650]], first latch operand minus D [1045], output enabled before expiry False.
- fifo-phase-count-first — next publication offsets [], first latch operand minus D [133], output enabled before expiry True.
- rr-collision-count-first — next publication offsets [[531,664]], first latch operand minus D [1082], output enabled before expiry False.
- rr-phase-count-first — next publication offsets [], first latch operand minus D [140], output enabled before expiry True.

## Replay / 재검증

Run `python3 scripts/replay.py .` with aarch64-linux-gnu-nm installed. The replayer restores each complete guest RAM image from deduplicated pages, verifies the capture hashes, walks each role’s page tables, checks the three ELF hashes and rescores the original UART. Each result.json contains all decision site IDs, individual private decision samples, the actual arm/close/fault/fall timestamps, selected gaps and signed intervals. Each retire.dis locates the two counter reads around the real store and readback. ELF files and VM RAM contain the synthetic fixture. The packet includes no host-memory dump.

재생기는 중복 제거한 가상 RAM 페이지를 복원하고 원본 캡처 해시와 ELF를 확인한다. 프로세스별 페이지 표를 따라 수신자 사설 시각과 프레임을 읽고 UART 판정을 반복한다. release 및 완료 간격과 실제 검출·차단·전체 응답의 상한은 개별적으로 확인해야 한다. 고정 클록의 일치는 실제 운용 스케줄링이나 물리 출력에 대한 보장이 아니다.

</details>

[Original captures and replay v2](native-permit-phase-refined.tar.xz) (SHA-256 `d4d1aaccc61b7ed18902c63e5c3002c988854f0c5ed9a67366f56882314100bc`).

<details>
<summary>Per-boot results v2</summary>

# Closed-gate expiry and completion / 게이트 폐쇄 후 만료와 완료

One Cortex-A72, one CPU, QEMU virt and a 62.5 MHz guest counter. The operating-clock and single-thread instruction-clock launches are scored separately. Each row is a distinct cold boot. A tick is 16 ns. The instruction-clock arm uses shift 0. The virtual ENABLE was enabled by real recovery before loss in the measured phase cases.

단일 Cortex-A72 QEMU 가상 장치의 62.5 MHz 카운터로 각 콜드 부팅을 독립 판정했다. 실제 재무장으로 출력을 올린 뒤 게이트를 닫았다. 실행 클록과 고정 명령 클록을 구분한다. 한 틱은 16 ns이다.

I is the third accepted wire issue scheduled by the sender. EVENT 7 and the accepted COMMIT occur later. T8=I+1,250,000−1 and T9=T8+312,500. Decision columns give site 3 COMPLETE and the next site 1 CHECK operands minus T8/T9. Retire intervals give both counter endpoints minus those same targets. A [0,1] or [−1,0] observation leaves the equality unresolved. Release upper uses actual requested release to observed final active-store readback. Gap upper covers every consecutive successful retire.

I는 송신자가 정한 세 번째 허가의 발행 시각이다. 실제 전달·반영은 그 뒤였다. 판정 열은 i8 COMPLETE와 다음 i9 CHECK의 실제 피연산자 편차이고, 완료 열은 active 필드 저장 전후의 시각 편차이다. 구간이 0과 이웃 틱을 함께 포함하면 정확한 일치를 입증할 수 없다. 최대 간격은 전체 완료 쌍에 걸쳐 계산한다.

|Boot / 부팅|Inputs / 인수|Decision Δ8,Δ9|Retire Δ8 / Δ9|25/5/30 ms upper (ticks)|Release upper|Max gap upper|Safety / scheduler drops|
|---|---|---|---|---|---:|---:|---|
|fifo-phase-complete-count-cal01|00435 00421 00 00|[-22,-287]|[[-11,-11],[13,13]]|[1497941,10,1497946]|1341|313292|0 / 236|
|fifo-phase-complete-count-cal02|00424 00434 00 00|[-11,-300]|[[0,0],[0,0]]|[1497928,10,1497933]|1341|313292|0 / 236|
|fifo-phase-complete-count-first|00000 00000 00 00|[426,134]|[[435,435],[421,421]]|[1185863,10,1185868]|1341|313292|0 / 236|
|fifo-phase-complete-count-repeat2|00424 00434 00 00|[-11,-300]|[[0,0],[0,0]]|[1497928,10,1497933]|1341|313292|0 / 236|
|fifo-phase-complete-count-repeat3|00424 00434 00 00|[-11,-300]|[[0,0],[0,0]]|[1497928,10,1497933]|1341|313292|0 / 236|
|fifo-phase-complete-default-first|00000 00000 00 00|[20274,8037]|[[20509,20515],[12730,12736]]|[1165884,1686,1166144]|161619|324988|0 / 234|
|rr-phase-complete-count-cal01|00463 00449 00 00|[-22,-308]|[[-12,-11],[14,14]]|[1497834,10,1497839]|1393|313317|0 / 235|
|rr-phase-complete-count-cal02|00451 00463 00 00|[-10,-322]|[[0,1],[0,0]]|[1497820,10,1497825]|1393|313317|0 / 235|
|rr-phase-complete-count-cal03|00451 00463 01 00|[-10,-322]|[[0,1],[0,0]]|[1497820,10,1497825]|1393|313317|0 / 235|
|rr-phase-complete-count-cal04|00451 00463 02 00|[-10,-322]|[[1,1],[0,0]]|[1497820,10,1497825]|1393|313317|0 / 235|
|rr-phase-complete-count-cal05|00452 00463 02 00|[-11,-322]|[[0,0],[0,0]]|[1497820,10,1497825]|1393|313317|0 / 235|
|rr-phase-complete-count-first|00000 00000 00 00|[453,140]|[[463,463],[449,449]]|[1185783,10,1185788]|1393|313317|0 / 235|
|rr-phase-complete-count-repeat2|00452 00463 02 00|[-11,-322]|[[0,0],[0,0]]|[1497820,10,1497825]|1393|313317|0 / 235|
|rr-phase-complete-count-repeat3|00452 00463 02 00|[-11,-322]|[[0,0],[0,0]]|[1497820,10,1497825]|1393|313317|0 / 235|
|rr-phase-complete-default-first|00000 00000 00 00|[14127,8923]|[[15815,15820],[13161,13166]]|[1167016,1315,1167236]|175903|321623|0 / 234|

## Original selectors / 원래 시험


## Replay / 재검증

Run `python3 scripts/replay.py .` with aarch64-linux-gnu-nm installed. The replayer restores each complete guest RAM image from deduplicated pages, verifies the capture hashes, walks each role’s page tables, checks the three ELF hashes and rescores the original UART. Each result.json contains all decision site IDs, individual private decision samples, the actual arm/close/fault/fall timestamps, selected gaps and signed intervals. Each retire.dis locates the two counter reads around the real store and readback. ELF files and VM RAM contain the synthetic fixture. The packet includes no host-memory dump.

재생기는 중복 제거한 가상 RAM 페이지를 복원하고 원본 캡처 해시와 ELF를 확인한다. 프로세스별 페이지 표를 따라 수신자 사설 시각과 프레임을 읽고 UART 판정을 반복한다. release 및 완료 간격과 실제 검출·차단·전체 응답의 상한은 개별적으로 확인해야 한다. 고정 클록의 일치는 실제 운용 스케줄링이나 물리 출력에 대한 보장이 아니다.

</details>
