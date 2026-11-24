# Proposed CHG-088 timer acknowledgement correction / 타이머 승인 순서 수정안

[Native workload #177](https://github.com/Romanof123/JNOX/issues/177) · [Original source and verification #178](https://github.com/Romanof123/JNOX/pull/178) · [First IRQ #181](https://github.com/Romanof123/JNOX/issues/181) · [Startup compatibility #182](https://github.com/Romanof123/JNOX/issues/182)

The original one-process startup selector also requires a separate compatibility check. Its candidate build stopped on a duplicate local assembly label. The correction gives the old vector's park label a distinct name. An independent verifier must build that selector and repeat its original EL0 startup, unsupported write and exit-37 observations alongside the four weighted cases. Retain both verdicts and the first compiler error.

기존 단일 프로세스 기동 프로브는 후보 빌드에서 같은 이름의 어셈블리 레이블이 겹쳤다. 원래 벡터의 대기 레이블 이름을 분리한 뒤, 가중 작업 네 건과 별도로 기존 프로브의 빌드, EL0 진입, 지원하지 않는 쓰기 호출과 종료 코드 37을 독립 검증한다. 최초 컴파일 오류와 각 시험 판정도 보존한다.

The first four weighted native launches reached EL0 entry at 0x400000 and then a lower-EL IRQ. All four emitted the failure terminal. Three retained RAM snapshots identify guard 11. The original guard stopped before recording the IAR, timer control and compare operands. Neither the interrupt ID nor the particular failed comparison can be recovered from those observations alone.

최초 네 실행은 0x400000에서 EL0에 진입한 뒤 하위 EL IRQ를 받고 실패 종료했다. 보존된 RAM 세 건은 가드 11을 가리킨다. 기존 가드는 IAR, 타이머 제어값, 비교값을 기록하기 전에 중단했다. 이 자료만으로 실제 인터럽트 ID나 실패한 비교식을 특정할 수 없다.

The original handler disabled CNTP before reading GICC_IAR. On a level-sensitive input this can withdraw the request before acknowledgement. For the candidate, match the existing virt weighted and AMC handlers: read the full IAR once with the source still asserted, snapshot CNTP_CVAL and CNTP_CTL, disable the source, retain the original timer-ID/control/timestamp checks, then EOI with the same full IAR. SVC never acknowledges an IRQ. [Existing weighted IRQ](../kernel/src/weighted.rs) · [Native candidate](../kernel/src/native_vruntime.rs)

기존 처리기는 GICC_IAR 판독 전에 CNTP를 껐다. 레벨 감응 입력에서는 승인 전에 요청이 해제될 수 있다. 후보는 기존 virt 가중치 처리기처럼 요청이 유지된 상태에서 전체 IAR를 한 번 읽고 CNTP_CVAL과 CNTP_CTL을 기록한 뒤 타이머를 끈다. 기존 IRQ ID·제어·시각 조건이 모두 맞을 때 같은 전체 IAR로 EOI한다. SVC는 IAR를 읽지 않는다.

On a candidate guard-11 failure only, the last committed row has tag 13, entry tick and PID, followed by lease sequence, vector offset, full IAR, raw CNTP_CTL, CNTP_CVAL, pre-ERET tick and interrupted ELR. Keep the terminal rejected. The row earns zero service and cannot appear in a successful replay. Check each operand separately even if several fail. Absence of this row in the original is expected.

후보에서 가드 11이 실패할 때만 마지막 행(태그 13)에 진입 틱과 PID, 임대 순번, 벡터 오프셋, 전체 IAR, CNTP_CTL, CNTP_CVAL, ERET 직전 틱, 중단된 ELR을 남긴다. 실패 판정을 보존하며 서비스 시간을 인정하지 않는다. 각 조건의 실패 여부를 독립적으로 판독한다.

Freeze the corrected kernel and unchanged application before repeating the original four positions under the same default clock and limits. Save first UART and exception trace even if physical RAM capture fails. Verify the linked IAR load precedes timer disable on the IRQ path. Require unchanged acceptance criteria for full share and wake workloads; crossing the first IRQ alone closes no workload gate. Preserve and report both cohorts separately.

수정 커널과 기존 응용 ELF를 고정한 뒤 같은 기본 시계 및 한도로 원래 네 위치를 재실행한다. RAM 수집 실패 시에도 최초 UART와 예외 추적을 남긴다. 링크된 IRQ 경로에서 IAR 판독이 타이머 차단보다 앞서는지 대조한다. 최초 IRQ 통과와 전체 지분·깨움 기준 충족은 각각 평가하고, 두 코호트의 결과는 분리해 보존한다.
