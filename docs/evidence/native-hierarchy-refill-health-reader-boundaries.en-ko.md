# Second corrected reader — independent offline results / 두 번째 리더 독립 검증

Reader e78007940a44bbb8735c5931737985c4987455f1. Original evidence f5e72daa51fbff45481d2bd0984599b8305ad40f and first correction supplement 1f45a832. Six independent aggregate contradictions still accepted; offline reader qualification incomplete. No guest execution or new timing determination. 세 요약 요소인 최초 위반·최대 지연·IRQ 및 행 개수의 모순 여섯 건을 거부하지 못했다. 과거 관측 및 실측 시간은 그대로다.

All 12 frozen hierarchy UARTs agree field-for-field with original JSON, including independent exit-vector checks and their first/max evidence. Wake-trace first 61, fixed refill D=250487263 E=260476531, lag 9989268, complete/valid/late; other 11 within limit. The original workload product checks are unchanged in the frozen native report. All three legacy/default UARTs lacking a footer remain incomplete. Historical published misses 4201556 and 6487068 remain failed. Nine earlier overlap failures and the original two malformed controls now rejected. Valid crops at 0,10,60,61,62,88 preserve first/max/late. Five ordered selector controls and four maintained test groups passed. Coherently modeled anomalous entry CTL (host fixture only) was reported invalid/unqualified/1; contradictory sticky CTL was rejected/2. 각 단계의 실제 종료값, 최초·최대 지연 및 과거 패키지 산출물을 유지했다. 새 실행은 없다.

|Control|Expected CLI|Observed CLI|Verdict|
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
|only-late-impossible-maximum|2|1|FAIL|
|impossible-remaining-IRQ-count|2|1|FAIL|
|consistent-invalid-hardware-readback|1|1|pass|
|inconsistent-invalid-hardware-readback|2|2|pass|
|all-late-retained-wrong-max|2|1|FAIL|
|too-many-violations-after-FIRST|2|1|FAIL|
|final-omitted-terminal-counted-as-irq|2|1|FAIL|
|all-scored-retained-wrong-within-limit-max|2|0|FAIL|

The omitted-detail controls compare the remaining IRQ and decision capacities, including the mandatory terminal exit, possible violation positions after the first, and maxima that must already be known from retained scores/late rows or the unique first late observation. The coherent late report keeps CLI 1. A within-limit verdict with a fabricated maximum also passed as 0. Each failed control changes only an offline record, with explicit input and CLI output preserved. No guest, host scheduler or hardware causation follows from these replay controls.

실제 입력의 시간 실패와 리더의 오류는 별개다. 이전 리더의 최초 실패 결과는 원자료에 그대로 보전했다.
