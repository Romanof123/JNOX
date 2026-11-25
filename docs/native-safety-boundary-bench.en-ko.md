# Engineering observation of exact permit expiry / 허가 만료 경계 관측

[#11](https://github.com/Romanof123/JNOX/issues/11) · [#389](https://github.com/Romanof123/JNOX/issues/389) · [ADR-001](safety-parameters.en.md)

The original normal-clock FIFO and RR observations were late. Preserve both. Instruction-count TCG can schedule a virtual timer deadline, but subsequent native instructions still take virtual time before the sender publishes or the controller checks. Qtest clock stepping controls device-model time; it provides no evidence that ordinary EL0 execution reached either operation at the stepped tick. Stock facilities provide no guaranteed exact native collision. [QEMU TCG instruction counting](https://www.qemu.org/docs/master/devel/tcg-icount.html) · [QTest clock protocol](https://www.qemu.org/docs/master/devel/testing/qtest.html)

## Prospective serialized lane

1. Reserve four separate engineering first positions, FIFO/collision, RR/collision, FIFO/phase and RR/phase. Keep the three actual native ELFs, original single-check stimuli, directed loan, supervisor, output and capture protocol. Copy the frozen normal invocation into each receipt; select single-thread TCG and one CPU, adding fixed `-icount shift=0,align=off,sleep=off`. Preflight option support against the actual QEMU executable. Record all hashes, full invocation, actual CNTFRQ and scenario args. Do not change the operating-clock invocation or reinterpret its results.
2. Let I be the captured last accepted permit's wire issue; record separately whether this is synthetic or an observed creation tick. Require initially enabled output, exact integer ticks D=I+20F/1000 and H=5F/1000 for frequency F, checked for overflow. Collision means the **next sequence of the same open generation**: opening a new generation resets the state and belongs to a separate case. Require no intermediate issue, an open gate and an actual CRC-valid next frame.
3. Require successful ring publication at D, expiry decision at D, and the frame available before the controller tries to apply it. Join EVENT 11 status, handle and captured fields to the sender bytes and EVENT 7/private snapshot. Exact publication is established by the existing bracket only if before=after=D. A wider bracket containing D is ambiguous. Verify EVENT 3's decision operand, old accepted sequence, rejection, latched state and EVENT 2 output transition/readback. Arrival after expiry can produce the same final state. A scheduled issue or timer interrupt at D cannot timestamp ring publication.
4. Phase requires an actual freshness decision at D−1 and the next actual decision exactly at D−1+H. Enumerate every intervening check, including checks inside kernel calls. Require fresh at the former, fault at the latter, a captured gate closure with no intervening delivery, and independent evidence for output transition timing. EVENT 10 brackets COMPLETE but cannot identify every fresh-path decision. If needed freeze a separate observation-only kernel that logs the existing sampled freshness operand, caller, generation, sequence, old/new fault state and losses. A later logger counter read cannot recover the original operand.
5. Preserve each first run, all signed tick deviations and loss counts. Report timing from bracket endpoints, retained private state, and precisely which predicates remain unobserved. A later separately hashed observer of the real publication store may narrow its time bound only with justified uncertainty. No guest-clock rewriting or freezing through a critical operation. Private exact-clock clones remain independent semantics controls. Even reproducible engineering equality supplies no normal-clock scheduling bound.

## 한국어 인계

정상 클록 FIFO/RR 첫 관측은 모두 늦었으며 그대로 보존한다. 각 정책의 collision/phase 네 위치를 별도로 동결한다. 같은 실제 EL0 송신자·제어기·감독자와 단일 검사 자극을 사용하고 위 고정 icount 설정, 전체 명령행, 실행 파일 해시와 CNTFRQ를 기록한다. 타이머가 목표에 도달한 뒤에도 게시는 명령을 실행해야 한다. qtest 장치 시간 조절로 사용자 작업의 완료를 입증할 수 없다.

마지막 수신 이슈 I와 주파수 F로 D와 H를 정확히 계산한다. 켜진 출력과 동일 세대 다음 순번을 사용한다. 새 세대 열기는 초기화이므로 다른 사례다. 성공한 이벤트 11의 두 끝이 D인지, 실제 만료 판단 피연산자도 D인지 확인하고 수신·거부·래치·실제 출력 전이를 연결한다. D를 포함하는 넓은 구간은 미정이다. phase는 D−1의 실제 신선 판정과 정확히 H틱 뒤 다음 검사의 만료를 요구한다. 중간 검사도 모두 추적한다. 기록이 부족하면 별도 빌드에서 원래 판단 시각 피연산자·결과·순번 및 손실을 기록한다. 첫 실행과 편차를 보존하며 해상도나 계측이 충분하지 않으면 미관측으로 남긴다. 공학용 관측과 정상 클록 수용은 별도로 판정한다.

## Native REARM at 15 and 20 ms / 네이티브 재무장 15·20 ms

Reserve four separate cold cases at the original 62.5 MHz counter frequency, FIFO and RR at each age. Boot with ENABLE low and the gate open in a new generation. Record successful commits of two consecutive set permits with wire issues I and I + 625,000 ticks. Tag scheduled wire issues explicitly. For the first supervisor REARM call, retain the exact counter value passed to `eligible`, its decision, the authenticated caller, generation, both permit issues and the pending command state. Require the original two-frame streak and no earlier REARM. The observation build must reuse the actual decision sample.

At I + 937,500 ticks (15 ms), require supervisor acceptance, successful controller consumption while both permits are fresh, then a bracketed ENABLE 0→1 and readback. At I + 1,250,000 ticks (20 ms), require refusal on the first command with the first permit exactly expired, no pending command, and output still low. Score attempted deadlines and recorded operand ticks separately with signed deviations and zero safety-event loss. Retain all first captures, including any miss. An admissible result for one operation supplies no timing bound for the other operation or for normal-clock phase completion.

원래 62.5 MHz에서 두 재무장 나이와 FIFO·RR을 조합해 네 번 따로 부팅한다. 동일 세대의 연속 허가 두 건을 실제 반영한 뒤, 감독자 첫 명령의 판정에 쓰인 카운터 값과 허가 두 건의 발행 시각을 보존한다. 15 ms에는 명령 수락, 제어기의 유효성 재확인, 가상 출력의 상승과 판독까지 확인한다. 20 ms에는 첫 허가가 정확히 만료된 순간의 거부, 대기 명령 없음, 출력 차단 유지를 확인한다. 요청 시각과 실제 판정 시각의 차이, 첫 실행의 실패와 기록 손실을 함께 남긴다.

## Measured native REARM decision / 네이티브 재무장 실측

REQ-019 · VER-019 · EVD-311 · [#398](https://github.com/Romanof123/JNOX/issues/398). On the single-CPU 62.5 MHz Cortex-A72 virtual bench, the supervisor's first native REARM decision hit the requested tick in three cold boots for each of FIFO/15 ms, FIFO/20 ms, RR/15 ms and RR/20 ms. These 12 measurements used the fixed instruction-count clock. The two synthetic wire issues in each generation were 625,000 ticks apart. At 937,500 ticks after the first issue the supervisor accepted. The controller checked both permits again while fresh and the kernel changed the virtual ENABLE register to 1 approximately 1.008 ms after command acceptance. At exactly 1,250,000 ticks the first issue had expired. The supervisor refused, with no pending command or output rise.

At one tick before the 20 ms boundary both supervisors queued the command. By the controller's next check the first permit had expired. The pending command cleared and the output stayed inhibited. Both one-tick-late commands were refused. The four unadjusted ordinary-clock supervisor samples were late by 9,696–14,105 ticks. Across all 24 functional boots the dedicated safety journal lost zero records. Every boot had a completed-check gap above 5 ms. The maximum conservative upper bound was 5.262096 ms on the ordinary clock and 5.013008 ms on the instruction-count clock. The original refused package and all subsequent runs are included. [Independent captures, host package checks and replay](evidence/native-permit-rearm-ticks.en-ko.md).

단일 Cortex-A72의 명령 수 기반 가상 시계에서 FIFO·RR 각각 15 ms와 20 ms 판정을 새 부팅으로 세 번씩 재현했다. 15 ms에는 감독자 명령이 수락되고, 약 1.008 ms 후 제어기의 재확인을 거쳐 가상 출력이 켜졌다. 정확히 20 ms에는 첫 허가가 만료돼 명령이 거부되고 출력이 계속 꺼져 있었다. 1틱 이른 명령은 대기 중 만료됐고 1틱 늦은 명령은 바로 거부됐다. 기본 시계 네 번에서는 목표 틱보다 늦었다. 24회 모두 5 ms 검사 완료 간격을 초과했다. [원본 및 재검증](evidence/native-permit-rearm-ticks.en-ko.md).

## Worst check phase on the protected channel / 최악 검사 위상의 보호 경로

REQ-019 · VER-019 · EVD-311 · [#400](https://github.com/Romanof123/JNOX/issues/400). After accepted sequence 3 and confirmed device enable, the supervisor closed the normal input and the producer's next acquisition was refused. At 62.5 MHz let I be this permit's wire issue, D=I+1,250,000 and H=312,500. The original delivery followed I by approximately 1.006 ms in the selected runs. FIFO and RR each reproduced the following two engineering observations in three cold boots per case.

| Native event | Observation on the instruction-count clock |
| --- | --- |
| Two consecutive protected freshness decisions | The successful COMPLETE decision at D−1 found the old permit fresh. The next protected CHECK at D−1+H latched expiry, followed by an actual virtual output fall. Private receiver sampling continued between the protected decisions. |
| Two consecutive completed activations in separate boots | The two actual active-to-inactive stores, each bracketed through readback, occurred exactly at D−1 and D−1+H. Between these stores the next opening CHECK caused the latch. Its operand preceded the second store by 300 ticks under FIFO and 322 under RR. |

All 55 measured new-case boots met the separately evaluated virtual 25/5/30 ms response limits after actual gate closure. Each exceeded the 5 ms maximum interval between completions elsewhere in the run. All 59 cold launches, including the four original controls, the crossed store brackets and all operating-clock samples, are preserved. The original collision controls never established enabled output; [#401](https://github.com/Romanof123/JNOX/issues/401) tracks the open-gate arrival case. [Original captures, executable analysis and replay](evidence/native-permit-worst-phase.en-ko.md).

순번 3의 실제 수신과 출력 활성화를 확인한 뒤 게이트를 닫았다. 명령 수 기반 시계에서 FIFO와 RR 각각 보호 판정 두 건의 시각을 3회씩, 별도 실행에서 실제 활성화 종료 저장 두 건의 시각을 3회씩 재현했다. 종료 저장 사이에는 다음 CHECK가 먼저 래치를 설정했다. 수신기 자체 시각, 보호 판정, 출력 전이와 종료 저장을 각각 측정했다. 새 시험 55회 모두 차단 후 25/5/30 ms 반응 한도를 지켰고, 전체 실행에서는 매번 5 ms 완료 간격을 초과했다. [한·영 근거와 원본](evidence/native-permit-worst-phase.en-ko.md).

## Open-gate arrival ordering / 게이트가 열린 상태의 도착 순서

REQ-019 · VER-019 · EVD-311 · [#401](https://github.com/Romanof123/JNOX/issues/401). Each of 12 native cold boots established generation 1, sequence 3 and ENABLE high before the sender staged the same valid sequence-4 loan it later published. All apps kept their authenticated roles and the gate stayed open. FIFO and RR each used three fixed publication calendars with the ordinary clock and instruction counting. The nominal expiry was D, twenty milliseconds after the third wire issue. The actual control decision was late in every boot.

| Fixed publication calendar | Actual order across the four boots | Protected outcome |
| --- | --- | --- |
| E+39 ms, one millisecond before D | Publication before D and before the expiry CHECK | The already queued permit did not renew sequence 3. The output fell. |
| E+40 ms, the same requested time as D | D elapsed, then publication completed, then CHECK observed expiry | The queued frame was received after expiry and rejected. |
| E+41 ms, one millisecond after D | CHECK observed expiry before publication | Later delivery did not re-enable the output. |

All 12 terminal register readbacks were low. The sender staging and publication, kernel receipt and receiver's private 44 bytes agreed. The protected controller kept sequence 3; the supervisor subsequently observed the latched fault. Actual CHECK−D ranged from +134 to +956 ticks on the instruction-count clock and from +4,033 to +25,693 ticks on the default clock. Every boot had a completed-check gap above 5 ms; the largest observed conservative upper bound was 6.207904 ms. Publication coincident with the exact expiry decision remains open. A separate offline instruction and clock analysis identifies a mandatory cross-role exception path in the inspected kernels. [Twelve original captures and independent replay](evidence/native-permit-arrival-first.en-ko.md) · [Instruction-clock evidence](evidence/native-permit-collision-order.en-ko.md).

12회 모두 게이트가 열려 있고 실제 출력이 켜진 상태에서 정상 순번 4 프레임을 시험했다. 네 번은 D 전에 게시됐고 네 번은 D 이후 실제 만료 검사 전에 게시됐으며 네 번은 검사 뒤 게시됐다. 모두 이전 허가의 만료를 래치하고 출력을 차단했다. 동시 시각을 요청한 네 번도 실제 게시와 판정은 다른 틱에 있었다. 전체 검사 완료 간격의 보수적 상한은 최대 6.207904 ms였다. [실측 표 및 원본](evidence/native-permit-arrival-first.en-ko.md).
