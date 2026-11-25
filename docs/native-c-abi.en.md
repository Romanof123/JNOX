# Native AArch64 C application ABI v1

[한국어](native-c-abi.md) · CHG-086 · Proposed bootstrap contract for #21 and #24 · [POSIX source profile](posix-profile.en.md) · [User architecture](architecture.en.md)

A developer can target a freestanding C application with the native SDK. The first workload takes one input pathname, reads decimal telemetry records, writes its summary to fd 1 or a diagnostic to fd 2, and returns a status from main. Its source also serves as the portability fixture. ELF construction, EL0 entry, a working file service, and successful application completion are separate evidence milestones.

## Image and entry

Use little-endian AArch64 LP64, AAPCS64 integer calling convention and a static ET_EXEC. Native builds use Clang targeting aarch64-none-elf and LLD. Link file-backed RX at 0x00400000, with _start at that address. Use separate R and RW loads as needed, at most eight loads in [0x00400000,0x00800000). Require page-aligned offsets and virtual starts, disjoint 4096-byte page ranges and no writable executable pages. Zero every memory-only tail, including BSS. Exclude runtime relocations, interpreters, dynamic linkage and TLS segments. The SDK uses general-purpose registers only.

Trusted launch configuration selects the native v1 personality. Check the SDK's SHF_ALLOC SHT_NOTE section .note.jnox.abi in the read-only load against that configuration. The ELF note contains little-endian namesz=5, descsz=4, type=1, owner bytes JNOX followed by NUL and three zero pad bytes, then a u32 version 1. It needs no PT_NOTE. The marker identifies an ABI; authorization requires independent executable provenance, loader validation and process capabilities. Reject a version or configured personality mismatch before entry.

The loader prepares a writable, non-executable user stack disjoint from image pages. SP is 16-byte aligned. At SP is u64 argc in 0..INT_MAX; at SP+8 are argc little-endian pointers to accessible NUL-terminated argument strings followed by a null pointer. The launcher owns the bounds and overflow checks on these objects. No envp or auxv is consumed. Startup passes argc and argv in x0 and x1 to main, then sign-extends its int result into the exit request. A returned exit request trips a fault. The default bootstrap uses one thread per address space and its original errno storage. The opt-in two-thread profile assigns distinct errno cells to main and worker. No signal-handler or async-signal-safe qualification has been performed.

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

## Package two cooperative native processes

Build the Rust peer separately and pass its ELF to the packager alongside the primary C source. To use an already linked primary, replace `--source PATH.c` with `--elf PATH.elf`. From the repository root on Linux or WSL:

~~~sh
run=$(mktemp -d)
sh sdk/native-aarch64/build-rust.sh examples/native-cooperative-rust/Cargo.toml "$run/peer.elf"
python3 tools/build_native_el0.py --source examples/portability/native_cooperative_report.c --peer-elf "$run/peer.elf" --out "$run/package"
qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -accel tcg -m 128M -nodefaults -display none -monitor none -no-reboot -serial stdio -kernel "$run/package/kernel.elf"
~~~

The [C primary](../examples/portability/native_cooperative_report.c) calls `sched_yield()` from `<sched.h>`. The [Rust peer](../examples/native-cooperative-rust/src/main.rs) uses safe `jnox_native::yield_now()`, which returns `Result<(), i32>`. Yield hands CPU0 to a ready peer. If the caller is alone, yield succeeds immediately. An absolute sleep blocks its caller and lets a ready peer run. Each process keeps a private stack and writable data, so both examples use virtual address `0x402000` for different accumulators. The UART reports each PID and exit status. After both processes exit, the guest parks. Stop QEMU with Ctrl+C.

## Scheduling independently linked applications

