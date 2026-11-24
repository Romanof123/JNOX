# Open-ticket audit, second independent review / 공개 티켓 감사 2차 독립 검증

**HOLD / 보류.** Candidate b679915183020b64eae721d781b9484b7a9105a2, baseline c31db86b83181e476ab28fcd243b2182c1e7a6c9, #191 and #193. A legitimate reference on a separate plain paragraph disappears when an earlier example contains a literal opening HTML tag. 앞 예시에 HTML 여는 태그를 표시하면 다음 일반 문단의 유효한 참조까지 사라집니다.

Independent host CLI, one technical slot, zero guests. The first private oracle covered 31 positions (22 bodies and nine roles). All matched. A second, source-informed oracle froze five more positions before execution. Two passed and three failed. The 36 positions and both independent expectation files are archived separately from the seven passing repository tests. 최초 독립 기대값 31건 일치, 소스 검토 후 추가 고정한 5건 중 3건 오거부, 저장소 회귀 시험 7건 통과.

For each failed position the PR's first paragraph consisted of either an inline code span displaying the opening code tag, a tilde-fenced HTML example containing an opening pre tag, or a blockquote containing an opening code tag. After a blank line the ordinary paragraph said “Tracks #6”. Issue #6 linked back to the PR. All three exited 1. HTML removal runs before code/quote isolation and consumes the remaining input. 인라인 코드, 물결표 울타리, 인용문 세 경우 모두 상호 링크가 있었지만 종료값은 1이었습니다. 제외 문맥 안의 HTML 태그가 뒤의 일반 문단에 영향을 주지 않도록 수정해야 합니다.

All 12 prior CLI inputs were reopened from the original archived HOLD, including both historical 40-item exports. Both archives replayed successfully. Release passed, and HTML examples and indented quotes were rejected as intended. The prior negation fixture remains a syntactically reciprocal mention requiring PM judgment. 기존 입력 12건을 재실행해 과거 세 결함의 수정 결과를 확인했습니다. 부정 표현의 의미와 지원 범위 밖 마크업은 PM이 판단합니다.

The live REST export on 2026-11-24 at 15:46 UTC followed the server's next link through 20 and 19 rows. The final response contained only a previous link. The archived input replayed with exit 0, 39 distinct open items, 30 issues, nine PRs and one recognized owner each. 수집 시점의 전체 두 페이지를 보존했습니다. 열린 항목 39건의 소유 역할과 PR 9건의 상호 참조를 확인했습니다.

| PR | Reciprocal mentions / 상호 참조 | Primary tracking issue from body and issue scope / 본문·범위 기준 주 이슈 |
|---|---|---|
| 126 | 123 | 123 |
| 133 | 99 | 99 |
| 135 | 99 | 99 |
| 138 | 123 | 123 |
| 153 | 123 | 123 |
| 172 | 21, 123 | 123 |
| 178 | 12, 21, 177, 184 | 177 |
| 191 | 6, 193 | 6 |
| 192 | 123 | 123 |

The inventory establishes mentions within the capture. The primary mapping is a separate reading of work scope, including #21 as a release gate and #193 as a correction under #6. Final ownership decisions remain with PM. 자동 도구는 보존한 본문의 상호 언급을 검사합니다. 표 오른쪽 열은 사람이 별도로 해석한 작업 범위이며 최종 책임 판정은 PM이 담당합니다.

Evidence includes raw inputs, REST page bodies and headers, CLI outputs, the first review archive contents, and both expectation files. Replay a captured inventory with Python and the candidate's tools/open_tracking.py. 원본 및 추가 입력과 출력은 증거 ZIP에 수록했습니다.

[Raw captures, frozen cases and replay / 원본·사전 입력·재생](open-tracking-second.zip) · SHA-256 0c07af2a78813737f3cc212f90e5fe88230c44ea3f9adccb1c7cea6ea4f84d3e.
