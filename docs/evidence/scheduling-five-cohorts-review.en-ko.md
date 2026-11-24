# PR 210 independent release verification / 독립 릴리스 검토
PASS for documentation on cb6b4f79f68e1ea557ba169d5b35b39acce24590 against 07fe7ef3c444a207f9ce4e013eff06aeb7b7cdc1. Both editions independently agree with the pinned reports and archives. Exactly five rows changed in each edition and all existing residual-gate cells match the parent.
한영 각 다섯 행의 문서 반영을 승인한다. 최초 실패와 별도 후속 실행 분모가 유지되며 제품 릴리스 범위와 잔여 기준은 동일하다.
* POL-007 #203 merged. Absent inputs denied, zero workload events. Two effective default captures truncated before a separate complete run. Two earlier QEMU invocations stopped before boot. 실제 승인 상한·기본 클록·M3 간섭·보드 WCET 미검증.
* POL-008 #201 held. Both prepared and mailbox first cohorts scored 7/8 separately. Position 1 measured 68,393 and 64,696 counter ticks respectively against 62,500 ticks (1 ms). Mailbox excess 2,196 ticks, before selection, zero worker returns. 8/8 및 배포 설정 격리 기준 미달.
* POL-011 #178 combined branch. Both share positions passed, wake/default lease 14 elapsed 890,546 ticks against H=781,250. Excess 109,296. Wake/icount passed, weighted total 3/4. Standalone startup excluded. 원인·통합 작업 조합의 허용 오차 검증 미완료.
* POL-013 #204 held. Stock QEMU six first positions across three modes, 0/6. M reached terminal verdict before an interrupted M PC or remote resume was observed. SGIR versus final add unresolved. 원격 루프 내 재개, 실물 이동·캐시 간섭 미검증.
* POL-014 #207 held. Default, Rust-prepared and Rust-stages each 4/4. Selected-return 3/4 with traced PI reason 16. 제품 상한·중첩 깊이·M3 최대 차단 미검증.

Extract and run python replay.py offline. Reports retain the original article and archive SHA-256 values. No guest launch or remote write.
압축 해제 후 python replay.py로 보관 자료를 오프라인 대조할 수 있다.

[Complete offline release matrix verification / 다섯 정책 출시표의 오프라인 검증 자료](scheduling-five-cohorts-review.zip) · SHA-256 9938280930d2a9a57d87d73458c13786eee14937f47b4a49d41cccd2018e0547.
