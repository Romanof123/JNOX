---
name: 일반 엔지니어링 작업·결함 / Engineering task or defect
about: 작업 또는 결함을 기준선, 검증 및 종결 결정과 연결합니다. / Link the work or defect to its baseline, verification and disposition.
title: ''
---

기존 기록은 해당 위치를 링크하고, 적용되지 않는 항목에는 이유를 한 줄로 적습니다. / Link existing records at their source. Give a brief reason for any inapplicable field.

## 작업과 책임 / Work and ownership

- 기대 동작, 관측한 문제 또는 작업 목표 / Expected behavior, observed defect or task objective
- 재현 조건과 정확한 기준선(커밋·구성·대상) / Reproduction conditions and exact baseline (commit, configuration, target)
- 담당자와 역할, 상위 이슈, 관련 추적 ID / Owner and role, parent issue and applicable trace IDs
- 영향받는 상위 요구·설계·가정·위험 및 관련 변경 요청 / Affected upstream requirements, designs, assumptions, hazards and linked change request

## 검증과 종결 / Verification and closure

- 실행 전 고정한 판정 기준과 대상 버전 / Acceptance criteria frozen before execution and candidate version
- 독립 검증 담당자 및 원본 판정 링크 / Independent verifier and link to their verdict
- 첫 시도 결과(실패·미관측 포함), 남은 차단 요인 및 담당 이슈 / First-attempt outcomes including failures or missing observations, remaining blockers and their issues
- 관련 PR과 증거에서 이 이슈로 돌아오는 링크, 종결 또는 보류 결정과 게이트 상태 / Backlinks from PR and evidence, closure or deferral decision and gate status
