# System requirement graph contract (#194)

Both editions supply declarations under `docs/`. For each basename below use
`.en.md` (English) and `.md` (Korean). OBJ in `product-plan`, HAZ/THR/ASM in
`safety-security-concept`, REQ in `requirements` and VER in `verification-plan`
are the backtick-quoted IDs at the beginning of the first table cell. REQ rows
also have a name. DES in `architecture` is the ID starting a level-two heading.
IDs have three decimal digits. References in prose, acceptance criteria, and
later table cells cannot introduce IDs.

Each REQ row's second cell lists sources and its fourth lists design and plan
IDs. Every listed allocation must appear in `trace/links.csv`. Additional
typed CSV links are shown. OBJ/HAZ/THR derive requirements (`derives`), and
ASM limits them (`constrains`). A requirement allocates behavior to DES
(`implemented-by-design`) and allocates planned verification to VER
(`verified-by`). Product evidence and safety approval require separate records.

Run `python tools/requirement_graph.py` in a pinned checkout. An optional
positional ID selects incident requirement links in either direction. Show each
REQ, actual directed triples, CSV lines and bilingual declaration locations.
Filtering changes display only. Exit nonzero for missing input, malformed CSV,
duplicate triples, undefined covered endpoints anywhere in the CSV, duplicate or
different declarations between editions, different REQ allocations, missing
listed allocations, or a REQ lacking any upstream, design or plan connection.
A structurally complete graph says nothing about translation quality.
Retain the first result for independent disposition under #6.

## 시스템 요구사항 그래프 계약

`docs/`에서 같은 이름의 영어판(`.en.md`)과 한국어판(`.md`)을 읽는다.
`product-plan`의 OBJ, `safety-security-concept`의 HAZ/THR/ASM,
`requirements`의 REQ, `verification-plan`의 VER는 표 첫 칸 처음에
백틱으로 묶인 세 자리 번호로 선언한다. REQ에는 이름도 붙인다.
`architecture`의 DES는 2단계 제목 첫 ID로 선언한다. 본문이나 인수 기준,
다른 표 칸에 등장하는 번호는 선언에서 제외한다.

REQ의 둘째 칸에 적은 출처 및 넷째 칸의 설계·검증 배정은 모두
`trace/links.csv`에 있어야 한다. CSV에 추가로 명시한 유효한 연결도
표시한다. OBJ/HAZ/THR의 `derives`는 요구사항의 도출 근거, ASM의
`constrains`는 적용 제약이다. REQ에서 DES로 가는 `implemented-by-design`은
설계 배정, VER로 가는 `verified-by`는 검증 계획 배정이다. 제품 증거와
안전 승인은 별도 기록으로 판단한다.

고정한 체크아웃에서 `python tools/requirement_graph.py`를 실행한다. 선택
인자로 ID를 주면 전체 검사 후 해당 ID와 연결된 요구사항 간선만 표시한다.
요구사항별로 실제 방향과 관계, CSV 행, 한영 선언 파일과 행을 출력한다.
입력 누락, CSV 형식 오류, 중복 간선, CSV 전체의 미정의 대상 클래스 ID,
선언 중복이나 한영 선언·배정 차이, 명시한 배정 누락, 출처·설계·계획 중
하나라도 없는 요구사항은 실패로 처리한다. 번역의 의미 일치는 사람이
판단한다. 최초 결과를 보존하여 #6에서 독립 판정한다.
