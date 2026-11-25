# Native AArch64 C application ABI v1

[한국어](native-c-abi.md) · CHG-086 · Proposed bootstrap contract for #21 and #24 · [POSIX source profile](posix-profile.en.md) · [User architecture](architecture.en.md)

A developer can target a freestanding C application with the native SDK. The first workload takes one input pathname, reads decimal telemetry records, writes its summary to fd 1 or a diagnostic to fd 2, and returns a status from main. Its source also serves as the portability fixture. ELF construction, EL0 entry, a working file service, and successful application completion are separate evidence milestones.

## Image and entry

Use little-endian AArch64 LP64, AAPCS64 integer calling convention and a static ET_EXEC. Native builds use Clang targeting aarch64-none-elf and LLD. Link file-backed RX at 0x00400000, with _start at that address. Use separate R and RW loads as needed, at most eight loads in [0x00400000,0x00800000). Require page-aligned offsets and virtual starts, disjoint 4096-byte page ranges and no writable executable pages. Zero every memory-only tail, including BSS. Exclude runtime relocations, interpreters, dynamic linkage and TLS segments. The SDK uses general-purpose registers only.

Trusted launch configuration selects the native v1 personality. Check the SDK's SHF_ALLOC SHT_NOTE section .note.jnox.abi in the read-only load against that configuration. The ELF note contains little-endian namesz=5, descsz=4, type=1, owner bytes JNOX followed by NUL and three zero pad bytes, then a u32 version 1. It needs no PT_NOTE. The marker identifies an ABI; authorization requires independent executable provenance, loader validation and process capabilities. Reject a version or configured personality mismatch before entry.

The loader prepares a writable, non-executable user stack disjoint from image pages. SP is 16-byte aligned. At SP is u64 argc in 0..INT_MAX; at SP+8 are argc little-endian pointers to accessible NUL-terminated argument strings followed by a null pointer. The launcher owns the bounds and overflow checks on these objects. No envp or auxv is consumed. Startup passes argc and argv in x0 and x1 to main, then sign-extends its int result into the exit request. A returned exit request trips a fault. Bootstrap permits exactly one thread per address space; launching a second thread must fail until thread-local errno and a thread context ABI exist. The address-returning errno accessor can later select a per-thread slot without changing application source. No signal-handler or async-signal-safe qualification has been performed.

## Service ABI

EL0 executes SVC #0x4a01 with x8 holding the service ID. Arguments occupy x0, x1 and x2. EL1 returns a signed LP64 result in x0 and preserves the other user general-purpose registers and SP. Extend v1 by allocating new service IDs and keep existing numbers and semantics stable. Unknown native IDs return -ENOSYS (38).

| ID | Service | Arguments | Successful result |
|---|---|---|---|
| 1 | exit | signed int status | terminate this process |
| 2 | open | pathname pointer, flags, zero | nonnegative int fd; bootstrap supports O_RDONLY=0 |
| 3 | read | signed int fd, writable user buffer, size_t count | bytes transferred, zero at EOF |
| 4 | write | signed int fd, readable user buffer, size_t count | bytes transferred |
| 5 | close | signed int fd | zero |

Errors are raw signed values -1 through -4095. The C wrappers return -1 and set errno to the positive native error number. Successful calls leave errno intact. Native EINTR=4 permits caller-controlled retries; partial transfers are returned as counts. Unsupported open flags fail with EINVAL=22. Count results fit ssize_t. Future EL1 services must bound path scans, validate user spans and access direction, cap transfer size, enforce descriptor rights and release resources on exit. The launcher must install functioning stdout/stderr. The SDK exports requests to the supervisor; file and device effects require implemented services and an executed guest.

## Acceptance

After the unchanged workload and SDK are present in one checkout, run the following from the repository root with Clang and LLD available.

    sh sdk/native-aarch64/build.sh examples/portability/telemetry_monitor.c telemetry-native.elf

Record compiler and linker versions, source and ELF hashes, program headers, section flags, syscall opcodes and absence of unresolved runtime imports. Validate the resulting bytes with the independent static load planner. A future supervised boot must show the original argv path, actual file reads, partial/error cases, stdout/stderr bytes and the observed exit status. Keep the native profile distinct from the Linux binary personality and qualify additional POSIX interfaces, concurrency and TLS independently.

