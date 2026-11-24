# Independent release audit verification / 출시 근거 검사기 독립 검증

Candidate / 후보 bc5732ab0c7613194169453e632d1c38c6079db4
Contract / 기준 261bd49815b05ab50018fd2149e03ef8e8432db5

## English

PASS for the documented host checker. On Windows build 26200, CPython 3.9.16 AMD64, the intact catalog returned 0 with all 18 policies and zero findings. Removing only the AMC edge returned 1 with the single POL-015/EVD-058 finding. The mutex claim belongs to the live-switch class; deleting it produced the single POL-014/EVD-057 finding.

For each language independently, 16 tests modified the real release table while the counterpart carried neutral text. Inline links after escaped pipes and defined full, collapsed and shortcut references produced exactly the intended finding. Undefined references, code, comments, escaped brackets, images, unlinked and formatted labels, the approval cell, detached rows, prose, another table and a shared record all returned 0. The scheduling suite passed 10 tests and the requirement graph suite passed six. The complete graph CLI returned 0 with 159 output lines.

Fresh isolated interpreters ran the CLI entry point with site imports disabled and an audit hook rejecting socket operations. Cold import paths resolved inside the two exact vendored wheels. Both SHA-256 values matched the pinned contract. Their original MIT texts and the upstream markdown-it notice were present. results.json records hashes, paths and copyright notices. The original checker missed the AMC omission. The first repair exposed the genuine mutex omission and missed an escaped pipe. The interim repair handled the pipe but misread literal code and missed defined references. history.txt preserves the direct CLI outputs. Scope is host citation ownership. Guest and physical performance were outside the run.

## 한국어

문서에 명시된 호스트 검사 범위에서 통과했다. Windows 빌드 26200과 64비트 CPython 3.9.16에서 원본 목록의 18개 정책을 검사해 지적 0건과 종료 코드 0을 확인했다. AMC 연결 하나만 삭제하자 POL-015/EVD-058 누락 1건으로 종료 코드 1이 반환됐다. 뮤텍스 근거는 실제 문맥 전환으로 분류됐으며 해당 연결을 삭제하면 POL-014/EVD-057 누락만 보고됐다.

영문·국문 각각 실제 표를 바꾸고 반대 언어의 대응 칸을 중립 문구로 둔 뒤 16개씩 검사했다. 이스케이프한 세로줄 뒤의 인라인 링크와 표 밖에서 정의한 전체·축약·단축 참조는 누락을 정확히 검출했다. 정의 없는 참조, 코드, 주석, 이스케이프한 괄호, 이미지, 일반 텍스트와 서식 있는 레이블, 승인 열, 표 밖의 행과 본문, 다른 표, 공유 근거는 오탐을 일으키지 않았다. 일정 근거 검사 10개와 요구사항 그래프 검사 6개가 통과했고 전체 그래프 명령은 159행을 출력하고 정상 종료했다.

각 검사에서 독립 인터프리터를 시작하고 사이트 패키지를 비활성화했으며 소켓 작업을 거부하는 감사 훅을 적용했다. 두 모듈은 고정된 wheel 내부의 정확한 경로에서 로드됐다. 두 파일의 SHA-256은 기준 문서와 일치했고 원본 MIT 라이선스와 markdown-it 원저작자 고지가 보존됐다. 최초 검사기는 AMC 연결 누락을 놓쳤다. 첫 수정은 기존 뮤텍스 누락을 찾아냈지만 이스케이프된 세로줄 뒤의 근거는 놓쳤다. 중간 수정은 세로줄을 처리했으나 코드 예시를 실제 링크로 오인하고 정의된 참조를 놓쳤다. 이번 결과는 호스트 인용 관계 판정에 한정된다. 게스트 실행과 실기 성능 검증은 수행하지 않았다.

## Reproduce / 재현

Use a clean disposable checkout at the candidate with its Git ancestors available. Both scripts temporarily modify tracked inputs and restore them. Run from this archive directory. 후보와 이전 커밋이 있는 깨끗한 임시 체크아웃에서 실행한다. 두 스크립트는 검사 후 입력 파일을 복구한다.

    python -X utf8 verify.py PATH_TO_CHECKOUT
    python -X utf8 history.py PATH_TO_CHECKOUT

[Independent offline controls and historical replay / 오프라인 독립 대조 및 기존 판정 재생](release-case-policy-link-verified.zip) · SHA-256 7dbac027a6cb907f90f303b2aa170328b2e6187de0cb8575759790035e02b730.
