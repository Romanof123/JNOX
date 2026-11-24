# Legacy selector baseline / 기존 선택기 빌드 기준

Public origin/main and clean detached checkout 8609aab7eeb509b7aa032fcf6cafe124770ab751. All 10 WSL builds succeeded. 전체 10개 빌드 성공. 게스트 실행 및 시간 측정 없음. Sizes represent linked ELF fields, without physical memory or WCET measurements.

Run from PowerShell with `wsl env -i HOME=/home/ywcho PATH=/usr/bin:/bin bash /mnt/d/codex/jnox-baseline-248250-evidence/build.sh`. See build.sh, per-case build.log and tools.txt. kernel/build.rs selects board virt and deadline or fixed profiles without Cargo features. docs/ordinary-deadline-plan.md defines four deadline helpers with optional JNOX_DEADLINE_TRACE=1. docs/coop-runtime.en.md and build.rs select coop-events and coop-single with fixed trace unset. kernel/.cargo/config.toml targets aarch64-unknown-none and cortex-a72.

All sizes in bytes. LOAD is the concatenation of p_filesz bytes from each PT_LOAD at its p_offset in program-header order. manifest.json also records each segment and hash. .bss includes the stack array shown in the subset column. .stack is a separate reservation. 스택 배열은 .bss에 이미 포함된다. 정렬 패딩 때문에 섹션 합과 세그먼트 길이는 다를 수 있다.

|Case|ELF|LOAD|.text|.rodata|.data|.bss|Stack array subset|.stack|
|---|---:|---:|---:|---:|---:|---:|---:|---:|
|coop-events-plain|106336|21650|19260|2098|288|82152|81920|16384|
|coop-single-plain|106168|21074|18728|2050|288|82152|81920|16384|
|ordinary-edf-overrun-plain|103064|21053|18356|2373|320|76344|65536|16384|
|ordinary-edf-overrun-trace|103064|21053|18360|2373|320|76344|65536|16384|
|ordinary-edf-plain|103064|21053|18356|2373|320|76344|65536|16384|
|ordinary-edf-trace|103064|21053|18360|2373|320|76344|65536|16384|
|ordinary-llf-overload-plain|103064|20869|18172|2373|320|76344|65536|16384|
|ordinary-llf-overload-trace|103064|20869|18176|2373|320|76344|65536|16384|
|ordinary-llf-plain|103064|20869|18172|2373|320|76344|65536|16384|
|ordinary-llf-trace|103064|20869|18176|2373|320|76344|65536|16384|

[Ten original ELF files, loadable bytes, hashes and build commands](deadline-cooperative-legacy-loads.zip) · SHA-256 76fbb0044d84a03ebb4c2f422492bbbf8068670c746bcd2a19a16d61b496f29f.
