---
name: 기획·설계 변경 요청 / Planning and design change
about: 기획 전제부터 검증 증거까지 변경 영향을 기록합니다. / Trace the impact from planning assumptions to verification evidence.
title: '[CHG] '
---

## 변경 이유와 재현 근거 / Reason for change and supporting evidence

- 변경 요청 ID / Change request ID `CHG-…`
- 기존 기준선 또는 관련 버전 / Current baseline or relevant version
- 기대하는 동작과 현재 문제 / Expected behavior and observed problem

## 시스템팀의 상위 검토 / Systems team review of upstream impact

- 관련 기획·가정·위험·위협 / Related objectives, assumptions, hazards and threats (`OBJ`, `ASM`, `HAZ`, `THR`)
- 재설계를 시작할 최초 영향 지점 / Earliest affected decision from which redesign must begin
- 변경되지 않는 상위 전제의 유지 근거 / Rationale for retaining upstream assumptions that still hold

## 승인과 재검증 / Approval and reverification

- 수정할 기록 / Records to revise (`REQ`, `DES`, `ADR`, `IMP`)
- 다시 실행할 시험과 재검증이 필요한 증거 / Tests to rerun (`VER`) and evidence requiring reverification (`EVD`)
- 비영향 항목의 판단 근거 / Rationale for items assessed as unaffected
- 시스템 결정과 승인 기록, 관련 PR / System decision, approval record and related pull request
