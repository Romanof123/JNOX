# JNOX #245 final independent verification / 최종 독립 검증

PASS / 통과. Public main be9b1cd61a1815422f9c0febdb767a6932e06bf1 and PR/branch dedd993430d3455345cbc0cb3730a6786c8faa4f matched freshly queried Git refs. Detached base and head were separate. No builds or guest runs.

The immutable #238 report at 1ee5db1038f93eb0d630ccf4a6738460e8ab5cb4 records five P/default and two Q/default ordinary B launches with misses. Each had one missed window, so 7/20 launches and 7/230 windows. Diagnostic D independently missed 7/230 and the full matrix missed 25/920, all generation 2 under default clock. The final gate row represents those units correctly.

04:47:50 KST archive replay on pinned main matched its transcript and hash. It covered 32 issues, 17 reciprocal primary PR mappings, 24 requirements, 18 policies, 58 numbered records, 1,335 distinct typed edges and 337 endpoint nodes. Original response headers were unavailable, as the final text acknowledges.

04:54:01 KST separate archive contained HTTP 200, Date Tue, 24 Nov 2026 19:54:01 GMT, ETag, request ID and null Link. The single page ended pagination. SHA-256 of archived body fc7fd584bc090e9de4971a05d52733c3ae724e76040786542e1972c3c03ac1a2 matched metadata. Follow-up ZIP digest matched the report. Independent replay verified all 50 records each had one team, all 18 PR primary pairs appeared in the reciprocal checker output, and the 32 issues were accompanied by 18 PRs. Set comparison identified only added #245, linked to #6. Both README files point to #6. Both lifecycle documents resolve to the final report and its relative links resolve. The reviewed Markdown files decoded as UTF-8.

공개 브랜치·PR·기준 리비전을 확인했다. #238 보통 B는 실행 20회 중 7회에 한 구간씩 초과해 230구간 중 7구간이다. 진단 D도 별도로 7/230이고 전체는 25/920이다. 최초 보관본에는 원래 HTTP 헤더가 없으며 후속 수집은 별도 시점이다. 후속 보관본의 본문 해시·HTTP 200·다음 페이지 부재와 담당 팀·역방향 연결을 검증했다. 이슈 32개는 유지됐고 #6을 주 이슈로 하는 PR #245만 추가됐다. 양 언어 README와 수명주기 문서의 연결 및 기준 리비전의 추적 그래프 재생을 확인했다. 커널 빌드와 게스트 실행은 수행하지 않았다.

[Final replay of both inventories, their origins and the public tracking links](pm-scheduling-inventory-final.zip) · SHA-256 63ce5c012a823b65951ca6113ad3f0d68cc81ad25fc76aa02ae383e5edfd0cba.
