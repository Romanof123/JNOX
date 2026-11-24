# Independent PM inventory review / PM 독립 검토

Public PR #245 head 1844e064469a7a80fdd813018eff1e9c79f70047, base be9b1cd61a1815422f9c0febdb767a6932e06bf1. Both public git objects and REST PR metadata agree. The report SHA-256 is a64637c5e19b9c0a384d0f527167c554191591aabde531f474384d8bfd5d37b9, the published ZIP is 9d96de9fbd6d5d17cbaab8f3b49f18c381dfe2f18f88090b16a8aefde62783c1 and saved JSON is 2dedd67bec6aa4ac1350f7c62dd05af5a57170ad8a465ed5bde4643d7fd5c47f.

## Result / 판정

Inventory and replay pass. Clarify #238's unit in the gate table. Seven of 20 ordinary retained B launches contained an over-limit window (7/230 windows). Diagnostic D separately had 7/230 across 20 launches. The whole matrix had 25/920 windows, all default-clock generation 2. Five fresh controls passed and nine historical controls replayed structurally, retaining a historical timing miss. #236's original feature cohort passed chain 2/2 and controls 4/4. Combined integration passed chain 2/2, plain PI/ceiling 2/2 and traced controls 0/2 (reason 16, zero chunks). No new physical measurement.

목록·재생 검증 통과. #238 표의 7/20에 실행 단위를 밝혀야 한다. 보존형 일반 B에서 최초 실행 20회 중 7회가 각각 한 번씩 시한을 초과했고 시간창 기준으로는 7/230이다. 진단 D도 별도로 7/230이다. #236은 기능 브랜치에서 대조군 4/4였으나 통합 시 일반 대조 2/2, 추적 대조 0/2였다.

Capture 2026-11-24 19:47:50 UTC / 11월 25일 04:47:50 KST. #245 opened at 19:49:53 UTC, 123 seconds later. Independent comparisons covered all 32 issue titles and reverse PR lists, all 49 team labels, all 17 primaries and archived update timestamps. Both lifecycle pages link to the head report. The archive contains normalized records plus one-page metadata, without the original HTTP pagination headers. Pagination history cannot be independently reconstructed from those bytes alone.

32개 이슈의 제목·역방향 목록, 49개 항목의 단일 담당, PR 17개의 주 이슈를 대조했다. 양 언어 수명주기 문서에서 기록이 연결된다. #245는 수집 123초 뒤 생성됐다. 보관본에는 원 HTTP 페이지 헤더가 없어 당시 페이지 순회의 독립 증명에는 한계가 있다.

## Manual primary issue review / 주 이슈 의미 대조

#126→123 follows reservation implementation #125 and the ordinary reservation work item #123. #133→99 defines the missing partition observations. #135→99 implements that plan. #138→123 implements #134 for that work item. #153→123 puts the #150 diagnostic on the reservation experiment. #172→123 is the integrated reservation candidate named in the issue; the PR assigns its ordinary workload gate to #123 and product admission to #21. #178→177 names the first independent verdict owner in Korean. #192→123 explicitly labels the parent and separately identifies integration and release. #201→123, #204→47, #207→49, #213→94, #218→217, #231→230, #236→232 and #238→233 explicitly declare the primary. #219→216 implements normal native P/Q there, separating parent release and native dependency. All corresponding issue bodies link back.

각 쌍은 본문의 과제 목적·주관 선언과 역링크를 함께 대조했다. #126·#172·#178 등 여러 번호를 언급하는 경우 구현 계획, 통합 후보, 제품 출시 게이트를 구분했다.

Documented replay from extracted raw/ against detached base reproduced 24 requirements, 18 policies, 58 numbered records, 1,335 unique directed typed edges, 337 endpoint nodes. EVD-091 through 094 resolve from the finite index. Public gate bodies still show 0/18 scheduling, 0/10 IPC, item/workload (#1), safety/FTTI (#10), physical calibrated equipment and synchronized captures (#34), and historical predicate/risk dispositions (#79/#110) open.

추출본에서 지정 리비전으로 재검산한 결과 요구사항 24개, 정책 18개, 번호 근거 58개, 고유 간선 1,335개, 노드 337개였다. 스케줄링 0/18과 IPC 0/10이며 제품·안전·실기 승인과 두 역사적 실패의 원인·위험 처분은 미결이다. 커널 빌드와 게스트 실행은 수행하지 않았다.

[Independent review of all named primary issues, owners, release gates and original evidence](pm-scheduling-inventory-review.zip) · SHA-256 5bdde90efe8845ea81ad89bffed822d78c988ef9ad4c34dc541933432f7fc710.
