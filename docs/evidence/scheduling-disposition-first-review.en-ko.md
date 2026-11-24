# Independent release review / 독립 릴리스 검토

**Changes requested / 수정 필요.** Public docs/telemetry-hierarchy-dispositions at a88fb97f41f75a430e31adcff1011df44452ed27. Parent inventory baseline 7db361161f6943f5b330ba1d2b8de495295d538d. #222 tracks #220 for #6 and #21.

README.md line 13 calls the metric “threshold crossings”. The actual workload in kernel/src/batch_telemetry.rs lines 27–29 and 74 receives 18, 42, 43, 65535. Three samples are at least 42 and the series crosses upward once. Replace the words with “counted samples at or above 42”. The Korean README and both release rows describe the measured quantity correctly. 영문 README에서 임계값을 넘나든 횟수로 표현했으나 실제 결과는 42 이상인 표본 세 개다. 한국어 README 및 양쪽 근거표의 표현과 일치하도록 영문을 수정해야 한다.

Both matrices retain all earlier row evidence and remaining criteria against the archived main. All 18 release decisions stay open. Both READMEs link the correct evidence. POL-003 records six first launches and nine decisions per journal, three compatibility images and three supplemental journals. POL-016 records four candidate boots, dispatches 8/14, three windows, receipts totaling 100224, parent debts 20082/9875/8563, zero idle debit, maximum reaction 21377 and 10162 controls. POL-004 held single-decision qualification is 0/8. POL-008 integrated/absolute/prepared/mailbox/ordinary-frame results are 2/8, 3/8, 7/8, 7/8, 3/8 respectively. Final frame has two pre-publication and three worker-marker failures. Machine output records exact immutable source links, archive digests, and captured candidate revisions.

두 언어 표에서 이전 근거와 미결 기준을 보존했다. 정책 18개 모두 미승인이다. POL-004 정상 자격은 0/8이고 POL-008 다섯 코호트의 완주는 차례로 2/8, 3/8, 7/8, 7/8, 3/8이다. 실기나 게스트 재실행은 이번 문서 검토 범위 밖이었다.

Both lifecycle pages link the timestamped snapshot. Archive and raw JSON match published SHA-256. Checked 31 archived issues and 15 PRs with all 15 primary mappings and next PR owners, including #135→#99 team:systems and #222→#220 team:verification. Replayed reciprocal checker. Both static audits and 28 focused tests passed (6+10+12). 두 절차 문서의 동일 시각 자료와 원본 해시, 주 이슈 15건 및 담당 팀, 회귀 검사 28건을 확인했다.

## Reproduction

Run from the fresh checkout with the report directory at ../jnox222-proof, extracting the PM ZIP to ../jnox222-proof/pm. Run each command serially.

    git clone --depth 2 --branch docs/telemetry-hierarchy-dispositions https://github.com/Romanof123/JNOX.git
    git checkout a88fb97f41f75a430e31adcff1011df44452ed27
    git fetch --depth=1 origin 7db361161f6943f5b330ba1d2b8de495295d538d
    python -B ../jnox222-proof/verify.py
    python -B tools/open_tracking.py ../jnox222-proof/pm/open-items.json
    python -B tools/requirement_graph.py
    python -B tools/scheduling_evidence.py
    python -B -m unittest discover -s tools -p test_requirement_graph.py
    python -B -m unittest discover -s tools -p test_scheduling_evidence.py
    python -B -m unittest discover -s tools -p test_open_tracking.py

[Independent first documentation review / 최초 문서 독립 검토](scheduling-disposition-first-review.zip) · SHA-256 dca38095b64d957cee49df397bbc23780885a3b9bf0e001f4e21e75ba43ccc9d.
