# #213 독립 검증 / Independent verification

Candidate 689a4c62131c594c7b6593d28d75c0479272a8e0. Parent ca0b3dd3f87f0f459449271d957af1eb8e77d2cd. Frozen plan c45948ff8c3d9f453c24f9199887ef26f88fc96c. Actual Windows Rust 1.95.0 release AArch64 builds, jobs=1, Ubuntu QEMU 6.2.0 TCG virt/GICv2 on native ext4. No physical board. freeze-plan.json fixes order before runtime and freeze.json fixes ELF, QEMU and argv for every position. Each UART and full exception/PL061 trace copied and hashed on D before continuing. All 20 main and six legacy first captures reached CONTROL END without timeout. Guest termination only establishes transcript closure.

## Healthy gate / 실제 정상 판정

Original control_observations.py healthy returned **nonqualifying in each of eight cold positions**, including 0/4 default candidate A0. The real command occurred only in position 06 after seven independent acceptances. Its first issue gap was 576072 ticks, below the required 625000. No rearm and no modeled ON at any position. Seven other boots latched and accepted zero. Actual default eligible positions 0/4 A0 candidate, 0/4 A1/A2 operational (pre-worker refusal). No target release or operational promotion. Exact same-clock 10 ms issue spacing, 5 ms finish/check spacing, 250 µs control service, 0.5 ms IRQ, 1 ms phase, expiry-priority and the separate 25/5/30 ms response bounds remain original. An initially OFF trace supplies no powered-on loss transition.

기존 정상 판정기로 최초 8회 모두 nonqualifying이었다. 기본 후보 A0 4회 중 실제 수용 가능 판정은 0회였다. DM 06은 7회 발행·수신 후 명령 1건을 기록했으나 최초 간격 576072틱으로 정확한 625000틱을 충족하지 못했다. 다른 7회는 래치 후 수신 0건이었다. 재무장과 출력 상승은 전부 0건이었다. 기본 A1/A2 운영 4회도 원가 상한 누락으로 작업 진입 전 거부되었다. 기존 시간 한도를 유지한다. 실제 장치와 켜진 출력의 손실 억제 시간은 미검증이다.

|Position|First service|First IRQ lateness|First phase completion delay|First issue gap|Issued/accepted|Original first rejected condition|
|---|---:|---:|---:|---:|---|---|
|01-rm-P-a0|19354|12207|57656|570232|7/0|actual delivery|
|02-rm-C-a0|34075|19367|71833|536291|7/0|phase envelope|
|03-rm-C-a0|18185|17288|65443|561931|7/0|phase envelope|
|04-rm-P-a0|38241|17150|81193|527767|7/0|phase envelope|
|05-dm-P-a0|18033|12814|60691|564329|7/0|actual delivery|
|06-dm-C-a0|15394|11711|50026|576072|7/7|rearm issue spacing|
|07-dm-C-a0|16198|12974|56328|568311|7/0|actual delivery|
|08-dm-P-a0|17860|12930|59140|567057|7/0|actual delivery|

