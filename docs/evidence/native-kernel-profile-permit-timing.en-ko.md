# Kernel profiles on protected FIFO and RR / 커널 프로파일 독립 실측

Candidate f48c41aa3e3a8aa33c9a96c4b0fe82534083a417 on c5ab397e. Both profiles refuse timing admission. Every one of the 96 qualified workload captures gives a full phase lower bound above the inclusive 62500-tick limit. Bare native default returned 37. Direct default and explicit release produced identical bytes. Speed produced different bytes. The first healthy FIFO release/speed ELFs measured 237328/253792 bytes with executable LOAD file sizes 128416/153096 bytes. Same per-position user bytes, arguments, routes, fixed priorities, policy, RR quantum and original clock verified against the archived launch. The four historical receipts (fbbb5ce, 7c5b0c2d, fused c5ab397e, same-image clock diagnostic) retain their original decisions.

Two prospectively ordered paired blocks: 34 positions, each release→speed then speed→release. All 136 fresh guests captured terminal UART and RAM. The 96 qualification/load boots and 24 authentic default controls used wall-clock TCG. Sixteen authentic boundary-observer boots retained their historical shift-0 clock. Cortex-A72, one vCPU, 62.5 MHz. No warm-up or discarded capture. Isolated direct builds, a default bare guest and ten two-profile regression/control captures are outside this denominator. An initial shell directory command failed before the source checkout and before any guest or build.

| Matched endpoint (ticks) | Speed lower | Speed higher | Median speed minus release |
|---|---:|---:|---:|
| max_ctrl_hi | 31 | 17 | -6284.5 |
| later_max | 27 | 21 | -1449 |
| job_max_hi | 5 | 43 | 15967 |
| max_sender_hi | 17 | 31 | 12813 |
| max_complete_gap_upper | 23 | 25 | 524.5 |

Block 1 FIFO healthy job 0 A full response measured 119720–119781 release and 113148–113202 speed. B measured 22456–22517 and 31602–31652. All 96 had 32 completed checks, observed adjacent gap upper ≤262298, sender upper ≤304389 and finite whole-job upper ≤276529. Block 2 speed FIFO/live had an open controller response and incomplete pre-wake chains on job 0 A/B. Twenty boots exceeded the 31250-tick grid IRQ upper criterion. Every deliberate overrun retained its original timer-driven loop. All eight ordinary healthy samples encoded 625000-tick first-pair spacing and showed ENABLE=1 at 18 and 37 ms. Full wire values, private sender and receiver data, open intervals, retained-prefix and journal losses, per-job brackets, rank, PPI30/PPI27, compare readbacks, offsets and fault intervals are included. The fixed journal lost 244–376 suffix records. Sixteen authentic collision boots exited 71/72/95, with the other 120 cohort boots at 71/72/73. No exact coincidence follows from nearby timestamps.

Regression exit vectors agreed under release and speed: finite timeout and subsequent indefinite reply 54/55/70, priority donation 51/52/53, same-root idle save/restore and distinct-root fixture 69/70/70, ordinary grant/transfer/revoke 73/74/70 and incompatible permit-size control 82/90/70. Physical WCET, comprehensive interference envelopes and physical device observations remain unestablished.

한국어: FIFO·RR 34개 위치를 두 블록에서 release→speed, speed→release 순으로 교차해 136회 부팅했다. 위상·IRQ 부하 실험 96회 모두 최초부터 62500틱 위상 상한을 초과해 양쪽 승인을 거부한다. 속도 프로파일의 최대 위상 응답 상한은 48쌍 중 31쌍에서 감소했지만 전체 작업 응답 상한은 43쌍에서 증가했다. 실제 시계의 정상 첫 작업과 의도적 초과를 그대로 포함했다. 두 번째 블록 speed FIFO/live는 첫 작업의 대기 관측 일부가 누락됐다. 단일 가상 CPU 표본이며 보드 최악시간을 증명하지 않는다.

## Replay / 재현

Extract the packet to a directory and run `python3 replay.py NEW_DIRECTORY --reduce` with GNU aarch64-linux-gnu-nm. It verifies the terminal raw hashes for every boot and recomputes the four RAM/UART observation sets. Per-boot archives reference content-addressed ELFs in objects/. Compare endpoints.json, controls.json, pairs.json, identity.json, regression.json and the four compressed observations. schedule.json records the frozen order, progress.log all launches. source-inputs.tgz contains the selected source/config files, SDK commands and build logs. Check out the stated commit for the full candidate source. The shipping-tree receipt is separate.

## Raw replay / 원본 재생

Download and unpack all six parts in one directory. / 여섯 묶음을 같은 디렉터리에 푼 뒤 원본을 결합한다.

- [native-kernel-profile-permit-timing-01.tar.gz](native-kernel-profile-permit-timing-01.tar.gz) SHA-256 `1487d9e8f55016e0e674d106d96bd5535ab8f188e19490aaeecedc1470513d5e`
- [native-kernel-profile-permit-timing-02.tar.gz](native-kernel-profile-permit-timing-02.tar.gz) SHA-256 `a3feb4577a667aad7bdcd12a7616e8d2336faf9b3bfed42a3f494543684de663`
- [native-kernel-profile-permit-timing-03.tar.gz](native-kernel-profile-permit-timing-03.tar.gz) SHA-256 `9ef0f01e7990b3bc56eb100481ead1ae4a4be605b5039312f3cd700d59a97f0d`
- [native-kernel-profile-permit-timing-04.tar.gz](native-kernel-profile-permit-timing-04.tar.gz) SHA-256 `af8dd9c48038456d34b065da485cc7b0df7d4de81f67c5ab7801bac45b2992d5`
- [native-kernel-profile-permit-timing-05.tar.gz](native-kernel-profile-permit-timing-05.tar.gz) SHA-256 `0dd4fccd0f4d6d339a9e221f789aeaf12581ba9ed62b8ef001306225e23a567e`
- [native-kernel-profile-permit-timing-06.tar.gz](native-kernel-profile-permit-timing-06.tar.gz) SHA-256 `63c624672fa3d8af5faf3f821e7744643c0484f25fa61855f4a606ef9dcafa04`

~~~sh
cat permit-profiles-first.part0[1-6] > permit-profiles-first.tgz
sha256sum permit-profiles-first.tgz
~~~

SHA-256 `72d72ba8f20f6889a8db4874b0602858b87311f888447eaab41b0e122710d9c7`
