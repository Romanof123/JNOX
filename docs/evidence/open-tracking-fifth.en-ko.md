# Independent fifth host audit / 제5차 독립 호스트 검증

Public branch pm/open-ticket-reconciliation was observed at 28ec55e55adaf1ceb58119860de222c0d7e73959. Its immediate parent c1be68083de8086107cfb078794b51cae0961192 records the prospective behavior in both lifecycle guides. The bounded checker passes this audit. 공식 릴리스와 의미상 주 이슈 승인 권한은 PM에 있습니다.

The first classification note was saved before candidate and prior result inspection. It defines classes but does not enumerate individual inputs. The exact 34-row oracle was saved after reading the previous report and parser, before running the candidate on those inputs. These 34 are independent execution checks with partial prior visibility, not blind discovery. 원칙을 먼저 고정했고 구체적인 34건은 기존 보고서와 검사기를 읽은 뒤 실제 시험 전에 고정했습니다.

All 34 fresh positions matched (24 body forms, 10 role inputs including every recognized role). The 120 retained positions matched the original intended exits (12 first, 36 second, 31 third, 38 fourth, 3 IPv6 probes). This includes the fourth audit’s two ambiguous-markup examples and the previously lost separate shorthand following an IPv6 URI. Five original REST graphs replayed successfully (40, 40, 39, 41, 38 records). All 12 current unit tests passed. 새 입력 34건, 기존 입력 120건, 원래 목록 5개와 저장소 시험 12건이 모두 기대값과 일치했습니다.

Live REST pagination returned 20 + 20 unique items, 30 issues and 10 PRs, November 24 2026 16:02:35–36 UTC. The final page had no next relation. All had one recognized next-action role. Live checker exit 0. PR descriptions and issue scope support the following primary assignments. 공개 목록에서 이슈 30건과 PR 10건을 확인했고 각 항목의 담당 팀 표시는 한 개였습니다. 본문과 이슈 범위에서 확인한 주 이슈는 아래와 같습니다.

| PR | Primary issue / 주 이슈 |
|---|---|
|126, 138, 153, 172, 192|123|
|133, 135|99|
|178|177|
|191|6|
|197|47|

Issue 193 tracks the checker corrections. Other reciprocal mentions, particularly those on 172, 178, 191 and 192, include parents, narrower follow-up work and admission gates. The parser accepts a plain local token even in “No relationship to #6”. Its output describes textual reciprocity. Negation, compound markup, ambiguous link intent and official release require human adjudication. Issue/PR state can also change between REST page reads. #193은 수정 작업을 추적합니다. 부정문에 쓰인 번호도 텍스트 참조로 집계됩니다. 복합 마크업 및 인계 의도, 주 이슈 확정과 공식 출시는 담당자의 판단 대상입니다.

Separate main-only check at 19015ff40af521aa209a4ef616cefa5b4d8b02f0: exit 0, 24 requirement reports. 별도 main 검사 종료 코드 0, 요구사항 보고 24건.

Reproduce with Python and requests, a checkout at the candidate SHA, and replay.py PATH_TO_CHECKOUT from an extracted copy. Source archives are tracked at that SHA in docs/evidence. The captured live inventory is retained for offline replay via python -B tools/open_tracking.py PATH_TO_INVENTORY. Re-running the collector records current public state. 격리한 후보 체크아웃을 replay.py에 전달하면 재실행할 수 있습니다. 공개 목록은 시점에 따라 달라집니다.

[Live and archived inventories, oracle and collected output / 실제·과거 목록 및 재생](open-tracking-fifth.zip) · SHA-256 579b42a306fba52ac93a7c55a01e361048d7ea35b7866e8f7b3bd5104f4fa14b.
