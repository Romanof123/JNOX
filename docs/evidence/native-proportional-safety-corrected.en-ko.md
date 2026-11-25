# Corrected native proportional scheduling / 수정 후보 네이티브 비례 스케줄링

Functional Safety reviewer · #337, POL-010 · source f3be8089034a70bfa4baf35e81ee520f05813526.

**Decision: HOLD the named QEMU deployment at the Functional Safety gate.** Recommend POL-010 record the finite native application cases as independently verified, with Release disposition pending one positive native fault-recovery control. The earlier f107 disposition stands for its own candidate.

The seven first launches support the measured single-vCPU Cortex-A72 virt/GICv2/TCG deployment at quantum 625000. Six application replays match external exception/return PCs, saved frames, private roots and mathematical outputs. Both 1/2/4 seeds meet the 25 occupancy windows over 48 grants. Stable stride passes 48 service prefixes, and both dynamic runs cover three yields and sleeps. Raw elapsed service and grant counts retain their distinct meanings. The public and raw interfaces rejected the impossible floor and zero quantum. The report identifies the inherited checker mismatch on the isolated raw refusal and preserves its original log.

**Diagnostic finding.** The host harness recovered actual/expected ownership, the absent-owner sentinel and elapsed service from the corrected production policy and record format. It checked all three operand pairs, first-write retention and state after refusal. The disassembled guest separates sites 24 and 29, records site 17, samples context and emits PREDICATE, GUARD_EVENT and FAULT before parking. All six clean RAM captures had an empty first-record slot. Thus the corrected implementation and host evidence address the original ambiguity, but the previous requirement for independent recovery on a separate native control is still outstanding. The deployed sink has yet to yield a positive pre-park record whose RAM, register context and UART agree. The successful captures and build-time admission refusals leave that observation open.

**Release condition.** Use one separately identified diagnostic guest that makes the running owner and caller differ on the ordinary stop/rejection path. Preserve source, exact diagnostic ELF, raw UART, parked RAM and external exception/PC evidence. Independently decode site 17, predicate 10, caller, actual running owner and elapsed service from the first record, correlate the event and fault context with the intended invocation and observed PC/timer state, and verify the record survives parking. Share the production sink and preserve ordinary firsts. One such positive control suffices for this remaining gate when joined to the existing host ordering/overflow controls and compiled-path evidence. A new natural failure retains priority in its own run. The diagnostic has no bearing on #79's historical branch, the operational fault rate or customer hazard acceptance. Physical interference, timing and product risk remain with the target deployment and authorized product owner.

**기능안전 판정 — 지정 QEMU 배포의 Release 이관을 보류한다.** POL-010에는 수정 후보에서 유한 네이티브 앱 사례의 독립 검증 완료와 네이티브 최초 실패 복구 대조 1건 미완료를 기록하도록 권고한다. f107 당시 판정은 그대로 보존한다.

단일 Cortex-A72 virt/GICv2/TCG, 양자 625000에서 최초 부팅 7건을 검토했다. 정책 실행 6건의 예외·복귀 PC, 저장 프레임·사설 루트·앱 결과를 대조했고 두 시드의 25개 창, 스트라이드 48개 접두 구간, 동적 작업별 양보·수면 3회가 확인됐다. 배정 횟수와 경과 서비스는 별도 관측이다. 공개·원시 경로에서 잘못된 하한과 양자를 거부했으며 원시 거부 로그와 기존 검사기 메시지 차이를 보고서에 보존했다.

호스트 대조는 실제 정책에서 보유자 없음과 불일치를 구분하고 세 피연산자, 최초 기록 보존, 거부 후 상태를 확인했다. 네이티브 ELF에는 분리된 24/29, 17번 기록, 레지스터 채취와 UART 출력 후 정지 경로가 있다. 정상 게스트 6건의 최초 기록은 모두 비어 있었다. 게스트 정지 시점에서 최초 기록·문맥·출력을 함께 복구하는 기존 조건이 남아 있다.

별도 식별한 네이티브 대조 1건에서 실행 소유자와 호출자를 다르게 하여 실제 거부 경로에 진입시킨다. 소스·ELF·UART·정지 RAM·외부 예외 및 PC를 보존하고, site 17/predicate 10의 호출자·실제 소유자·서비스 시간을 독립 복구하여 이벤트·레지스터 및 타이머 문맥과 대조한다. 정지 후 기록 보존까지 확인하면 기존 호스트·ELF 근거와 합쳐 이 보류 조건을 충족한다. 원래 정상 최초 실행과 #79 실패 판정은 유지한다. 과거 원인과 운용 고장률은 미확정이며 실기 간섭·시간 및 고객 제품 위험 수용에는 별도 근거와 권한이 필요하다.

## Reviewed records / 검토 근거

D:/codex/.jnox-work/338-verification-f3be/ · SHA-256

| File | Digest |
|---|---|
| iv-report.en-ko.md | 7791eb658acf0b804670204657ff1ccf65fb2fa9911ce7d9f48f02ae75070fa7 |
| native-proportional-f3be.tar.gz | 8d9898848c5896ae45ab2289842bfe8ac457325c3352801a0a12d52a76ffe016 |

Functional Safety reviewed the independent evidence without additional builds or guest execution. 기능안전 검토 단계에서 추가 빌드나 게스트 실행은 수행하지 않았다.
