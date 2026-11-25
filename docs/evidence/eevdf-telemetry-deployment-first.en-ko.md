# EVD-134 Independent Verification / 독립 검증

QUALIFIED for the adopted finite virtual telemetry workload. 채택한 단일 코어 가상 텔레메트리 구성에서 모든 수치 및 산출물 기준을 충족했다.

Candidate 6c22596f71e956c9db8b97548dc91de8d79e43cb, accepted main 93635a0dd59237f7c861c1db6117866d24bd95eb. Clean detached Linux checkout. The operator command ran once in private WSL ext4; build/capture/replay exited 0. UTC 05:32:13.059347547 through 05:32:15.515305483 on 2026-11-25. Operator elapsed 2.456 s, capture launch-to-close 0.687970 s. Cortex-A72, one CPU, virt/GICv2, PPI 30, QEMU 6.2.0 TCG, default clock, 128 MiB, Rust 1.95.0, 62.5 MHz, Q=625000. The three EL1 entries each use their own 16 KiB stack. 이전 캡처와 별도로 새 게스트를 한 번 실행했다.

ELF 4960853a65263c461ca126b9fcaaec5530c57a35806fe548f9785d140335a6e3; session 0f5c88026958d28e98835ad4a99f4869e7122c492dbc39fb9ea69db8aa365ffb.

| Start offset / 시작 (Q) | Delivered / 공급 (counts) | Signed errors / 작업별 오차 A, B, C (counts) |
|---:|---:|---|
| 0 | 19932823 | -186091/6, 2255945/12, -627921/4 |
| 8 | 19962444 | 3817325/6, -3444367/12, -1396761/4 |
| 16 | 19967678 | 8085281/30, 2299411/30, -1730782/5 |
| 24 | 19966223 | 3304847/6, -166447/3, -990651/2 |
| 32 | 19968349 | 10703491/15, -2631293/30, -6258563/10 |

| Wake / 기상 | Due | PPI acceptance | Task first work | Due-to-first | Wake-to-first |
|---:|---:|---:|---:|---:|---:|
| 1 | 5086268 | 5095012 | 5718990 | 632722 | 623978 |
| 2 | 10086268 | 10091468 | 10092814 | 6546 | 1346 |
| 3 | 15086268 | 15090100 | 15180862 | 94594 | 90762 |
| 4 | 20086268 | 20090808 | 20814210 | 727942 | 723402 |
| 5 | 25086268 | 25089872 | 25834844 | 748576 | 744972 |
| 6 | 30086268 | 30090376 | 30845620 | 759352 | 755244 |
| 7 | 35086268 | 35090737 | 35856075 | 769807 | 765338 |
| 8 | 40086268 | 40090061 | 40856658 | 770390 | 766597 |

Due-to-first p50/p95/max 727942/770390/770390 counts, wake-to-first 723402/766597/766597. Each window delivered at least 28Q, all 15 absolute errors and eight due-to-first samples were within 4Q. 다섯 창의 최소 공급량, 작업별 오차 15개 및 응답 8개가 각각 기준을 충족했다.

Rows 41→42 applied the fifth-wait change (B 1,Q to 2,2Q), reference 6877015, numerator 6877351→13754366, zero lag residual. All 84 restored PC/SP decisions, 73 IRQ, 11 SVC and 84 ERET events agree. Both preempted batch entries resumed and all tasks drained. 복귀 PC/SP와 예외 순서, 다섯 번째 대기의 설정 변경 및 세 작업 종료를 검증했다. See controls/current/pc-sp.json and controls/current/independent.json.

Product tuples (ID, count, sum, weighted sum) / 작업별 산출물

| ID | Count | Sum | Weighted sum |
|---:|---:|---:|---:|
| 2 | 128 | 27028 | 115128 |
| 0 | 134753344 | 3208814004 | 12835256170 |
| 1 | 96681280 | 2302222980 | 9208891742 |

EVD-004 retains controlled selection scope. EVD-023 independently reconstructed all twelve selections, the ineligible earliest deadline, live changes, the ELF task code and raw trace. Wake responses 2843013 and 3643095 counts, four and five service units. EVD-034 original verifier regenerated its recorded 128-grant/16-wake summary, peak absolute error 1.1/1.6/2.2Q, maximum 5 intervening grants and 3718362 counts. EVD-039 regenerated the 256-grant/20-wake summary, two accepted and two rejected state-preserving changes, maximum 266/60Q, 6 grants and 4599989 counts. Each used its original numerical limits. 기존 대조군은 각각의 원래 조건에서 재생했다.

Original #169 public archive 148e362e5e14c37e1d351df6180f10ae37a5fece35f7ef7ff7655e843c40c452 and member hashes verified. Original private session 68cca2efd813872eb02c6868fccbbe4e4e2ae2c22f76a602e4413c755bf0cf16 and normalized public session 02c1ac6316ebe22fa54d8b5b40f70f93ddeca356a8ab58fb9a65a56f67b656c6 both replayed identically on numerical outcomes (84 rows, due-to-first maximum 817052). Independently rebuilt current historic fixture variants and compared each entry, every PT_LOAD header and payload to #169-frozen variants. All three match; current telemetry also matches the original #169 loaded image. The four-file candidate diff from accepted main contains only the operator and documentation. 후보와 과거 적재 이미지를 비교했고 이전 세션의 응답과 신규 응답을 별도 기록했다.

Current offline replay returned 0; wrong digest INVALID/1; missing required capture input UNOBSERVED/2. Independent zero-fairness, zero-response, missing-response controls rejected on original and current traces. 오프라인 해시 오류와 파일 누락의 종료값은 각각 1과 2이며, 독립 검증기의 세 음성 대조도 양쪽 캡처에서 거부됐다. Exact argv, raw trace, hashes and results are in first-run/; independent results and replay sources in controls/. The independent audit script differs from the archived original only in its directory selection. From the extraction root, set up controls/current/first pointing to ../../first-run/capture before rerunning that independent script.

Physical hardware, sustained admission, independent EL0 applications and product safety require separate measurements. 실기기 지연, 지속 부하 보장, 독립 EL0 실행 및 제품 안전성은 별도 검증 범위다.