All times above use ticks at 62.5 MHz. Candidate minus parent for first service, IRQ lateness and phase delay respectively, in pair order: RM (+14721,+7160,+14177), (-20056,+138,-15750); DM (-2639,-1103,-10665), (-1662,+44,-2812). These four finite contrasts do not identify a transferable cost reduction. All per-phase and per-slice measurements including later execution are available in raw/*/observation.json, linked-events.json, original-gate.json and UART/trace. The single candidate service below 15625 did not qualify the full chain.

위 표의 단위는 62.5 MHz 카운터 틱이다. 첫 서비스·IRQ·위상 지연의 후보−기준 차이는 RM 두 쌍에서 부호가 달랐다. DM의 첫 서비스는 두 쌍 모두 감소했지만 전체 정상 통과는 0회였다. 각 회차의 이후 위상·슬라이스·판정·복귀와 원본은 raw 폴더에 보존했다.

## Controls / 억제 대조

Four default operational A1/A2 runs recorded mask 4095, zero jobs/issues/acceptances, initial and terminal OFF. Four default characterization runs stopped before all seven sender opportunities (28 unavailable across four) with zero attempts. First faults in RM A1/A2 and DM A1/A2 respectively were 107/103/107/103. Four original icount OFF runs completed, each 7/7 actual permits, first gap 625000 ticks, command and zero output edges. The full PL061 stream was observed throughout. The live OFF guard had zero alarms, external output bit zero initialized OFF, masked data writes and reads stayed OFF. No approved cost report was supplied. Tick equality from icount OFF confers no default operational qualification.

기본 운영 4회는 작업 전 누락 마스크 4095로 거부되었다. 기본 OFF 4회는 발행 시도 전에 종료했으며 첫 거부 원인은 RM과 DM 각각 107/103이었다. 원래 icount OFF 4회는 각 7회 발행·수신, 최초 간격 625000틱, 명령 및 출력 OFF를 기록했다. 모든 비교에서 외부 PL061 관측을 유지했다. 운영 비용 상한과 실물 증거는 확보되지 않았다.

## Linked evidence / 연결된 코드

All 24 rebuilt images contain disassemblies and symbol maps. RM candidate descriptor normal phase 0 saves/masks at worker entry, phase 1 saves/masks at 0x40202ec4/c8, both enter the single pending-index check at 0x40202dd0. Normal decision calls check at 0x40202df0. Miss branches to complete_missed_phase at 0x40202de0, rejoins mask accounting at 0x40202e58, restores DAIF at 0x40202eb4 and takes SVC2 at 0x40202ef0. The SVC redirect writes final_check into saved ELR at 0x40202134. The returned worker executes final_check at 0x40201a74. Its guarded call returns before SVC2 at 0x40201a7c. Source changes are confined to control_runtime plus the plan.

Separate saved-state controls used the linked RM candidate ELF and original icount clock for reproducibility. An intentional pending index 0 was injected at the first decision or immediately before normal SVC1. The first case reached the final evaluation and real restore, with IRQ mask clear before SVC2 (saved x25=832). The second case took the post-decision redirect and returned to the same worker SP before the guarded final check, again with IRQ clear at SVC2. Neither control is an ordinary scheduler miss. Two earlier debugger attempts timed out; their available raw streams are preserved. Single-instruction stepping then observed both paths. One first diagnostic debugger process required explicit termination. No first main position was replaced.

정상 후보의 각 위상은 판정 보호구간을 한 번 지난다. 누락 인덱스 처리 후 실제 DAIF 복원이 SVC2보다 앞서며, 판정 후 누락에서는 SVC가 저장 ELR을 최종 검사로 지정해 같은 작업자 스택으로 복귀했다. 별도 저장상태 실험에서 두 경로와 실제 IRQ 마스크 해제를 확인했다. 이 결과는 일반 부팅에서 누락이 발생했다는 증거로 사용하지 않는다. 초기 디버거 시도 두 건의 타임아웃 원본도 보존했다.

## Legacy and historical cohorts / 기존 비교군

All six candidate stock/cold/marker PT_LOAD comparisons against the immediate source parent changed. All six were smoked. Stock and marker tests per policy ran the periodic healthy workload to a closed terminal with no rearm or ON. The two first cold diagnostics saw missing fw_cfg input → sentinel 4294967295 and row 60 reason 1, independently OFF. Historical EVD-050 storage had 32 positions (RM healthy 0/16). EVD-051 had 24 service positions and 0/24 healthy, two DM descriptors received all seven. EVD-054 integration recorded 0/20 operational and three A0 seven-accept runs with short first spacing. Prior 16 default observer OFF boots had 0/112 sender attempts. These historical runs retain their own denominators.

기존 stock·cold·marker 6개 조합의 적재 바이트는 바로 앞 기준과 모두 달랐다. 각 후보의 최초 스모크를 보존했다. cold 최초 2회는 fw_cfg 입력 누락을 감지하고 OFF로 종료했다. 과거 저장 32회, 서비스 24회, 통합 운영 20회 및 OFF 관측 16회의 결과는 각각 별도 판정이다.
The 208 phase-join.json records comprise 26 phases for each of eight A0 boots. They retain dispatch, actual freshness input, completed decision, upper phase completion, IRQ, projected phase target and first subsequent accounting sample separately. The terminal latch flag was 1 in every A0 boot except 06, whose flag was 0. Position 06 issued its separate command at 1223131 after issues at 285542 and 861614, with no successful rearm. All four icount OFF runs recorded actual command ingress at the exact declared target, before the subsequent controller acquisition, while the live output remained OFF.

A0 8회의 위상별 208개 레코드에는 디스패치, 신선도 판정 입력·완료, 위상 종료 상한, IRQ, 목표, 후속 서비스 과금 시각이 들어 있다. 06의 래치 플래그는 0이며 명령은 1223131틱에 도착했지만 재무장은 없었다. icount OFF 4회에서는 목표와 일치하는 명령 도착 및 이후 제어기 수신을 확인했다.

The two separately frozen supplemental cold boots consumed the original named read-only fw_cfg byte ASCII 1. Each reported row 48 mode 1, reached periodic phase decisions, no row 60 input refusal, and CONTROL END. RM and DM terminal output was 0 and scheduler fault flag was 1. Independent PL061 indicated zero throughout both. These are accepted diagnostic-input smoke results and leave the first sentinel-refusal captures intact.

추가 cold 대조 2회는 원래 fw_cfg 모드 1 바이트를 실제로 읽고, 입력 거부 없이 주기 작업을 수행한 뒤 종료했다. RM·DM 모두 출력 0, 스케줄러 고장 플래그 1이며 외부 PL061도 OFF였다. 최초 입력 누락 거부와 이 추가 결과를 별도로 보존했다.

[Independent first controls, phase data and archived replay / 독립 최초 실행과 위상별 원자료](periodic-single-decision-first.zip) · SHA-256 182687f07828db620c136cfde03ca25b955e3629493e2dbbe9d8d7f19f71711a.
