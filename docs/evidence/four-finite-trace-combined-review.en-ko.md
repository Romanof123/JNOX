# JNOX #240 / #239 · Independent offline review / 독립 오프라인 검증

PASS · The public branch resolved to 211febe81eaba8c5a786b0f02b43a08bab20ce46, tree 1e01391d764381abd62deec95c59bc24d044cc9b, parents d8a2fe94e1316f85337879d6533b49b58b3314c3 and 7c84e293d4d91933cf93b55554acf8334dbf5702. Both canonical languages expose exactly the expected CHG and IMP tokens, original issue/PR, source revision and archive digest. Source bytes and capture ZIP SHA-256 values are in results.txt.

통과. 원본 이슈·PR·소스 리비전·캡처 해시와 한영 변경 및 구현 번호가 일치한다. 아래 표는 2026-11-25 사후 색인을 검증한 것으로, 새 실행 결과를 뜻하지 않는다.

| EVD / CHG | Origin | POL / IMP | Historical observation / 원본 관측 |
|---|---|---|---|
|091|#211/#214|003/091|Six virt UARTs show three jobs and 65542, three journals have nine events each. 게스트 6건과 진단 저널 3건.|
|092|#212/#215|016/038|14 recurring dispatch decisions, total 100224, 10162 host checks. 기존 IMP-038 재사용, 호스트 비교와 게스트 영수증을 구별했다.|
|093|#221/#223|018/093|Preserved ordinary capture and corrected replay show reserved slots 2/5/8 and restored PCs. 일반 작업 합계 62, 예약 작업 합계 9900.|
|094|#228/#229|013/094|Six stock-model CRC results after in-loop interruption. Both global runs resumed on CPU1, four others on CPU0. 기존 작은 작업 0/6 및 모델 한정 범위를 보존한다.|

All main edges survive, 1271 to 1335 unique triples (+64), 337 distinct nodes. REQ-002/005/023, DES-003/012, partial VER-002/005/023 and each policy reach the corresponding record. Pi 4 additionally traces REQ-008/DES-004/VER-008. Both release matrices preserve the complete main POL-015 row byte-for-byte and link all four indexed records. The requirement graph and scheduling evidence checker passed, zero findings, 18 focused tests passed. The earlier failed review remains historical. The index describes its first review at 9fe4170, whereas the retained first-review report explicitly identifies tested candidate 9c7e6aa. Consult the retained report for the measured revision.

기존 그래프 간선 1271개를 모두 보존하고 64개를 추가했다. 요구사항·설계·정책·부분 검증·근거 경로와 한영 행렬의 기존 AMC 측정 행을 확인했다. 검사기 두 개 및 집중 테스트 18개 통과. 최초 실패 검토의 대상 커밋은 색인의 9fe4170 표기와 보관 보고서의 9c7e6aa 표기가 서로 다르므로 실제 검토 대상은 원본 보고서를 참조한다.

Offline replay uses Python 3 from an exact source checkout with Git history available. Run python -B /path/to/replay.py /path/to/git/repository. The replay extends the archived first-review assertions with fresh bilingual token and post-merge matrix checks. No build, guest boot, deployed acceptance or physical test was performed. 재현에는 해당 소스와 과거 Git 객체가 필요하며 이번 작업은 기존 기록만 판독했다.
Both first-review commit IDs resolve to identical tree 401fef83e67465e9d347a07ec47e8e2912794c25. The discrepancy affects revision attribution only. 두 과거 커밋의 트리는 같아 검토 내용에는 차이가 없다.

[Complete corrected trace audit and reproducible records](four-finite-trace-combined-review.zip) · SHA-256 8c853babe8c242ab9a2eb1fade71e5153a48bb6d3ca9da570267addce4986541.
