# Third independent open-ticket audit / 공개 티켓 3차 독립 검증

**HOLD / 보류** · Candidate eccd137033bfaa798374146d75e0d3ffc1cb69e3 · plan 1e58656c333d1770d1d6eabd4e08b807ecbf5727 · PR #191 / correction #193.

A bare foreign URL can create a reciprocal link. In the two-row fixture, PR #193 mentions issue #191. Issue #191's entire body is `https://github.com/other/JNOX/wiki/Related(#193)`. Expected exit 1. Actual exit 0 reports PR #193 reciprocates #191. The same occurred with `https://example.org/guide(#193)`. Both include the number inside a foreign URL. Angle-bracket and labeled-link counterparts were rejected. This can approve an untracked PR and exceeds the documented manual-review allowance for ambiguous markup. Consume or discard the entire foreign URL including balanced parentheses before checking bare references. Preserve legitimate prose such as “Tracks (#193)”. Rerun all four cases and the existing controls.

외부 URL만 적힌 이슈 본문이 상호 인계 증거로 잘못 승인됩니다. URL 처리기가 여는 괄호에서 멈춘 뒤 주소 안 번호를 독립 참조로 읽습니다. 자동 검사에서 추적 누락을 놓칠 수 있습니다. 괄호를 포함한 외부 URL 전체를 제외하고 일반 문장의 괄호 참조는 유지한 뒤 재검증해야 합니다.

Before opening candidate source or results, the independent reviewer froze 27 positions covering ordinary references, valid paragraphs after excluded examples, multiline HTML, unsupported references, seven recognized roles and a spurious role. All matched. All 12 first-review inputs and all 36 second-review inputs matched their corrected expectations, including three previously lost legitimate paragraphs. Nine repository tests passed. The four subsequent foreign-URL expectations were frozen after source review and before their CLI execution. Two exposed false links. One host slot, no guests. Results concern the supported syntax and captured inventory. PM interprets scope and negation.

후보 구현과 결과를 읽기 전에 고정한 독립 입력 27건, 과거 독립 입력 48건, 저장소 시험 9건은 모두 통과했습니다. 앞 예시 때문에 다음 문단이 누락되던 세 사례는 수정됐습니다. 소스 확인 후 실행 전에 별도 고정한 외부 URL 4건 중 2건에서 거짓 연결이 발생했습니다. 호스트 검증 슬롯 한 개를 사용했고 게스트는 실행하지 않았습니다. 부정문과 실제 담당 범위는 PM이 판단합니다.

At 15:51 UTC on November 24, 2026, public REST pagination returned 20 + 20 + 1 unique open items with no next link on the final page. All 31 issues and 10 PRs carried exactly one recognized team label. Every PR contained at least one reciprocal mention. Primary tracking was assessed separately using PR descriptions and issue scope. Capture time and permissions bound this inventory.

2026년 11월 25일 00:51 한국 시각 공개 API 전체 페이지에서 열린 이슈 31건과 PR 10건을 확인했습니다. 각 항목에는 유효한 담당 팀 하나가 있었고 모든 PR에 상호 언급이 있었습니다. 주 추적 이슈는 본문과 업무 범위로 따로 판정했습니다.

| PR | Reciprocal mentions / 상호 언급 | Primary / 주 추적 |
|---|---|---|
|126|123|123|
|133|99|99|
|135|99|99|
|138|123|123|
|153|123|123|
|172|21, 123|123|
|178|12, 21, 177, 184|177|
|191|6, 193|6|
|192|21, 123|123|
|195|6, 194, 196|194|

Replay any input with `python tools/open_tracking.py <input.json>` from the specified candidate. The inventory, response navigation links, frozen inputs and actual outputs are included. 지정한 후보 버전에서 같은 명령으로 보존된 입력을 다시 실행할 수 있습니다.

[Raw inventory and frozen examples / 전체 목록 및 사전 입력](open-tracking-third.zip) · SHA-256 53960a9eb2e28be7aaa957bfa2667817f9c8a27c37896015f4b751693824682a.
