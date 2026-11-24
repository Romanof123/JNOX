# Four finite scheduling cases / 스케줄링 사례 네 건

On 2026-11-25, [#239](https://github.com/Romanof123/JNOX/issues/239) requested retrospective indexing of four independently captured guest workloads on main `bd897ccd121021dcabb73b5e1fdf6c74b6a970a8`. The identifiers below date from this indexing decision. The original contracts, source revisions, verdicts, dates and archives remain the authority for what ran.

2026-11-25에 본선의 네 독립 게스트 실행 기록을 사후 색인한다. 아래 신규 번호는 이번 색인 시점에 부여한다. 원래 계약, 소스 리비전, 최초 판정·날짜와 원본 자료가 실제 실행의 근거다.

| Policy and contract / 정책·사전 계약 | Origin / 이슈·병합 요청 | Index / 사후 색인 |
|---|---|---|
| POL-003 [Admitted telemetry / 텔레메트리 배치](batch-telemetry-profile.en-ko.md) | [#211](https://github.com/Romanof123/JNOX/issues/211) · [#214](https://github.com/Romanof123/JNOX/pull/214) | CHG-091 · IMP-091 · [EVD-091 EN](evidence/evd-091.en.md) · [한국어](evidence/evd-091.md) |
| POL-016 [Recurring hierarchy / 반복 계층 서비스](nested-group-recurring-service.en-ko.md) | [#212](https://github.com/Romanof123/JNOX/issues/212) · [#215](https://github.com/Romanof123/JNOX/pull/215) | CHG-092 · existing / 기존 IMP-038 · [EVD-092 EN](evidence/evd-092.en.md) · [한국어](evidence/evd-092.md) |
| POL-018 [Ordinary extension jobs / 일반 확장 작업](extension-normal-jobs.md) | [#221](https://github.com/Romanof123/JNOX/issues/221) · [#223](https://github.com/Romanof123/JNOX/pull/223) | CHG-093 · IMP-093 · [EVD-093 EN](evidence/evd-093.en.md) · [한국어](evidence/evd-093.md) |
| POL-013 [Pi 4 model CRC stream / Pi 4 모델 CRC 스트림](boards/pi4-stream-main.en-ko.md) | [#228](https://github.com/Romanof123/JNOX/issues/228) · [#229](https://github.com/Romanof123/JNOX/pull/229) | CHG-094 · IMP-094 · [EVD-094 EN](evidence/evd-094.en.md) · [한국어](evidence/evd-094.md) |

Use existing REQ-002/005/023, DES-003/012 and partial VER-002/005/023 for the three single-core cases. Include REQ-008, DES-004 and partial VER-008 for the Pi 4 model's CPU placement and saved context. Join each change to its requirements and each measured implementation to its design, policy, verification and canonical bilingual evidence. Use live-switch evidence only for captured guest dispatch and restored frames. Link each language record directly to the original report and ZIP. [#6](https://github.com/Romanof123/JNOX/issues/6) tracks independent graph/matrix replay. [#21](https://github.com/Romanof123/JNOX/issues/21) retains deployment acceptance and [#34](https://github.com/Romanof123/JNOX/issues/34) physical board qualification.

단일 코어 세 사례는 기존 REQ-002/005/023, DES-003/012 및 부분 검증 VER-002/005/023에 연결한다. Pi 4 모델의 CPU 배치·문맥에는 REQ-008, DES-004, 부분 검증 VER-008도 연결한다. 각 변경의 요구사항과 측정된 구현의 설계·정책·검증·한영 근거를 연결한다. 실제 게스트 디스패치와 프레임 복원 범위에서만 전환 근거로 분류하고 원본 보고서와 ZIP을 직접 연결한다. 독립 그래프·행렬 재생은 #6, 배포 승인은 #21, 실기 검증은 #34에서 추적한다.

## Provenance notation correction / 출처 식별자 표기 수정

On 2026-11-25 independent review of [#240](https://github.com/Romanof123/JNOX/pull/240) at `9fe4170e9084046433c7913d340ce6a783073a5a` reported four bilingual provenance mismatches. Both language catalogs must expose each change and implementation ID as a complete token. In the four Korean evidence records a following Hangul particle prevents the checker's Unicode word boundary from matching the last digit. Delimit the literal identifier in Markdown and keep the particle outside it. Following the correction, independently replay all four policy paths and compare the exact change and implementation sets across languages. The first review and the four original guest results retain their own revisions and verdicts.

2026-11-25 독립 검토에서 네 근거 문서의 한영 출처 식별자가 일치하지 않는다는 최초 판정이 나왔다. 두 언어의 색인에서 변경·구현 번호를 각각 독립된 토큰으로 읽을 수 있어야 한다. 한국어 근거 문서에서 번호 끝 숫자와 조사가 바로 이어져 검사기가 단어 경계를 찾지 못했다. 식별자를 마크다운 코드로 표기하고 조사는 닫는 백틱 뒤에 붙인다. 수정 후 독립 검증자는 정책 경로 네 개를 모두 다시 추적하고 양쪽 언어에서 추출한 변경·구현 번호 집합을 대조한다. 최초 검토와 원래 게스트 실행 네 사례의 판정은 각각 당시 리비전에 귀속된다.

[First review and four findings](evidence/four-finite-trace-first-review.en-ko.md) and [corrected combined audit](evidence/four-finite-trace-combined-review.en-ko.md).
