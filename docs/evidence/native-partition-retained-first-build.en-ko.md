# Independent PR #238 / #233 verification · 독립 검증

Public branch kernel/native-retained-frame-banks, commit 714f89f5bd010ab6e2eecac03fbb44446c431453. Detached WSL checkout, repository-pinned Rust 1.95.0, one build job and one workload at a time. No tracked changes. Run scripts record the public entry points, input and prospective order.

All eight first kernel packages (P/Q × A/B/C/D) failed with rustc E0658 at kernel/src/native_partition.rs:231. The cfg attribute occurs on an array element expression. Plain and irq-only builds additionally report that removing an expression is unsupported there. User ELF compilation had completed in each case. No P/Q kernel was linked. All 80 ordered capture invocations stopped before QEMU when copying the absent kernel. Counts: 80 planned positions, 80 host-side errors, zero cold starts and zero measured timing failures or passes. No replacements. Linked sizes, addresses, 104-word comparisons, selection/restore state, release versus safe-context timing, phase deltas, first IRQ and later user windows cannot be measured on this revision. Acceptance is blocked at build.

Fresh weighted share and wake on both clocks and startup on default each built, booted and replayed successfully (5 captures). The exact reader completed all nine public historical ordinary controls. Historical P/default alone returned timing status 1, generation 2. Opening 6459487, previous closing IRQ entry 6492117, selection 6527026, return sample 6527494, first user 6532879. Opening-to-user 73392 ticks exceeds 62500 by 10892. This is historical control data.

공개 커밋에서 P/Q와 A/B/C/D 조합 8개의 최초 패키지 빌드가 모두 실패했다. Rust 1.95.0이 native_partition.rs 231행 배열 원소에 적용한 cfg 속성을 거부했다. 80개 예정 위치의 캡처 명령은 모두 커널 ELF 복사 중에 중단됐다. QEMU 부팅 수는 0이며 시간 한도, 주소, 104워드 프레임 동일성, 복원 상태, 링크 크기에는 실측 결과가 없다. 최초 기록을 보존했고 대체 실행을 하지 않았다.

같은 소스로 새로 빌드한 가중치 share/wake 두 클록과 기본 클록 startup 대조군은 총 5회 캡처·재생에 성공했다. 과거 공개 대조군 9개를 재생했으며 과거 P/default 2세대만 73,392틱으로 62,500틱 기준을 10,892틱 초과했다. 현재 커밋의 P/Q 시간 판정은 빌드 단계에서 보류한다.

Replay: from the named commit, set PYTHONPATH for pyelftools and use native_weighted_replay.py or native_el0_replay.py under tools, passing each extracted controls directory and the SHA-256 of its session.json. Historical packages come from docs/evidence/native-partition-ordinary-first.zip at the named commit. All first package logs and user ELFs, position logs, new controls' full sessions, and historical replay outputs are included. Cargo build caches and the redundant unpacked historical archive are excluded.

[Raw package logs and control boots](native-partition-retained-first-build.zip) · SHA-256 dff4f85b9688a7c297dc5eb7323e2579dcae351bc1733b29f8cfa62426025c21.
