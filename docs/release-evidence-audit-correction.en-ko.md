# Release matrix audit correction / 출시 근거표 추적 검사 수정

The first [#209](https://github.com/Romanof123/JNOX/pull/209) candidate `b129f1fde2bbe54ccf8521bdbd9b67bb1c407f85` exposed a missing POL-014 claim on the unchanged matrix. IMP-057 ran real LOW/HIGH protected-data handoffs on one emulated Cortex-A72. On `e716871c7c9e25ad1b3fa28dc19f73af2944862c` the three anchor cohorts covered 3/4, 2/4 and 4/4 first boots. Six traced boots correlated exception entries and returns. Restore the live-switch relationship for this record. Product blocking bounds and physical timing remain open. [Integrated report and archive](evidence/ordinary-mutex-anchor-integrated/README.md)

최초 후보는 기존 표에서 POL-014 연결 누락을 발견했다. IMP-057은 단일 에뮬레이션 코어에서 LOW/HIGH 사이의 보호 데이터 인계를 실행했다. 세 통합 집단은 각각 3/4, 2/4, 4/4였고 추적 실행 여섯 건에서 예외 진입·복귀를 대조했다. 해당 실행 근거에 문맥 전환 연결을 복구한다. 제품 차단 시간과 실기 판정은 미결이다.

For [#208](https://github.com/Romanof123/JNOX/issues/208), recognize the declared three-column table by its header and delimiter in either language. Read labelled EVD links in its recorded-subcases cell, treating an escaped pipe as cell content. Exclude fenced examples, detached row-like prose and the approval column. Retain the specific first candidate and independent findings for comparison.

한국어·영어 표에서 열 제목과 구분 행으로 지정한 근거표만 읽는다. 기존 근거 칸의 이스케이프된 세로줄은 같은 칸의 문자로 처리한다. 코드 블록 예시, 표 밖의 행 모양 문장, 남은 승인 칸은 근거 소유 판정에서 제외한다. 최초 후보와 독립 검증 결과를 비교용으로 보존한다.

## Offline CommonMark parser / 오프라인 CommonMark 파서

At candidate `08219c9`, code spans, HTML comments and escaped opening brackets produced false claims, and a defined reference link was missed. Parse each whole language document with markdown-it-py’s CommonMark preset and table extension. Walk top-level tables bearing the declared headers. Within the recorded cell, count Markdown links whose label is a plain EVD number. Reference definitions can appear outside the table. The rule concerns local citation ownership. It does not resolve linked resources or establish release approval.

두 언어 문서 전체를 CommonMark와 표 확장으로 파싱한다. 지정한 최상위 표의 기존 근거 칸에서 일반 텍스트 EVD 번호가 붙은 실제 링크를 찾는다. 표 밖에서 정의한 참조 링크도 포함한다. 범위는 로컬 인용 소유 관계이며 대상 문서 검증 및 출시 승인은 별도다.

| Offline artifact / 오프라인 파일 | SHA-256 from official release metadata / 공식 배포 해시 |
| --- | --- |
| [markdown_it_py-3.0.0-py3-none-any.whl](https://pypi.org/pypi/markdown-it-py/3.0.0/json) | `355216845c60bd96232cd8d8c40e8f9765cc86f46880e43a8fd22dc1a1a8cab1` |
| [mdurl-0.1.2-py3-none-any.whl](https://pypi.org/pypi/mdurl/0.1.2/json) | `84008a41e51615a49fc9966191ff91509e3c40b939176e643fd50a5c2196b8f8` |

Version 3.0.0 supports Python 3.9 and requires only mdurl at runtime. Preserve the original pure-Python wheels including their MIT license files and the upstream markdown-it notice. Import them locally within the host checker. Exercise inline and defined reference links after escaped pipes, with controls for code, HTML comments, escaped brackets, images, unlinked text, approval columns and unrelated tables in both languages. Preserve the independently established mutex edge and prior failed revisions.

Python 3.9를 지원하는 3.0.0과 실행 의존성 mdurl만 보관한다. 원본 wheel 안의 MIT 라이선스와 원작 markdown-it 고지를 유지하고 호스트 검사기에서 로컬로 읽는다. 양 언어에서 이스케이프된 세로줄 뒤의 인라인·참조 링크, 코드·주석·이스케이프·이미지·일반 텍스트·승인 칸·다른 표 대조군을 확인한다. 기존 뮤텍스 전환 근거와 최초 실패 기록은 유지한다.