## Package and run a native clock application

Service 6 implements `clock_gettime(CLOCK_MONOTONIC, tp)`. At `svc #0x4a01`, x8 holds 6, x0 holds clock ID 1 and x1 points to a writable LP64 timespec (two signed 64-bit fields, seconds followed by nanoseconds). A successful call returns zero. The native error convention above applies.

Run from the repository root on Linux with the native build prerequisites, QEMU, AArch64 nm and the Rust target `aarch64-unknown-none-softfloat`. Choose fresh output directories.

    python3 tools/build_native_el0.py --source examples/portability/clock_checksum.c --out c-package
    sh sdk/native-aarch64/build-rust.sh examples/native-clock-rust/Cargo.toml clock-rust.elf
    python3 tools/build_native_el0.py --elf clock-rust.elf --out rust-package
    python3 tools/native_el0_capture.py c-run c-package/kernel.elf c-package/user.elf examples/portability/clock_checksum.c "$(git rev-parse HEAD)" --expect-exit 0
    python3 tools/native_el0_capture.py rust-run rust-package/kernel.elf rust-package/user.elf examples/native-clock-rust/src/main.rs "$(git rev-parse HEAD)" --expect-exit 0

The manifest records the selected C file or prelinked ELF and its SHA-256. Explicit inputs select the native process launcher with argv[0] set to /bin/app. The capture archives the supplied source under the historical name `probe.c`, including Rust source. Its expected exit checks the terminal UART line. Use each capture's printed session digest in the clock replay. It checks the embedded ELF, user mappings, two successful EL0 clock calls, the QEMU exception transitions and exit 0.

    python3 tools/native_el0_replay.py c-run "$C_SESSION_SHA256" --clock-app
    python3 tools/native_el0_replay.py rust-run "$RUST_SESSION_SHA256" --clock-app

The default replay checks the original exit-37 workload. For that control, run `python3 tools/build_native_el0.py --out startup-package`.

Track the native service in #286 alongside #12, #8, #21 and #162. REQ-014 covers development and observability APIs. REQ-018 separately requires independent response to clock and other faults. [Requirements](requirements.en.md)

## Sleep until a monotonic deadline

Use the same boot-domain clock as the deadline. For periodic reporting, add each period to the previous deadline so that a late wake leaves the next target anchored to the original schedule. The profile covers one native process on one awake AArch64 QEMU virt CPU.

In C, read the current time with `clock_gettime(CLOCK_MONOTONIC, &target)`, advance and normalize the timespec, then call `clock_nanosleep(CLOCK_MONOTONIC, TIMER_ABSTIME, &target, 0)`. Check the return value directly. Success is zero and failures return a positive errno code. The function preserves `errno` and leaves a supplied remaining-time object untouched. The [complete C example](../examples/portability/periodic_report.c) advances the deadline by 40 ms three times.

In Rust, obtain a `core::time::Duration` with `jnox_native::monotonic_time()?`, add the interval with `checked_add`, then call `jnox_native::sleep_until(target)?`. An already-due deadline returns immediately. The [complete Rust example](../examples/native-periodic-rust/src/main.rs) reads the clock after each wake and checks that it reached the deadline.

From the repository root on Linux or WSL, with the native SDK build and capture dependencies installed, package and run both examples:

~~~sh
run=$(mktemp -d)
rev=$(git rev-parse HEAD)
python3 tools/build_native_el0.py --source examples/portability/periodic_report.c --out "$run/c"
sh sdk/native-aarch64/build-rust.sh examples/native-periodic-rust/Cargo.toml "$run/report.elf"
python3 tools/build_native_el0.py --elf "$run/report.elf" --out "$run/rust"
python3 tools/native_el0_capture.py "$run/c-first" "$run/c/kernel.elf" "$run/c/user.elf" examples/portability/periodic_report.c "$rev" --expect-exit 0
python3 tools/native_el0_capture.py "$run/rust-first" "$run/rust/kernel.elf" "$run/rust/user.elf" examples/native-periodic-rust/src/main.rs "$rev" --expect-exit 0
~~~
