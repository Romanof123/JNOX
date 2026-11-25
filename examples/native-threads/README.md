# Joinable native sensor applications / 조인 가능한 센서 앱

From the repository root, build and install each single application ELF in its own two-thread kernel package:

```sh
mkdir -p output/native-threads
python3 tools/build_native_el0.py --out output/native-threads/c --threads 2 --source examples/native-threads/sensor.c
sh sdk/native-aarch64/build-rust.sh examples/native-threads-rust/Cargo.toml output/native-threads/energy.elf
python3 tools/build_native_el0.py --out output/native-threads/rust --threads 2 --elf output/native-threads/energy.elf
```

Boot one package at a time. Select the package by setting APP to c or rust.

```sh
APP=c
qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -accel tcg -m 128M -nodefaults -display none -monitor none -serial stdio -no-reboot -kernel "output/native-threads/$APP/kernel.elf"
```

The C worker calibrates 256 samples with offsets 1 and 2. Main independently sums the original samples to 32,640. The first worker yields each quarter, sleeps until a monotonic deadline 20 ms in the future and returns its descriptor. Main blocks in join. The second worker explicitly exits with its descriptor before main joins. Expected sums are 32,896 and 33,152, with exit status 17. Inspect sensor_input, sensor_jobs (both output arrays, sums, progress and errno observations), sensor_handles, sensor_joined, sensor_main_checksum, sensor_main_errno_cell, sensor_deadline_ns, sensor_wake_ns and sensor_stale_error. Concurrent progress flags use C atomics. Inputs are immutable after create, and main reads the completed array after join. Both descriptors have static lifetime.

The standalone Rust worker computes 256 squares from samples 0 through 255. Its static atomic buffers are energy_samples and energy_squares. Main records 32,640 in energy_main_sum, joins, validates every square and checks energy_total equals 5,559,680. Expected exit status is 29. The returned address identifies the static energy_total object. The worker yields after each quarter.

The pthread wrappers return positive errors directly and preserve errno. The C application checks ESRCH and preservation of the caller's output for a reaped handle. Clock and close errors set the main and worker errno cells to EINVAL and ENOSYS respectively. Successful worker-slot reuse starts with zero errno. Application statuses 101–108 and 111–115 identify failed checks. The applications modify their own shared memory. Runtime and wake observations depend on the guest counter and host scheduling.

저장소 루트에서 위 명령으로 각각 단일 ELF와 두 스레드용 커널을 패키징한다. C 앱은 두 보정 배열 전체와 합계, 결과 포인터, 세대별 핸들 및 errno를 검증한 뒤 17로 종료한다. 첫 조인은 작업자가 20 ms 절대 대기를 수행하는 동안 차단되고 두 번째 조인은 종료한 작업자의 결과를 회수한다. Rust 앱은 정적 원자 배열에 256개 제곱값을 저장하고 조인 후 모든 값과 총합을 확인하여 29로 종료한다.
