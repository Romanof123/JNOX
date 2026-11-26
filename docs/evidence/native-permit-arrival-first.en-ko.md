# Permit expiry and arrival / 허가 만료와 수신

Twelve separate cold boots ran the three frozen application calendars under FIFO and RR on the ordinary and instruction-counted clocks. In every boot, the controller and authority confirmed sequence 3 with ENABLE high and gate open before staging. Sequence 4 retained its actual grant handle across staging, publication, acquire and release. The sender, protected snapshot and receiver agreed on all 44 bytes, the CRC and the frame semantics at D. Sequence 4 carried the scheduled E+39 ms issue. Its actual publication time is bounded by event 11.

고정 일정 세 가지를 일반 시계와 명령 수 기반 시계에서 FIFO·RR로 각각 한 번씩 실행했다. 12회 모두 제어기와 감독자는 순번 3에서 게이트 열림과 출력 1을 확인했다. 순번 4의 동일한 대여 핸들을 통해 송신 원본·커널 스냅샷·수신 원본 44바이트가 일치했고 CRC와 D에서의 유효성을 확인했다.

D equals the accepted third permit’s wire issue plus 1,250,000 ticks. The protected expiry operand was late in all 12 boots. For each before calendar, publication preceded D. For each same calendar, publication followed D and preceded the actual expiry operand. For each after calendar it followed both. Actual receive snapshots followed expiry in every case. In each boot the device journal recorded one rise followed by one fall during expiry, terminal ENABLE read back zero, the controller skipped COMMIT of sequence 4 and the authority observed the latched fault with gate open at E+48 and E+65 ms. Each boot had at least one inter-completion gap above 312,500 ticks (5 ms). Exact expiry at D and exact publication at D remain unobserved. These measurements apply to the frozen zero-adjustment invocations.

D는 수락된 순번 3의 발행 시각에서 1,250,000틱 뒤다. 모든 만료 판정은 D보다 늦었다. 게시 구간은 before에서 D 이전, same에서 D 이후이면서 실제 만료 이전, after에서 실제 만료 이후였다. 수신 스냅샷은 모두 실제 만료 이후다. 출력은 만료 처리 중 1에서 0으로 바뀌었고 순번 4는 수락되지 않았다. 두 감독자 관측에서 게이트 열림과 고장 래치가 유지됐다. 12회 모두 완료 간격이 5 ms를 초과한 구간이 있었다. 이 실행에서 D와 정확히 일치한 판정 또는 게시는 관측되지 않았다.

One tick is 16 ns. Signed deltas below are measured from D. The publication endpoints bound the successful ring operation. The receipt column uses the protected receive-snapshot journal timestamp. The last column bounds the largest inter-completion gap over the full captured boot.

1틱은 16 ns이다. 표의 차이는 D 기준이며 게시 열은 실제 게시의 하한·상한, 수신 열은 커널 스냅샷 기록 시각이다. 마지막 열은 해당 부팅의 최대 완료 간격 범위다.

| Clock | Policy | Calendar | Publication − D | Expiry − D | Receipt − D | Max gap (ticks) |
|---|---|---|---:|---:|---:|---:|
| default | fifo | arrival-before | [-49404, -49124] | +4033 | +8237 | [370273, 370640] |
| default | fifo | arrival-same | [6615, 7005] | +15159 | +18991 | [360808, 361817] |
| default | fifo | arrival-after | [71997, 72534] | +11623 | +389517 | [380134, 380824] |
| default | rr | arrival-before | [-40453, -40009] | +13132 | +19509 | [363721, 364274] |
| default | rr | arrival-same | [9799, 10313] | +25693 | +31941 | [353044, 353713] |
| default | rr | arrival-after | [70540, 70947] | +4197 | +387189 | [387367, 387994] |
| count0 | fifo | arrival-before | [-61684, -61551] | +134 | +364 | [374491, 374522] |
| count0 | fifo | arrival-same | [392, 524] | +919 | +1148 | [373706, 373738] |
| count0 | fifo | arrival-after | [62781, 62914] | +139 | +375351 | [375439, 375470] |
| count0 | rr | arrival-before | [-61660, -61528] | +141 | +379 | [374474, 374505] |
| count0 | rr | arrival-same | [405, 537] | +956 | +1194 | [373659, 373691] |
| count0 | rr | arrival-after | [62788, 62920] | +146 | +375366 | [375456, 375487] |

[Raw captures and offline replay](native-permit-arrival-first.tar.xz) (SHA-256 `de1c4e65cccf29c801d508f5b7729c700151d990ef96e76a9c854e2fece5892c`).
