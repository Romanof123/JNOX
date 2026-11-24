# Independent public package review / 공개 패키지 독립 검증

Software / 소프트웨어: 096e5a99efc0d5ef12e50e995e44e666fb99cb43
Plan / 계획: 52e396e2817d061d49bfd2030e52fa437fe30103, docs/partition-first-irq-phase.md.

The public selector passes for P and Q. With ambient JNOX_PARTITION_PHASE=irq-v1 present, omitted selectors produced zero defined phase symbols. Explicit --partition-phase irq-v1 produced exactly one per kernel. The user images matched byte-for-byte. The only additional manifest key is partition_phase, set to irq-v1. Mocked child environments verified user compiler isolation. Three incompatible apps and irq-v2 returned 2 before output creation. Three legacy apps packaged normally, without a diagnostic symbol, and the Cargo selector suite passed.

P/Q 공개 옵션은 통과했다. 환경변수를 설정한 상태에서도 명시적으로 옵션을 전달한 커널에만 계측 심볼이 하나 있다. 사용자 ELF는 쌍별로 동일했다. 진단 매니페스트에 partition_phase만 추가됐다. 비호환 앱 세 개 및 잘못된 값은 출력 생성 전에 종료 코드 2로 거부됐다. 기존 앱 세 개와 Cargo 선택자 검증도 통과했다.

| App | Package | User SHA-256 | Kernel SHA-256 |
|---|---|---|---|
| p | plain | 4969269f4f9a6cf5d75e35678586f02bbccbf219129851fc73b15744aeb04e0c | 69eecfd9cc84c9f4e463f7339d468d395db29281accf80c7bf3629f2efc69672 |
| p | phase | 4969269f4f9a6cf5d75e35678586f02bbccbf219129851fc73b15744aeb04e0c | 11a3654d6c599355212148f27518720b168d946934a675b3fb8f6ee22b2a0cdd |
| q | plain | 4969269f4f9a6cf5d75e35678586f02bbccbf219129851fc73b15744aeb04e0c | 6381e63ee0faa6b335c14543676b3ac6fd0bd1ee728beba1ae9f44ff6b724eeb |
| q | phase | 4969269f4f9a6cf5d75e35678586f02bbccbf219129851fc73b15744aeb04e0c | 91d728eed030f9a935bf9be1835e998408ff0d4b4c9732c6ae17164724e61d12 |

Five first cold pairs per table and clock, AB/BA/AB/BA/AB. All 40 captures and 20 paired replays completed. Independent arithmetic and raw-RAM checks cover 210 transitions, including repeated PIDs. Each individual transition is in transitions.tsv. There were no retries. Default plain first-opening misses occurred on P rounds 1,4,5 and Q round 4. Worst plain margins were −35,516 and −2,599 ticks respectively. Default diagnostic P missed 5/5 and Q 1/5. All misses were generation 2. No icount3 misses. At 62.5 MHz the original 62,500-tick limit remains binding. The diagnostic packaging correction passes. The ordinary scheduling acceptance remains open.

테이블·클록별 최초 다섯 쌍을 교대 순서로 실행했다. 재시도 없이 40회 캡처와 20쌍 디코딩이 완료됐고, 같은 PID 전환을 포함한 210개 전환을 RAM 원자료와 대조했다. 기본 클록 일반 커널은 P의 1·4·5회차와 Q의 4회차에서 2세대가 기한을 넘었다. icount3에서는 초과가 없었다. 계측 패키징 수정은 통과했으며 일반 스케줄링의 62,500틱 기준은 미충족이다.

summary.txt lists first/later median phase-minus-plain differences in IRQ-to-selection, selection-to-return, return-to-user and opening-to-user order, plus plain and phase minimum margins. Raw ticks include observer costs. / summary.txt는 최초·후속 전환별 네 구간 중앙값 차이와 양쪽 최소 여유를 기록한다. 계측 비용을 차감하지 않았다.

[Corrected CLI and all forty first normal boots / 수정된 CLI와 최초 실행 40회](native-partition-packager-corrected-first.zip) · SHA-256 3d43b257ae7344798410af500280a8990ea3762e84bcab8c21a74741060939d3.
