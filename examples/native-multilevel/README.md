# Multilevel queue workloads

Build all four independent ELFs once, from the repository root in Linux or WSL. Use the same files for all variants. The CRC and Rust histogram use the existing [CBS inputs and host oracle](../native-cbs/README.md).

~~~sh
sh sdk/native-aarch64/build.sh examples/native-multilevel/report.c report.elf
sh sdk/native-aarch64/build.sh examples/native-multilevel/yield.c yield.elf
sh sdk/native-aarch64/build.sh examples/native-cbs/crc.c crc.elf
sh sdk/native-aarch64/build-rust.sh examples/native-cbs-rust/Cargo.toml histogram.elf

mlq() { python3 tools/build_native_el0.py --policy mlq --quantum-ticks 625000 "$@"; }
mlfq() { python3 tools/build_native_el0.py --policy mlfq --allotment-ticks 625000,1250000,2500000 --boost-ticks 6250000 "$@"; }

mlq --out strict --elf report.elf --class 7 --peer-elf crc.elf --peer-class 1
mlfq --out feedback --elf report.elf --peer-elf crc.elf
mlq --out strict-three --elf crc.elf --class 7 --peer-elf histogram.elf --peer-class 7 --peer-elf report.elf --peer-class 1
mlq --out equal-yield --elf yield.elf --class 7 --peer-elf crc.elf --peer-class 7
mlfq --out cumulative-yield --elf yield.elf --peer-elf crc.elf
mlfq --out solo-yield --elf yield.elf
mlfq --out idle-report --elf report.elf
~~~

Use the reference physical counter frequency of 62,500,000 Hz. Freeze the toolchain, inputs, binaries and disassemblies before the first boot. For example, launch strict on the stock virtual machine as follows:

~~~sh
qemu-system-aarch64 -machine virt,gic-version=2 -cpu cortex-a72 -smp 1 -accel tcg \
    -m 128M -nodefaults -display none -monitor none -no-reboot -serial stdio \
    -kernel strict/kernel.elf
~~~

Capture strict plain then externally exception-traced, followed by feedback plain then traced. An external QEMU exception log can be enabled with `-d int -D exceptions.log`; use a fresh file for each capture. Apply the [design's decision and observation criteria](../../docs/native-multilevel-applications.en-ko.md) to each journal.

Both C alarm workers regenerate 4096 volatile bytes per block using (17*i + 3 + block) modulo 256. Every block contains sixteen complete byte permutations and therefore 16×16 = 256 bytes at least 240. Map count_alarms to the packaged ELF disassembly.

| ELF | Final u32 private_blocks | Final u64 private_bytes | Final u32 private_result | Exit |
| --- | ---: | ---: | ---: | ---: |
| report | 3 | 12288 | 768 | 53 |
| yield | 16384 | 67108864 | 4194304 | 59 |

The reporter stores each absolute deadline in request_ns[3] and its first post-wake clock sample in observed_ns[3], both u64 nanoseconds. Each sample must reach its target. The u32 arrays round_count and cumulative_count must equal [256,256,256] and [256,512,768]. The yield client calls sched_yield after publishing every complete block, including the last. Use recorded service to evaluate cross-yield demotion. If the bounded journal fills before exit, retain its drop count and obtain final products independently.

## M4 telemetry deployment / M4 경보 게시 배포

From the repository root in Linux or WSL, use the normal AArch64 SDK and package tools above, GNU AArch64 binutils, stock QEMU and Python with pyelftools. Keep a new output directory for each deployment. The commands below bind the application sources and compiler to both independently built ELFs, then package those same bytes under each policy.

~~~sh
set -eu
out=$(mktemp -d)
rev=$(git rev-parse HEAD)
printf '%s\n' "$rev" > "$out/revision"
git diff --binary HEAD > "$out/source-changes.patch"
clang --version > "$out/compiler.txt"
ld.lld --version > "$out/linker.txt"
sha256sum examples/native-multilevel/{report.c,alarms.h} examples/native-cbs/{crc.c,stream.h,ramp.h} examples/native-edf/clock.h > "$out/sources.sha256"
sh sdk/native-aarch64/build.sh examples/native-multilevel/report.c "$out/report.elf"
sh sdk/native-aarch64/build.sh examples/native-cbs/crc.c "$out/crc.elf"
sha256sum "$out/report.elf" "$out/crc.elf" > "$out/apps.sha256"
package() {
    python3 tools/build_native_el0.py --out "$out/$1" --elf "$out/report.elf" \
        --peer-elf "$out/crc.elf" "${@:2}"
}
package strict --policy mlq --class 7 --peer-class 1 --quantum-ticks 625000 --arg=--publication-deadline-ms=40
package feedback --policy mlfq --allotment-ticks 625000,1250000,2500000 --boost-ticks 6250000 --arg=--publication-deadline-ms=40
package compat --policy mlq --class 7 --peer-class 1 --quantum-ticks 625000
package invalid --policy mlq --class 7 --peer-class 1 --quantum-ticks 625000 --arg=--publication-deadline-ms=0
~~~

In Bash, the following function boots a fresh guest and retains UART and terminal RAM. Traced captures also retain QEMU's external exception log. It prints the capture status and writes the result JSON even when the reporter returns a valid deadline miss. Each launch creates a new capture directory; keep the original for each position.

~~~sh
capture() {
    local pkg="$out/$1" dir="$out/$1-$2" expected="$3" status=0
    shift 3
    python3 tools/native_el0_capture.py "$dir" "$pkg/kernel.elf" "$pkg/user.elf" \
        examples/native-multilevel/report.c "$rev" --manifest "$pkg/manifest.json" \
        --expect-exit "$expected" "$@" || status=$?
    printf 'capture %s status %s\n' "$dir" "$status"
    python3 tools/read_multilevel_telemetry.py "$pkg" "$dir" > "$dir/results.json"
}
capture strict plain 53 --plain
capture strict traced 53
capture feedback plain 53 --plain
capture feedback traced 53
capture compat plain 53 --plain
capture invalid plain 106 --plain
~~~

The option sets the publication deadline in milliseconds. Each round reports its requested release, first post-wake sample, completed and cumulative counts, completion marker, fresh post-publication sample and application verdict (0 unscored, 1 pass, 2 miss). The sampled delay and margin are in nanoseconds. Null denotes a missing publication sample. The configured reporter should return 53 with counts 256/256/256 and cumulative totals 256/512/768; 107 records a deadline miss and 106 rejects invalid input before publication. The CRC should return 17 with a matching CRC and byte count for the actual retained blocks. The terminal snapshot gives final outputs; release chronology and progress during individual sleeps require the separate scheduling evidence in the [deployment contract](../../docs/multilevel-telemetry-deployment.en-ko.md).

저장소 루트의 Bash에서 같은 두 ELF를 한 번 빌드해 엄격 큐와 피드백 큐에 배포한다. 설정값은 밀리초 단위이며 각 캡처의 results.json에는 요청 시각·기상 직후 시각·게시 직후 시각, 라운드별 결과와 누적 결과, 마감 여유(나노초), 두 프로세스의 종료 코드와 CRC 재계산값을 기록한다. 완료 표지와 새 표본이 없으면 지연은 null이다. 종료값 53은 정상, 106은 입력 오류, 107은 마감 초과다. 무인자 호환 실행에서는 기한을 판정하지 않는다.
