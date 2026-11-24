# JNOX #240 / #239 independent historical trace audit · 독립 사후 검증

**FAIL / 수정 필요** at 9c7e6aa1b83380b003ee421893e94da787efaec1. All four Korean canonical records attach 는 directly to CHG and IMP identifiers. Python regex word boundaries in tools/scheduling_evidence.py:96 then miss both identifiers. English resolves both in each case. The real repository audit exits 1 with exactly four findings. Marking identifiers as inline code before adding the particle should allow both languages to resolve identically. Graph checker passed and 18 focused graph, scheduling evidence and CRC stream tests passed.

한국어 네 문서에서 CHG 및 IMP 번호에 붙인 조사 ‘는’ 때문에 기존 검사기가 두 식별자를 모두 놓친다. 영어 문서는 정상이다. 실데이터 검사에서 불일치 네 건으로 종료 코드 1을 확인했다. 각 번호를 백틱으로 감싸고 뒤에 조사를 붙이면 검사기의 단어 경계를 보존할 수 있다.

Earlier graph 7c771bbc71f47dad32add2a4dd0a7b163f7ad7fb retained all edges. Candidate adds 64 unique triples. All four reach REQ-002/005/023 via DES-003/012, IMP, matching partial VER and EVD, and the Pi 4 case also reaches REQ/DES/VER-008/004/008. CHG and POL links resolve. IMP-038 is intentionally reused. EN/KO show the 2026-11-25 retrospective date, historical issue/PR, matching source and ZIP digests, and extant contract/report/archive links. results.txt records exact source revision and file SHA-256 plus capture ZIP SHA-256 per row.

기존 그래프의 모든 간선을 보존하며 추가 간선은 64개다. 요구사항·설계·구현·부분 검증·근거 경로, 변경 및 정책 경로, 계층형 사례의 기존 IMP-038 재사용을 확인했다. 양 언어에서 사후 색인 날짜와 각 원본 이슈·PR·소스·캡처를 대조했다.

| Evidence / 근거 | Historical cross-check / 과거 자료 대조 |
|---|---|
|091 · #211/#214|Six UART logs show three jobs, summary 65542, and the three diagnostic journals contain nine events apiece. 게스트 여섯 건, 호스트 파서 통제는 별도.|
|092 · #212/#215|Recurring ledger has 14 dispatches, both recurring UARTs reach 100224, archived host model records 10162 checks. 반복 실행의 디스패치·누적 영수증·호스트 검사 구분.|
|093 · #221/#223|Historic source gives sums 62 and 9900. Corrected crosscheck on preserved captures shows reserved slots 2/5/8 and restored PCs on six redispatches. 원본 캡처와 수정된 재생기 결과를 대조했다.|
|094 · #228/#229|Six stock-model logs contain final ordered CRC summary. Both global replays resume on CPU1, other four on CPU0, all qualify in-loop. 소형 워크로드 기존 0/6 판정 유지.|

Replay from the exact checkout using Python 3, Git and the local historical commit objects. Run python -B /path/to/replay.py. Its exit zero reproduces the documented failing candidate. This inspection reads retained logs, archived replay output and source. No builds or guest boots were run. Product acceptance, physical timing, migration and isolation remain open. 이번 검증은 보관 기록 판독이며 신규 게스트 부팅·빌드 및 제품·실기 승인은 범위 밖이다.

[First trace audit, archived inputs and independent checks](four-finite-trace-first-review.zip) · SHA-256 d97483d5e704a93854623cac02896b8b9f46ab0e4ec2df602fc60820f1045940.