Provide the primary with `--source` or `--elf` and append up to two `--peer-elf` arguments. Slot 0 is the primary. Slots 1 and 2 follow argument order. Without a policy these apps yield cooperatively. With `--policy fifo` or `--policy rr`, supply one `--priority` for slot 0 and one `--peer-priority` for each peer. Priorities range from 0 to 255, highest first. RR requires a shared quantum of 1 through 9223372036854775807 physical counter ticks. FIFO takes no quantum.

Build three ELF files and boot the package from Linux or WSL. The package manifest lists the input identity, ELF digest and priority of each slot. The serial console stays attached when the kernel parks after the final exit.

~~~sh
run=$(mktemp -d)
sh sdk/native-aarch64/build.sh examples/native-fixed/calendar_report.c "$run/high.elf"
sh sdk/native-aarch64/build.sh examples/native-fixed/prime_low.c "$run/a.elf"
sh sdk/native-aarch64/build-rust.sh examples/native-fixed-rust/Cargo.toml "$run/b.elf"
python3 tools/build_native_el0.py --out "$run/rr" --elf "$run/high.elf" \
    --peer-elf "$run/a.elf" --peer-elf "$run/b.elf" --policy rr \
    --priority 7 --peer-priority 3 --peer-priority 3 --quantum-ticks 1250000
qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 \
    -accel tcg -m 128M -nodefaults -display none -monitor none -no-reboot \
    -serial stdio -kernel "$run/rr/kernel.elf"
~~~

The small examples use a 1,250,000-tick quantum so the two priority-3 calculations overlap multiple quanta. The original frozen example used 12,500,000 ticks. The quantum is a package setting in physical counter ticks. [Native FIFO and RR evidence](evidence/evd-115.en.md)

## Weighted CPU service for native applications

Give the primary ELF one `--weight` and each peer ELF one `--peer-weight` in argument order. Weights from 1 through 4,294,967,295 specify relative CPU service while clients remain ready. The package uses one common grant in physical counter ticks. From the repository root on Linux or WSL, package a C CRC-32 calculation and a Rust bit-count histogram:

~~~sh
run=$(mktemp -d)
sh sdk/native-aarch64/build.sh examples/native-weighted/checksum.c "$run/crc.elf"
sh sdk/native-aarch64/build-rust.sh examples/native-weighted-rust/Cargo.toml "$run/hist.elf"
python3 tools/build_native_el0.py --out "$run/weighted" --elf "$run/crc.elf" \
    --peer-elf "$run/hist.elf" --policy weighted --weight 1 --peer-weight 2 --quantum-ticks 625000
qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -accel tcg \
    -m 128M -nodefaults -display none -monitor none -no-reboot -serial stdio -kernel "$run/weighted/kernel.elf"
~~~

[examples/native-weighted](../examples/native-weighted/README.md) · [EVD-116](evidence/evd-116.en.md)

## Sampled EDF for native applications

Package one to three independently linked C or Rust ELFs with `--policy edf`. Supply the primary tuple with `--job` and one `--peer-job` per peer ELF in argument order. Each tuple gives release time, absolute deadline and estimated CPU cost in 100 ms grid units from a common epoch. In this finite CPU0 admission profile, 0 ≤ release < deadline ≤ 16 and cost must be positive. Admission models independent jobs using their declared costs; compare actual completion with the deadline separately. For example, `1,4,1` releases the urgent job at 100 ms, due at 400 ms with 100 ms of estimated CPU demand. From the repository root on Linux or WSL:

~~~sh
run=$(mktemp -d)
sh sdk/native-aarch64/build.sh examples/native-edf/batch.c "$run/batch.elf"
sh sdk/native-aarch64/build-rust.sh examples/native-edf-rust/Cargo.toml "$run/histogram.elf"
sh sdk/native-aarch64/build.sh examples/native-edf/urgent.c "$run/urgent.elf"
python3 tools/build_native_el0.py --out "$run/edf" --policy edf \
    --elf "$run/batch.elf" --job 0,8,2 \
    --peer-elf "$run/histogram.elf" --peer-job 0,12,8 \
    --peer-elf "$run/urgent.elf" --peer-job 1,4,1
qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -accel tcg \
    -m 128M -nodefaults -display none -monitor none -no-reboot -serial stdio -kernel "$run/edf/kernel.elf"
~~~

[examples/native-edf](../examples/native-edf/README.md) · [EVD-117](evidence/evd-117.en.md)

## Recurring native jobs under RM and DM

Package one to three independently built applications. Each tuple is `phase_ns,period_ns,relative_deadline_ns`. The deadline must be positive and at most the period. Positive durations must convert exactly to physical timer ticks. Here C runs every 100 ms with a 90 ms deadline, and Rust every 120 ms with a 40 ms deadline. RM ranks the shorter period first. DM ranks the shorter relative deadline first. From the repository root on Linux or WSL:

~~~sh
run=$(mktemp -d)
sh sdk/native-aarch64/build.sh examples/native-periodic-job/crc.c "$run/crc.elf"
sh sdk/native-aarch64/build-rust.sh examples/native-periodic-job-rust/Cargo.toml "$run/hist.elf"
for policy in rm dm; do
    python3 tools/build_native_el0.py --out "$run/$policy" --policy "$policy" \
        --elf "$run/crc.elf" --periodic 0,100000000,90000000 \
        --peer-elf "$run/hist.elf" --peer-periodic 0,120000000,40000000
done
qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -accel tcg \
    -m 128M -nodefaults -display none -monitor none -no-reboot -serial stdio -kernel "$run/rm/kernel.elf"
~~~

Use `$run/dm/kernel.elf` to boot the DM package. Each process enters main once. After each completed batch, C calls `jnox_periodic_wait_next()` from `<jnox/periodic.h>` (0 on success, −1 with errno on failure). Rust calls `jnox_native::wait_next_period()`, returning `Result<(), i32>`. Successful return resumes the same process for its next generation, with its private data and stack preserved. Exit finishes the current generation. [EVD-118](evidence/evd-118.en.md)

## Hard CPU reservations for native applications

Build the [C CRC worker and Rust histogram worker](../examples/native-cbs/README.md) with the public SDK. From the repository root on Linux or WSL, follow the linked build and QEMU commands. The package step for those two ELFs is:

~~~sh
python3 tools/build_native_el0.py --out isolated --policy cbs --elf crc.elf --reservation 1,4 --peer-elf histogram.elf --peer-reservation 1,4
~~~

Specify one quota,period tuple per ELF in slot order, using counts of 50 ms slots. In this example each application receives one nominal grant per four slots (200 ms); unused capacity stays idle. Use positive integer quotas no larger than their periods. The sum of the quota/period fractions must be at most one. The normal loader accepts a primary application and up to two peer ELFs. Each worker retains its private block count and result across grants. Physical service is measured separately from nominal entitlement.

## Sampled LLF for finite native jobs

Use `--policy llf` with the same 100 ms release, absolute deadline and estimated-cost tuples used by native EDF. At a grid sample the kernel chooses the smallest signed laxity (deadline minus grid index minus remaining cost). A waiting job can take over even when no job arrives. An equal-laxity incumbent keeps running. For example, package the checksum with `--job 0,6,1` and the independently built histogram with `--peer-job 0,7,3`. The [worked example](../examples/native-llf/README.md) gives the complete SDK build, package and boot commands for both histogram inputs.

## Native class and feedback queues

Use `--policy mlq` for fixed classes. Assign the primary with `--class` and each peer with `--peer-class` in ELF order; larger classes run first. `--quantum-ticks` sets the turn length among equal-class peers. For example, select class 7 for the alarm reporter and class 1 for CRC:

~~~sh
python3 tools/build_native_el0.py --out strict --policy mlq --quantum-ticks 625000 --elf report.elf --class 7 --peer-elf crc.elf --peer-class 1
~~~

