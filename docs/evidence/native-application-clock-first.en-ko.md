# Independent native application clock verification · 독립 검증

At commit 7af3ca22b88c9ea9d81f4495b37613f241b67adc, both ordinary native applications built with the public SDK, packaged for native process launch, and exited 0 on their first QEMU boots. Each archived exception stream contains two successful clock calls and one exit from AArch64 EL0 using SVC #0x4a01. Offline replay checks service 6, CLOCK_MONOTONIC 1, result 0, the mapped ELF and exit. QEMU used virt, Cortex-A72, one CPU and TCG. The kernel samples CNTPCT_EL0 relative to its initialized boot-domain epoch and scales using CNTFRQ_EL0. Both stopped domains held 62,500,000 Hz.

| Application | First sample (s, ns) | Last sample (s, ns) | Work in optimized image |
|---|---|---|---|
| C SDK Adler-32 of Wikipedia | (0, 1715056) | (0, 2014704) | Calls 0x40015c/0x40027c surround the completed checksum store at 0x400278. Exit verifies 0x11e60398. |
| External Cargo crate, FNV-1a of hello | (0, 1716736) | (0, 1989488) | Calls 0x40003c/0x4000cc surround five multiplies and the store at 0x4000c4. Stopped RAM holds 0x4f9f2cab. |

Both C timespecs survive in RAM. Rust reuses the ABI output address. The first nanoseconds survived in the second helper frame's saved x20, identified from the exact executable. The initial seconds are inferred from the SDK's nonnegative range check, the application's successful order check, and the observed final seconds of zero. The second Rust timespec is directly readable. These guest timestamps are observations from individual emulation runs.

The initial source 46a87fb7177e748963726f44f262525a51d865fd also booted both applications successfully. Inspection found the C final checksum combine and Rust FNV multiplies had moved after the end sample. The independent external Rust variant completed the arithmetic between samples and preserved both complete pairs. The corrected checkout differs only in the two example sources. The earlier unchanged startup probe returned 37, and write gave -38 (ENOSYS). Another public C SDK application made 16 reads, retained 16 normalized increasing pairs in RAM, and exited 0 after 17 traced SVC entries. The diagnostic journal retained eight events.

First package builds and boots succeeded. The first offline replays on this verifier failed because Python lacked pyelftools. Extracting Ubuntu python3-pyelftools locally and exporting its dist-packages on PYTHONPATH allowed replay of those same captures. An intermediate replay invocation also lost shell variable quoting. The full commands, original run logs and snapshots are included. For reproduction, install Clang/LLD, AArch64 binutils, QEMU, Python with pyelftools and Rust 1.95.0 with aarch64-unknown-none and aarch64-unknown-none-softfloat. In a fresh directory clone JNOX into src, checkout the corrected commit, and run corrected/check.sh from the parent directory (place check.sh beside src and adjust its CARGO_HOME/RUSTUP_HOME for your machine). Use corrected/decode.py and corrected/rust_stack.py from that directory for additional RAM observations. The SDK dependency points at the checkout. The application crate lives outside it.

## 한국어

수정본에서 C 앱과 별도 Cargo 프로젝트의 Rust 앱 모두 공개 SDK로 빌드한 후 첫 QEMU 부팅에서 종료 코드 0을 반환했다. 두 앱 모두 EL0에서 서비스 6을 두 번 호출했다. 최적화된 실행 파일을 확인해 두 호출 사이에서 체크섬 계산과 저장을 마치는 것을 검증했다. C의 두 시각과 Rust의 마지막 시각은 정지 RAM에서 직접 읽었다. Rust의 첫 나노초는 스택에 보존된 x20에서 읽었고, 첫 초가 0이라는 값은 SDK 범위 검사와 앱의 순서 검사로 추론했다. 위 표의 값은 가상 머신에서 관측한 값이다.

첫 소스에서는 일부 계산이 종료 시각 뒤로 이동했다. 수정 전후의 실행 자료를 모두 보관했다. 커널과 SDK가 동일한 이전 대조군에서는 기존 시작 프로브가 37로 종료했고 write가 -38을 반환했다. 별도 C 앱은 진단 기록 8개 한도를 넘겨 16개의 시각을 정상적으로 읽고 종료했다. 실제 보드 실행은 이번 검증 범위에 포함되지 않는다.

[Native Rust and C application clock runs / 네이티브 Rust와 C 앱 시계 실행](native-application-clock-first.zip) · SHA-256 c5d102342d7e2e07a3d5fe91d317d8460c273f309f280da9598440aff86eca95.
