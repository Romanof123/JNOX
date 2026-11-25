# Offline reader verification / 오프라인 리더 검증

Correction dfc6eb3d11c937878399f4c857039576f142b800 against the unchanged first evidence in f5e72daa51fbff45481d2bd0984599b8305ad40f. Offline only. No guest ran and no latency was remeasured. Independent verdict: the two initial reader contradictions are fixed; the corrected reader still accepts contradictory idle and runtime fields. Reader qualification remains incomplete. 원래 리더 반례 두 건은 수정됐다. 유휴 경계와 실행 기록의 모순을 허용하는 아홉 대조 때문에 리더 전체 판정은 보류한다.

All 12 captured hierarchy UARTs reproduced every pre-existing JSON field and the original status. Wake-trace remains late/1, decision 61, D=250487263 E=260476531, lag 9989268 ticks. The 11 others remain within_limit/0. Three comparators/default without a health footer correctly return incomplete/2. The published historical misses remain 4201556/6487068 ticks. The original app results, first ordering and raw observers retain their prior scope. 원래 실행의 산출물과 시계열 및 최초 초과 판정은 유지했다.

Archived false_terminal previously returned 0 and lost_wrong_arm returned 1; both now return 2. The old lost_wrong_arm fixture omits topology, so its rejection alone cannot demonstrate the identity fix. A new crop of the original real wake-trace with all topology retained independently rejects the wrong arm, event and pre-CVAL fields on its first omitted row. Six valid crops retain 0/10/60/61/62/88 rows respectively, with original sticky first and maximum. Complete recorded outputs show all results. Five selector positives through Hierarchy.decision cover idle tie, wake first, grid first, out-of-range horizon, exact in-range distance. The maintained three unittest groups passed separately.

|Control|Expected CLI|Observed CLI|Control verdict|
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
|impossible-unmask|1,2|0|FAIL|
|masked-interrupted-idle|1,2|0|FAIL|
|wrong-idle-DAIF|1,2|0|FAIL|
|unmatched-idle-landing|1,2|0|FAIL|
|unmatched-pre-cval|1,2|0|FAIL|
|spontaneous-wake|1,2|0|FAIL|
|inverted-entry-settlement|1,2|0|FAIL|
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
|missing-landing|2|1|FAIL|
|missing-spsr|2|1|FAIL|

The nine failures derive from independent single-field substitutions on the frozen bytes or their valid retained prefix. Each raw fixture and output is included. The first omitted row’s identity and previous selected arm are recoverable even when subsequent detail is omitted. The saved exception SPSR and masked last-before-unmask sample have distinct meanings. A cross-row mismatch, inverted stage/idle timestamps or a fabricated startup wake must not produce valid instrumentation; a real hardware discrepancy must remain reportable. No host attribution, clock calibration or release-bound claim follows from this reader evaluation.

각 반례는 기존 UART 또는 기존 행의 접두부에서 한 필드만 바꾼 호스트 대조다. 실제 게스트의 타이머 동작이나 과거 초과를 변경하지 않았다.
