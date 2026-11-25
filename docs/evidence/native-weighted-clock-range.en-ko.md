# Native application clock verification / 네이티브 앱 시계 검증

Public JNOX PR #297, bbf9f907d93962dd87f6d529f788482fbf90320b. Each of six package manifests records this commit and an empty tracked diff. All six first guest runs used a single emulated Cortex-A72, QEMU TCG virt, default clock at 62,500,000 Hz. Only one guest or builder ran at a time. The earlier 100d32e results stay attached to their original builds.

| First guest run | Evidence |
|---|---|
| Extended weighted 1:2 | 36/36 contiguous grants, 7,555,343 and 15,079,901 service ticks. 12 and 24 interrupted productive PCs. Both full RAM products and exits 237/70. |
| Extended weighted 1:2:3 | 60/60 grants, 6,290,395 / 12,579,045 / 18,843,351 ticks. 10/20/30 productive PCs. All full RAM products and exits 237/70/179. |
| Three 40 ms sleeps | Three successful IRQ/admission/EL0 clock brackets. Peer service 2,468,036 / 2,500,972 / 2,502,026 ticks, above 3Q each. Reporter charged zero during each block. Private times matched all targets and clock rows. Exits 237/7. |
| Weighted / fixed RR | Same two SDK client ELFs. A normalized 150,000,000,000-second absolute request yielded EOVERFLOW (75). At this frequency it fits the unsigned target but exceeds the signed forward interval. An elapsed request succeeded immediately. A 20 ms absolute sleep completed after its target. The peer ran and exited while the client slept. Private values survived across calls and yields. Client/peer exited 109/29. |
| Cooperative | Same positive and elapsed checks, peer, values and exits. Its separate 400,000,000,000-second input exceeded unsigned conversion range and returned EOVERFLOW. |

Q=625000, H=781250, G=156250 timer ticks. First-window timing, exact rational selection replay and all terminal products passed. 211/409/13 event rows with zero drops. End-to-end upper wake bounds 645469/636544/636088 ticks. RAM sampling recorded peer progress advancing across each reporter phase. See scores.json, pc-and-clock.json and raw UART/RAM/QMP captures. The ordinary elapsed and signed-forward controls cover the observed values at this emulated frequency. Physical hardware, integrated load, exhaustive input boundaries and fresh icount runs are outside these measurements.

공개 수정 커밋을 깨끗한 작업 트리에서 빌드했다. 기본 시계에서 1:2 가중치의 첫 36회와 1:2:3의 첫 60회가 정해진 서비스·지연 기준을 충족했다. 세 번의 40 ms 수면 모두 상대 앱이 기준 이상의 CPU 시간을 받았고, 복귀 시각과 앱 전용 메모리 계산값을 확인했다. 이벤트 손실은 0건이다. 가중치 및 고정 RR 모드의 동일한 SDK 앱이 부호 있는 전방 간격의 범위를 넘는 정상 형식 요청에서 EOVERFLOW(75)를 받았다. 이어 이미 경과한 요청과 20 ms 수면에 성공했고, 두 앱 모두 전용 값을 유지하며 109/29로 끝났다. 협력형 모드에서는 별도 입력으로 부호 없는 변환 오류와 정상 수면을 확인했다. 이번 결과의 범위는 QEMU 기본 시계이며, 이전 커밋의 측정 기록은 별도로 보존했다.

Offline replay from the archive root on Linux: run sha256sum -c SHA256SUMS, python3 score.py and python3 pc_clock.py. Compare outputs with the supplied JSON. Source ELF build recipes, input oracles, first-run order, package manifests, disassembly, SDK client sources and raw logs are included. Pre-guest setup failures involved missing Git metadata, an existing staging path, an oracle path and shell line endings. All were corrected before the affected guests began.
