# Independent #209 review / 독립 검증

Frozen cases are in cases.txt. First untouched head b129f1f returned 1 for POL-014/EVD-057. Base 07fe7ef3 returned 0. The new finding is genuine under prospective 34cdc87: both release rows cite EVD-057, POL-014 directly owns IMP-057, IMP-057 tested in EVD-057, and the typed policy edge is absent. The healthy-pristine expectation failed; this is not a false red. Removing POL-015/EVD-058 produced an old false green and a second, correct candidate finding.

최초 원본 검사에서 기준판은 통과했고 후보판은 POL-014/EVD-057 누락을 정확히 검출했다. POL-015/EVD-058까지 삭제하면 기준판은 여전히 통과하고 후보판은 두 누락을 모두 보고했다. 실제 기록에서 누락된 POL-014 연결 한 줄을 복사본에 보충하자, 18개 정책과 54개 근거를 담은 전체 JSON이 두 버전에서 동일했다.

With that disposable positive control, either language alone and a reordered ordinary row detected the targeted deletion. Context/shared, pending physical work, unrelated prose and wrong family yielded no finding or classification change. However an escaped vertical bar before the citation in the recorded cell caused a candidate false green (RC0); the escaped character belongs to the same Markdown cell. This affects parser placement, not a policy's evidence classification.

영문 단독·국문 단독 인용 및 행 순서 변경에서도 누락을 검출했다. 공유 근거, 실기 예정 항목, 일반 문장, 다른 정책의 인용은 분류를 바꾸지 않았다. 단, 근거 칸의 인용 앞에 이스케이프한 세로줄을 넣으면 후보판이 누락을 놓쳤다. 기존 검사 포함 일정 근거 단위 시험 8건과 요구사항 그래프 시험 6건이 통과했고 전체 요구사항 그래프는 종료값 0이었다. 변경은 두 문서와 검사기 및 그 시험 파일에 한정됐다.

Reproduce on a disposable clean clone containing the three revisions, with Python on PATH. Run python replay.py PATH_TO_CLONE then python control.py PATH_TO_CLONE. Both scripts switch commits and restore tracked fixtures. results.json retains the first outcomes; healthy-control.json contains the separate corrected-data control. No build, guest, board, or remote write.

[Original and candidate controls / 최초 검사와 후보 대조 자료](release-case-policy-link-first.zip) · SHA-256 efbe4feb2da63d033693e5e0b23d5eab75c9452711eeb48e0a966bb314730e4a.
