# Fourth corrected reader — independent offline verification / 네 번째 리더 검증

Candidate 15a25a539898a4501f35b0fc247e30fdbc2dcaa4. All earlier false acceptances in the preserved cohorts now reject. Two register-evidence controls still show impossible idle state accepted as valid. Reader qualification remains incomplete. No guest reruns or new latency measurement. 이전 반례는 모두 거부했지만 유휴 경계의 두 레지스터 모순을 정상으로 판독했다.

All 12 genuine health records preserved every original reported field and status; three legacy/default records returned incomplete. All 1,506 genuine complete/partial views preserved first/max/status/product exits. The known timing failure remains 9,989,268 ticks on wake-trace row 61, with true products and matching exits. Original 4,201,556 and 6,487,068 tick failures retain their dispositions. Hardware readback fault and malformed sticky copy remain distinguishable. Unknown intervening arm programming or masked landing is not reconstructed. A mapped prior CVAL or retained-arm landing mismatch rejects, while compatible changes beyond the observed prefix report the limited comparison. 실제 접두부 1,506개와 원래 산출물·시간 판정은 일치한다. 누락 구간의 관측되지 않은 값을 단정하지 않는다.

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
|only-late-impossible-maximum|2|2|pass|
|impossible-remaining-IRQ-count|2|2|pass|
|consistent-invalid-hardware-readback|1|1|pass|
|inconsistent-invalid-hardware-readback|2|2|pass|
|all-late-retained-wrong-max|2|2|pass|
|too-many-violations-after-FIRST|2|2|pass|
|final-omitted-terminal-counted-as-irq|2|2|pass|
|all-scored-retained-wrong-within-limit-max|2|2|pass|
|omitted-arm-before-retained-transfer|2|2|pass|
|omitted-arm-finishes-after-own-landing|2|2|pass|
|unknown-second-omitted-maximum|1|1|pass|
|unknown-regular-maximum|0|0|pass|
|startup-cannot-directly-arm-idle-first|2|2|pass|
|mapped-prior-cval-mismatch|2|2|pass|
|unknown-prior-cval-between-missing-arms|1|1|pass|
|mapped-landing-mismatch|2|2|pass|
|unmapped-landing-within-known-interval|1|1|pass|
|readback-reserved-ctl|2|2|pass|
|retained-interrupted-idle-fiq-mask-mismatch|1,2|0|FAIL|
|omitted-DAIF-reserved-bits|2|1|FAIL|

On the retained saturated idle IRQ row 3, the original masked pre-unmask DAIF and saved exception PSTATE agree on F; the offline fixture changes only saved SPSR.F. Between the DAIF read and the interrupted idle loop the assembly clears only I. The altered record still produces valid/within_limit/0. Separately, the valid 60-row prefix of wake-trace acquires only bit 0 in the raw MRS DAIF value of its sticky FIRST; that bit is reserved zero for this AArch64 register. It still produces valid/late/1. These two mutations corrupt corroborating register evidence, without changing E or D. The limits of pre-unmask and WFI timestamps and of host attribution remain those of the original native report.

두 변형은 서로 대조 가능한 마스크 상태 또는 예약 비트에만 영향을 준다. 측정된 게스트 카운터와 원래 시간 초과는 바뀌지 않았다.
