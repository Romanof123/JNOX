# Independent fourth host assessment / 제4차 독립 호스트 검증

Candidate 36eca868d7bd9c5b2f8fd868f9534365b8e2484b, prospective parent 1653496e21f5a9f88682792cbdf0fc7c395555e9. No guests or repository mutations. Both Python sources decoded strictly as UTF-8 and compiled, and all 11 shipped tests passed. First, second and third retained inputs matched all 79 expected exits (12 + 36 + 31). Independent 38-position oracle frozen before execution. Thirty-six matched, including the formerly failing parenthesized fragments, surrounding punctuation, roles, quote/code suppression and recovery in the following ordinary paragraph.

Mixed-markup limitation. A single foreign URL-shaped token containing a canonical autolink or Markdown link returned 0 in two fixtures, where 1 was expected. Both inputs contain literal brackets outside strict RFC URI syntax. They probe the documented unsupported-token handling, distinct from plain foreign URLs with ordinary parentheses, which passed. The second fixture's full PR body was https://else.invalid/doc([link](https://github.com/Romanof123/JNOX/issues/6)) and the issue referenced the PR. Extracting the Markdown destination synthesizes whitespace boundaries that the author never wrote. Unsupported markup must have a human review path. These examples do not establish a failure for simple standalone canonical Markdown links.

두 파이썬 파일은 UTF-8로 로드되고 저장소 시험 11건과 과거 입력 79건이 통과했습니다. 독립 기대값 38건 중 36건이 일치했습니다. 복합 마크업의 한계는 원래 공백이 없는 외부 URL 모양 토큰 안의 Markdown 링크 또는 자동 링크를 상호 참조로 승인한 두 건입니다. 두 토큰은 엄격한 URI 문법 밖에 있는 리터럴 괄호 문자를 포함합니다. 일반 외부 URL의 괄호·조각과 별도로 작성한 단축 참조 처리는 해당 입력에서 통과했습니다.

The public REST capture at 2026-11-24 15:56:24–25 UTC returned 20 + 18 open records, 29 issues and 9 PRs, unique IDs, with no further next page. All carried one recognized team. Both bodies in each primary pair reference the counterpart. Semantic review of PR purpose and issue descriptions produced the following ownership. PR 191 explicitly calls issue 6 its parent and issue 193 owns the corrections. The release gate 21 and other cross-references accompanying 172, 178 and 192 have a different purpose from their primary issue.

공개 REST 두 페이지에서 열린 이슈 29건과 PR 9건을 확인했습니다. 주 추적 이슈는 각 PR과 이슈의 본문에서 양방향으로 확인하고 범위를 대조했습니다. 시점과 공개 접근 권한에 한정한 결과입니다. 표의 팀은 해당 PR의 다음 조치 담당입니다.

| PR | Team / 담당 | Reciprocal / 상호 언급 | Primary / 주 이슈 |
|---|---|---|---|
|126|integration|123|123|
|133|integration|99|99|
|135|integration|99|99|
|138|integration|123|123|
|153|integration|123|123|
|172|integration|21, 123|123|
|178|integration|12, 21, 177, 184|177|
|191|verification|6, 193|6|
|192|development|21, 123|123|

**HOLD on separately written plain references.** The valid IPv6 URI followed by a space and a plain shorthand, `https://[2001:db8::1]/guide(a)#6 #6`, returned 1 instead of 0. The URI alone correctly returned 1. The equivalent DNS-host URI followed by space and shorthand correctly returned 0. Brackets are required by URI syntax around the IPv6 literal. They are not unresolved Markdown in this case. The last shorthand is unambiguously separate, as required in the prospective agreement. Filtering the whole line drops this evidence. Retain the following ordinary token when skipping valid URI tokens. In contrast, both mixed-markup strings actually contain canonical Markdown destinations and can legitimately require PM adjudication. Their outcomes are not independently decisive.

**보류.** 올바른 IPv6 URL 뒤에 공백으로 분리된 일반 참조를 붙이면 같은 줄 전체를 제외하여 유효한 상호 참조가 누락됩니다. 위 입력의 기대값은 0, 실행값은 1입니다. URL 단독과 DNS 이름 대조군은 기대대로 동작했습니다. 이 경우의 대괄호는 IPv6 호스트 표기에 필수입니다. 앞의 복합 마크업 두 사례는 실제 정규 링크 목적지를 포함하므로 수동 판정 범위로 구별합니다.

Finite coverage only. Compound or ambiguous Markdown, link intent and negation require PM inspection. The collector followed the final page link. GitHub can change between page requests.

[Raw cases, capture and independent replay / 원본·목록·재생](open-tracking-fourth.zip) · SHA-256 e6a2c48a7c12f2b6abee8df8bb75fe555c7dd69bce08ecb5cc9a16df6ab25c1a.