For feedback queues, use `--policy mlfq --allotment-ticks 625000,1250000,2500000 --boost-ticks 6250000` with the same ELF arguments and omit the class parameters. These are physical counter ticks. On the reference 62.5 MHz clock the three level allotments are 10, 20 and 40 ms and the boost interval is 100 ms. The [worked examples](../examples/native-multilevel/README.md) build the C reporter, CRC and yield workers and Rust histogram and boot the packages.

## Private native process arguments

Each occurrence of `--arg`, `--peer-arg` or `--peer2-arg` adds one UTF-8 string to slot 0, 1 or 2, respectively. The fixed first argument is /bin/app. Quote spaces, pass an empty string explicitly, and use `--arg=-west` for a leading dash. Per slot, up to 15 extra strings fit in 1,024 bytes including their NUL terminators. The native main receives argc and argv on its own stack. A slot with no extras preserves the original 80-byte layout at SP 0x01010fb0, including pointers, terminators and padding. The startup probe retains its separate original 80-byte layout.

~~~sh
sh sdk/native-aarch64/build.sh examples/native-argv/sensor.c sensor.elf
python3 tools/build_native_el0.py --out sensor-pair --elf sensor.elf \
    --arg sum --arg 16 --arg 32 --arg 'west rack' \
    --peer-elf sensor.elf --peer-arg minmax --peer-arg 64 --peer-arg 96 --peer-arg ''
~~~

[Boot the package and try the Rust UTF-8 example](../examples/native-argv/README.md)

## Create and join a native worker thread

From the repository root, package one C source or one prelinked Rust ELF with `--threads 2`. This selects a single cooperative process on CPU0 with main and one reusable worker. Its globals and argv strings belong to the process. Each thread has its own stack, saved registers and errno cell.

~~~sh
python3 tools/build_native_el0.py --out output/native-threads/c --threads 2 --source examples/native-threads/sensor.c
~~~

Call `pthread_create(&handle, NULL, start, arg)` to queue the worker. Main continues until a scheduling point. Only main joins the worker: `pthread_join(handle, &result)` waits for a live worker or collects an exited worker immediately. The slot is reusable after join. A busy slot returns EAGAIN; a reaped handle returns ESRCH. These functions return positive errors directly and preserve errno. A worker may return a pointer to shared data that survives its exit. [C and Rust build and boot examples](../examples/native-threads/README.md)

## Round robin within one native process

Keep `--threads 2` alone for the default cooperative main and worker. Add `--policy rr --quantum-ticks 625000` to rotate the two equal-rank runnable threads on CPU0. At the reference 62.5 MHz clock that quantum is 10 ms. Both threads share the application address space and keep their own saved frame and stack. [Build and boot the same stream ELF under both policies](../examples/native-thread-rr/README.md)

~~~sh
python3 tools/build_native_el0.py --out output/thread-rr/rr --source examples/native-thread-rr/stream.c --threads 2 --policy rr --quantum-ticks 625000
~~~

## Process-private mutexes

Use an explicitly initialized, process-private `pthread_mutex_t` at a stable address. The SDK provides `pthread_mutex_init(&m, NULL)`, `pthread_mutex_lock`, `pthread_mutex_trylock`, `pthread_mutex_unlock` and `pthread_mutex_destroy` for cooperative and RR two-thread packages. Up to four mutexes may be live per process. A busy trylock returns positive EBUSY directly and preserves errno. A blocking lock resumes after the owner transfers ownership. Destroy the unlocked mutex after all users finish. [Build and boot the shared C/Rust sensor](../examples/native-thread-mutex/README.md)

## Reserve recurring service cells for two deadline jobs

Supply two ordinary job tuples, `--deadline-service-elf service.elf`, an increasing calendar such as `--deadline-service-cells 1,3`, and `--deadline-switch-cap 6`. The 16-cell calendar uses 100 ms units. The service starts once and reads the calendar from argv[1]. After each nonfinal result call `jnox_deadline_service_wait_next()` from `<jnox/deadline.h>` or Rust `jnox_native::deadline_service_wait_next()` to receive the next activation ordinal. Exit after the final result. The unused tail of a reserved cell stays idle. [Build, package and boot examples](../examples/native-deadline-service/README.md)

