# Independent structural verification / 독립 구조 검증

PASS at 9ad9917d620fed9ae25ec4a38764e71828478002 under the scope recorded at a106db797db961988eae79b2a01053a5aca051f5. Expectations were frozen from controlled sources before opening historical evidence. A separately implemented oracle found 80 declarations per language, 24 requirements, 124 distinct explicitly allocated triples and 135 covered CSV triples. All 81 CLI reports (full and every declared ID) matched exactly, including incoming/outgoing direction, relationship, CSV row and both declaration locations.

수정본에서 한영 각 80개 선언과 요구사항 24개를 확인했다. 양 언어에 명시된 고유 배정 124개가 일치했으며 CSV에는 검사 범위 내 연결 135개가 있었다. 전체 및 모든 ID 조회 81건에서 실제 방향·관계·CSV 행·한영 선언 위치까지 독립 계산과 일치했다.

CHG-075 affects REQ-002/005/008/023 retained the first records on original lines 1194–1197. Only repeated lines 1248–1251 disappeared. With the archived baseline CSV restored, the corrected tool reproduced the archived full stdout, stderr and nonzero exit with four duplicates.

CHG-075 최초 기록 1194–1197행이 유지되었고 반복된 1248–1251행만 삭제됐다. 이전 CSV로 재실행한 결과 전체 표준 출력·오류와 실패 종료 코드가 보존된 최초 기록과 일치했다.

All 28 isolated controls matched. Each of six classes received an extra reversed edge and a wrongly labelled edge. An unrelated VER-011 lookup still failed and named the CSV line, actual triple and expected triple. REQ-to-REQ failed. Five legal neighboring edges passed. Separate controls covered missing declarations in each language, one-language allocations in both editions, and loss of upstream, design or verification coverage. Duplicate, undefined endpoint and malformed CSV controls failed as expected.

격리한 대조 시험 28건이 기대 결과와 일치했다. 여섯 클래스 각각에 방향 또는 관계 오류를 추가하면 무관한 VER-011 조회에서도 실제·기대 연결과 오류 행이 보고됐다. 요구사항 간 간선은 거부하고 CHG·VER·OBJ 관련 인접 간선 5건은 허용했다. 한영 각각의 선언 누락과 배정 차이, 출처·설계·검증 연결 상실, 중복·미정의·형식 오류도 검출했다.

The typed-direction verdict covers REQ with OBJ/HAZ/THR/ASM/DES/VER and the REQ-to-REQ prohibition. Other classes retain existing graph rules. Translation meaning, allocation adequacy, implementation, test evidence, safety approval and release suitability require separate assessment.

방향·관계 판정 범위는 REQ와 OBJ/HAZ/THR/ASM/DES/VER 간 연결 및 REQ 간 연결 금지다. 다른 클래스에는 기존 그래프 규칙이 적용된다. 번역 의미, 배정의 적정성, 구현·시험 증거, 안전 승인 및 출시 적합성은 별도 평가 대상이다.

Replay in a checkout of the pinned source with Python 3. Use an empty output directory. / 고정 커밋 체크아웃에서 빈 출력 디렉터리를 지정한다.

    python replay.py PATH_TO_CHECKOUT OUTPUT_DIRECTORY

[Reproducible source joins, reports and controls / 원천 대조·원본 결과·검증 코드](requirement-graph-corrected.zip) · SHA-256 38a53e8c2fe7d8430f3faff90560b8f9f3a80a2d6d853521d3ab196362020177.
