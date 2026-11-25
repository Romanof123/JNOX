# Corrected checker / 수정 검사기
78d3376f1376eee61851171d96e615b941596ef4. The 24 previously retained first-position qualification captures yielded byte-identical results to the original verifier. All 960 segment intervals, dispatch choices, source archive digests and booted image digests independently cross-checked. All runs retain FAIL. Worst-case admission remains unknown.

Eleven negative controls for captured/staged/embedded ELF integrity, UART, cross-paired kernel or launch revision/sender identity now returned 2 with zero segments. Correctly ordered optional peer copies retain the expected 40 segments; the originals with only embedded peers do too. An additional platform identity control still returned 1 and 40 segments after the receipted launcher CPU changed to cortex-a53. The authentic original captures used cortex-a72. This leaves platform selection as an open validation defect.

24개 원본의 960개 구간과 스케줄러 선택은 기존 결과와 일치했다. ELF·UART·커널·리비전 변형 11건은 모두 분석 전에 거부됐다. Cortex-A53으로 수정한 실행 메타데이터는 거부되지 않았다. 실제 캡처는 Cortex-A72에서 실행됐다. 모든 실측 실패와 분석적 수용 보류는 그대로다.

The original independent archive was D:/codex/permit389-admission-7cafd13/review.tar.gz, SHA-256 caae53761b568c72e2e18d6608f13d9413703f2ca8cf9717874d15ef76471250. Original raw captures reside in D:/codex/permit389-first-fbbb. Commands were python3 verify.py OUTPUT EXTRACTED_ROOT ORIGINAL_REVIEW ORIGINAL_CAPTURES and python3 audit.py OUTPUT EXTRACTED_ROOT ORIGINAL_CAPTURES, then binary comparison of both crosscheck.json files.

[Raw captures and offline replay](native-safety-admission-tool-second.tar.gz) (SHA-256 `919167326c3f247a41491757ac61e8b2ad7527d6dc46b0dbfadb76d48713885c`).