## Run two native applications on the Pi 4 model

From the repository root, build the C and Rust prime counters. Package the same two ELFs at priority 2 on CPU0 under FIFO or RR. The quantum is in timer ticks.

~~~sh
sh sdk/native-aarch64/build.sh examples/native-fixed/prime_low.c low.elf
sh sdk/native-aarch64/build-rust.sh examples/native-fixed-rust/Cargo.toml high.elf
python3 tools/build_native_el0.py --out pi-fifo --board pi4 --policy fifo --elf low.elf --priority 2 --peer-elf high.elf --peer-priority 2
python3 tools/build_native_el0.py --out pi-rr --board pi4 --policy rr --quantum-ticks 1250000 --elf low.elf --priority 2 --peer-elf high.elf --peer-priority 2
qemu-system-aarch64 -M raspi4b -accel tcg -display none -serial stdio -kernel pi-rr/kernel8.img
~~~

For FIFO, change the final image path to `pi-fifo/kernel8.img`. Direct loading bypasses EEPROM, SD and VideoCore firmware. The command below stages the pinned firmware for a FAT32 card. Add `--firmware-dir pinned-boot` for an existing firmware tree. Physical boot and board timing require separate observations. [Prepare and verify the card](boards/pi4-p1.en.md)

~~~sh
python3 tools/package_pi4.py --kernel pi-rr/kernel8.img --output pi-rr-boot
~~~

## Assign repeating CPU windows to two applications

On single-CPU AArch64 virt, a names the primary ELF and b the peer. This table repeats 100 ms for a, 50 ms for b and 50 ms of kernel idle. Supply 2–16 windows including both applications. If an owner blocks or exits, its remaining window stays idle.

~~~sh
python3 tools/build_native_el0.py --out table-p --policy partition --partition-windows a:100,b:50,idle:50 --elf energy.elf --peer-elf density.elf
~~~

[Build the energy and bit-density workers and boot the package on QEMU](../examples/native-cyclic/README.md)

## Run an aperiodic service with a background application

On single-CPU virt the primary ELF runs the service and the peer ELF runs background work. This example budgets two service grants per five 50 ms slots. Each grant consumes a whole unit. A ready background worker can use the rest of the slot when the service blocks. Sporadic credit capacity is 1–8 replenishment records. For polling or deferrable, change the policy name and omit --server-credits.

~~~sh
python3 tools/build_native_el0.py --out ss --policy sporadic --server-slot-ms 50 --server-budget 2,5 --server-credits 2 --elf service.elf --peer-elf background.elf
~~~

[SDK build / QEMU 실행](../examples/native-aperiodic/README.md)

## Keep weighted quanta active during solo service

On single-CPU virt, add --weighted-continuous to keep the quantum timer active even when only one app is runnable. Omit the flag for legacy weighted operation with uninterrupted solo service. Assign each ELF a positive weight and specify the quantum in timer ticks.

~~~sh
python3 tools/build_native_el0.py --out share --policy weighted --weighted-continuous --weighted-notify closures:36 --quantum-ticks 625000 --elf monitor.elf --weight 1 --arg A --arg share --peer-elf monitor.elf --peer-weight 2 --peer-arg B --peer-arg share
~~~

The optional --weighted-notify closures:N counts full quanta closed by timer IRQ. Use wakes:N for actual wakeups from sleep. In a package with notification enabled, C code can include `<jnox/weighted_notify.h>` and read the latch with `jnox_weighted_notification()`. It changes once from zero to one. Each app continues until it chooses to exit.

[Archive extraction and QEMU commands / 원본 추출 및 실행](../examples/native-weighted/README.md#original-weighted-monitor--원래-가중치-모니터) · [#327](native-weighted-continuous.en-ko.md)
