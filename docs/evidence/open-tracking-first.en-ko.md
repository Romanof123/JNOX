# #191 · Independent verification / 독립 검증

**HOLD / 보류** · 5bae30bc7017d26582f86e1ce3b8c1b366835b0c · #6, REQ-012/013.

Two anonymous public REST reads returned the same 40 ordered IDs (29 issues, 11 PRs). The first used four pages of 10, following Link until no next relation remained. The second used one page of 100. Captured from 2026-11-24T15:36:43.795897+00:00. Each row had one lifecycle-compatible team label. Live CLI exit 0. Five supplied unit tests passed. Two reads establish an observation window. PM retains authority over tracking intent.

익명 공개 REST 목록을 두 번 수집해 열린 이슈 29건과 PR 11건의 ID가 모두 일치함을 확인했다. 각 항목의 담당 팀 표시는 한 개였다. 실제 목록과 단위 시험 5개는 통과했다. 주 담당 이슈의 의미와 승인 여부는 PM이 판단한다.

Primary issue by manual reading / 본문을 대조한 주 이슈: #126→#123, #133→#99, #135→#99, #138→#123, #153→#123, #172→#123, #178→#177, #186→#184 (follow-up #190), #189→#47, #191→#6, #192→#123. The CLI also reports parent, dependency and follow-up pairs for #172, #178 and #186.

| Case / 사례 | Expected / 기대 | Observed / 관찰 |
|---|---:|---:|
| Valid reciprocal pair / 상호 참조 | 0 | 0 |
| Lifecycle Release team / 배포팀 | 0 | 1 |
| Unknown owner / 미등록 팀 | 1 | 1 |
| Duplicate ID / 중복 번호 | 2 | 2 |
| Foreign repository, inline code, HTML comment / 외부 저장소·인라인 코드·주석 | 1 each | 1 each |
| Two spaces before a blockquote / 두 칸 들여쓴 인용 | 1 | 0 |
| HTML code example / HTML 코드 예시 | 1 | 0 |

Lifecycle role tables in both languages include Release. Its canonical team label is rejected. Quote handling misses ordinary indented Markdown quotes. HTML code examples count as handoffs. A negated sentence also passes lexical matching, illustrating the documented human intent review. Preserve these original results through correction and re-verification.

양쪽 생명주기 문서에 명시된 배포팀이 거부된다. 들여쓴 인용과 HTML 코드 예시가 실제 인계로 잘못 인정된다. 부정문도 번호 참조로 집계되므로 PM의 의미 검토가 필요하다. 수정 후 이 최초 결과를 보존하여 재검증해야 한다.

| ID | Type / 구분 | Team / 담당 팀 |
|---:|---|---|
| 1 | Issue | team:systems |
| 2 | Issue | team:systems |
| 3 | Issue | team:integration |
| 5 | Issue | team:systems |
| 6 | Issue | team:pm |
| 7 | Issue | team:systems |
| 8 | Issue | team:systems |
| 9 | Issue | team:verification |
| 10 | Issue | team:functional-safety |
| 11 | Issue | team:systems |
| 12 | Issue | team:integration |
| 21 | Issue | team:integration |
| 24 | Issue | team:pm |
| 34 | Issue | team:integration |
| 41 | Issue | team:verification |
| 47 | Issue | team:integration |
| 49 | Issue | team:development |
| 54 | Issue | team:development |
| 79 | Issue | team:development |
| 94 | Issue | team:development |
| 99 | Issue | team:development |
| 101 | Issue | team:development |
| 110 | Issue | team:development |
| 123 | Issue | team:integration |
| 126 | PR | team:integration |
| 133 | PR | team:integration |
| 135 | PR | team:integration |
| 137 | Issue | team:systems |
| 138 | PR | team:integration |
| 153 | PR | team:integration |
| 162 | Issue | team:systems |
| 172 | PR | team:integration |
| 177 | Issue | team:development |
| 178 | PR | team:integration |
| 184 | Issue | team:verification |
| 186 | PR | team:verification |
| 189 | PR | team:verification |
| 190 | Issue | team:verification |
| 191 | PR | team:verification |
| 192 | PR | team:verification |

[Captured inventory, expected outcomes and commands / 목록·사전 판정·검증 명령](open-tracking-first.zip) · SHA-256 ff22d737f4531ac7db268c7f664a9f59c187f3bad323675d29ea9c4833c715a9.
