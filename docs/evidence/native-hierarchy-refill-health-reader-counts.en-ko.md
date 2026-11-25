# Third offline health-reader verification / 세 번째 오프라인 리더 검증

Candidate a242e441f90ecd60140a6d6076f48a992953c61e. Independent reader qualification remains incomplete. The six previously observed aggregate failures now reject with exit 2. Three directly contradictory sticky FIRST records remain accepted as valid/late. No guest runs or new timing measurements. 이전 여섯 요약 모순을 모두 거부했으나, 유휴 최초 위반의 시간·시작 상태 모순 세 건은 허용했다.

Frozen 12 hierarchy UARTs reproduced every pre-existing field, product exit vector and original status. Three legacy/default captures remain incomplete for health. All 1,506 possible genuine retained-prefix lengths across the 12 captures preserved timing, instrumentation, first, maximum and exits. Two changed but feasible omitted maxima were accepted with appropriately limited detail scope. Coherent faulty timer snapshot remained invalid/unqualified, while contradicting copies of the observation were rejected. Positive selector and maintained test logs archived. 원래 참인 접두부 1,506개를 모두 검증했고 실제 측정과 인위적으로 잘라낸 상세 행은 구별했다.

Genuine wake-trace first continues to report row 61, D=250487263, E=260476531, 9989268 ticks late relative to its programmed compare, 8036143 over the limit. Historical firsts 4201556 and 6487068 retain their original failed verdicts. No new claim about the interval or its cause.

|Offline control|Expected CLI|Observed CLI|Verdict|
|---|---|---:|---|
|child-leaf-plain|0|0|pass|
|child-leaf-trace|0|0|pass|
|default-plain|2|2|pass|
|original-saturated-plain|2|2|pass|
|original-swap-plain|2|2|pass|
|saturated-passive|0|0|pass|
|saturated-plain|0|0|pass|
|saturated-trace|0|0|pass|
|single-plain|0|0|pass|
|single-trace|0|0|pass|
|swap-passive|0|0|pass|
|swap-plain|0|0|pass|
|swap-trace|0|0|pass|
|wake-plain|0|0|pass|
|wake-trace|1|1|pass|
|original_false_terminal|2|2|pass|
|original_lost_wrong_arm|2|2|pass|
|impossible-unmask|1,2|2|pass|
|masked-interrupted-idle|1,2|2|pass|
|wrong-idle-DAIF|1,2|2|pass|
|unmatched-idle-landing|1,2|2|pass|
|unmatched-pre-cval|1,2|2|pass|
|spontaneous-wake|1,2|2|pass|
|inverted-entry-settlement|1,2|2|pass|
|wrong-arm-kind|1,2|2|pass|
|bad-entry-PC|1,2|2|pass|
|lost-previous-id|1,2|2|pass|
|valid-prefix-0|1|1|pass|
|valid-prefix-10|1|1|pass|
|valid-prefix-60|1|1|pass|
|valid-prefix-61|1|1|pass|
|valid-prefix-62|1|1|pass|
|valid-prefix-88|1|1|pass|
|missing-arm-id|2|2|pass|
|missing-pre-cval|2|2|pass|
|missing-event|2|2|pass|
|missing-landing|2|2|pass|
|missing-spsr|2|2|pass|
|only-late-impossible-maximum|2|2|pass|
|impossible-remaining-IRQ-count|2|2|pass|
|consistent-invalid-hardware-readback|1|1|pass|
|inconsistent-invalid-hardware-readback|2|2|pass|
|all-late-retained-wrong-max|2|2|pass|
|too-many-violations-after-FIRST|2|2|pass|
|final-omitted-terminal-counted-as-irq|2|2|pass|
|all-scored-retained-wrong-within-limit-max|2|2|pass|
|omitted-arm-before-retained-transfer|2|1|FAIL|
|omitted-arm-finishes-after-own-landing|2|1|FAIL|
|unknown-second-omitted-maximum|1|1|pass|
|unknown-regular-maximum|0|0|pass|
|startup-cannot-directly-arm-idle-first|2|1|FAIL|

The changed records in the three failures assert an omitted arm that began before an observed completed transfer, an arm that finished after its own idle landing, and a refill interruption directly following the necessarily active startup grant. All three can be disproved from surviving operands, row order and the published startup topology semantics. Conversely, with room for an additional unseen score of the right class there is no basis to replace the independent maximum with the retained maximum. Host fixtures establish reader behavior alone.

누락 부분 전체를 복원할 수는 없지만 보존된 이전 시각·최초 유휴 착지·최초 실행권 정보와 직접 모순되는 세 반례는 별도의 리더 실패다. 실행 파일 및 실제 장치 지연에 대한 추가 판정은 없다.
