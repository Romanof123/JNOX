# Release matrix audit correction / 출시 근거표 추적 검사 수정

The first [#209](https://github.com/Romanof123/JNOX/pull/209) candidate `b129f1fde2bbe54ccf8521bdbd9b67bb1c407f85` exposed a missing POL-014 claim on the unchanged matrix. IMP-057 ran real LOW/HIGH protected-data handoffs on one emulated Cortex-A72. On `e716871c7c9e25ad1b3fa28dc19f73af2944862c` the three anchor cohorts covered 3/4, 2/4 and 4/4 first boots. Six traced boots correlated exception entries and returns. Restore the live-switch relationship for this record. Product blocking bounds and physical timing remain open. [Integrated report and archive](evidence/ordinary-mutex-anchor-integrated/README.md)

최초 후보는 기존 표에서 POL-014 연결 누락을 발견했다. IMP-057은 단일 에뮬레이션 코어에서 LOW/HIGH 사이의 보호 데이터 인계를 실행했다. 세 통합 집단은 각각 3/4, 2/4, 4/4였고 추적 실행 여섯 건에서 예외 진입·복귀를 대조했다. 해당 실행 근거에 문맥 전환 연결을 복구한다. 제품 차단 시간과 실기 판정은 미결이다.

For [#208](https://github.com/Romanof123/JNOX/issues/208), recognize the declared three-column table by its header and delimiter in either language. Read labelled EVD links in its recorded-subcases cell, treating an escaped pipe as cell content. Exclude fenced examples, detached row-like prose and the approval column. Retain the specific first candidate and independent findings for comparison.

한국어·영어 표에서 열 제목과 구분 행으로 지정한 근거표만 읽는다. 기존 근거 칸의 이스케이프된 세로줄은 같은 칸의 문자로 처리한다. 코드 블록 예시, 표 밖의 행 모양 문장, 남은 승인 칸은 근거 소유 판정에서 제외한다. 최초 후보와 독립 검증 결과를 비교용으로 보존한다.
