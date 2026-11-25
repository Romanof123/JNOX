# Native weighted application inputs

From the repository root, build each C file separately using
`sh sdk/native-aarch64/build.sh examples/native-weighted/checksum.c checksum.elf`
(substitute the chosen C name). Build the histogram with
`sh sdk/native-aarch64/build-rust.sh examples/native-weighted-rust/Cargo.toml histogram.elf`.

| Application | Fixed input and complete expected private_result | Final private_progress | Success status |
| --- | --- | ---: | ---: |
| checksum | CRC-32/ISO-HDLC over 65,536 copies of bytes 0 through 255, 0x2a223dad | 16777216 bytes | 52 |
| histogram | 262,144 blocks of 256 bytes, each shifted by its block index modulo 256. Bit-count bins 0 through 8 equal [262144, 2097152, 7340032, 14680064, 18350080, 14680064, 7340032, 2097152, 262144] | 67108864 bytes | 197 |
| continuous_reporter | Sum of squared signed samples -128 through 127 for 524,288 frames, 733030121472 | 134217728 samples | 242 |
| reporter | Three fresh absolute 40 ms waits, one successful deadline check per bit, mask 7 | 3 reports | 7 |
| boundary | Common absolute grid wake, second wait and final yield, result 17 | 3 | 17 |
| boundary_peer | Common absolute grid wake and exit, result 19 | 2 | 19 |

The CRC can be independently computed with Python zlib.crc32 over the specified byte sequence. Histogram bin k is 262144 times the binomial coefficient C(8,k). The histogram status is the sum of bin[k] times (k+1) squared, modulo 251. The checksum and energy statuses are the full result modulo 251. The energy is 524288 times the sum of i squared from -128 through 127. In the timed reporter the three requested absolute deadlines and subsequent user clock observations are uint64 nanosecond arrays report_deadline_ns and report_observed_ns. Each boundary client records its own grid target in boundary_deadline_ns.

Use checksum/histogram for the pair and swapped-weight control, checksum/reporter for the timed pair, checksum/histogram/reporter for mixed 1:2:3 and checksum/histogram/continuous_reporter for all-ready 1:2:3. Run boundary alone for solo yield and with boundary_peer for the idle and exit control. Match installed ELF symbols checksum_stream, histogram_stream and measure_energy to preemption PCs. Check per-slot private RAM against the table. Apply the predeclared first-36 and first-60 rules to the recorded observations.

## Original weighted monitor / 원래 가중치 모니터

Run from the repository root in a POSIX shell with GNU sha256sum, unzip and QEMU.
The following commands extract the original ELF into a new directory and check both archive and ELF digests before packaging.
저장소 루트에서 아래 명령으로 원본 아카이브와 ELF 해시를 확인한 뒤 두 역할에 같은 ELF를 사용한다.

~~~sh
set -eu
out=$(mktemp -d)
rev=95493792c85547c5c2e515faa352471f9169737a
archive=docs/evidence/native-weighted-integration-first.zip
git cat-file blob "$rev:$archive" > "$out/original.zip"
printf '%s  %s\n' 33b2cbe90b60c12691db1ee73ec039e807713b2545b7ecd90bd741509b598dc9 "$out/original.zip" | sha256sum -c -
members=build/0ad1eda8fafdf45d948c3761af938cf18e68df80/native
for arm in share wake; do
    elf="$out/$arm.elf"
    unzip -p "$out/original.zip" "$members/weighted-$arm/user.elf" > "$elf"
    printf '%s  %s\n' 9d3c6350a04536da091dd7bc77cfdcc81729f8baa431d6315e5258c28da67206 "$elf" | sha256sum -c -
    case "$arm" in
        share) horizon=closures:36 ;;
        wake) horizon=wakes:3 ;;
    esac
    python3 tools/build_native_el0.py --out "$out/$arm" --board virt --policy weighted \
        --weighted-continuous --weighted-notify "$horizon" --quantum-ticks 625000 \
        --elf "$elf" --weight 1 --arg A --arg "$arm" \
        --peer-elf "$elf" --peer-weight 2 --peer-arg B --peer-arg "$arm"
done
qemu-system-aarch64 -machine virt,gic-version=2 -accel tcg -cpu cortex-a72 -smp 1 \
    -m 128M -display none -serial stdio -kernel "$out/share/kernel.elf"
~~~

For the wake run, use the same QEMU command with `"$out/wake/kernel.elf"`. The first word at 0x01200000 becomes 1 after the selected number of full timer closures or timer-processed wakes. The monitor exits voluntarily after observing it. 기상 실험은 같은 QEMU 명령에서 wake 커널을 지정한다. 통지 횟수에 도달하면 읽기 전용 페이지의 첫 단어가 1이 되고 모니터가 스스로 종료한다.
