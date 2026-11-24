# CPU0 typed QMP independent review / CPU0 QMP 형식 독립 검증

Public source 10413a1ae6a34b85caf414444e26b23758b39879. Eight first cold boots on WSL ext4, Rust 1.95.0 locked release -j1, serial Cortex-A72 single CPU QEMU 6.2 TCG. All eight public boot/seal/decode commands exited 0. Saturating traces explicitly decode incomplete due to loss. The per-run source status lists only Python bytecode cache as untracked. Tracked source matched the pinned commit. The runner, original captures, manifests, logs, raw oracle and per-run section comparisons accompany this report.

| Frame / mode | Run UUID | W | Q | S | E | E−W | Timer overlap E−S | Kernel/app loss |
|---|---|---:|---:|---:|---:|---:|---:|---|
| 1-active | c05e4b6d-3a91-489f-842b-fc54719fda93 | 5167452 | 5172303 | 7647360 | 7654923 | 2487471 | 5530 | [0, 0] |
| 1-disabled | dc5da887-85cd-4685-b03f-807d32483920 | 5140165 | — | 7617598 | 7624113 | 2483948 | — | — |
| 1-off | cae1ae31-3269-439c-bb04-02ccee1c9afb | 5130655 | — | 7609316 | 7615326 | 2484671 | — | — |
| 1-saturate | b6dec71e-febd-4964-bb3e-aca7005e18d7 | 5263374 | — | 7703910 | 7712365 | 2448991 | — | [22, 1] |
| unset-active | 6d888355-a465-435b-a939-b219b61625d2 | 5162847 | 5167451 | 7630834 | 7636834 | 2473987 | 4538 | [0, 0] |
| unset-disabled | 61443035-54b8-4bfd-9755-98510a79437d | 5147459 | — | 7618157 | 7624189 | 2476730 | — | — |
| unset-off | 77315452-6859-495c-aeae-7da926109e1d | 5135229 | — | 7599286 | 7604951 | 2469722 | — | — |
| unset-saturate | cd78566c-4051-4153-8986-48ff2816eb36 | 5142917 | — | 7619807 | 7625720 | 2482803 | — | [22, 1] |

All boots had integer CPU index zero, real empty-object stop acknowledgement, then running=false/paused and a terminal controller value of zero. Production code and the instrumented mock peer establish save/command/extraction order. Genuine captures retain the replies; they do not carry a wire-level command transcript. All 32 worker publications reproduced from raw work counts and supplier sequence. Each run met B=23U=14,375,000 ticks (U=625,000) with six timer IRQs and frame/peer/wake checks. oracle.json contains each digest, lateness, gaps, service durations and joins. Frame restoration in full mode also matched the interrupt transcript. OFF symbol absent, disabled commits and losses zero. Both saturation records retained their first four events and lost exactly 22 kernel and one application event. Missing Q in those rows is withheld.

Ten host Python tests and three Rust producer tests passed. Historical manifests replayed on copies using the pinned decoder: all eight 0174fe3 old-format successes passed and all eight original 33d8fbf receipts failed with missing halted.json. All 24 UUIDs are distinct. The raw oracle is adapted from the prior independent witness parser, with new inputs, typed assertions and unchanged packet reconstruction.

Historical joint controls are retained as earlier observations. Git comparisons from 33d8fbf through 10413a1 found identical arch, boards, kernel, sdk, examples, tools/qemu_boot.py and tools/mlfq_wake_capture.py. The 0174fe3→10413a1 shared capture helper change is confined to capture_halt; the earlier nontrace client bypasses that function by its ordinary default. Prior nontrace boot log and fresh host legacy path test retained. sections.json compares fresh/first-joint section sizes and both byte hashes for loadable code/data. Stack allocation is static. Dynamic stack high-water, IRQ mask duration and WCET were unmeasured. The nested IRQ control is a host model.

공개 소스의 WSL ext4 독립 최초 부팅 8건 모두 부팅·봉인·복호화 명령에 성공했다. 각 실행에서 단일 정수 CPU0, 빈 객체 stop 응답, 정지 상태를 확인했다. 모의 QMP와 소스로 응답 저장·정지 확인·메모리 읽기 순서를 검증했다. 원본 워크 카운트로 결과 32개와 공급 순서를 재계산했고 8건 모두 23U 이내였다. 활성 모드의 타이머 중첩과 포화 모드의 손실(커널 22, 응용 1)은 표에 표시했다. 정상 구형 기록 8건은 통과했고 정지 응답이 없는 최초 8건은 다시 거부됐다. 과거 공동 대조군은 입력 동일성을 확인해 유지했다. 스택 최대 사용량, 인터럽트 차단 시간, 최악 실행 시간은 실측하지 않았다.

[Typed CPU0 first receipts, original records and replay / 최종 CPU0 원본과 검산](sched-app-trace-typed-first.zip) · SHA-256 e329fcda9d439a877658514a7c5479ab0d95aaeb53fa1eda2aa97573fdc16936.
