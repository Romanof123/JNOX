# Independent host trace review / 독립 호스트 추적성 검토

Public revision 86b044666fd94fa465847cdb1c3989fe44559344, contract 38839ad0406a7ad109858ca3b089866d818f9c3e. Host Python CLI only. All 24 requirement blocks and their typed CSV triples, English/Korean source locations, and all 80 declaration queries matched the document-derived oracle. VER-011 returns REQ-011 and REQ-021 (CSV 75, 150). Repeating the original full run reproduced stdout, stderr and exit 1.

공개 리비전에서 요구사항 24건과 선언 ID 80건의 조회 결과 및 양언어 원문 행이 기대값과 일치했다. VER-011 역조회는 REQ-011과 REQ-021을 반환한다. 전체 실행을 반복해 같은 출력과 종료 코드 1을 확인했다.

Four original duplicate CHG-075 affects edges at CSV 1248–1251 repeat 1194–1197 (REQ-002/005/008/023). Removing only the repeated rows from a temporary fixture yields exit 0. The checker detects disconnected upstream coverage, unrelated undefined endpoints even under filters, duplicate and malformed CSV, missing or repeated declarations in either language and wrong-kind Markdown allocations. Two additional invalid CSV triples (wrong relation/target pairing, reversed direction) pass silently on the clean fixture. The contract specifies display of typed links and does not explicitly require rejection of these extras. Broad typed-CSV validation is therefore unproven.

원본 CSV 1248–1251행은 1194–1197행의 CHG-075 연결 네 건을 중복한다. 임시 복제본에서 중복만 제거하면 종료 코드가 0이다. 한영 문서의 잘못된 배정은 검출했으나 CSV에 추가한 관계·대상 유형 오류와 방향 오류는 진단 없이 통과했다. 계약은 이런 추가 연결의 거부를 명시하지 않는다. 제품 동작이나 안전 승인은 검토 범위 밖이다.

Freeze order: independent declarations, allocations and per-REQ CSV rows were saved in frozen.json before reading checker results. The freeze script checked missing allocations and undefined endpoints, but did not separately classify duplicate triples. These were independently recounted from raw CSV after the first checker output. The candidate checkout was read only. No product build or guest execution.

Reproduce from a clean checkout of the pinned public commit. Create a fresh output directory, then run:

    python oracle.py PATH_TO_CHECKOUT OUTPUT_DIR
    python check.py PATH_TO_CHECKOUT OUTPUT_DIR

queries.json preserves full stdout, stderr and exit code for each query. The empty key denotes the full run. mutations.json contains all controls. Expected output comparison checks block headers and typed triples with line locations independent of ordering.

[Frozen source joins, 81 queries and regression evidence / 사전 원천 대조·조회 81건·회귀 시험](requirement-graph-first.zip) · SHA-256 6222639874e546b96d048cfb14dd960be2bb49cfcaf915e58f81cba594e09407.
