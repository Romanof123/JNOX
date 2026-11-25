# #289 / #290 · Independent native absolute sleep verification
Source 446b5d57d27f86d29b748a8962bb2f9dcfc4d9bc. Plan 320b0b75396273ab070943079fdfa9da91482040. One ordinary default-clock QEMU virt/GICv2 Cortex-A72 TCG CPU, one native EL0 process per cold boot. C used the SDK and Rust used Cargo with aarch64-unknown-none-softfloat. ELF inputs, commands, host launch, UART, QEMU GIC/exception/CPU trace, RAM, application sources and hashes are in first-originals.zip. Both first executions completed with exit 0. No interrupts were injected. Timing values below belong to the emulated clock.

| App | Absolute target → subsequent application clock read (ns) | Terminal report in process RAM |
|---|---|---|
| C | 97,854,848 → 99,511,280 · 137,854,848 → 139,345,952 · 177,854,848 → 179,207,968 | 6 at 0x1010f4c |
| Rust | 97,045,200 → 98,930,384 · 137,045,200 → 138,384,256 · 177,045,200 → 178,673,024 | 6 at 0x1010f6c |

Each future call saved frame 0x40630d40, programmed its target, reached WFI on idle stack 0x4062d020, took an EL1 IRQ, and acknowledged GIC ID 30. The CPU recorded the blocked state and due counter in each handler. The ready path restored TTBR0 0x4020b000 and returned to the application after SVC (C 0x400168, Rust 0x400524). Each application repeated its first target before the next future call. Both took the immediate due branch and returned 0 without switching to idle or taking another timer IRQ. The C error returned positive 22 with errno 4 and remaining (17,23) preserved. Rust returned overflow 75 inside the safe wrapper. The old C clock sample and designated startup profile replayed successfully, exiting 0 and 37. An earlier auxiliary launch of the startup probe used the generic process argument profile and exited 90 at its argc check. Both original captures are retained.

한국어 · C와 Rust 일반 앱이 각각 이전 절대 기한에 40 ms를 더해 세 번 대기했다. 각 회차에서 독립 EL1 스택의 WFI, 실제 GIC PPI 30 승인, 기한 도달 확인, 동일 프레임·주소 공간 복원, EL0 복귀 후 시계 읽기를 추적했다. 첫 기한을 재요청했을 때 두 앱 모두 차단 없이 성공했다. 두 앱의 실제 스택에서 누계 6을 읽었고 정상 종료를 확인했다. C의 양수 EINVAL과 errno·remaining 보존, Rust의 오버플로 오류 75도 확인했다. 보조 검사 중 일반 인자 프로필로 잘못 패키징한 시작 예제는 90으로 종료했다. 지정된 레거시 프로필로 패키징한 시작 예제는 37, 기존 시계 예제는 0으로 종료했다. 실기기 지연은 측정하지 않았다.

The first-originals.zip SHA-256 is 260fc6a55355c93d83d82d1ae758d3d798e5c5f31dbf05c0297f635c4f72b31e. The separate analysis.zip contains verify.py, decoded.jsonl, disassembly, terminal-phase trace excerpts and the two legacy replay results. The verifier uses the source tree's native_el0_replay.py and pyelftools.

[Original cold app boots](native-absolute-sleep-first.zip) · [Independent decoding and replay](native-absolute-sleep-analysis.zip)
